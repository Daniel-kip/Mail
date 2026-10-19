//! Platform-originated email (account verification, password reset,
//! invitations). Enqueued on the outbox inside the caller's transaction and
//! delivered by the worker through the same relay as tenant mail.

use serde_json::json;
use sqlx::PgExecutor;

use crate::error::ApiResult;

pub struct SystemEmail {
    pub to: String,
    pub subject: String,
    pub text: String,
    pub html: String,
}

pub async fn enqueue<'e, E: PgExecutor<'e>>(executor: E, email: SystemEmail) -> ApiResult<()> {
    sqlx::query("INSERT INTO outbox_jobs (kind, payload, max_attempts) VALUES ('system_email', $1, 10)")
        .bind(json!({ "to": email.to, "subject": email.subject, "text": email.text, "html": email.html }))
        .execute(executor)
        .await?;
    Ok(())
}

pub fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Conservative, table-based layout that renders consistently in clients.
fn layout(product: &str, heading: &str, paragraphs: &[String], action: Option<(&str, &str)>, footnote: &str) -> String {
    let body: String = paragraphs
        .iter()
        .map(|p| {
            format!(
                r#"<p style="margin:0 0 16px;font-size:15px;line-height:22px;color:#334155">{}</p>"#,
                escape_html(p)
            )
        })
        .collect();
    let button = action
        .map(|(label, url)| {
            format!(
                r#"<table role="presentation" cellpadding="0" cellspacing="0" style="margin:8px 0 24px"><tr><td style="background:#1f5f52;border-radius:6px"><a href="{url}" style="display:inline-block;padding:12px 22px;font-size:15px;font-weight:600;color:#ffffff;text-decoration:none">{label}</a></td></tr></table><p style="margin:0 0 16px;font-size:13px;line-height:19px;color:#64748b">If the button does not work, copy this link into your browser:<br><span style="word-break:break-all;color:#1f5f52">{url}</span></p>"#,
                url = escape_html(url),
                label = escape_html(label)
            )
        })
        .unwrap_or_default();
    format!(
        r#"<!doctype html><html><body style="margin:0;padding:0;background:#f1f5f9;font-family:-apple-system,Segoe UI,Roboto,Helvetica,Arial,sans-serif">
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="background:#f1f5f9;padding:32px 12px"><tr><td align="center">
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="max-width:560px;background:#ffffff;border-radius:10px;border:1px solid #e2e8f0">
<tr><td style="padding:28px 32px 8px;font-size:14px;font-weight:700;color:#0f172a;letter-spacing:.02em">{product}</td></tr>
<tr><td style="padding:8px 32px 24px"><h1 style="margin:0 0 16px;font-size:21px;line-height:28px;color:#0f172a">{heading}</h1>{body}{button}
<p style="margin:0;font-size:12px;line-height:18px;color:#94a3b8">{footnote}</p></td></tr></table></td></tr></table></body></html>"#,
        product = escape_html(product),
        heading = escape_html(heading),
        footnote = escape_html(footnote),
    )
}

pub fn verify_email(product: &str, to: &str, name: &str, link: &str) -> SystemEmail {
    let p = vec![
        format!("Hi {name},"),
        "Confirm your email address to finish setting up your account. Domain verification and live API keys are enabled once your address is confirmed.".to_string(),
    ];
    SystemEmail {
        to: to.to_string(),
        subject: format!("Confirm your email for {product}"),
        text: format!("{}\n\n{}\n\nConfirm: {link}\n\nThis link expires in 48 hours. If you did not create an account, ignore this email.", p[0], p[1]),
        html: layout(product, "Confirm your email address", &p, Some(("Confirm email", link)),
            "This link expires in 48 hours. If you did not create an account, you can ignore this email."),
    }
}

pub fn password_reset(product: &str, to: &str, link: &str) -> SystemEmail {
    let p = vec!["We received a request to reset the password for your account. All other signed-in sessions will be signed out after the reset.".to_string()];
    SystemEmail {
        to: to.to_string(),
        subject: format!("Reset your {product} password"),
        text: format!("{}\n\nReset: {link}\n\nThis link expires in 1 hour and can be used once. If you did not request it, ignore this email.", p[0]),
        html: layout(product, "Reset your password", &p, Some(("Choose a new password", link)),
            "This link expires in 1 hour and can be used once. If you did not request a reset, your password is unchanged."),
    }
}

pub fn invitation(product: &str, to: &str, org_name: &str, inviter: &str, role: &str, link: &str) -> SystemEmail {
    let p = vec![format!("{inviter} invited you to join {org_name} on {product} as {role}.")];
    SystemEmail {
        to: to.to_string(),
        subject: format!("You're invited to {org_name} on {product}"),
        text: format!("{}\n\nAccept: {link}\n\nThis invitation expires in 7 days.", p[0]),
        html: layout(
            product,
            &format!("Join {org_name}"),
            &p,
            Some(("Accept invitation", link)),
            "This invitation expires in 7 days.",
        ),
    }
}

/// Sent to the employee's personal (recovery) address when IT creates a
/// mailbox with "email the activation link".
pub fn mailbox_activation(product: &str, to: &str, address: &str, org_name: &str, link: &str) -> SystemEmail {
    let p = vec![
        format!("{org_name} has created a work email address for you: {address}."),
        "Choose your own password to activate it. Nobody at your organization will see or know this password.".to_string(),
        "After activating, you can read your mail in the browser or connect it to Gmail, Apple Mail or Outlook on your phone using the step-by-step guide.".to_string(),
    ];
    SystemEmail {
        to: to.to_string(),
        subject: format!("Activate your work email {address}"),
        text: format!("{}\n\n{}\n\n{}\n\nActivate: {link}\n\nThis link expires in 7 days and works once. If you were not expecting this, ignore this email.", p[0], p[1], p[2]),
        html: layout(product, "Activate your work email", &p, Some(("Choose my password", link)),
            "This link expires in 7 days and can be used once. If you were not expecting this email, you can ignore it."),
    }
}

pub fn mailbox_password_reset(product: &str, to: &str, address: &str, link: &str) -> SystemEmail {
    let p = vec![
        format!("A password reset was requested for your work email {address}."),
        "After you choose a new password, your phone and computer email apps will ask for it again. Enter the new password there.".to_string(),
    ];
    SystemEmail {
        to: to.to_string(),
        subject: format!("Reset the password for {address}"),
        text: format!("{}\n\n{}\n\nReset: {link}\n\nThis link expires in 1 hour and works once. If you did not ask for this, ignore this email; your password is unchanged.", p[0], p[1]),
        html: layout(product, "Reset your work email password", &p, Some(("Choose a new password", link)),
            "This link expires in 1 hour and can be used once. If you did not request a reset, your password is unchanged."),
    }
}

/// Notice of a security-sensitive change, so an account owner learns about
/// a takeover attempt even when the attacker controls the session.
pub fn security_alert(product: &str, to: &str, name: &str, what: &str, context: &str, link: &str) -> SystemEmail {
    let p = vec![
        format!("Hello {name},"),
        format!("{what} on your {product} account."),
        format!("When and where: {context}."),
        "If this was you, no action is needed. If not, reset your password now and review your active sessions and two-factor settings.".to_string(),
    ];
    SystemEmail {
        to: to.to_string(),
        subject: format!("Security alert: {what}"),
        text: format!("{}\n\n{}\n{}\n\n{}\n\nReview your account: {link}", p[0], p[1], p[2], p[3]),
        html: layout(
            product,
            "Security alert",
            &p,
            Some(("Review my account", link)),
            "You receive this message for every security change. It cannot be turned off.",
        ),
    }
}

pub fn invoice_issued(
    product: &str,
    to: &str,
    org_name: &str,
    number: &str,
    total: &str,
    due: &str,
    link: &str,
) -> SystemEmail {
    let p = vec![
        format!("A new invoice {number} for {org_name} is available."),
        format!("Amount due: {total}. Due date: {due}."),
        "Pay in seconds with M-Pesa from the invoice page, or follow the payment instructions on the invoice."
            .to_string(),
    ];
    SystemEmail {
        to: to.to_string(),
        subject: format!("Invoice {number} from {product}"),
        text: format!("{}\n{}\n{}\n\nView invoice: {link}", p[0], p[1], p[2]),
        html: layout(
            product,
            &format!("Invoice {number}"),
            &p,
            Some(("View invoice", link)),
            "You receive this email because you are the billing contact or an owner of this account.",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mailbox_emails_never_contain_passwords_and_escape_names() {
        let m =
            mailbox_activation("Mailvora", "p@gmail.com", "amina@school.ac.ke", "<b>School</b>", "https://x/y?t=1&u=2");
        assert!(!m.html.contains("<b>School</b>"));
        assert!(m.html.contains("t=1&amp;u=2"));
        assert!(m.text.contains("7 days"));
        let r = mailbox_password_reset("Mailvora", "p@gmail.com", "amina@school.ac.ke", "https://x/r");
        assert!(r.subject.contains("amina@school.ac.ke"));
    }

    #[test]
    fn escapes_user_content() {
        let m = invitation("EH", "x@y.z", "<script>alert(1)</script>", "Eve", "admin", "https://a/b?c=1&d=2");
        assert!(!m.html.contains("<script>"));
        assert!(m.html.contains("&lt;script&gt;"));
        assert!(m.html.contains("c=1&amp;d=2"));
    }
}
