//! Roles, permissions and API key scopes.
//!
//! Console users get permissions from their membership role; API keys get
//! exactly the scopes they were created with. Both resolve to the same
//! `Permission` values so every endpoint performs one uniform check.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    #[serde(rename = "messages:send")]
    MessagesSend,
    #[serde(rename = "messages:read")]
    MessagesRead,
    #[serde(rename = "templates:read")]
    TemplatesRead,
    #[serde(rename = "templates:write")]
    TemplatesWrite,
    #[serde(rename = "domains:read")]
    DomainsRead,
    #[serde(rename = "domains:write")]
    DomainsWrite,
    #[serde(rename = "mailboxes:read")]
    MailboxesRead,
    #[serde(rename = "mailboxes:write")]
    MailboxesWrite,
    /// Help employees without seeing or setting passwords: send activation
    /// and reset links, unlock after failed sign-ins (ICT support).
    #[serde(rename = "mailboxes:assist")]
    MailboxesAssist,
    #[serde(rename = "suppressions:read")]
    SuppressionsRead,
    #[serde(rename = "suppressions:write")]
    SuppressionsWrite,
    #[serde(rename = "webhooks:read")]
    WebhooksRead,
    #[serde(rename = "webhooks:write")]
    WebhooksWrite,
    #[serde(rename = "usage:read")]
    UsageRead,
    // Human-only permissions (never grantable to API keys).
    #[serde(rename = "api_keys:manage")]
    ApiKeysManage,
    #[serde(rename = "members:read")]
    MembersRead,
    #[serde(rename = "members:manage")]
    MembersManage,
    #[serde(rename = "organization:manage")]
    OrganizationManage,
    #[serde(rename = "audit:read")]
    AuditRead,
    #[serde(rename = "billing:read")]
    BillingRead,
    #[serde(rename = "billing:manage")]
    BillingManage,
}

use Permission::*;

impl Permission {
    pub fn as_str(self) -> &'static str {
        match self {
            MessagesSend => "messages:send",
            MessagesRead => "messages:read",
            TemplatesRead => "templates:read",
            TemplatesWrite => "templates:write",
            DomainsRead => "domains:read",
            DomainsWrite => "domains:write",
            MailboxesRead => "mailboxes:read",
            MailboxesWrite => "mailboxes:write",
            MailboxesAssist => "mailboxes:assist",
            SuppressionsRead => "suppressions:read",
            SuppressionsWrite => "suppressions:write",
            WebhooksRead => "webhooks:read",
            WebhooksWrite => "webhooks:write",
            UsageRead => "usage:read",
            ApiKeysManage => "api_keys:manage",
            MembersRead => "members:read",
            MembersManage => "members:manage",
            OrganizationManage => "organization:manage",
            AuditRead => "audit:read",
            BillingRead => "billing:read",
            BillingManage => "billing:manage",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        ALL.iter().copied().find(|p| p.as_str() == s)
    }

    /// Whether the permission changes state (blocked while suspended).
    pub fn is_write(self) -> bool {
        matches!(
            self,
            MessagesSend
                | TemplatesWrite
                | DomainsWrite
                | MailboxesWrite
                | MailboxesAssist
                | SuppressionsWrite
                | WebhooksWrite
                | ApiKeysManage
                | MembersManage
                | OrganizationManage
                | BillingManage
        )
    }
}

pub const ALL: &[Permission] = &[
    MessagesSend,
    MessagesRead,
    TemplatesRead,
    TemplatesWrite,
    DomainsRead,
    DomainsWrite,
    MailboxesRead,
    MailboxesWrite,
    MailboxesAssist,
    SuppressionsRead,
    SuppressionsWrite,
    WebhooksRead,
    WebhooksWrite,
    UsageRead,
    ApiKeysManage,
    MembersRead,
    MembersManage,
    OrganizationManage,
    AuditRead,
    BillingRead,
    BillingManage,
];

/// Scopes an API key may hold. Key, member and organization management
/// always require an interactive (and possibly MFA-protected) session.
pub const API_KEY_SCOPES: &[Permission] = &[
    MessagesSend,
    MessagesRead,
    TemplatesRead,
    TemplatesWrite,
    DomainsRead,
    MailboxesRead,
    MailboxesWrite,
    SuppressionsRead,
    SuppressionsWrite,
    WebhooksRead,
    WebhooksWrite,
    UsageRead,
];

pub const DEFAULT_API_KEY_SCOPES: &[Permission] = &[MessagesSend, MessagesRead];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Viewer,
    /// ICT support: read-only plus employee assistance (no passwords).
    Support,
    Developer,
    Admin,
    Owner,
}

impl Role {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "viewer" => Some(Role::Viewer),
            "support" => Some(Role::Support),
            "developer" => Some(Role::Developer),
            "admin" => Some(Role::Admin),
            "owner" => Some(Role::Owner),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Role::Viewer => "viewer",
            Role::Support => "support",
            Role::Developer => "developer",
            Role::Admin => "admin",
            Role::Owner => "owner",
        }
    }

    pub fn allows(self, p: Permission) -> bool {
        let viewer = matches!(
            p,
            MessagesRead
                | TemplatesRead
                | DomainsRead
                | MailboxesRead
                | SuppressionsRead
                | WebhooksRead
                | UsageRead
                | MembersRead
        );
        let support = matches!(p, MailboxesAssist);
        let developer = matches!(p, MessagesSend | TemplatesWrite | SuppressionsWrite | WebhooksWrite | ApiKeysManage);
        let admin = matches!(
            p,
            DomainsWrite
                | MailboxesWrite
                | MailboxesAssist
                | MembersManage
                | OrganizationManage
                | AuditRead
                | BillingRead
                | BillingManage
        );
        match self {
            Role::Viewer => viewer,
            Role::Support => viewer || support,
            Role::Developer => viewer || developer,
            Role::Admin | Role::Owner => viewer || developer || admin,
        }
    }

    pub fn permissions(self) -> Vec<&'static str> {
        ALL.iter().filter(|p| self.allows(**p)).map(|p| p.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_matrix() {
        assert!(Role::Viewer.allows(MessagesRead));
        assert!(!Role::Viewer.allows(MessagesSend));
        assert!(Role::Developer.allows(MessagesSend));
        assert!(!Role::Developer.allows(DomainsWrite));
        assert!(!Role::Developer.allows(MembersManage));
        assert!(Role::Admin.allows(MembersManage));
        assert!(Role::Owner.allows(AuditRead));
        // ICT support assists employees but cannot provision or reconfigure.
        assert!(Role::Support.allows(MailboxesAssist));
        assert!(Role::Support.allows(MailboxesRead));
        assert!(!Role::Support.allows(MailboxesWrite));
        assert!(!Role::Support.allows(MessagesSend));
        assert!(!Role::Developer.allows(MailboxesAssist));
        assert!(!Role::Developer.allows(BillingManage));
        assert!(Role::Admin.allows(BillingManage));
    }

    #[test]
    fn scope_roundtrip() {
        for p in ALL {
            assert_eq!(Permission::parse(p.as_str()), Some(*p));
        }
        assert!(!API_KEY_SCOPES.contains(&ApiKeysManage));
        assert!(!API_KEY_SCOPES.contains(&BillingManage));
        assert!(!API_KEY_SCOPES.contains(&MailboxesAssist));
    }
}
