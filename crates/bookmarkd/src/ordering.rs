//! Anchor-based moves include off-page siblings and commit with the library transaction.
use crate::model::Library;
use anyhow::{Result, bail, ensure};
use bookmarkd_auth::now;
use serde_json::{Value, json};

pub fn move_item(l: &mut Library, kind: &str, v: &Value) -> Result<Value> {
    if v["revision"].as_i64() != Some(l.revision) {
        bail!("VERSION_CONFLICT:{}", json!({"revision": l.revision}));
    }
    let id = v["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("缺少拖动项目"))?;
    let anchor = v["anchor_id"].as_str();
    let placement = v["placement"].as_str().unwrap_or("after");
    ensure!(["before", "after"].contains(&placement), "无效放置位置");
    ensure!(anchor != Some(id), "不能拖到自身");
    let mut order: Vec<String>;
    let pinned;
    if kind == "bookmarks" {
        let scope = v["scope"].as_str().unwrap_or("folder");
        ensure!(
            ["library", "folder", "pinned"].contains(&scope),
            "无效排序范围"
        );
        pinned = scope == "pinned";
        let index = l
            .bookmarks
            .iter()
            .position(|b| b.id == id && b.deleted_at.is_none())
            .ok_or_else(|| anyhow::anyhow!("NOT_FOUND"))?;
        if v["version"].as_i64() != Some(l.bookmarks[index].version) {
            bail!("VERSION_CONFLICT:{}", json!(l.bookmarks[index]));
        }
        let target = if scope == "folder" {
            ensure!(v.get("folder_id").is_some(), "缺少目标目录");
            let target: Option<String> = serde_json::from_value(v["folder_id"].clone())?;
            l.check_folder(&target, false)?;
            target
        } else {
            l.bookmarks[index].folder_id.clone()
        };
        let mut siblings: Vec<_> = l
            .bookmarks
            .iter()
            .filter(|b| {
                b.id != id
                    && b.deleted_at.is_none()
                    && match scope {
                        "folder" => b.folder_id == target,
                        "pinned" => b.pinned,
                        _ => true,
                    }
            })
            .collect();
        siblings.sort_by_key(|b| {
            (
                if pinned {
                    b.pinned_position
                } else {
                    b.position
                },
                std::cmp::Reverse(b.created_at),
                &b.id,
            )
        });
        order = siblings.iter().map(|b| b.id.clone()).collect();
        // Validate the anchor before changing either membership or order.
        insert(&mut order, id, anchor, placement)?;
        let b = &mut l.bookmarks[index];
        b.folder_id = target;
        if pinned {
            b.pinned = true;
        }
        b.version += 1;
        b.updated_at = now();
    } else {
        pinned = false;
        ensure!(v.get("parent_id").is_some(), "缺少上级目录");
        let target: Option<String> = serde_json::from_value(v["parent_id"].clone())?;
        l.check_folder(&target, false)?;
        let index = l
            .folders
            .iter()
            .position(|f| f.id == id && f.deleted_at.is_none())
            .ok_or_else(|| anyhow::anyhow!("NOT_FOUND"))?;
        if v["version"].as_i64() != Some(l.folders[index].version) {
            bail!("VERSION_CONFLICT:{}", json!(l.folders[index]));
        }
        ensure!(
            !target
                .as_ref()
                .is_some_and(|p| l.descendants(id).contains(p)),
            "不能将目录移入自身或后代"
        );
        let mut siblings: Vec<_> = l
            .folders
            .iter()
            .filter(|f| f.id != id && f.deleted_at.is_none() && f.parent_id == target)
            .collect();
        siblings.sort_by_key(|f| (f.position, &f.id));
        order = siblings.iter().map(|f| f.id.clone()).collect();
        insert(&mut order, id, anchor, placement)?;
        l.folders[index].parent_id = target;
        l.folders[index].version += 1;
    }
    let positions: std::collections::HashMap<_, _> = order
        .iter()
        .enumerate()
        .map(|(pos, id)| (id.as_str(), pos as i64))
        .collect();
    if kind == "bookmarks" {
        for b in &mut l.bookmarks {
            if let Some(pos) = positions.get(b.id.as_str()) {
                let current = if pinned {
                    &mut b.pinned_position
                } else {
                    &mut b.position
                };
                if *current != *pos {
                    *current = *pos;
                    if b.id != id {
                        b.version += 1;
                    }
                    b.updated_at = now();
                }
            }
        }
    } else {
        for f in &mut l.folders {
            if let Some(pos) = positions.get(f.id.as_str())
                && f.position != *pos
            {
                f.position = *pos;
                if f.id != id {
                    f.version += 1;
                }
            }
        }
    }
    l.validate()?;
    Ok(json!({"ok": true, "revision": l.revision + 1}))
}
fn insert(order: &mut Vec<String>, id: &str, anchor: Option<&str>, placement: &str) -> Result<()> {
    let index = if let Some(anchor) = anchor {
        order
            .iter()
            .position(|x| x == anchor)
            .ok_or_else(|| anyhow::anyhow!("放置目标已变化或不在目标目录"))?
            + usize::from(placement == "after")
    } else {
        order.len()
    };
    order.insert(index, id.into());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        model::{Bookmark, Folder},
        store::Store,
    };
    fn folder(id: &str, parent: Option<&str>) -> Folder {
        Folder {
            id: id.into(),
            name: id.into(),
            parent_id: parent.map(String::from),
            position: 0,
            version: 1,
            deleted_at: None,
            deletion_batch: None,
        }
    }
    #[test]
    fn moves_across_folders_and_back_without_losing_metadata() {
        let mut l = Library::default();
        l.folders.push(folder("f", None));
        let mut b = Bookmark::from_input(
            &json!({"url_raw":"https://example.com","notes":"keep","tags":["tag"]}),
        )
        .unwrap();
        b.id = "b".into();
        l.bookmarks.push(b);
        move_item(
            &mut l,
            "bookmarks",
            &json!({"id":"b","version":1,"revision":0,"scope":"folder","folder_id":"f"}),
        )
        .unwrap();
        assert_eq!(l.bookmarks[0].folder_id.as_deref(), Some("f"));
        move_item(
            &mut l,
            "bookmarks",
            &json!({"id":"b","version":2,"revision":0,"scope":"folder","folder_id":null}),
        )
        .unwrap();
        assert!(l.bookmarks[0].folder_id.is_none());
        assert_eq!(l.bookmarks[0].notes, "keep");
        assert_eq!(l.bookmarks[0].tags, vec!["tag"]);
    }
    #[test]
    fn reorder_preserves_off_page_rows_and_pinned_order_is_independent() {
        let mut l = Library::default();
        for i in 0..150 {
            let mut b =
                Bookmark::from_input(&json!({"url_raw":"https://example.com","pinned":true}))
                    .unwrap();
            b.id = i.to_string();
            b.position = i;
            b.pinned_position = i;
            l.bookmarks.push(b);
        }
        move_item(&mut l, "bookmarks", &json!({"id":"149","version":1,"revision":0,"scope":"library","anchor_id":"1","placement":"before"})).unwrap();
        let mut rows: Vec<_> = l.bookmarks.iter().collect();
        rows.sort_by_key(|b| b.position);
        assert_eq!(rows[0].id, "0");
        assert_eq!(rows[1].id, "149");
        assert_eq!(rows[2].id, "1");
        assert_eq!(rows[149].id, "148");
        assert_eq!(l.bookmarks[149].pinned_position, 149);
        let before: Vec<_> = l.bookmarks.iter().map(|b| b.position).collect();
        move_item(&mut l, "bookmarks", &json!({"id":"149","version":2,"revision":0,"scope":"pinned","anchor_id":"0","placement":"before"})).unwrap();
        assert_eq!(
            before,
            l.bookmarks.iter().map(|b| b.position).collect::<Vec<_>>()
        );
    }
    #[test]
    fn invalid_moves_and_stale_revisions_roll_back() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::init(&d.path().join("bookmarks.db")).unwrap();
        s.mutate(None, "", |l| {
            l.folders = vec![
                folder("parent", None),
                folder("child", Some("parent")),
                folder("peer", None),
            ];
            Ok(Value::Null)
        })
        .unwrap();
        for v in [
            json!({"id":"parent","version":1,"revision":1,"parent_id":"child"}),
            json!({"id":"child","version":1,"revision":0,"parent_id":null}),
            json!({"id":"child","version":1,"revision":1,"parent_id":null,"anchor_id":"missing"}),
        ] {
            assert!(s.mutate(None, "", |l| move_item(l, "folders", &v)).is_err());
            assert_eq!(s.read().unwrap().revision, 1);
            assert_eq!(
                s.read().unwrap().folders[1].parent_id.as_deref(),
                Some("parent")
            );
        }
        s.mutate(None,"",|l|move_item(l,"folders",&json!({"id":"child","version":1,"revision":1,"parent_id":null,"anchor_id":"peer","placement":"before"}))).unwrap();
        assert!(s.read().unwrap().folders[1].parent_id.is_none());
    }
}
