//! IMAP access to Dovecot for webmail and quota reporting.
//!
//! The platform authenticates the mailbox user itself (Argon2 hash in the
//! database, lockout, sessions) and then opens IMAP as Dovecot's *master
//! user* on that mailbox (`address*master`). Mailbox passwords therefore
//! never need to be stored recoverably or kept in the browser.
//!
//! One short-lived connection per request keeps the design stateless; the
//! IMAP server sits on the private network next to the API.

use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use async_imap::{types::Fetch, Session};
use futures::TryStreamExt;
use mail_parser::MimeHeaders;
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::TcpStream,
};

use crate::config::{ImapConfig, TlsMode};

const TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug)]
pub enum ImapStream {
    Plain(TcpStream),
    Tls(Box<tokio_rustls::client::TlsStream<TcpStream>>),
}

impl AsyncRead for ImapStream {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            ImapStream::Plain(s) => Pin::new(s).poll_read(cx, buf),
            ImapStream::Tls(s) => Pin::new(s.as_mut()).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for ImapStream {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<std::io::Result<usize>> {
        match self.get_mut() {
            ImapStream::Plain(s) => Pin::new(s).poll_write(cx, buf),
            ImapStream::Tls(s) => Pin::new(s.as_mut()).poll_write(cx, buf),
        }
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            ImapStream::Plain(s) => Pin::new(s).poll_flush(cx),
            ImapStream::Tls(s) => Pin::new(s.as_mut()).poll_flush(cx),
        }
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            ImapStream::Plain(s) => Pin::new(s).poll_shutdown(cx),
            ImapStream::Tls(s) => Pin::new(s.as_mut()).poll_shutdown(cx),
        }
    }
}

pub type ImapSession = Session<ImapStream>;

async fn open_stream(cfg: &ImapConfig) -> anyhow::Result<ImapStream> {
    let tcp = tokio::time::timeout(TIMEOUT, TcpStream::connect((cfg.host.as_str(), cfg.port))).await??;
    match cfg.tls {
        TlsMode::None => Ok(ImapStream::Plain(tcp)),
        TlsMode::Implicit => {
            let mut roots = tokio_rustls::rustls::RootCertStore::empty();
            roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            let provider = std::sync::Arc::new(tokio_rustls::rustls::crypto::ring::default_provider());
            let config = tokio_rustls::rustls::ClientConfig::builder_with_provider(provider)
                .with_safe_default_protocol_versions()?
                .with_root_certificates(roots)
                .with_no_client_auth();
            let connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(config));
            let name = tokio_rustls::rustls::pki_types::ServerName::try_from(cfg.host.clone())?;
            Ok(ImapStream::Tls(Box::new(connector.connect(name, tcp).await?)))
        }
        TlsMode::StartTls => {
            anyhow::bail!("IMAP_TLS=starttls is not supported; use 'tls' (port 993) or 'none' on a private network")
        }
    }
}

/// Open an authenticated session on `address` using the master user.
pub async fn connect(cfg: &ImapConfig, address: &str) -> anyhow::Result<ImapSession> {
    let stream = open_stream(cfg).await?;
    let mut client = async_imap::Client::new(stream);
    // Consume the server greeting before issuing commands.
    let _greeting = tokio::time::timeout(TIMEOUT, client.read_response())
        .await?
        .ok_or_else(|| anyhow::anyhow!("IMAP server closed the connection"))??;
    let login = format!("{address}*{}", cfg.master_user);
    let session = tokio::time::timeout(TIMEOUT, client.login(login, &cfg.master_password))
        .await?
        .map_err(|(e, _)| anyhow::anyhow!("IMAP login failed: {e}"))?;
    Ok(session)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Folder {
    pub name: String,
    pub display_name: String,
    pub role: Option<&'static str>,
    pub total: u32,
    pub unseen: u32,
}

fn role_of(name: &str, attributes: &[String]) -> Option<&'static str> {
    let has = |a: &str| attributes.iter().any(|x| x.eq_ignore_ascii_case(a));
    if name.eq_ignore_ascii_case("INBOX") {
        Some("inbox")
    } else if has("\\Sent") {
        Some("sent")
    } else if has("\\Drafts") {
        Some("drafts")
    } else if has("\\Trash") {
        Some("trash")
    } else if has("\\Junk") {
        Some("junk")
    } else if has("\\Archive") {
        Some("archive")
    } else {
        match name.to_ascii_lowercase().as_str() {
            "sent" | "sent items" | "sent messages" => Some("sent"),
            "drafts" => Some("drafts"),
            "trash" | "deleted items" => Some("trash"),
            "junk" | "spam" => Some("junk"),
            "archive" => Some("archive"),
            _ => None,
        }
    }
}

pub async fn list_folders(session: &mut ImapSession) -> anyhow::Result<Vec<Folder>> {
    let names: Vec<_> = session.list(Some(""), Some("*")).await?.try_collect().await?;
    let mut folders = Vec::new();
    for n in &names {
        let attributes: Vec<String> = n.attributes().iter().map(|a| format!("{a:?}")).map(normalize_attr).collect();
        if attributes.iter().any(|a| a.eq_ignore_ascii_case("\\Noselect")) {
            continue;
        }
        let name = n.name().to_string();
        let status = session.status(&name, "(MESSAGES UNSEEN)").await?;
        folders.push(Folder {
            display_name: decode_modified_utf7(&name),
            role: role_of(&name, &attributes),
            total: status.exists,
            unseen: status.unseen.unwrap_or(0),
            name,
        });
    }
    let order = |f: &Folder| match f.role {
        Some("inbox") => 0,
        Some("drafts") => 1,
        Some("sent") => 2,
        Some("archive") => 3,
        Some("junk") => 4,
        Some("trash") => 5,
        _ => 6,
    };
    folders.sort_by(|a, b| {
        order(a).cmp(&order(b)).then(a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()))
    });
    Ok(folders)
}

/// imap-proto exposes attributes as enum variants (`Sent`, `NoSelect`, …)
/// or `Extension("\Foo")`; normalize both to the RFC form `\Foo`.
fn normalize_attr(debug: String) -> String {
    for prefix in ["Extension(\"", "Custom(\""] {
        if let Some(inner) = debug.strip_prefix(prefix).and_then(|s| s.strip_suffix("\")")) {
            return inner.replace("\\\\", "\\");
        }
    }
    if debug.starts_with('\\') {
        debug
    } else {
        format!("\\{debug}")
    }
}

#[cfg(test)]
#[test]
fn attribute_normalization() {
    assert_eq!(normalize_attr("Sent".into()), "\\Sent");
    assert_eq!(normalize_attr("NoSelect".into()), "\\NoSelect");
    assert_eq!(normalize_attr("Extension(\"\\\\Important\")".into()), "\\Important");
    assert_eq!(role_of("Sent Items", &[normalize_attr("Sent".into())]), Some("sent"));
}

pub fn find_role<'a>(folders: &'a [Folder], role: &str) -> Option<&'a Folder> {
    folders.iter().find(|f| f.role == Some(role))
}

#[derive(Debug, serde::Serialize)]
pub struct Summary {
    pub uid: u32,
    pub from: Option<String>,
    pub from_address: Option<String>,
    pub to: Vec<String>,
    pub subject: String,
    pub date: Option<String>,
    pub size: u32,
    pub seen: bool,
    pub flagged: bool,
    pub answered: bool,
    pub has_attachments: bool,
}

fn flag_set(f: &Fetch, flag: &str) -> bool {
    f.flags().any(|x| format!("{x:?}").eq_ignore_ascii_case(flag))
}

/// Newest-first page of a folder, optionally filtered by an IMAP TEXT search.
pub async fn list_messages(
    session: &mut ImapSession,
    folder: &str,
    page: u32,
    per_page: u32,
    query: Option<&str>,
) -> anyhow::Result<(usize, Vec<Summary>)> {
    session.select(folder).await?;
    let criteria = match query {
        Some(q) if !q.trim().is_empty() => format!("TEXT {}", quote(q.trim())),
        _ => "ALL".to_string(),
    };
    let mut uids: Vec<u32> = session.uid_search(&criteria).await?.into_iter().collect();
    uids.sort_unstable_by(|a, b| b.cmp(a));
    let total = uids.len();
    let start = (page.saturating_sub(1) * per_page) as usize;
    let slice: Vec<u32> = uids.into_iter().skip(start).take(per_page as usize).collect();
    if slice.is_empty() {
        return Ok((total, vec![]));
    }
    let set = slice.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
    let fetches: Vec<Fetch> = session
        .uid_fetch(&set, "(UID FLAGS RFC822.SIZE BODY.PEEK[HEADER.FIELDS (FROM TO CC SUBJECT DATE CONTENT-TYPE)])")
        .await?
        .try_collect()
        .await?;
    let mut out: Vec<Summary> = fetches
        .iter()
        .filter_map(|f| {
            let uid = f.uid?;
            let header = f.header().or_else(|| f.body()).unwrap_or_default();
            let parsed = mail_parser::MessageParser::default().parse_headers(header);
            let (from, from_address) = parsed
                .as_ref()
                .and_then(|m| m.from())
                .and_then(|a| a.first())
                .map(|a| (a.name().map(str::to_string), a.address().map(str::to_string)))
                .unwrap_or((None, None));
            let to = parsed
                .as_ref()
                .and_then(|m| m.to())
                .map(|list| list.iter().filter_map(|a| a.address().map(str::to_string)).collect())
                .unwrap_or_default();
            let content_type = parsed
                .as_ref()
                .and_then(|m| m.content_type())
                .map(|ct| format!("{}/{}", ct.ctype(), ct.subtype().unwrap_or_default()).to_ascii_lowercase())
                .unwrap_or_default();
            Some(Summary {
                uid,
                from,
                from_address,
                to,
                subject: parsed.as_ref().and_then(|m| m.subject()).unwrap_or("(no subject)").to_string(),
                date: parsed.as_ref().and_then(|m| m.date()).map(|d| d.to_rfc3339()),
                size: f.size.unwrap_or(0),
                seen: flag_set(f, "Seen"),
                flagged: flag_set(f, "Flagged"),
                answered: flag_set(f, "Answered"),
                has_attachments: content_type == "multipart/mixed",
            })
        })
        .collect();
    out.sort_unstable_by_key(|s| std::cmp::Reverse(s.uid));
    Ok((total, out))
}

/// Full raw message. Does not set \Seen (BODY.PEEK).
pub async fn fetch_raw(session: &mut ImapSession, folder: &str, uid: u32) -> anyhow::Result<Option<Vec<u8>>> {
    session.select(folder).await?;
    let fetches: Vec<Fetch> = session.uid_fetch(uid.to_string(), "(UID BODY.PEEK[])").await?.try_collect().await?;
    Ok(fetches.iter().find(|f| f.uid == Some(uid)).and_then(|f| f.body().map(<[u8]>::to_vec)))
}

pub async fn store_flags(
    session: &mut ImapSession,
    folder: &str,
    uids: &[u32],
    add: bool,
    flags: &str,
) -> anyhow::Result<()> {
    if uids.is_empty() {
        return Ok(());
    }
    session.select(folder).await?;
    let set = uid_set(uids);
    let op = if add { format!("+FLAGS.SILENT ({flags})") } else { format!("-FLAGS.SILENT ({flags})") };
    let _: Vec<Fetch> = session.uid_store(&set, &op).await?.try_collect().await?;
    Ok(())
}

pub async fn move_messages(
    session: &mut ImapSession,
    folder: &str,
    uids: &[u32],
    destination: &str,
) -> anyhow::Result<()> {
    if uids.is_empty() {
        return Ok(());
    }
    session.select(folder).await?;
    session.uid_mv(uid_set(uids), destination).await?;
    Ok(())
}

/// Permanently remove messages (used for deletes from Trash).
pub async fn expunge(session: &mut ImapSession, folder: &str, uids: &[u32]) -> anyhow::Result<()> {
    if uids.is_empty() {
        return Ok(());
    }
    session.select(folder).await?;
    let set = uid_set(uids);
    let _: Vec<Fetch> = session.uid_store(&set, "+FLAGS.SILENT (\\Deleted)").await?.try_collect().await?;
    let _: Vec<u32> = session.uid_expunge(&set).await?.try_collect().await?;
    Ok(())
}

pub async fn append(session: &mut ImapSession, folder: &str, flags: &str, raw: &[u8]) -> anyhow::Result<()> {
    session.append(folder, Some(flags), None, raw).await?;
    Ok(())
}

/// (used, limit) in bytes from Dovecot's quota plugin, if quotas are enabled.
pub async fn quota_usage(cfg: &ImapConfig, address: &str) -> anyhow::Result<Option<(u64, u64)>> {
    let mut session = connect(cfg, address).await?;
    let result = session.get_quota_root("INBOX").await;
    let _ = session.logout().await;
    let (_roots, quotas) = result?;
    for q in quotas {
        for r in q.resources {
            if format!("{:?}", r.name).to_ascii_uppercase().contains("STORAGE") {
                // IMAP QUOTA reports STORAGE in KiB (RFC 2087).
                return Ok(Some((r.usage * 1024, r.limit * 1024)));
            }
        }
    }
    Ok(None)
}

fn uid_set(uids: &[u32]) -> String {
    uids.iter().map(u32::to_string).collect::<Vec<_>>().join(",")
}

/// IMAP quoted string with escaping; rejects control characters.
fn quote(s: &str) -> String {
    let cleaned: String = s.chars().filter(|c| !c.is_control()).take(200).collect();
    format!("\"{}\"", cleaned.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Decode IMAP modified UTF-7 (RFC 3501 §5.1.3) folder names for display.
pub fn decode_modified_utf7(name: &str) -> String {
    use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine};
    let mut out = String::new();
    let mut rest = name;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('-') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let encoded = &after[..end];
        if encoded.is_empty() {
            out.push('&');
        } else if let Ok(bytes) = STANDARD_NO_PAD.decode(encoded.replace(',', "/")) {
            let units: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
            out.push_str(&String::from_utf16_lossy(&units));
        } else {
            out.push('&');
            out.push_str(encoded);
            out.push('-');
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Validate a folder name supplied by the browser before sending it to IMAP.
pub fn check_folder_name(name: &str) -> Result<(), &'static str> {
    if name.is_empty() || name.len() > 200 || name.chars().any(|c| c.is_control() || c == '*' || c == '%') {
        return Err("Invalid folder name");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutf7() {
        assert_eq!(decode_modified_utf7("INBOX"), "INBOX");
        assert_eq!(decode_modified_utf7("&ZeVnLIqe-"), "日本語");
        assert_eq!(decode_modified_utf7("A &- B"), "A & B");
    }

    #[test]
    fn quoting() {
        assert_eq!(quote("a\"b\\c\r\n"), "\"a\\\"b\\\\c\"");
        assert!(check_folder_name("Sent").is_ok());
        assert!(check_folder_name("x\r\nA1 LOGOUT").is_err());
        assert!(check_folder_name("*").is_err());
    }

    #[test]
    fn roles() {
        assert_eq!(role_of("INBOX", &[]), Some("inbox"));
        assert_eq!(role_of("Whatever", &["\\Sent".into()]), Some("sent"));
        assert_eq!(role_of("Spam", &[]), Some("junk"));
    }
}
