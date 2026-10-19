//! Domain services shared by the API handlers and workers.

pub mod audit;
pub mod billing;
pub mod breach;
pub mod dkim;
pub mod dns;
pub mod doveadm;
pub mod events;
pub mod google;
pub mod imap;
pub mod mail_client;
pub mod mailbox_tokens;
pub mod mailtools;
pub mod pesatide;
pub mod sanitize;
pub mod sending;
pub mod sieve;
pub mod sms;
pub mod system_mail;
pub mod templates;
pub mod totp;
pub mod unsubscribe;
pub mod webhooks;
