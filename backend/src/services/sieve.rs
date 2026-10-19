//! Out-of-office (vacation) auto-replies through Dovecot Pigeonhole.
//!
//! Dovecot's LMTP runs the mailbox's active Sieve script at delivery time,
//! so auto-replies are produced by the mail server itself (correct
//! `Auto-Submitted`, per-sender rate limiting via `:days`, no reply to
//! mailing lists or bulk mail per RFC 5230). The platform installs the
//! script over ManageSieve (RFC 5804) on the private network, logging in as
//! the Dovecot master user on the mailbox, the same trust model as webmail
//! IMAP. User text is escaped into Sieve quoted strings; it can never alter
//! the script structure.

use std::time::Duration;

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chrono::NaiveDate;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
};

use crate::config::{ImapConfig, SieveConfig};

pub const SCRIPT_NAME: &str = "mailvora-out-of-office";
const TIMEOUT: Duration = Duration::from_secs(8);

pub struct Vacation<'a> {
    pub subject: &'a str,
    pub body: &'a str,
    pub starts_on: Option<NaiveDate>,
    pub ends_on: Option<NaiveDate>,
}

/// Escape text into a Sieve quoted string (RFC 5228 §2.4.2).
fn quoted(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\r' => {}
            '\n' => out.push_str("\r\n"),
            c if c.is_control() && c != '\t' => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn script(v: &Vacation) -> String {
    let action = format!("vacation :days 1 :subject {} {};", quoted(v.subject), quoted(v.body));
    let mut conditions = Vec::new();
    if let Some(d) = v.starts_on {
        conditions.push(format!("currentdate :value \"ge\" \"date\" \"{}\"", d.format("%Y-%m-%d")));
    }
    if let Some(d) = v.ends_on {
        conditions.push(format!("currentdate :value \"le\" \"date\" \"{}\"", d.format("%Y-%m-%d")));
    }
    let header = "# Managed by Mailvora. Edit the auto-reply in webmail settings; manual changes are replaced.\r\n\
                  require [\"vacation\", \"date\", \"relational\"];\r\n";
    if conditions.is_empty() {
        format!("{header}{action}\r\n")
    } else {
        format!("{header}if allof ({}) {{\r\n    {action}\r\n}}\r\n", conditions.join(", "))
    }
}

struct Conn {
    reader: BufReader<TcpStream>,
}

impl Conn {
    async fn open(cfg: &SieveConfig) -> anyhow::Result<Self> {
        let stream = tokio::time::timeout(TIMEOUT, TcpStream::connect((cfg.host.as_str(), cfg.port))).await??;
        let mut c = Conn { reader: BufReader::new(stream) };
        c.expect_ok().await?; // greeting / capabilities
        Ok(c)
    }

    /// Read lines until a final OK / NO / BYE response.
    async fn expect_ok(&mut self) -> anyhow::Result<()> {
        for _ in 0..200 {
            let mut line = String::new();
            let n = tokio::time::timeout(TIMEOUT, self.reader.read_line(&mut line)).await??;
            if n == 0 {
                anyhow::bail!("ManageSieve connection closed");
            }
            let upper = line.trim_start().to_ascii_uppercase();
            if upper.starts_with("OK") {
                return Ok(());
            }
            if upper.starts_with("NO") || upper.starts_with("BYE") {
                anyhow::bail!("ManageSieve refused the command: {}", line.trim().chars().take(200).collect::<String>());
            }
        }
        anyhow::bail!("ManageSieve response too long")
    }

    async fn command(&mut self, cmd: &[u8]) -> anyhow::Result<()> {
        self.reader.get_mut().write_all(cmd).await?;
        self.expect_ok().await
    }
}

async fn session(sieve: &SieveConfig, imap: &ImapConfig, address: &str) -> anyhow::Result<Conn> {
    let mut c = Conn::open(sieve).await?;
    let authcid = format!("{address}*{}", imap.master_user);
    let initial = B64.encode(format!("\0{authcid}\0{}", imap.master_password));
    c.command(format!("AUTHENTICATE \"PLAIN\" \"{initial}\"\r\n").as_bytes()).await?;
    Ok(c)
}

/// Install and activate the vacation script for `address`.
pub async fn install(sieve: &SieveConfig, imap: &ImapConfig, address: &str, v: &Vacation<'_>) -> anyhow::Result<()> {
    let body = script(v);
    let mut c = session(sieve, imap, address).await?;
    let mut cmd = format!("PUTSCRIPT \"{SCRIPT_NAME}\" {{{}+}}\r\n", body.len()).into_bytes();
    cmd.extend_from_slice(body.as_bytes());
    cmd.extend_from_slice(b"\r\n");
    c.command(&cmd).await?;
    c.command(format!("SETACTIVE \"{SCRIPT_NAME}\"\r\n").as_bytes()).await?;
    let _ = c.command(b"LOGOUT\r\n").await;
    Ok(())
}

/// Deactivate and delete the vacation script (idempotent).
pub async fn remove(sieve: &SieveConfig, imap: &ImapConfig, address: &str) -> anyhow::Result<()> {
    let mut c = session(sieve, imap, address).await?;
    c.command(b"SETACTIVE \"\"\r\n").await?;
    // Deleting a script that does not exist returns NO; that is fine.
    let _ = c.command(format!("DELETESCRIPT \"{SCRIPT_NAME}\"\r\n").as_bytes()).await;
    let _ = c.command(b"LOGOUT\r\n").await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_text_cannot_escape_the_quoted_string() {
        let s = script(&Vacation {
            subject: "Away \"now\"; discard;",
            body: "Back Monday.\nUrgent: call \\ IT",
            starts_on: NaiveDate::from_ymd_opt(2026, 12, 20),
            ends_on: NaiveDate::from_ymd_opt(2027, 1, 4),
        });
        assert!(s.contains(r#":subject "Away \"now\"; discard;""#));
        assert!(s.contains("call \\\\ IT"));
        assert!(s.contains("\"ge\" \"date\" \"2026-12-20\""));
        assert!(s.contains("\"le\" \"date\" \"2027-01-04\""));
        // Exactly one action, inside the date condition.
        assert_eq!(s.matches("vacation :days").count(), 1);
        assert!(s.starts_with("# Managed by Mailvora"));
    }

    #[test]
    fn undated_script_is_unconditional() {
        let s = script(&Vacation { subject: "Out", body: "Away", starts_on: None, ends_on: None });
        assert!(!s.contains("allof"));
        assert!(s.contains("vacation :days 1 :subject \"Out\" \"Away\";"));
    }
}
