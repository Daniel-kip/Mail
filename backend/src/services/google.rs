//! "Continue with Google" (OpenID Connect, authorization code + PKCE).
//!
//! The web app redirects to Google and receives the code; this service
//! exchanges it at Google's token endpoint over TLS with the client secret.
//! Per OIDC Core §3.1.3.7 an ID token received directly from the token
//! endpoint this way may be trusted on TLS server validation, so its claims
//! are checked (issuer, audience, expiry, nonce, verified email) without a
//! JWKS signature check.

use std::time::Duration;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::Deserialize;

use crate::{
    config::GoogleConfig,
    error::{ApiResult, AppError},
};

const TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";

#[derive(Debug, Deserialize)]
pub struct Claims {
    pub iss: String,
    pub aud: String,
    pub sub: String,
    pub exp: i64,
    pub email: Option<String>,
    #[serde(default)]
    pub email_verified: bool,
    pub name: Option<String>,
    pub nonce: Option<String>,
}

pub struct Exchange<'a> {
    pub code: &'a str,
    pub code_verifier: &'a str,
    pub redirect_uri: &'a str,
    pub nonce: &'a str,
}

pub async fn exchange(cfg: &GoogleConfig, req: &Exchange<'_>) -> ApiResult<Claims> {
    #[derive(Deserialize)]
    struct TokenResponse {
        id_token: Option<String>,
        error: Option<String>,
        error_description: Option<String>,
    }
    let res = reqwest::Client::new()
        .post(TOKEN_ENDPOINT)
        .timeout(Duration::from_secs(15))
        .form(&[
            ("code", req.code),
            ("client_id", cfg.client_id.as_str()),
            ("client_secret", cfg.client_secret.as_str()),
            ("redirect_uri", req.redirect_uri),
            ("grant_type", "authorization_code"),
            ("code_verifier", req.code_verifier),
        ])
        .send()
        .await
        .map_err(|e| AppError::unavailable("google_unreachable", format!("Google could not be reached: {e}")))?;
    let body: TokenResponse = res
        .json()
        .await
        .map_err(|_| AppError::unavailable("google_bad_response", "Google returned an unexpected response"))?;
    let token = body.id_token.ok_or_else(|| {
        let why = body.error_description.or(body.error).unwrap_or_else(|| "no ID token".into());
        AppError::bad_request("google_sign_in_failed", format!("Google sign-in failed: {why}"))
    })?;
    let claims = decode(&token)?;
    validate(&claims, &cfg.client_id, req.nonce, chrono::Utc::now().timestamp())?;
    Ok(claims)
}

fn decode(id_token: &str) -> ApiResult<Claims> {
    let payload = id_token
        .split('.')
        .nth(1)
        .ok_or_else(|| AppError::bad_request("google_sign_in_failed", "Malformed ID token"))?;
    let bytes = URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .map_err(|_| AppError::bad_request("google_sign_in_failed", "Malformed ID token"))?;
    serde_json::from_slice(&bytes).map_err(|_| AppError::bad_request("google_sign_in_failed", "Malformed ID token"))
}

fn validate(c: &Claims, client_id: &str, nonce: &str, now: i64) -> ApiResult<()> {
    let fail = |m: &str| Err(AppError::bad_request("google_sign_in_failed", m.to_string()));
    if c.iss != "https://accounts.google.com" && c.iss != "accounts.google.com" {
        return fail("ID token was not issued by Google");
    }
    if c.aud != client_id {
        return fail("ID token was issued for a different application");
    }
    if c.exp < now - 60 {
        return fail("Google sign-in expired; try again");
    }
    if nonce.is_empty() || c.nonce.as_deref() != Some(nonce) {
        return fail("Google sign-in could not be matched to this browser; try again");
    }
    if c.email.is_none() || !c.email_verified {
        return fail("Your Google account has no verified email address");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claims(nonce: &str, exp: i64) -> Claims {
        Claims {
            iss: "https://accounts.google.com".into(),
            aud: "client".into(),
            sub: "123".into(),
            exp,
            email: Some("a@gmail.com".into()),
            email_verified: true,
            name: Some("A".into()),
            nonce: Some(nonce.into()),
        }
    }

    #[test]
    fn validates_issuer_audience_expiry_and_nonce() {
        assert!(validate(&claims("n1", 2_000), "client", "n1", 1_000).is_ok());
        assert!(validate(&claims("n1", 2_000), "other", "n1", 1_000).is_err());
        assert!(validate(&claims("n1", 2_000), "client", "n2", 1_000).is_err());
        assert!(validate(&claims("n1", 100), "client", "n1", 1_000).is_err());
        let mut c = claims("n1", 2_000);
        c.email_verified = false;
        assert!(validate(&c, "client", "n1", 1_000).is_err());
        let token =
            format!("h.{}.s", URL_SAFE_NO_PAD.encode(br#"{"iss":"accounts.google.com","aud":"c","sub":"9","exp":5}"#));
        assert_eq!(decode(&token).unwrap().sub, "9");
    }
}
