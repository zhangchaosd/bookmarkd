use anyhow::{Result, ensure};
use bookmarkd_auth::now;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use unicode_normalization::UnicodeNormalization;
pub fn normalize(s: &str) -> String {
    s.nfkc()
        .flat_map(char::to_lowercase)
        .collect::<String>()
        .replace('ß', "ss")
        .replace('ς', "σ")
}
pub fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Bookmark {
    pub id: String,
    pub url_raw: String,
    pub title: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub folder_id: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub position: i64,
    #[serde(default)]
    pub pinned_position: i64,
    pub version: i64,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub original_created_at: Option<i64>,
    #[serde(default)]
    pub deleted_at: Option<i64>,
    #[serde(default)]
    pub deletion_batch: Option<String>,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Folder {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub position: i64,
    pub version: i64,
    #[serde(default)]
    pub deleted_at: Option<i64>,
    #[serde(default)]
    pub deletion_batch: Option<String>,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Library {
    pub format: String,
    pub format_version: u32,
    pub revision: i64,
    pub folders: Vec<Folder>,
    pub bookmarks: Vec<Bookmark>,
    pub preferences: Value,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub exported_at: i64,
    #[serde(default)]
    pub includes_trash: bool,
}
impl Default for Library {
    fn default() -> Self {
        Self {
            format: "bookmarkd-export".into(),
            format_version: 1,
            revision: 0,
            folders: vec![],
            tags: vec![],
            bookmarks: vec![],
            preferences: json!({"version":1,"theme":"system","density":"comfortable","new_tab":true}),
            exported_at: 0,
            includes_trash: true,
        }
    }
}
pub fn valid_url(raw: &str) -> Result<url::Url> {
    ensure!(raw.len() <= 16384, "URL 超过长度限制");
    let u = url::Url::parse(raw.trim())?;
    ensure!(
        ["http", "https"].contains(&u.scheme())
            && u.host_str().is_some()
            && u.username().is_empty()
            && u.password().is_none(),
        "只允许不含账号密码的 HTTP(S) 网址"
    );
    Ok(u)
}
pub fn name(s: &str) -> Result<()> {
    ensure!(
        !s.trim().is_empty() && s.chars().count() <= 128,
        "名称应为 1–128 个字符"
    );
    Ok(())
}
impl Bookmark {
    pub fn from_input(v: &Value) -> Result<Self> {
        let now = now();
        let mut b = Self {
            id: id(),
            url_raw: String::new(),
            title: String::new(),
            notes: String::new(),
            folder_id: None,
            tags: vec![],
            pinned: false,
            position: -now,
            pinned_position: -now,
            version: 1,
            created_at: now,
            updated_at: now,
            original_created_at: None,
            deleted_at: None,
            deletion_batch: None,
        };
        b.patch(v)?;
        Ok(b)
    }
    pub fn patch(&mut self, v: &Value) -> Result<()> {
        if let Some(s) = v.get("url_raw") {
            self.url_raw = s.as_str().unwrap_or("").trim().into();
        }
        if let Some(s) = v.get("title") {
            self.title = s.as_str().unwrap_or("").into();
        }
        if let Some(s) = v.get("notes") {
            self.notes = s.as_str().unwrap_or("").into();
        }
        if let Some(s) = v.get("folder_id") {
            self.folder_id = s.as_str().map(String::from);
        }
        if let Some(s) = v.get("tags") {
            self.tags = serde_json::from_value(s.clone())?;
        }
        if let Some(b) = v.get("pinned") {
            self.pinned = b.as_bool().unwrap_or(false);
        }
        let mut seen = HashSet::new();
        self.tags.retain(|t| seen.insert(normalize(t.trim())));
        let u = valid_url(&self.url_raw)?;
        if self.title.trim().is_empty() {
            self.title = u.host_str().unwrap_or("收藏").into();
        }
        self.validate()?;
        self.updated_at = now();
        Ok(())
    }
    pub fn validate(&self) -> Result<()> {
        valid_url(&self.url_raw)?;
        ensure!(
            self.title.chars().count() <= 512
                && self.notes.chars().count() <= 16384
                && self.tags.len() <= 64,
            "字段超过长度限制"
        );
        for tag in &self.tags {
            name(tag)?;
        }
        Ok(())
    }
    pub fn score(&self, q: &str) -> u8 {
        if q.trim().is_empty() {
            return 0;
        }
        let q = normalize(q.trim());
        let title = normalize(&self.title);
        let host = valid_url(&self.url_raw)
            .ok()
            .and_then(|u| u.host_str().map(normalize))
            .unwrap_or_default();
        if title == q || host == q {
            0
        } else if title.starts_with(&q) {
            1
        } else if title.contains(&q) {
            2
        } else if normalize(&self.url_raw).contains(&q) {
            3
        } else {
            4
        }
    }
    pub fn matches(&self, q: &str) -> bool {
        let hay = normalize(&format!(
            "{} {} {} {}",
            self.title,
            self.url_raw,
            self.notes,
            self.tags.join(" ")
        ));
        normalize(q).split_whitespace().all(|w| hay.contains(w))
    }
}
impl Library {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.format == "bookmarkd-export" && self.format_version == 1,
            "不支持的文件格式"
        );
        ensure!(self.bookmarks.len() <= 50000, "最多 50000 条收藏");
        for tag in &self.tags {
            name(tag)?;
        }
        let mut ids = HashSet::new();
        for f in &self.folders {
            ensure!(ids.insert(&f.id), "目录 ID 重复");
            name(&f.name)?;
            ensure!(f.version > 0, "无效版本");
            let mut seen = HashSet::new();
            let mut parent = f.parent_id.as_ref();
            seen.insert(&f.id);
            while let Some(p) = parent {
                ensure!(seen.insert(p) && seen.len() <= 32, "目录成环或超过 32 层");
                let ancestor = self
                    .folders
                    .iter()
                    .find(|f| &f.id == p)
                    .ok_or_else(|| anyhow::anyhow!("目录引用不存在"))?;
                ensure!(
                    f.deleted_at.is_some() || ancestor.deleted_at.is_none(),
                    "父目录已删除"
                );
                parent = ancestor.parent_id.as_ref();
            }
        }
        let mut names = HashSet::new();
        for f in self.folders.iter().filter(|f| f.deleted_at.is_none()) {
            ensure!(
                names.insert((f.parent_id.clone(), normalize(f.name.trim()))),
                "同级目录名称重复"
            );
        }
        let mut bids = HashSet::new();
        for b in &self.bookmarks {
            ensure!(bids.insert(&b.id) && b.version > 0, "收藏 ID 或版本无效");
            b.validate()?;
            self.check_folder(&b.folder_id, b.deleted_at.is_some())?;
        }
        Ok(())
    }
    pub fn check_folder(&self, f: &Option<String>, deleted: bool) -> Result<()> {
        if let Some(id) = f {
            ensure!(
                self.folders
                    .iter()
                    .any(|f| &f.id == id && (deleted || f.deleted_at.is_none())),
                "目标目录不存在或已删除"
            );
        }
        Ok(())
    }
    pub fn descendants(&self, id: &str) -> HashSet<String> {
        let mut ids = HashSet::from([id.to_owned()]);
        loop {
            let n = ids.len();
            for f in &self.folders {
                if f.parent_id.as_ref().is_some_and(|p| ids.contains(p)) {
                    ids.insert(f.id.clone());
                }
            }
            if n == ids.len() {
                break;
            }
        }
        ids
    }
    pub fn import(&mut self, other: &Library, target: Option<String>, keep: bool) -> Result<Value> {
        other.validate()?;
        self.check_folder(&target, false)?;
        let mut map: HashMap<String, String> = HashMap::new();
        let mut pending: Vec<_> = other.folders.iter().collect();
        while !pending.is_empty() {
            let before = pending.len();
            pending.retain(|f| {
                if f.parent_id.as_ref().is_some_and(|p| !map.contains_key(p)) {
                    return true;
                }
                let parent = f
                    .parent_id
                    .as_ref()
                    .map(|p| map[p].clone())
                    .or(target.clone());
                // Merge matching active folders; keep deleted identities separate.
                let existing = self.folders.iter().find(|x| {
                    f.deleted_at.is_none()
                        && x.deleted_at.is_none()
                        && x.parent_id == parent
                        && normalize(x.name.trim()) == normalize(f.name.trim())
                });
                let new_id = if let Some(existing) = existing {
                    existing.id.clone()
                } else {
                    let mut new = (*f).clone();
                    new.id = id();
                    new.parent_id = parent;
                    let new_id = new.id.clone();
                    self.folders.push(new);
                    new_id
                };
                map.insert(f.id.clone(), new_id);
                false
            });
            ensure!(pending.len() < before, "目录引用成环");
        }
        for tag in &other.tags {
            if !self.tags.iter().any(|t| normalize(t) == normalize(tag)) {
                self.tags.push(tag.clone());
            }
        }
        let mut added = 0;
        let mut skipped = 0;
        for b in &other.bookmarks {
            let mut b = b.clone();
            b.id = id();
            b.folder_id = b
                .folder_id
                .as_ref()
                .map(|p| map[p].clone())
                .or(target.clone());
            let u = valid_url(&b.url_raw)?.to_string();
            if !keep
                && self.bookmarks.iter().any(|x| {
                    x.deleted_at == b.deleted_at
                        && x.folder_id == b.folder_id
                        && x.title == b.title
                        && x.notes == b.notes
                        && valid_url(&x.url_raw).is_ok_and(|x| x.as_str() == u)
                })
            {
                skipped += 1;
                continue;
            }
            self.bookmarks.push(b);
            added += 1;
        }
        self.validate()?;
        Ok(json!({"added":added,"skipped":skipped}))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_and_urls() {
        let b = Bookmark::from_input(
            &json!({"url_raw":"http://192.168.1.1/a%20b","title":"中文学习 Rust","notes":"100% _"}),
        )
        .unwrap();
        assert!(b.matches("中文 RUST"));
        assert!(b.matches("中"));
        assert!(b.matches("% _"));
        assert!(!b.matches("文 python"));
        for s in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "https://user:pass@example.com",
        ] {
            assert!(valid_url(s).is_err());
        }
    }
    #[test]
    fn folder_cycles() {
        let mut l = Library::default();
        l.folders.push(Folder {
            id: "a".into(),
            parent_id: Some("a".into()),
            name: "a".into(),
            position: 0,
            version: 1,
            deleted_at: None,
            deletion_batch: None,
        });
        assert!(l.validate().is_err());
    }
}
