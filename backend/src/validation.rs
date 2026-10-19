//! Server-side input validation shared by every endpoint.
//!
//! Email headers are the main injection surface: any user-controlled value
//! that ends up in a header (names, subjects, addresses) must be free of
//! CR, LF and NUL, otherwise a caller could inject extra headers or
//! recipients (guide 03: "Do not let template variables inject headers").

use once_cell::sync::Lazy;
use regex::Regex;

use crate::error::{ApiResult, AppError};

static LOCAL_PART_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[A-Za-z0-9!#$%&'*+/=?^_`{|}~-]+(\.[A-Za-z0-9!#$%&'*+/=?^_`{|}~-]+)*$").unwrap());
static HOSTED_LOCAL_PART_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-z0-9]([a-z0-9._+-]{0,62}[a-z0-9])?$").unwrap());
static LABEL_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$").unwrap());
static SLUG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[a-z0-9][a-z0-9_-]{0,79}$").unwrap());

/// True if the string contains characters that are never legal in a header value.
pub fn has_header_breaking_chars(s: &str) -> bool {
    s.chars().any(|c| c == '\r' || c == '\n' || c == '\0')
}

pub fn header_safe(field: &str, value: &str, max_len: usize) -> ApiResult<()> {
    if has_header_breaking_chars(value) {
        return Err(AppError::validation(format!("{field} must not contain line breaks")));
    }
    if value.chars().count() > max_len {
        return Err(AppError::validation(format!("{field} must be at most {max_len} characters")));
    }
    Ok(())
}

/// Normalize and validate a DNS domain name. Unicode names are converted to
/// their ASCII (punycode) form so lookups and comparisons are unambiguous.
pub fn normalize_domain(input: &str) -> ApiResult<String> {
    let trimmed = input.trim().trim_end_matches('.');
    let ascii = idna::domain_to_ascii(trimmed)
        .map_err(|_| AppError::validation(format!("{input:?} is not a valid domain name")))?
        .to_ascii_lowercase();
    if ascii.len() < 3 || ascii.len() > 253 {
        return Err(AppError::validation("Domain name length is invalid"));
    }
    let labels: Vec<&str> = ascii.split('.').collect();
    if labels.len() < 2 || !labels.iter().all(|l| LABEL_RE.is_match(l)) {
        return Err(AppError::validation(format!("{input:?} is not a valid domain name")));
    }
    if labels.last().map_or(true, |tld| tld.chars().all(|c| c.is_ascii_digit())) {
        return Err(AppError::validation("Domain must end in an alphabetic top-level domain"));
    }
    Ok(ascii)
}

/// A parsed, normalized email address (`local@domain`, domain lowercased).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailAddress {
    pub local: String,
    pub domain: String,
}

impl EmailAddress {
    pub fn as_string(&self) -> String {
        format!("{}@{}", self.local, self.domain)
    }
}

impl std::fmt::Display for EmailAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}", self.local, self.domain)
    }
}

pub fn parse_email(field: &str, input: &str) -> ApiResult<EmailAddress> {
    let s = input.trim();
    let invalid = || AppError::validation(format!("{field}: {input:?} is not a valid email address"));
    if s.len() > 254 || has_header_breaking_chars(s) {
        return Err(invalid());
    }
    let (local, domain) = s.rsplit_once('@').ok_or_else(invalid)?;
    if local.is_empty() || local.len() > 64 || !LOCAL_PART_RE.is_match(local) {
        return Err(invalid());
    }
    let domain = normalize_domain(domain).map_err(|_| invalid())?;
    Ok(EmailAddress { local: local.to_string(), domain })
}

/// Local parts we accept for hosted mailboxes and aliases: deliberately
/// stricter than RFC 5322 so addresses are unambiguous and filesystem-safe.
pub fn hosted_local_part(input: &str) -> ApiResult<String> {
    let lp = input.trim().to_ascii_lowercase();
    if !HOSTED_LOCAL_PART_RE.is_match(&lp) || lp.contains("..") {
        return Err(AppError::validation(
            "Local part may contain a-z, 0-9, '.', '_', '+', '-' and must start and end with a letter or digit",
        ));
    }
    Ok(lp)
}

pub fn slug(field: &str, input: &str) -> ApiResult<String> {
    let s = input.trim().to_ascii_lowercase();
    if !SLUG_RE.is_match(&s) {
        return Err(AppError::validation(format!(
            "{field} may contain a-z, 0-9, '-', '_' (max 80 chars) and must start with a letter or digit"
        )));
    }
    Ok(s)
}

/// Derive a URL-safe slug from a free-text name.
pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in name.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    let trimmed: String = out.trim_matches('-').chars().take(48).collect();
    if trimmed.len() < 2 {
        "org".to_string()
    } else {
        trimmed
    }
}

pub fn non_empty(field: &str, value: &str, max_len: usize) -> ApiResult<String> {
    let v = value.trim();
    if v.is_empty() {
        return Err(AppError::validation(format!("{field} is required")));
    }
    if v.chars().count() > max_len {
        return Err(AppError::validation(format!("{field} must be at most {max_len} characters")));
    }
    Ok(v.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emails() {
        let e = parse_email("to", "Jane.Doe+tag@Example.COM").unwrap();
        assert_eq!(e.as_string(), "Jane.Doe+tag@example.com");
        for bad in ["", "no-at", "@x.com", "a@", "a@b", "a b@c.com", "a@c.com\r\nBcc: x@y.z", "a..b@c.com", "a@1.2"] {
            assert!(parse_email("to", bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn domains() {
        assert_eq!(normalize_domain("Mail.Example.org.").unwrap(), "mail.example.org");
        assert_eq!(normalize_domain("bücher.de").unwrap(), "xn--bcher-kva.de");
        for bad in ["localhost", "-a.com", "a_b.com", "a..com", "1.2.3.4"] {
            assert!(normalize_domain(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn header_injection() {
        assert!(header_safe("subject", "Hello\r\nBcc: evil@x.com", 100).is_err());
        assert!(header_safe("subject", "Hello", 100).is_ok());
    }

    #[test]
    fn slugs() {
        assert_eq!(slugify("Acme University (Main)"), "acme-university-main");
        assert_eq!(slugify("!!"), "org");
        assert!(hosted_local_part("john.doe").is_ok());
        assert!(hosted_local_part(".john").is_err());
        assert!(hosted_local_part("jo..hn").is_err());
    }
}
