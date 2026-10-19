//! HTML sanitization.
//!
//! Two profiles:
//!
//! * **Outbound** (templates, API sends): keeps the table/inline-style markup
//!   email needs, removes scripts, event handlers, forms, frames and
//!   dangerous URL schemes. Applied *after* template rendering so variables
//!   can never inject active content.
//! * **Inbound** (webmail display): same tag policy, links forced to open in
//!   a new context without referrer, and `cid:` images inlined as `data:`
//!   URIs. The result is shown in a sandboxed iframe (no scripts) whose own
//!   CSP blocks remote loads unless the reader opts in, so tracking pixels
//!   and CSS `url()` beacons do not fire by default (guide 04/05/07).

use std::collections::{HashMap, HashSet};

use ammonia::{Builder, UrlRelative};
use once_cell::sync::Lazy;
use regex::Regex;

const TAGS: &[&str] = &[
    "a",
    "abbr",
    "address",
    "article",
    "b",
    "bdi",
    "bdo",
    "blockquote",
    "br",
    "caption",
    "center",
    "cite",
    "code",
    "col",
    "colgroup",
    "dd",
    "del",
    "details",
    "dfn",
    "div",
    "dl",
    "dt",
    "em",
    "figcaption",
    "figure",
    "font",
    "footer",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hr",
    "i",
    "img",
    "ins",
    "kbd",
    "li",
    "main",
    "mark",
    "nav",
    "ol",
    "p",
    "pre",
    "q",
    "s",
    "samp",
    "section",
    "small",
    "span",
    "strike",
    "strong",
    "style",
    "sub",
    "summary",
    "sup",
    "table",
    "tbody",
    "td",
    "tfoot",
    "th",
    "thead",
    "time",
    "tr",
    "tt",
    "u",
    "ul",
    "var",
    "wbr",
];

const GENERIC_ATTRS: &[&str] = &[
    "style",
    "class",
    "id",
    "align",
    "valign",
    "width",
    "height",
    "bgcolor",
    "color",
    "dir",
    "lang",
    "title",
    "role",
    "border",
    "background",
];

fn base_builder() -> Builder<'static> {
    let mut b = Builder::default();
    b.tags(TAGS.iter().copied().collect::<HashSet<_>>());
    b.clean_content_tags(["script", "noscript", "iframe", "object", "embed", "template"].into_iter().collect());
    b.generic_attributes(GENERIC_ATTRS.iter().copied().collect::<HashSet<_>>());
    let mut tag_attrs: HashMap<&'static str, HashSet<&'static str>> = HashMap::new();
    tag_attrs.insert("a", ["href", "name", "target"].into_iter().collect());
    tag_attrs.insert("img", ["src", "alt", "width", "height", "border"].into_iter().collect());
    tag_attrs.insert("table", ["cellpadding", "cellspacing", "summary", "frame", "rules"].into_iter().collect());
    tag_attrs.insert("td", ["colspan", "rowspan", "nowrap", "abbr", "headers", "scope"].into_iter().collect());
    tag_attrs.insert("th", ["colspan", "rowspan", "nowrap", "abbr", "headers", "scope"].into_iter().collect());
    tag_attrs.insert("col", ["span"].into_iter().collect());
    tag_attrs.insert("colgroup", ["span"].into_iter().collect());
    tag_attrs.insert("font", ["face", "size"].into_iter().collect());
    tag_attrs.insert("ol", ["start", "type"].into_iter().collect());
    tag_attrs.insert("time", ["datetime"].into_iter().collect());
    b.tag_attributes(tag_attrs);
    b.strip_comments(true);
    b.url_relative(UrlRelative::Deny);
    b.link_rel(Some("noopener noreferrer nofollow"));
    b
}

static OUTBOUND: Lazy<Builder<'static>> = Lazy::new(|| {
    let mut b = base_builder();
    b.url_schemes(["http", "https", "mailto", "tel", "cid"].into_iter().collect());
    b
});

static INBOUND: Lazy<Builder<'static>> = Lazy::new(|| {
    let mut b = base_builder();
    b.url_schemes(["http", "https", "mailto", "tel", "data"].into_iter().collect());
    // Forced value; must not also be an allowed (pass-through) attribute.
    b.rm_tag_attributes("a", &["target"]);
    b.set_tag_attribute_value("a", "target", "_blank");
    // data: is allowed only for images (inlined cid parts); never for links.
    b.attribute_filter(|element, attribute, value| {
        if attribute == "href" && value.trim_start().to_ascii_lowercase().starts_with("data:") {
            return None;
        }
        if element == "img" && attribute == "src" {
            let v = value.trim_start().to_ascii_lowercase();
            if v.starts_with("data:") && !v.starts_with("data:image/") {
                return None;
            }
        }
        Some(value.into())
    });
    b
});

static STYLE_EXPRESSION: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)expression\s*\(|javascript:|behavior\s*:|-moz-binding").unwrap());

fn scrub_css(html: String) -> String {
    // Legacy CSS execution vectors; harmless in modern clients but trivial to remove.
    STYLE_EXPRESSION.replace_all(&html, "blocked:").into_owned()
}

/// Sanitize HTML that we will send.
pub fn outbound(html: &str) -> String {
    scrub_css(OUTBOUND.clean(html).to_string())
}

/// Wrap a sanitized fragment in a minimal, client-friendly document.
pub fn wrap_document(fragment: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"></head><body>{fragment}</body></html>"
    )
}

static REMOTE_REF: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)(src\s*=\s*["']?\s*https?:)|(url\(\s*["']?\s*https?:)|(background\s*=\s*["']?\s*https?:)"#)
        .unwrap()
});

pub struct InboundHtml {
    /// A complete document for an `<iframe sandbox srcdoc>`.
    pub document: String,
    pub has_remote_content: bool,
}

/// Sanitize received HTML for display. `inline_parts` maps Content-ID
/// (without angle brackets) to a `data:` URI.
pub fn inbound(html: &str, inline_parts: &HashMap<String, String>, allow_remote: bool) -> InboundHtml {
    // Resolve cid: references before sanitizing so the data: URIs pass the
    // image-only data: filter.
    let mut resolved = html.to_string();
    for (cid, data_uri) in inline_parts {
        resolved = resolved.replace(&format!("cid:{cid}"), data_uri);
    }
    let cleaned = scrub_css(INBOUND.clean(&resolved).to_string());
    let has_remote_content = REMOTE_REF.is_match(&cleaned);
    let img_src = if allow_remote { "data: https: http:" } else { "data:" };
    let csp = format!(
        "default-src 'none'; style-src 'unsafe-inline'; img-src {img_src}; font-src {font}; media-src 'none'",
        font = if allow_remote { "https: data:" } else { "data:" }
    );
    let document = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"{csp}\"><meta name=\"referrer\" content=\"no-referrer\"><base target=\"_blank\"><style>body{{margin:0;padding:16px;font-family:-apple-system,Segoe UI,Roboto,Helvetica,Arial,sans-serif;font-size:14px;line-height:1.5;color:#0f172a;word-wrap:break-word}}img{{max-width:100%;height:auto}}pre{{white-space:pre-wrap}}</style></head><body>{cleaned}</body></html>"
    );
    InboundHtml { document, has_remote_content }
}

/// Render plain text as safe HTML (for text-only messages in webmail).
pub fn text_to_html(text: &str) -> String {
    let escaped = crate::services::system_mail::escape_html(text);
    static URL: Lazy<Regex> = Lazy::new(|| Regex::new(r#"https?://[^\s<>"']+"#).unwrap());
    let linked = URL.replace_all(&escaped, |c: &regex::Captures| {
        let u = &c[0];
        format!("<a href=\"{u}\">{u}</a>")
    });
    format!("<pre style=\"font-family:inherit;white-space:pre-wrap;margin:0\">{linked}</pre>")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outbound_strips_active_content_but_keeps_layout() {
        let dirty = r##"<table cellpadding="4" style="width:100%"><tr><td bgcolor="#fff" onclick="x()">Hi</td></tr></table>
            <script>alert(1)</script><a href="javascript:alert(1)">x</a><a href="https://ok.example">ok</a>
            <img src="https://img.example/a.png" onerror="x()"><iframe src="https://evil"></iframe>
            <div style="background:url(javascript:alert(1))">s</div>"##;
        let clean = outbound(dirty);
        assert!(clean.contains("cellpadding=\"4\""));
        assert!(clean.contains("style=\"width:100%\""));
        assert!(!clean.contains("script"));
        assert!(!clean.contains("onclick"));
        assert!(!clean.contains("onerror"));
        assert!(!clean.contains("javascript:"));
        assert!(!clean.contains("iframe"));
        assert!(clean.contains("https://ok.example"));
    }

    #[test]
    fn inbound_blocks_remote_by_csp_and_inlines_cid() {
        let mut parts = HashMap::new();
        parts.insert("logo@x".to_string(), "data:image/png;base64,AAAA".to_string());
        let r = inbound(
            r#"<img src="cid:logo@x"><img src="https://tracker.example/p.gif"><a href="data:text/html,x">bad</a><a href="https://a.example">a</a>"#,
            &parts,
            false,
        );
        assert!(r.has_remote_content);
        assert!(r.document.contains("img-src data:;"));
        assert!(r.document.contains("data:image/png;base64,AAAA"));
        assert!(!r.document.contains("data:text/html"));
        assert!(r.document.contains("target=\"_blank\""));
        let allowed = inbound("<p>x</p>", &parts, true);
        assert!(allowed.document.contains("img-src data: https: http:"));
        assert!(!allowed.has_remote_content);
    }
}
