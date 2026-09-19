use crate::model::{Bookmark, Folder, Library, id};
use anyhow::{Result, ensure};
use scraper::{ElementRef, Html, Selector};
use serde_json::{Value, json};
#[derive(Clone)]
pub struct Import {
    pub library: Library,
    pub warnings: Vec<String>,
    pub expires: i64,
    pub digest: String,
}
pub fn parse(content: &str, format: &str) -> Result<(Library, Vec<String>)> {
    ensure!(content.len() <= 20 * 1024 * 1024, "文件超过 20 MiB");
    ensure!(
        !content.contains('\u{fffd}'),
        "编码错误：请转换为 UTF-8 后重试"
    );
    if format == "json" {
        let l: Library = serde_json::from_str(content.trim_start_matches('\u{feff}'))?;
        l.validate()?;
        return Ok((l, vec![]));
    }
    ensure!(format == "html", "不支持的格式");
    let html = Html::parse_document(content);
    let selector = Selector::parse("a, h3, dl").unwrap();
    let mut l = Library::default();
    let mut warnings = vec![];
    let mut folder_for_dl = std::collections::HashMap::new();
    let mut pending = std::collections::HashMap::new();
    for el in html.select(&selector) {
        let parent_dl = el
            .ancestors()
            .skip(1)
            .filter_map(ElementRef::wrap)
            .find(|e| e.value().name() == "dl")
            .map(|e| e.id());
        let parent = parent_dl
            .and_then(|x| folder_for_dl.get(&x).cloned())
            .flatten();
        match el.value().name() {
            "h3" => {
                let f = Folder {
                    id: id(),
                    name: el
                        .text()
                        .collect::<String>()
                        .trim()
                        .chars()
                        .take(128)
                        .collect(),
                    parent_id: parent.clone(),
                    position: l.folders.len() as i64,
                    version: 1,
                    deleted_at: None,
                    deletion_batch: None,
                };
                if f.name.is_empty() {
                    warnings.push("跳过无名称目录".into());
                    continue;
                }
                pending.insert(parent_dl, Some(f.id.clone()));
                l.folders.push(f);
            }
            "dl" => {
                let target = pending.remove(&parent_dl).flatten().or(parent);
                folder_for_dl.insert(el.id(), target);
            }
            "a" => {
                let raw = el.value().attr("href").unwrap_or("");
                match Bookmark::from_input(
                    &json!({"url_raw":raw,"title":el.text().collect::<String>(),"folder_id":parent}),
                ) {
                    Ok(mut b) => {
                        b.original_created_at =
                            el.value().attr("add_date").and_then(|s| s.parse().ok());
                        l.bookmarks.push(b);
                    }
                    Err(e) => warnings.push(format!(
                        "条目 {}：{}",
                        l.bookmarks.len() + warnings.len() + 1,
                        e
                    )),
                }
            }
            _ => {}
        }
        ensure!(l.bookmarks.len() <= 50000, "最多 50000 条收藏");
    }
    l.validate()?;
    Ok((l, warnings))
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
pub fn html(l: &Library) -> String {
    fn contents(l: &Library, p: Option<&str>, out: &mut String) {
        out.push_str("<DL><p>\n");
        for f in l
            .folders
            .iter()
            .filter(|f| f.deleted_at.is_none() && f.parent_id.as_deref() == p)
        {
            out.push_str(&format!("<DT><H3>{}</H3>\n", escape(&f.name)));
            contents(l, Some(&f.id), out);
        }
        for b in l
            .bookmarks
            .iter()
            .filter(|b| b.deleted_at.is_none() && b.folder_id.as_deref() == p)
        {
            out.push_str(&format!(
                "<DT><A HREF=\"{}\" ADD_DATE=\"{}\">{}</A>\n",
                escape(&b.url_raw),
                b.original_created_at.unwrap_or(b.created_at),
                escape(&b.title)
            ));
        }
        out.push_str("</DL><p>\n");
    }
    let mut out="<!DOCTYPE NETSCAPE-Bookmark-file-1>\n<META HTTP-EQUIV=\"Content-Type\" CONTENT=\"text/html; charset=UTF-8\">\n<TITLE>Bookmarks</TITLE>\n<H1>Bookmarks</H1>\n".to_owned();
    contents(l, None, &mut out);
    out
}
pub fn preview(i: &Import) -> Value {
    json!({"parsed":i.library.bookmarks.len(),"folders":i.library.folders.len(),"warnings":i.warnings,"expires_at":i.expires,"digest":i.digest})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn html_roundtrip() {
        let (l,w)=parse("<DL><p><DT><H3>中文</H3><DL><p><DT><A HREF=\"https://example.com/?a=1&amp;b=2\">标题</A><DT><A HREF=\"javascript:alert(1)\">bad</A></DL><p></DL>","html").unwrap();
        assert_eq!(w.len(), 1);
        assert_eq!(l.bookmarks.len(), 1);
        assert_eq!(l.bookmarks[0].folder_id.as_ref(), Some(&l.folders[0].id));
        let (again, _) = parse(&html(&l), "html").unwrap();
        assert_eq!(again.bookmarks[0].title, "标题");
        assert_eq!(again.bookmarks[0].url_raw, "https://example.com/?a=1&b=2");
    }
}
