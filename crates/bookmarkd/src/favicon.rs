//! Fetches a site's icon on request and returns it as a data URI.
//!
//! Only the signed-in owner can trigger this, for a URL they are bookmarking.
//! Responses are size-limited and accepted only when their bytes are a known
//! image format, so arbitrary pages are never stored or relayed.
use crate::model::{ICON_LIMIT, valid_icon, valid_url};
use anyhow::{Result, bail};
use base64::Engine;
use scraper::{Html, Selector};
use ureq::ResponseExt;

const PAGE_LIMIT: u64 = 1 << 20;
const ICON_BYTES: u64 = 70 * 1024;

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(10)))
        .user_agent(format!(
            "bookmarkd/{} (site icon)",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .into()
}

/// Image MIME subtype recognised from the leading bytes.
pub fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if bytes.starts_with(&[0, 0, 1, 0]) {
        Some("x-icon")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("gif")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("jpeg")
    } else if bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else if bytes.len() > 12 && &bytes[4..12] == b"ftypavif" {
        Some("avif")
    } else {
        let head = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]).to_lowercase();
        head.contains("<svg").then_some("svg+xml")
    }
}

/// Icon links declared by a page, best candidates first.
pub fn candidates(html: &str, base: &url::Url) -> Vec<String> {
    let doc = Html::parse_document(html);
    let selector = Selector::parse("link[rel][href]").unwrap();
    let mut found: Vec<(u8, String)> = doc
        .select(&selector)
        .filter_map(|link| {
            let el = link.value();
            let rel = el.attr("rel")?.to_ascii_lowercase();
            let rels: Vec<&str> = rel.split_ascii_whitespace().collect();
            // mask-icon is a monochrome stencil, not a usable icon.
            if !rels
                .iter()
                .any(|r| *r == "icon" || r.starts_with("apple-touch-icon"))
            {
                return None;
            }
            let href = el.attr("href")?.trim();
            let href = if href.starts_with("data:") {
                href.to_owned()
            } else {
                base.join(href).ok()?.to_string()
            };
            let size = el
                .attr("sizes")
                .and_then(|s| s.split(['x', 'X']).next()?.parse::<u32>().ok());
            let svg = el.attr("type") == Some("image/svg+xml") || href.ends_with(".svg");
            let rank = match (rels.contains(&"icon"), size) {
                _ if svg && rels.contains(&"icon") => 0,
                (true, Some(32..=96)) => 0,
                (false, _) => 1,
                (true, None) => 2,
                (true, Some(_)) => 3,
            };
            Some((rank, href))
        })
        .collect();
    found.sort_by_key(|(rank, _)| *rank);
    found.into_iter().map(|(_, href)| href).collect()
}

fn as_data_uri(bytes: &[u8]) -> Option<String> {
    let kind = sniff(bytes)?;
    let uri = format!(
        "data:image/{kind};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    );
    (uri.len() <= ICON_LIMIT).then_some(uri)
}

fn fetch_icon(agent: &ureq::Agent, href: &str) -> Option<String> {
    if href.starts_with("data:") {
        return valid_icon(href).then(|| href.to_owned());
    }
    let url = valid_url(href).ok()?;
    let mut response = agent.get(url.as_str()).call().ok()?;
    let bytes = response
        .body_mut()
        .with_config()
        .limit(ICON_BYTES)
        .read_to_vec()
        .ok()?;
    as_data_uri(&bytes)
}

/// Finds an icon for `page`: declared `<link rel=icon>` entries, then `/favicon.ico`.
pub fn fetch(page: &str) -> Result<String> {
    let page = valid_url(page)?;
    let agent = agent();
    let mut tried = vec![];
    let mut base = page.clone();
    let reachable = match agent.get(page.as_str()).call() {
        Ok(mut response) => {
            if let Ok(final_url) = url::Url::parse(&response.get_uri().to_string()) {
                base = final_url;
            }
            let html = response
                .body_mut()
                .with_config()
                .limit(PAGE_LIMIT)
                .read_to_string()
                .unwrap_or_default();
            tried = candidates(&html, &base);
            true
        }
        // Error statuses still prove the host answers; its /favicon.ico may exist.
        Err(ureq::Error::StatusCode(_)) => true,
        Err(_) => false,
    };
    if let Ok(fallback) = base.join("/favicon.ico") {
        tried.push(fallback.to_string());
    }
    for href in tried.iter().take(6) {
        if let Some(icon) = fetch_icon(&agent, href) {
            return Ok(icon);
        }
    }
    let host = page.host_str().unwrap_or_default();
    if reachable {
        bail!("{host} 没有可用的网站图标")
    }
    bail!("无法访问 {host}，请检查网址或服务器网络")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ranks_declared_icons() {
        let base = url::Url::parse("https://example.com/docs/page").unwrap();
        let html = r#"<head>
            <link rel="mask-icon" href="/mask.svg">
            <link rel="icon" sizes="16x16" href="/16.png">
            <link rel="apple-touch-icon" href="/touch.png">
            <link rel="shortcut icon" href="favicon.ico">
            <link rel="icon" sizes="32x32" href="https://cdn.example.com/32.png">
        </head>"#;
        assert_eq!(
            candidates(html, &base),
            [
                "https://cdn.example.com/32.png",
                "https://example.com/touch.png",
                "https://example.com/docs/favicon.ico",
                "https://example.com/16.png",
            ]
        );
    }
    #[test]
    fn sniffs_formats_and_rejects_pages() {
        assert_eq!(sniff(b"\x89PNG\r\n\x1a\nrest"), Some("png"));
        assert_eq!(sniff(&[0, 0, 1, 0, 1, 0]), Some("x-icon"));
        assert_eq!(
            sniff(b"<?xml version=\"1.0\"?><svg xmlns=\"x\"/>"),
            Some("svg+xml")
        );
        assert_eq!(sniff(b"<!doctype html><title>Not found</title>"), None);
        let uri = as_data_uri(b"\x89PNG\r\n\x1a\nrest").unwrap();
        assert!(uri.starts_with("data:image/png;base64,") && valid_icon(&uri));
        assert!(as_data_uri(&[0x89; 10]).is_none());
    }
    #[test]
    fn validates_stored_icons() {
        assert!(valid_icon("data:image/png;base64,iVBORw0KGgo="));
        assert!(!valid_icon("data:text/html;base64,PGgxPg=="));
        assert!(!valid_icon("https://example.com/favicon.ico"));
        assert!(!valid_icon("data:image/png;base64,<script>"));
        assert!(!valid_icon(&format!(
            "data:image/png;base64,{}",
            "A".repeat(ICON_LIMIT)
        )));
    }
}
