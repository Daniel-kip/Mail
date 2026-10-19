//! Per-domain DKIM keys.
//!
//! Each domain gets its own RSA-2048 key pair at registration. The private
//! key is sealed with the platform data key (AES-256-GCM) before it touches
//! the database; the public key is published by the customer in DNS.
//! RSA-2048 is used rather than Ed25519 because Ed25519 DKIM (RFC 8463) is
//! still not verified by every major receiver.

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use lettre::message::{
    dkim::{DkimCanonicalization, DkimCanonicalizationType, DkimConfig, DkimSigningAlgorithm, DkimSigningKey},
    header::HeaderName,
};
use rsa::{
    pkcs1::{EncodeRsaPrivateKey, LineEnding},
    pkcs8::EncodePublicKey,
    RsaPrivateKey,
};

use crate::{
    crypto::{purpose, SecretBox},
    error::{ApiResult, AppError},
};

pub struct DkimKeyPair {
    pub private_pem: String,
    /// Base64 SubjectPublicKeyInfo, i.e. the DNS `p=` value.
    pub public_b64: String,
}

pub async fn generate() -> ApiResult<DkimKeyPair> {
    tokio::task::spawn_blocking(|| -> anyhow::Result<DkimKeyPair> {
        let key = RsaPrivateKey::new(&mut rand::rngs::OsRng, 2048)?;
        let private_pem = key.to_pkcs1_pem(LineEnding::LF)?.to_string();
        let public_der = key.to_public_key().to_public_key_der()?;
        Ok(DkimKeyPair { private_pem, public_b64: B64.encode(public_der.as_bytes()) })
    })
    .await
    .map_err(AppError::internal)?
    .map_err(AppError::Internal)
}

/// The TXT record value customers publish at `<selector>._domainkey.<domain>`.
pub fn dns_record_value(public_b64: &str) -> String {
    format!("v=DKIM1; k=rsa; p={public_b64}")
}

pub fn seal(secrets: &SecretBox, private_pem: &str) -> Vec<u8> {
    secrets.seal(purpose::DKIM_KEY, private_pem.as_bytes())
}

/// Build a lettre signing config from a sealed key.
///
/// Signs the headers receivers care about for authenticity, with
/// relaxed/relaxed canonicalization, which survives benign rewrapping by
/// intermediate MTAs.
///
/// `Content-Type` is deliberately NOT in the list: lettre emits the
/// top-level `Content-Type` while formatting the body, after `sign()` runs,
/// so lettre would hash it as absent while receivers see it present and the
/// signature would fail everywhere. The MIME structure is still protected
/// by the body hash. Verified against an independent implementation
/// (dkimpy) by `tests/verify_dkim.py`; see the `signs_common_message_shapes`
/// test.
pub fn signing_config(secrets: &SecretBox, sealed: &[u8], selector: &str, domain: &str) -> anyhow::Result<DkimConfig> {
    let pem = secrets.open_string(purpose::DKIM_KEY, sealed)?;
    let key = DkimSigningKey::new(&pem, DkimSigningAlgorithm::Rsa)
        .map_err(|e| anyhow::anyhow!("invalid DKIM key for {domain}: {e:?}"))?;
    let headers = [
        "From",
        "To",
        "Cc",
        "Subject",
        "Date",
        "Message-ID",
        "Reply-To",
        "MIME-Version",
        "In-Reply-To",
        "References",
        "List-Unsubscribe",
        "List-Unsubscribe-Post",
    ]
    .into_iter()
    .map(HeaderName::new_from_ascii_str)
    .collect();
    Ok(DkimConfig::new(
        selector.to_string(),
        domain.to_string(),
        key,
        headers,
        DkimCanonicalization { header: DkimCanonicalizationType::Relaxed, body: DkimCanonicalizationType::Relaxed },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mail::mime::{build, Addr, OutAttachment, OutgoingEmail};

    /// Signs representative message shapes. With DKIM_DUMP_DIR set, writes
    /// each signed message plus the DNS public key so they can be verified
    /// by an independent implementation (see frontend/resources/wiki/Project-Status.md).
    #[tokio::test]
    async fn signs_common_message_shapes() {
        let keys = generate().await.unwrap();
        let secrets = SecretBox::new(&[9u8; 32]);
        let sealed = seal(&secrets, &keys.private_pem);
        let base = OutgoingEmail {
            from: Some(Addr::named(Some("Registrar".into()), "no-reply@school.example")),
            to: vec![Addr::new("student@example.org")],
            subject: "Your results are ready".into(),
            message_id: "<abc@school.example>".into(),
            ..Default::default()
        };
        let text_only = OutgoingEmail { text: Some("Hello\r\nResults attached.".into()), ..base.clone() };
        let alternative =
            OutgoingEmail { text: Some("Hello".into()), html: Some("<p>Hello</p>".into()), ..base.clone() };
        let mixed = OutgoingEmail {
            text: Some("See attachment".into()),
            html: Some("<p>See attachment</p>".into()),
            attachments: vec![OutAttachment {
                filename: "r.txt".into(),
                content_type: "text/plain".into(),
                data: b"A+".to_vec(),
            }],
            ..base.clone()
        };
        for (name, email) in [("text", text_only), ("alternative", alternative), ("mixed", mixed)] {
            let mut msg = build(&email).unwrap();
            msg.sign(&signing_config(&secrets, &sealed, "eh202610", "school.example").unwrap());
            let raw = msg.formatted();
            assert!(String::from_utf8_lossy(&raw).contains("DKIM-Signature:"));
            if let Ok(dir) = std::env::var("DKIM_DUMP_DIR") {
                std::fs::write(format!("{dir}/{name}.eml"), &raw).unwrap();
                std::fs::write(format!("{dir}/pubkey.txt"), &keys.public_b64).unwrap();
            }
        }
    }
}
