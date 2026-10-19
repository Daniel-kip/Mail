//! Audit trail for security- and tenant-relevant changes.
//!
//! Records are written in the same transaction as the change when one is
//! available, so an audited change can never commit without its record.
//! Never put secrets, passwords, message bodies or template variables in
//! `details`.

use serde_json::Value;
use sqlx::PgExecutor;
use uuid::Uuid;

use crate::{
    auth::{ClientInfo, Principal},
    error::ApiResult,
};

pub struct Entry<'a> {
    pub organization_id: Option<Uuid>,
    pub actor_type: &'static str,
    pub actor_id: Option<Uuid>,
    pub actor_label: Option<String>,
    pub action: &'a str,
    pub target_type: Option<&'a str>,
    pub target_id: Option<String>,
    pub client: Option<&'a ClientInfo>,
    pub details: Value,
}

impl<'a> Entry<'a> {
    /// An entry attributed to a tenant-scoped principal.
    pub fn by(principal: &Principal, client: &'a ClientInfo, action: &'a str) -> Self {
        let (actor_type, actor_id, label) = principal.audit_actor();
        Entry {
            organization_id: Some(principal.org_id),
            actor_type,
            actor_id: Some(actor_id),
            actor_label: Some(label),
            action,
            target_type: None,
            target_id: None,
            client: Some(client),
            details: Value::Object(Default::default()),
        }
    }

    pub fn user(org: Option<Uuid>, user_id: Uuid, email: &str, client: &'a ClientInfo, action: &'a str) -> Self {
        Entry {
            organization_id: org,
            actor_type: "user",
            actor_id: Some(user_id),
            actor_label: Some(email.to_string()),
            action,
            target_type: None,
            target_id: None,
            client: Some(client),
            details: Value::Object(Default::default()),
        }
    }

    pub fn target(mut self, kind: &'a str, id: impl ToString) -> Self {
        self.target_type = Some(kind);
        self.target_id = Some(id.to_string());
        self
    }

    pub fn details(mut self, details: Value) -> Self {
        self.details = details;
        self
    }

    pub async fn record<'e, E: PgExecutor<'e>>(self, executor: E) -> ApiResult<()> {
        sqlx::query(
            "INSERT INTO audit_logs
               (organization_id, actor_type, actor_id, actor_label, action, target_type, target_id, ip, user_agent, details)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(self.organization_id)
        .bind(self.actor_type)
        .bind(self.actor_id)
        .bind(self.actor_label)
        .bind(self.action)
        .bind(self.target_type)
        .bind(self.target_id)
        .bind(self.client.and_then(|c| c.ip_network()))
        .bind(self.client.and_then(|c| c.user_agent.clone()))
        .bind(self.details)
        .execute(executor)
        .await?;
        Ok(())
    }
}
