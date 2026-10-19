-- Phase 4: hosted mailboxes, aliases, webmail sessions and the read-only
-- views consumed by Postfix (pgsql lookup tables) and Dovecot (SQL passdb).

CREATE TABLE mailboxes (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID        NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    domain_id       UUID        NOT NULL,
    local_part      CITEXT      NOT NULL CHECK (local_part ~ '^[a-z0-9]([a-z0-9._+-]{0,62}[a-z0-9])?$'),
    address         CITEXT      NOT NULL UNIQUE,
    display_name    TEXT,
    -- Dovecot-compatible: '{ARGON2ID}$argon2id$v=19$m=…,t=…,p=1$salt$hash'
    password_hash   TEXT        NOT NULL,
    status          TEXT        NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'disabled', 'deleted')),
    quota_bytes     BIGINT      NOT NULL CHECK (quota_bytes > 0),
    -- Relative maildir path. Keyed by mailbox id, not address, so a mailbox
    -- created later with a recycled address can never inherit old mail.
    storage_key     TEXT        NOT NULL UNIQUE,
    owner_user_id   UUID        REFERENCES users(id) ON DELETE SET NULL,
    failed_login_count INTEGER  NOT NULL DEFAULT 0,
    locked_until    TIMESTAMPTZ,
    last_login_at   TIMESTAMPTZ,
    deleted_at      TIMESTAMPTZ,
    -- On deletion the address is released (local_part/address are rewritten
    -- to a tombstone) and the original is kept here for audit and purge.
    deleted_address CITEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (domain_id, local_part),
    UNIQUE (id, organization_id),
    FOREIGN KEY (domain_id, organization_id) REFERENCES domains(id, organization_id)
);
CREATE INDEX mailboxes_org_idx ON mailboxes (organization_id, created_at DESC);

-- Aliases deliver to mailboxes of the same tenant only. External forwarding
-- is intentionally not supported in the MVP (loop, SPF and abuse risks).
CREATE TABLE aliases (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id   UUID        NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    domain_id         UUID        NOT NULL,
    local_part        CITEXT      NOT NULL CHECK (local_part ~ '^[a-z0-9]([a-z0-9._+-]{0,62}[a-z0-9])?$'),
    address           CITEXT      NOT NULL UNIQUE,
    destinations      CITEXT[]    NOT NULL CHECK (cardinality(destinations) BETWEEN 1 AND 50),
    status            TEXT        NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'disabled')),
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (domain_id, local_part),
    FOREIGN KEY (domain_id, organization_id) REFERENCES domains(id, organization_id)
);
CREATE INDEX aliases_org_idx ON aliases (organization_id);

ALTER TABLE sessions ADD COLUMN mailbox_id UUID REFERENCES mailboxes(id) ON DELETE CASCADE;
ALTER TABLE sessions ADD CONSTRAINT sessions_principal_check CHECK (
    (kind = 'console' AND user_id IS NOT NULL AND mailbox_id IS NULL) OR
    (kind = 'webmail' AND mailbox_id IS NOT NULL AND user_id IS NULL)
);
CREATE INDEX sessions_mailbox_idx ON sessions (mailbox_id) WHERE revoked_at IS NULL;

ALTER TABLE messages ADD CONSTRAINT messages_mailbox_fk
    FOREIGN KEY (mailbox_id, organization_id) REFERENCES mailboxes(id, organization_id) ON DELETE SET NULL (mailbox_id);

-- ── Mail server lookup views ─────────────────────────────────────────────
-- A domain/mailbox is live for mail only when the domain is verified, mail
-- hosting is enabled and the tenant is active. Suspending a tenant therefore
-- stops inbound acceptance and IMAP/SMTP logins without touching stored mail.

CREATE VIEW mail_domains AS
SELECT d.domain_name::text AS domain
FROM domains d
JOIN organizations o ON o.id = d.organization_id
WHERE d.status = 'verified' AND d.mail_hosting_enabled AND o.status = 'active';

CREATE VIEW mail_mailboxes AS
SELECT m.address::text      AS address,
       m.password_hash      AS password,
       m.storage_key        AS storage_key,
       m.quota_bytes        AS quota_bytes
FROM mailboxes m
JOIN domains d       ON d.id = m.domain_id
JOIN organizations o ON o.id = m.organization_id
WHERE m.status = 'active'
  AND d.status = 'verified' AND d.mail_hosting_enabled
  AND o.status = 'active';

CREATE VIEW mail_aliases AS
SELECT a.address::text                           AS address,
       array_to_string(a.destinations::text[], ',') AS destinations
FROM aliases a
JOIN domains d       ON d.id = a.domain_id
JOIN organizations o ON o.id = a.organization_id
WHERE a.status = 'active'
  AND d.status = 'verified' AND d.mail_hosting_enabled
  AND o.status = 'active';

-- Which SASL logins may use a given envelope sender on submission
-- (Postfix smtpd_sender_login_maps + reject_sender_login_mismatch).
CREATE VIEW mail_sender_logins AS
SELECT address, address AS logins FROM mail_mailboxes
UNION ALL
SELECT address, destinations AS logins FROM mail_aliases;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'mv_mailserver') THEN
        GRANT SELECT ON mail_domains, mail_mailboxes, mail_aliases, mail_sender_logins TO mv_mailserver;
    END IF;
END
$$;
