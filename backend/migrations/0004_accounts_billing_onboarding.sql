-- Phase 5: account types and subscriptions, ICT support role, mailbox
-- lifecycle (activation and self-service recovery), API key restrictions,
-- scheduled and list-unsubscribe-aware messages, mail-client onboarding
-- (versioned client settings, setup analytics, connection verification) and
-- webmail preferences/contacts.
--
-- Expand-only: every change adds columns, tables or widens CHECK constraints,
-- so the previous application release keeps working against this schema.

-- ── Plans: audience, pricing and additional limits ────────────────────────
ALTER TABLE plans
    ADD COLUMN audience                TEXT    NOT NULL DEFAULT 'institution' CHECK (audience IN ('individual', 'institution')),
    ADD COLUMN description             TEXT    NOT NULL DEFAULT '',
    ADD COLUMN price_monthly_cents     INTEGER NOT NULL DEFAULT 0 CHECK (price_monthly_cents >= 0),
    ADD COLUMN price_per_mailbox_cents INTEGER NOT NULL DEFAULT 0 CHECK (price_per_mailbox_cents >= 0),
    ADD COLUMN annual_discount_percent INTEGER NOT NULL DEFAULT 0 CHECK (annual_discount_percent BETWEEN 0 AND 50),
    ADD COLUMN max_members             INTEGER NOT NULL DEFAULT 5  CHECK (max_members >= 1),
    ADD COLUMN max_api_keys            INTEGER NOT NULL DEFAULT 10 CHECK (max_api_keys >= 0),
    ADD COLUMN max_webhooks            INTEGER NOT NULL DEFAULT 5  CHECK (max_webhooks >= 0),
    ADD COLUMN trial_days              INTEGER NOT NULL DEFAULT 0  CHECK (trial_days BETWEEN 0 AND 90),
    ADD COLUMN is_public               BOOLEAN NOT NULL DEFAULT true,
    ADD COLUMN self_service            BOOLEAN NOT NULL DEFAULT true,
    ADD COLUMN sort_order              INTEGER NOT NULL DEFAULT 0;

-- Institutional plans (hosted work mailboxes + transactional API). Prices are
-- in minor units of BILLING_CURRENCY; operators can change them later.
UPDATE plans SET name = 'Institution Free', sort_order = 10, max_members = 3, max_api_keys = 5, max_webhooks = 2,
    description = 'Try hosted work email with a small team. No card or invoice required.'
WHERE id = 'free';
UPDATE plans SET name = 'Institution Standard', sort_order = 20, price_monthly_cents = 1900, price_per_mailbox_cents = 150,
    annual_discount_percent = 15, trial_days = 14, max_members = 15, max_api_keys = 25, max_webhooks = 10,
    description = 'For schools and offices: up to 50 work mailboxes, shared aliases and the full sending API.'
WHERE id = 'starter';
UPDATE plans SET name = 'Institution Premium', sort_order = 30, price_monthly_cents = 4900, price_per_mailbox_cents = 100,
    annual_discount_percent = 15, trial_days = 14, max_members = 50, max_api_keys = 100, max_webhooks = 25,
    description = 'For universities and larger organizations: up to 500 mailboxes and higher sending volume.'
WHERE id = 'pro';
UPDATE plans SET name = 'Enterprise', sort_order = 40, self_service = false, max_members = 500, max_api_keys = 500,
    max_webhooks = 100, description = 'Custom limits, dedicated support and contractual terms. Assigned by the platform operator.'
WHERE id = 'enterprise';

-- Individual (developer) plans: transactional relaying for one application
-- (invoices, receipts, notifications). No hosted mailboxes.
INSERT INTO plans (id, name, daily_send_limit, monthly_send_limit, max_domains, max_mailboxes,
                   default_mailbox_quota_bytes, max_mailbox_quota_bytes, max_recipients_per_request,
                   api_requests_per_minute, audience, description, price_monthly_cents, annual_discount_percent,
                   max_members, max_api_keys, max_webhooks, sort_order)
VALUES
  ('developer',       'Developer',          100,   3000,  1, 0, 1073741824, 1073741824,  10,   60, 'individual',
   'Send receipts, invoices and notifications from one application.', 0,    0,  1,  3,  2, 1),
  ('developer_plus',  'Developer Plus',    2000,  50000,  3, 0, 1073741824, 1073741824,  50,  300, 'individual',
   'More volume, several sending domains and a teammate.',          1200, 15,  2, 10,  5, 2),
  ('developer_scale', 'Developer Scale',  10000, 300000, 10, 0, 1073741824, 1073741824, 100, 1200, 'individual',
   'High-volume transactional sending for a growing product.',      3900, 15,  3, 25, 10, 3);

-- ── Organizations: account type, billing profile, employee support ───────
ALTER TABLE organizations
    ADD COLUMN account_type    TEXT   NOT NULL DEFAULT 'institution' CHECK (account_type IN ('individual', 'institution')),
    ADD COLUMN billing_email   CITEXT,
    ADD COLUMN billing_name    TEXT   CHECK (length(billing_name) <= 200),
    ADD COLUMN billing_address TEXT   CHECK (length(billing_address) <= 1000),
    ADD COLUMN tax_id          TEXT   CHECK (length(tax_id) <= 64),
    ADD COLUMN country         TEXT   CHECK (country ~ '^[A-Z]{2}$'),
    ADD COLUMN support_email   CITEXT,
    ADD COLUMN support_phone   TEXT   CHECK (length(support_phone) <= 40),
    ADD COLUMN support_url     TEXT   CHECK (length(support_url) <= 500),
    ADD COLUMN support_hours   TEXT   CHECK (length(support_hours) <= 200),
    ADD COLUMN setup_help_text TEXT   CHECK (length(setup_help_text) <= 2000);

-- ── Subscriptions and invoices ───────────────────────────────────────────
-- organizations.plan_id stays the effective plan read by every limit check;
-- the subscription row carries the commercial lifecycle around it.
CREATE TABLE subscriptions (
    organization_id      UUID        PRIMARY KEY REFERENCES organizations(id) ON DELETE CASCADE,
    plan_id              TEXT        NOT NULL REFERENCES plans(id),
    status               TEXT        NOT NULL DEFAULT 'active' CHECK (status IN ('trialing', 'active', 'past_due', 'cancelled')),
    billing_cycle        TEXT        NOT NULL DEFAULT 'monthly' CHECK (billing_cycle IN ('monthly', 'annual')),
    current_period_start TIMESTAMPTZ NOT NULL DEFAULT now(),
    current_period_end   TIMESTAMPTZ NOT NULL DEFAULT (now() + interval '1 month'),
    trial_ends_at        TIMESTAMPTZ,
    trial_used           BOOLEAN     NOT NULL DEFAULT false,
    pending_plan_id      TEXT        REFERENCES plans(id),
    cancel_at_period_end BOOLEAN     NOT NULL DEFAULT false,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (current_period_end > current_period_start)
);
INSERT INTO subscriptions (organization_id, plan_id) SELECT id, plan_id FROM organizations;
CREATE INDEX subscriptions_period_idx ON subscriptions (current_period_end);

CREATE SEQUENCE invoice_number_seq START 1001;

CREATE TABLE invoices (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id   UUID        NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    number            TEXT        NOT NULL UNIQUE,
    kind              TEXT        NOT NULL CHECK (kind IN ('subscription', 'proration', 'manual')),
    status            TEXT        NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'paid', 'void')),
    currency          TEXT        NOT NULL CHECK (currency ~ '^[A-Z]{3}$'),
    period_start      TIMESTAMPTZ NOT NULL,
    period_end        TIMESTAMPTZ NOT NULL,
    lines             JSONB       NOT NULL DEFAULT '[]'::jsonb,
    subtotal_cents    BIGINT      NOT NULL CHECK (subtotal_cents >= 0),
    tax_rate_bps      INTEGER     NOT NULL DEFAULT 0 CHECK (tax_rate_bps BETWEEN 0 AND 5000),
    tax_cents         BIGINT      NOT NULL CHECK (tax_cents >= 0),
    total_cents       BIGINT      NOT NULL CHECK (total_cents >= 0),
    billing_snapshot  JSONB       NOT NULL DEFAULT '{}'::jsonb,
    issued_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    due_at            TIMESTAMPTZ NOT NULL,
    paid_at           TIMESTAMPTZ,
    payment_method    TEXT        CHECK (length(payment_method) <= 60),
    payment_reference TEXT        CHECK (length(payment_reference) <= 120),
    voided_at         TIMESTAMPTZ,
    notes             TEXT        CHECK (length(notes) <= 1000),
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (status <> 'paid' OR paid_at IS NOT NULL),
    CHECK (status <> 'void' OR voided_at IS NOT NULL)
);
CREATE INDEX invoices_org_idx ON invoices (organization_id, issued_at DESC);
CREATE INDEX invoices_open_idx ON invoices (due_at) WHERE status = 'open';
-- One subscription invoice per billing period (renewal runs are idempotent).
CREATE UNIQUE INDEX invoices_subscription_period_unique ON invoices (organization_id, period_start)
    WHERE kind = 'subscription' AND status <> 'void';

-- ── ICT support role ─────────────────────────────────────────────────────
-- Support staff can help employees (activation/reset links, unlocks) but can
-- neither set nor read passwords, nor change mailbox configuration.
ALTER TABLE memberships DROP CONSTRAINT memberships_role_check;
ALTER TABLE memberships ADD CONSTRAINT memberships_role_check
    CHECK (role IN ('owner', 'admin', 'developer', 'support', 'viewer'));
ALTER TABLE invitations DROP CONSTRAINT invitations_role_check;
ALTER TABLE invitations ADD CONSTRAINT invitations_role_check
    CHECK (role IN ('admin', 'developer', 'support', 'viewer'));

-- ── Mailbox lifecycle ────────────────────────────────────────────────────
-- 'pending' mailboxes exist (address reserved, mail accepted once active)
-- but cannot sign in until the employee chooses a password through a
-- single-use activation link. The mail_* views only expose 'active'.
ALTER TABLE mailboxes DROP CONSTRAINT mailboxes_status_check;
ALTER TABLE mailboxes ADD CONSTRAINT mailboxes_status_check
    CHECK (status IN ('pending', 'active', 'disabled', 'deleted'));
ALTER TABLE mailboxes
    ADD COLUMN recovery_email      CITEXT,
    ADD COLUMN department          TEXT CHECK (length(department) <= 120),
    ADD COLUMN job_title           TEXT CHECK (length(job_title) <= 120),
    ADD COLUMN activated_at        TIMESTAMPTZ,
    ADD COLUMN password_changed_at TIMESTAMPTZ;
UPDATE mailboxes SET activated_at = created_at, password_changed_at = updated_at WHERE status <> 'deleted';

CREATE TABLE mailbox_tokens (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    mailbox_id         UUID        NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    purpose            TEXT        NOT NULL CHECK (purpose IN ('activate', 'reset_password')),
    token_hash         BYTEA       NOT NULL UNIQUE,
    delivery           TEXT        NOT NULL CHECK (delivery IN ('email', 'handover', 'self_service')),
    created_by_user_id UUID        REFERENCES users(id) ON DELETE SET NULL,
    expires_at         TIMESTAMPTZ NOT NULL,
    used_at            TIMESTAMPTZ,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX mailbox_tokens_mailbox_idx ON mailbox_tokens (mailbox_id, purpose);

-- ── API key restrictions ─────────────────────────────────────────────────
ALTER TABLE api_keys
    ADD COLUMN description            TEXT     CHECK (length(description) <= 300),
    ADD COLUMN allowed_ips            INET[]   NOT NULL DEFAULT '{}' CHECK (cardinality(allowed_ips) <= 50),
    ADD COLUMN allowed_sender_domains CITEXT[] NOT NULL DEFAULT '{}' CHECK (cardinality(allowed_sender_domains) <= 50);

-- ── Messages: scheduling, cancellation, list-unsubscribe ─────────────────
ALTER TABLE messages DROP CONSTRAINT messages_status_check;
ALTER TABLE messages ADD CONSTRAINT messages_status_check CHECK (status IN (
    'scheduled', 'queued', 'processing', 'accepted_by_mta', 'delivered', 'deferred',
    'bounced', 'failed', 'suppressed', 'sandboxed', 'cancelled'));
ALTER TABLE messages
    ADD COLUMN scheduled_at     TIMESTAMPTZ,
    ADD COLUMN list_unsubscribe BOOLEAN NOT NULL DEFAULT false;
CREATE INDEX messages_scheduled_idx ON messages (organization_id, scheduled_at) WHERE status = 'scheduled';

-- ── Mail-client onboarding ───────────────────────────────────────────────
-- Public connection settings advertised to email apps. One platform default
-- (domain_id IS NULL) plus optional per-domain overrides for institutions
-- that run a branded endpoint (e.g. mail.university.edu with its own
-- certificate). When no row exists the API falls back to its environment.
CREATE TABLE mail_client_profiles (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_id         UUID UNIQUE REFERENCES domains(id) ON DELETE CASCADE,
    imap_host         TEXT        NOT NULL CHECK (length(imap_host) BETWEEN 3 AND 253),
    imap_port         INTEGER     NOT NULL CHECK (imap_port BETWEEN 1 AND 65535),
    imap_security     TEXT        NOT NULL CHECK (imap_security = 'tls'),
    smtp_host         TEXT        NOT NULL CHECK (length(smtp_host) BETWEEN 3 AND 253),
    smtp_port         INTEGER     NOT NULL CHECK (smtp_port BETWEEN 1 AND 65535),
    smtp_security     TEXT        NOT NULL CHECK (smtp_security IN ('tls', 'starttls')),
    smtp_alt_port     INTEGER     CHECK (smtp_alt_port BETWEEN 1 AND 65535),
    smtp_alt_security TEXT        CHECK (smtp_alt_security IN ('tls', 'starttls')),
    version           INTEGER     NOT NULL DEFAULT 1 CHECK (version > 0),
    updated_by        UUID        REFERENCES users(id) ON DELETE SET NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK ((smtp_alt_port IS NULL) = (smtp_alt_security IS NULL))
);
CREATE UNIQUE INDEX mail_client_profiles_default_unique ON mail_client_profiles ((true)) WHERE domain_id IS NULL;

CREATE TABLE mail_client_profile_history (
    id               BIGSERIAL PRIMARY KEY,
    profile_id       UUID,
    domain_id        UUID,
    domain_name      TEXT,
    version          INTEGER     NOT NULL,
    action           TEXT        NOT NULL CHECK (action IN ('created', 'updated', 'removed')),
    settings         JSONB       NOT NULL,
    change_note      TEXT        CHECK (length(change_note) <= 500),
    changed_by       UUID        REFERENCES users(id) ON DELETE SET NULL,
    changed_by_label TEXT,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX mail_client_profile_history_idx ON mail_client_profile_history (created_at DESC);

-- Aggregated onboarding analytics. No content, credentials or device
-- identifiers: only which guide was used and how far the employee got.
CREATE TABLE mailbox_setup_events (
    id              BIGSERIAL PRIMARY KEY,
    organization_id UUID        NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    mailbox_id      UUID        NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    event           TEXT        NOT NULL CHECK (event IN (
                        'guide_started', 'device_selected', 'client_selected', 'readiness_checked',
                        'step_viewed', 'guide_completed', 'help_viewed', 'profile_downloaded',
                        'test_requested', 'connection_verified', 'connection_failed')),
    device          TEXT        CHECK (device IN ('android', 'ios', 'computer', 'other')),
    client          TEXT        CHECK (client IN ('gmail', 'apple_mail', 'outlook', 'other', 'webmail')),
    detail          TEXT        CHECK (length(detail) <= 120),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX mailbox_setup_events_org_idx ON mailbox_setup_events (organization_id, created_at DESC);
CREATE INDEX mailbox_setup_events_mailbox_idx ON mailbox_setup_events (mailbox_id, created_at DESC);

-- Employee-driven connection proofs: an incoming check delivers a code into
-- the inbox that the employee reads on the phone; an outgoing check waits for
-- a message the employee sends from the phone to their own address.
CREATE TABLE mailbox_connection_checks (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID        NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    mailbox_id      UUID        NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    direction       TEXT        NOT NULL CHECK (direction IN ('incoming', 'outgoing')),
    code            TEXT        NOT NULL CHECK (code ~ '^[A-Z0-9]{6,10}$'),
    client          TEXT        CHECK (client IN ('gmail', 'apple_mail', 'outlook', 'other', 'webmail')),
    status          TEXT        NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'verified', 'expired')),
    attempts        INTEGER     NOT NULL DEFAULT 0,
    expires_at      TIMESTAMPTZ NOT NULL,
    verified_at     TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX mailbox_connection_checks_mailbox_idx ON mailbox_connection_checks (mailbox_id, created_at DESC);

-- ── Webmail preferences and personal contacts ────────────────────────────
CREATE TABLE mailbox_preferences (
    mailbox_id            UUID PRIMARY KEY REFERENCES mailboxes(id) ON DELETE CASCADE,
    signature             TEXT    CHECK (length(signature) <= 4000),
    messages_per_page     INTEGER NOT NULL DEFAULT 50 CHECK (messages_per_page BETWEEN 10 AND 100),
    auto_reply_enabled    BOOLEAN NOT NULL DEFAULT false,
    auto_reply_subject    TEXT    CHECK (length(auto_reply_subject) <= 200),
    auto_reply_body       TEXT    CHECK (length(auto_reply_body) <= 4000),
    auto_reply_starts_on  DATE,
    auto_reply_ends_on    DATE,
    auto_reply_synced_at  TIMESTAMPTZ,
    auto_reply_sync_error TEXT,
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (auto_reply_ends_on IS NULL OR auto_reply_starts_on IS NULL OR auto_reply_ends_on >= auto_reply_starts_on)
);

CREATE TABLE mailbox_contacts (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    mailbox_id   UUID        NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    name         TEXT        CHECK (length(name) <= 200),
    email        CITEXT      NOT NULL CHECK (length(email) BETWEEN 3 AND 254),
    phone        TEXT        CHECK (length(phone) <= 40),
    organization TEXT        CHECK (length(organization) <= 200),
    notes        TEXT        CHECK (length(notes) <= 2000),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (mailbox_id, email)
);
CREATE INDEX mailbox_contacts_name_idx ON mailbox_contacts (mailbox_id, lower(name));
