//! Outbound SMS for phone verification codes (Africa's Talking).
//!
//! Without SMS credentials, development deployments deliver the message to
//! Mailpit as an email so the whole sign-up flow stays testable offline;
//! production reports that phone verification is unavailable.

use std::time::Duration;

use serde_json::Value;

use crate::{
    error::{ApiResult, AppError},
    services::system_mail::{self, SystemEmail},
    state::AppState,
};

/// Normalize a phone number to E.164. Kenyan local formats (`07…`, `01…`,
/// `7…`, `2547…`) are accepted; other countries need the `+` prefix.
pub fn normalize(raw: &str) -> ApiResult<String> {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit() || *c == '+').collect();
    let e164 = if let Some(rest) = digits.strip_prefix('+') {
        format!("+{rest}")
    } else if digits.len() == 10 && (digits.starts_with("07") || digits.starts_with("01")) {
        format!("+254{}", &digits[1..])
    } else if digits.len() == 9 && (digits.starts_with('7') || digits.starts_with('1')) {
        format!("+254{digits}")
    } else if digits.len() == 12 && digits.starts_with("254") {
        format!("+{digits}")
    } else {
        return Err(AppError::validation("Enter a valid phone number, for example 0712 345 678 or +254 712 345 678"));
    };
    let body = &e164[1..];
    if !(8..=15).contains(&body.len()) || body.starts_with('0') || !body.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::validation("Enter a valid phone number, for example 0712 345 678 or +254 712 345 678"));
    }
    Ok(e164)
}

/// `+254712345678` → `+254 712 ••• 678`, for display in logs and responses.
pub fn mask(e164: &str) -> String {
    if e164.len() < 8 {
        return "•••".into();
    }
    format!("{}•••{}", &e164[..e164.len() - 6], &e164[e164.len() - 3..])
}

pub fn available(state: &AppState) -> bool {
    state.cfg.integrations.sms.is_some() || !state.cfg.is_production()
}

pub async fn send(state: &AppState, to: &str, text: &str) -> ApiResult<()> {
    let Some(cfg) = &state.cfg.integrations.sms else {
        if state.cfg.is_production() {
            return Err(AppError::unavailable(
                "sms_not_configured",
                "Phone verification is not available on this platform",
            ));
        }
        let digits: String = to.chars().filter(char::is_ascii_digit).collect();
        let email = SystemEmail {
            to: format!("sms-{digits}@sms.localhost"),
            subject: format!("SMS to {to}"),
            text: text.to_string(),
            html: format!("<p style=\"font-family:monospace\">{}</p>", system_mail::escape_html(text)),
        };
        system_mail::enqueue(&state.db, email).await?;
        tracing::info!(to = %mask(to), "development SMS delivered to Mailpit");
        return Ok(());
    };

    let mut form = vec![("username", cfg.username.as_str()), ("to", to), ("message", text)];
    if let Some(from) = cfg.sender_id.as_deref() {
        form.push(("from", from));
    }
    let res = reqwest::Client::new()
        .post(&cfg.api_url)
        .timeout(Duration::from_secs(15))
        .header("apiKey", &cfg.api_key)
        .header("Accept", "application/json")
        .form(&form)
        .send()
        .await
        .map_err(|e| AppError::unavailable("sms_failed", format!("The SMS gateway is unreachable: {e}")))?;
    let status = res.status();
    let body: Value = res.json().await.unwrap_or(Value::Null);
    let recipient = &body["SMSMessageData"]["Recipients"][0];
    // 100 Processed, 101 Sent, 102 Queued.
    let accepted = status.is_success() && matches!(recipient["statusCode"].as_i64(), Some(100..=102));
    if !accepted {
        let reason =
            recipient["status"].as_str().or(body["SMSMessageData"]["Message"].as_str()).unwrap_or("unknown error");
        tracing::warn!(to = %mask(to), %status, reason, "SMS rejected by gateway");
        return Err(AppError::unavailable("sms_failed", format!("The text message could not be sent ({reason})")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_kenyan_and_international_numbers() {
        for raw in ["0712345678", "0712 345 678", "712345678", "254712345678", "+254 712-345-678"] {
            assert_eq!(normalize(raw).unwrap(), "+254712345678", "{raw}");
        }
        assert_eq!(normalize("0110123456").unwrap(), "+254110123456");
        assert_eq!(normalize("+14155550123").unwrap(), "+14155550123");
        for bad in ["12345", "0812345678x", "+0123456789", ""] {
            assert!(normalize(bad).is_err(), "{bad}");
        }
        assert_eq!(mask("+254712345678"), "+254712•••678");
    }
}
