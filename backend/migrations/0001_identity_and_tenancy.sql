-- Phase 1: identity, tenancy, sessions, audit.
-- Conventions: UUID primary keys, CITEXT for case-insensitive addresses,
-- timestamptz everywhere, CHECK constraints for every enumerated column.

CREATE EXTENSION IF NOT EXISTS citext;

-- ── Plans: per-tenant limits enforced by the API ─────────────────────────
CREATE TABLE plans (
    id                          TEXT PRIMARY KEY,
    name                        TEXT    NOT NULL,
    daily_send_limit            INTEGER NOT NULL CHECK (daily_send_limit >= 0),
    monthly_send_limit          INTEGER NOT NULL CHECK (monthly_send_limit >= 0),
    max_domains                 INTEGER NOT NULL CHECK (max_domains >= 0),
    max_mailboxes               INTEGER NOT NULL CHECK (max_mailboxes >= 0),
    default_mailbox_quota_bytes BIGINT  NOT NULL CHECK (default_mailbox_quota_bytes > 0),
    max_mailbox_quota_bytes     BIGINT  NOT NULL CHECK (max_mailbox_quota_bytes > 0),
    max_recipients_per_request  INTEGER NOT NULL CHECK (max_recipients_per_request > 0),
    api_requests_per_minute     INTEGER NOT NULL CHECK (api_requests_per_minute > 0)
);

-- Conservative defaults (guide 03: "start with low limits").
INSERT INTO plans VALUES
  ('free',       'Free',          200,     3000,   2,    5,  1073741824,   2147483648,  10,   60),
  ('starter',    'Starter',      5000,   100000,  10,   50,  5368709120,  10737418240,  50,  300),
  ('pro',        'Pro',         50000,  1000000,  50,  500, 10737418240,  53687091200, 100, 1200),
  ('enterprise', 'Enterprise', 500000, 10000000, 500, 5000, 21474836480, 107374182400, 500, 6000);

-- ── Users (console identities) ───────────────────────────────────────────
CREATE TABLE users (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email               CITEXT      NOT NULL UNIQUE,
    full_name           TEXT        NOT NULL CHECK (length(full_name) BETWEEN 1 AND 200),
    password_hash       TEXT        NOT NULL,
    status              TEXT        NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'disabled')),
    is_platform_admin   BOOLEAN     NOT NULL DEFAULT false,
    email_verified_at   TIMESTAMPTZ,
    mfa_enabled         BOOLEAN     NOT NULL DEFAULT false,
    mfa_secret_enc      BYTEA,
    mfa_pending_secret_enc BYTEA,
    mfa_last_step       BIGINT,          -- last accepted TOTP time step (replay protection)
    failed_login_count  INTEGER     NOT NULL DEFAULT 0,
    locked_until        TIMESTAMPTZ,
    last_login_at       TIMESTAMPTZ,
    password_changed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (NOT mfa_enabled OR mfa_secret_enc IS NOT NULL)
);

-- Single-use tokens delivered by email (verification, password reset).
CREATE TABLE user_tokens (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    purpose     TEXT        NOT NULL CHECK (purpose IN ('verify_email', 'reset_password')),
    token_hash  BYTEA       NOT NULL UNIQUE,
    expires_at  TIMESTAMPTZ NOT NULL,
    used_at     TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX user_tokens_user_idx ON user_tokens (user_id, purpose);

-- ── Organizations (tenants) ──────────────────────────────────────────────
CREATE TABLE organizations (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name             TEXT        NOT NULL CHECK (length(name) BETWEEN 1 AND 200),
    slug             CITEXT      NOT NULL UNIQUE CHECK (slug ~ '^[a-z0-9][a-z0-9-]{1,62}$'),
    status           TEXT        NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'suspended')),
    suspended_reason TEXT,
    suspended_at     TIMESTAMPTZ,
    plan_id          TEXT        NOT NULL DEFAULT 'free' REFERENCES plans(id),
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE memberships (
    organization_id UUID        NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id         UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role            TEXT        NOT NULL CHECK (role IN ('owner', 'admin', 'developer', 'viewer')),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (organization_id, user_id)
);
CREATE INDEX memberships_user_idx ON memberships (user_id);

CREATE TABLE invitations (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID        NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    email           CITEXT      NOT NULL,
    role            TEXT        NOT NULL CHECK (role IN ('admin', 'developer', 'viewer')),
    token_hash      BYTEA       NOT NULL UNIQUE,
    invited_by      UUID        REFERENCES users(id) ON DELETE SET NULL,
    expires_at      TIMESTAMPTZ NOT NULL,
    accepted_at     TIMESTAMPTZ,
    revoked_at      TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX invitations_org_idx ON invitations (organization_id, created_at DESC);
-- At most one open invitation per address per organization.
CREATE UNIQUE INDEX invitations_open_unique ON invitations (organization_id, email)
    WHERE accepted_at IS NULL AND revoked_at IS NULL;

-- ── Browser sessions (opaque token in an HttpOnly cookie) ────────────────
-- Only the SHA-256 of the token is stored, so a database leak does not
-- expose usable session cookies.
CREATE TABLE sessions (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    kind                    TEXT        NOT NULL CHECK (kind IN ('console', 'webmail')),
    token_hash              BYTEA       NOT NULL UNIQUE,
    csrf_token              TEXT        NOT NULL,
    user_id                 UUID        REFERENCES users(id) ON DELETE CASCADE,
    active_organization_id  UUID        REFERENCES organizations(id) ON DELETE SET NULL,
    mfa_pending             BOOLEAN     NOT NULL DEFAULT false,
    ip                      INET,
    user_agent              TEXT,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at              TIMESTAMPTZ NOT NULL,
    revoked_at              TIMESTAMPTZ
);
CREATE INDEX sessions_user_idx ON sessions (user_id) WHERE revoked_at IS NULL;
CREATE INDEX sessions_expiry_idx ON sessions (expires_at);

-- ── Audit log (append-only by convention; the app never updates rows) ────
CREATE TABLE audit_logs (
    id              BIGSERIAL PRIMARY KEY,
    organization_id UUID        REFERENCES organizations(id) ON DELETE CASCADE,
    actor_type      TEXT        NOT NULL CHECK (actor_type IN ('user', 'api_key', 'mailbox', 'system', 'platform_admin')),
    actor_id        UUID,
    actor_label     TEXT,
    action          TEXT        NOT NULL,
    target_type     TEXT,
    target_id       TEXT,
    ip              INET,
    user_agent      TEXT,
    details         JSONB       NOT NULL DEFAULT '{}'::jsonb,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX audit_logs_org_idx ON audit_logs (organization_id, created_at DESC, id DESC);
