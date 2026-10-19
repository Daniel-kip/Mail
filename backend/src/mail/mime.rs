//! MIME message construction.
//!
//! Every outbound message (API, console test send, webmail, system mail) is
//! built here so headers are produced one way, by lettre's encoder, which
//! handles RFC 2047 encoding of non-ASCII names and subjects. Callers must
//! still validate inputs first (`validation::header_safe`).

use lettre::message::{
    header::{ContentType, HeaderName, HeaderValue},
    Attachment, Mailbox, Message, MultiPart, SinglePart,
};

#[derive(Debug, Clone)]
pub struct Addr {
    pub name: Option<String>,
    pub email: String,
}

impl Addr {
    pub fn new(email: impl Into<String>) -> Self {
        Self { name: None, email: email.into() }
    }
    pub fn named(name: Option<String>, email: impl Into<String>) -> Self {
        Self { name: name.filter(|n| !n.trim().is_empty()), email: email.into() }
    }
    fn mailbox(&self) -> anyhow::Result<Mailbox> {
        Ok(Mailbox::new(self.name.clone(), self.email.parse()?))
    }
}

#[derive(Debug, Clone)]
pub struct OutAttachment {
    pub filename: String,
    pub content_type: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Default)]
pub struct OutgoingEmail {
    pub from: Option<Addr>,
    pub to: Vec<Addr>,
    pub cc: Vec<Addr>,
    pub reply_to: Option<Addr>,
    pub subject: String,
    /// Full Message-ID including angle brackets.
    pub message_id: String,
    pub in_reply_to: Option<String>,
    pub references: Option<String>,
    pub text: Option<String>,
    pub html: Option<String>,
    pub attachments: Vec<OutAttachment>,
    /// Extra headers; names must be ASCII `X-…` tokens and values header-safe.
    pub headers: Vec<(String, String)>,
}

pub fn build(email: &OutgoingEmail) -> anyhow::Result<Message> {
    let from = email.from.as_ref().ok_or_else(|| anyhow::anyhow!("From is required"))?;
    let mut b = Message::builder()
        .from(from.mailbox()?)
        .subject(email.subject.clone())
        .message_id(Some(email.message_id.clone()))
        .date_now()
        .user_agent("Mailvora".to_string());
    for to in &email.to {
        b = b.to(to.mailbox()?);
    }
    if email.to.is_empty() && email.cc.is_empty() {
        // Drafts may have no recipients yet. lettre derives the SMTP envelope
        // from the headers and refuses an empty one, so give it an explicit
        // envelope. Drafts are only stored over IMAP, never submitted.
        let addr: lettre::Address = from.email.parse()?;
        b = b.envelope(lettre::address::Envelope::new(Some(addr.clone()), vec![addr])?);
    }
    for cc in &email.cc {
        b = b.cc(cc.mailbox()?);
    }
    if let Some(rt) = &email.reply_to {
        b = b.reply_to(rt.mailbox()?);
    }
    if let Some(irt) = &email.in_reply_to {
        b = b.in_reply_to(irt.clone());
    }
    if let Some(refs) = &email.references {
        b = b.references(refs.clone());
    }
    for (name, value) in &email.headers {
        let header_name =
            HeaderName::new_from_ascii(name.clone()).map_err(|_| anyhow::anyhow!("invalid header name {name}"))?;
        b = b.raw_header(HeaderValue::new(header_name, value.clone()));
    }

    let text_part = |t: &str| SinglePart::builder().header(ContentType::TEXT_PLAIN).body(t.to_string());
    let html_part = |h: &str| SinglePart::builder().header(ContentType::TEXT_HTML).body(h.to_string());

    let content: MultiPart = match (&email.text, &email.html) {
        (Some(t), Some(h)) => MultiPart::alternative().singlepart(text_part(t)).singlepart(html_part(h)),
        (None, Some(h)) => MultiPart::mixed().singlepart(html_part(h)),
        (Some(t), None) => MultiPart::mixed().singlepart(text_part(t)),
        (None, None) => MultiPart::mixed().singlepart(text_part("")),
    };

    if email.attachments.is_empty() {
        // Avoid a needless multipart wrapper around a single body.
        return Ok(match (&email.text, &email.html) {
            (Some(_), Some(_)) => b.multipart(content)?,
            (None, Some(h)) => b.singlepart(html_part(h))?,
            (Some(t), None) => b.singlepart(text_part(t))?,
            (None, None) => b.singlepart(text_part(""))?,
        });
    }

    let mut mixed = MultiPart::mixed().multipart(content);
    for a in &email.attachments {
        let ct = ContentType::parse(&a.content_type)
            .unwrap_or_else(|_| ContentType::parse("application/octet-stream").unwrap());
        mixed = mixed.singlepart(Attachment::new(a.filename.clone()).body(a.data.clone(), ct));
    }
    Ok(b.multipart(mixed)?)
}

/// `<id@domain>` using a time-ordered UUID for natural sorting in logs.
pub fn new_message_id(id: uuid::Uuid, domain: &str) -> String {
    format!("<{}@{}>", id.simple(), domain)
}

/// Make a user-supplied filename safe for a Content-Disposition header and
/// for any filesystem: strip paths, control characters and quotes.
pub fn sanitize_filename(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = base
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '"' | '<' | '>' | ':' | '|' | '?' | '*'))
        .take(150)
        .collect();
    let cleaned = cleaned.trim().trim_start_matches('.').to_string();
    if cleaned.is_empty() {
        "attachment".into()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_alternative_with_attachment() {
        let m = build(&OutgoingEmail {
            from: Some(Addr::named(Some("Ünïcode Sender".into()), "a@example.com")),
            to: vec![Addr::new("b@example.org")],
            subject: "Grüße".into(),
            message_id: "<x@example.com>".into(),
            text: Some("hi".into()),
            html: Some("<p>hi</p>".into()),
            attachments: vec![OutAttachment {
                filename: "r.txt".into(),
                content_type: "text/plain".into(),
                data: b"x".to_vec(),
            }],
            headers: vec![("X-Mailvora-Message-Id".into(), "abc".into())],
            ..Default::default()
        })
        .unwrap();
        let raw = String::from_utf8(m.formatted()).unwrap();
        assert!(raw.contains("multipart/mixed"));
        assert!(raw.contains("multipart/alternative"));
        assert!(raw.contains("Message-ID: <x@example.com>"));
        assert!(raw.contains("X-Mailvora-Message-Id: abc"));
        assert!(raw.contains("=?utf-8?"), "non-ASCII must be RFC 2047 encoded");
    }

    #[test]
    fn drafts_without_recipients_build() {
        let m = build(&OutgoingEmail {
            from: Some(Addr::new("a@example.com")),
            subject: "Unfinished".into(),
            message_id: "<d@example.com>".into(),
            text: Some("...".into()),
            ..Default::default()
        })
        .unwrap();
        assert!(!String::from_utf8(m.formatted()).unwrap().contains("To:"));
    }

    #[test]
    fn filenames() {
        assert_eq!(sanitize_filename("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_filename("C:\\x\\re\"port.pdf"), "report.pdf");
        assert_eq!(sanitize_filename("..."), "attachment");
    }
}
