-- Phase 2/3: domains, API keys, templates, suppressions, messages,
-- transactional outbox, webhooks, quotas and idempotency.

-- ── Domains ──────────────────────────────────────────────────────────────
-- The verification token is stored in plain text on purpose: the customer
-- must be able to see it to publish it, and once published it is public DNS
-- data. Ownership is proven by the record's presence, not its secrecy.
CREATE TABLE domains (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id       UUID        NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    domain_name           CITEXT      NOT NULL CHECK (length(domain_name) BETWEEN 3 AND 253),
    status                TEXT        NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'verified', 'failed')),
    verification_token    TEXT        NOT NULL,
    verified_at           TIMESTAMPTZ,
    last_checked_at       TIMESTAMPTZ,
    check_count           INTEGER     NOT NULL DEFAULT 0,
    last_check_result     JSONB,
    spf_ok                BOOLEAN     NOT NULL DEFAULT false,
    dkim_ok               BOOLEAN     NOT NULL DEFAULT false,
    dmarc_ok              BOOLEAN     NOT NULL DEFAULT false,
    mx_ok                 BOOLEAN     NOT NULL DEFAULT false,
    dkim_selector         TEXT        NOT NULL,
    dkim_private_key_enc  BYTEA       NOT NULL,
    dkim_public_key       TEXT        NOT NULL,
    mail_hosting_enabled  BOOLEAN     NOT NULL DEFAULT false,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (organization_id, domain_name),
    UNIQUE (id, organization_id)                -- target for composite tenant FKs
);
-- A domain name can be verified by only one tenant platform-wide.
CREATE UNIQUE INDEX domains_verified_global_unique ON domains (domain_name) WHERE status = 'verified';

-- ── API keys ─────────────────────────────────────────────────────────────
-- Format: mv_<env>_<12-char public id>_<40-char secret>. The public prefix
-- is indexed for lookup; only SHA-256(full key) is stored. Keys carry 238
-- bits of entropy, so a fast hash is appropriate (a slow KDF protects
-- low-entropy human passwords, which these are not).
CREATE TABLE api_keys (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID        NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name            TEXT        NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
    prefix          TEXT        NOT NULL UNIQUE,
    secret_hash     BYTEA       NOT NULL,
    environment     TEXT        NOT NULL CHECK (environment IN ('live', 'test')),
    scopes          TEXT[]      NOT NULL,
    created_by      UUID        REFERENCES users(id) ON DELETE SET NULL,
    expires_at      TIMESTAMPTZ,
    revoked_at      TIMESTAMPTZ,
    last_used_at    TIMESTAMPTZ,
    last_used_ip    INET,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX api_keys_org_idx ON api_keys (organization_id, created_at DESC);

-- ── Templates (immutable versions; the template points at the active one) ─
CREATE TABLE templates (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID        NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name            TEXT        NOT NULL CHECK (length(name) BETWEEN 1 AND 120),
    slug            CITEXT      NOT NULL CHECK (slug ~ '^[a-z0-9][a-z0-9_-]{0,79}$'),
    description     TEXT,
    status          TEXT        NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'archived')),
    active_version  INTEGER     NOT NULL DEFAULT 1,
    latest_version  INTEGER     NOT NULL DEFAULT 1,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (organization_id, slug),
    UNIQUE (id, organization_id)
);

CREATE TABLE template_versions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    template_id     UUID        NOT NULL,
    organization_id UUID        NOT NULL,
    version         INTEGER     NOT NULL CHECK (version > 0),
    subject         TEXT        NOT NULL CHECK (length(subject) BETWEEN 1 AND 998),
    html_body       TEXT,
    text_body       TEXT,
    variables       JSONB       NOT NULL DEFAULT '[]'::jsonb,
    created_by      UUID        REFERENCES users(id) ON DELETE SET NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (template_id, version),
    FOREIGN KEY (template_id, organization_id) REFERENCES templates(id, organization_id) ON DELETE CASCADE,
    CHECK (html_body IS NOT NULL OR text_body IS NOT NULL)
);

-- ── Suppressions ─────────────────────────────────────────────────────────
CREATE TABLE suppressions (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id   UUID        NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    email             CITEXT      NOT NULL,
    reason            TEXT        NOT NULL CHECK (reason IN ('hard_bounce', 'complaint', 'manual', 'unsubscribe')),
    source_message_id UUID,
    details           TEXT,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (organization_id, email)
);

-- ── Messages ─────────────────────────────────────────────────────────────
-- One row per envelope batch. API sends create one row per recipient so
-- each recipient has an independent status; webmail sends keep all
-- recipients of one MIME message together.
CREATE TABLE messages (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id     UUID        NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    batch_id            UUID        NOT NULL,
    source              TEXT        NOT NULL CHECK (source IN ('api', 'console', 'webmail')),
    api_key_id          UUID        REFERENCES api_keys(id) ON DELETE SET NULL,
    created_by_user_id  UUID        REFERENCES users(id) ON DELETE SET NULL,
    mailbox_id          UUID,
    domain_id           UUID,                -- NULL once the domain is deleted; history is kept
    from_address        CITEXT      NOT NULL,
    from_name           TEXT,
    envelope_from       TEXT        NOT NULL,
    to_address          TEXT        NOT NULL,
    envelope_recipients TEXT[]      NOT NULL CHECK (cardinality(envelope_recipients) > 0),
    reply_to            TEXT,
    reply_mode          TEXT        NOT NULL CHECK (reply_mode IN ('no_reply', 'reply_to_mailbox', 'reply_to_user')),
    subject             TEXT        NOT NULL,
    template_id         UUID,
    template_version    INTEGER,
    status              TEXT        NOT NULL CHECK (status IN (
                            'queued', 'processing', 'accepted_by_mta', 'delivered', 'deferred',
                            'bounced', 'failed', 'suppressed', 'sandboxed')),
    sandbox             BOOLEAN     NOT NULL DEFAULT false,
    smtp_message_id     TEXT        NOT NULL,
    attempts            INTEGER     NOT NULL DEFAULT 0,
    last_error          TEXT,
    last_smtp_code      INTEGER,
    tags                TEXT[]      NOT NULL DEFAULT '{}',
    metadata            JSONB       NOT NULL DEFAULT '{}'::jsonb,
    idempotency_key     TEXT,
    size_bytes          INTEGER     NOT NULL DEFAULT 0,
    accepted_at         TIMESTAMPTZ,
    bounced_at          TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (domain_id, organization_id) REFERENCES domains(id, organization_id) ON DELETE SET NULL (domain_id),
    FOREIGN KEY (template_id, organization_id) REFERENCES templates(id, organization_id)
);
CREATE INDEX messages_org_created_idx ON messages (organization_id, created_at DESC, id DESC);
CREATE INDEX messages_org_status_idx ON messages (organization_id, status, created_at DESC);
CREATE INDEX messages_batch_idx ON messages (batch_id);
CREATE INDEX messages_smtp_id_idx ON messages (smtp_message_id);

-- Bodies live apart from metadata so content can be purged on a retention
-- schedule while delivery history is kept.
CREATE TABLE message_contents (
    message_id  UUID PRIMARY KEY REFERENCES messages(id) ON DELETE CASCADE,
    raw_mime    BYTEA,
    html_body   TEXT,
    text_body   TEXT,
    purged_at   TIMESTAMPTZ
);

CREATE TABLE message_events (
    id              BIGSERIAL PRIMARY KEY,
    message_id      UUID        NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    organization_id UUID        NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    event_type      TEXT        NOT NULL,
    details         JSONB       NOT NULL DEFAULT '{}'::jsonb,
    occurred_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX message_events_message_idx ON message_events (message_id, occurred_at);

-- ── Idempotency (Stripe-style: same key + same body ⇒ same response) ─────
CREATE TABLE idempotency_keys (
    organization_id UUID        NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    key             TEXT        NOT NULL CHECK (length(key) BETWEEN 1 AND 255),
    request_hash    BYTEA       NOT NULL,
    response_status INTEGER,
    response_body   JSONB,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (organization_id, key)
);
CREATE INDEX idempotency_keys_created_idx ON idempotency_keys (created_at);

-- ── Usage counters for quota enforcement ─────────────────────────────────
CREATE TABLE usage_daily (
    organization_id UUID    NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    day             DATE    NOT NULL,
    messages_queued INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (organization_id, day)
);

-- ── Webhooks ─────────────────────────────────────────────────────────────
CREATE TABLE webhook_endpoints (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id   UUID        NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    url               TEXT        NOT NULL CHECK (length(url) BETWEEN 8 AND 2048),
    description       TEXT,
    events            TEXT[]      NOT NULL CHECK (cardinality(events) > 0),
    secret_enc        BYTEA       NOT NULL,
    status            TEXT        NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'disabled')),
    consecutive_failures INTEGER  NOT NULL DEFAULT 0,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (id, organization_id)
);

CREATE TABLE webhook_deliveries (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    endpoint_id      UUID        NOT NULL,
    organization_id  UUID        NOT NULL,
    event_id         UUID        NOT NULL,
    event_type       TEXT        NOT NULL,
    payload          JSONB       NOT NULL,
    status           TEXT        NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'succeeded', 'failed')),
    attempts         INTEGER     NOT NULL DEFAULT 0,
    response_status  INTEGER,
    response_excerpt TEXT,
    last_error       TEXT,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at     TIMESTAMPTZ,
    FOREIGN KEY (endpoint_id, organization_id) REFERENCES webhook_endpoints(id, organization_id) ON DELETE CASCADE
);
CREATE INDEX webhook_deliveries_endpoint_idx ON webhook_deliveries (endpoint_id, created_at DESC);

-- ── Transactional outbox ─────────────────────────────────────────────────
-- Rows are inserted in the same transaction as the business change they
-- represent. Workers claim with FOR UPDATE SKIP LOCKED and a lease; a
-- crashed worker's lease expires and the job is claimed again.
CREATE TABLE outbox_jobs (
    id              BIGSERIAL PRIMARY KEY,
    kind            TEXT        NOT NULL CHECK (kind IN ('send_message', 'deliver_webhook', 'system_email')),
    organization_id UUID        REFERENCES organizations(id) ON DELETE CASCADE,
    message_id      UUID        REFERENCES messages(id) ON DELETE CASCADE,
    webhook_delivery_id UUID    REFERENCES webhook_deliveries(id) ON DELETE CASCADE,
    payload         JSONB       NOT NULL DEFAULT '{}'::jsonb,
    status          TEXT        NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'processing', 'done', 'dead')),
    attempts        INTEGER     NOT NULL DEFAULT 0,
    max_attempts    INTEGER     NOT NULL DEFAULT 8,
    run_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    locked_by       TEXT,
    locked_until    TIMESTAMPTZ,
    last_error      TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX outbox_jobs_ready_idx ON outbox_jobs (kind, run_at) WHERE status IN ('pending', 'processing');

-- Wake workers immediately instead of waiting for the next poll.
CREATE FUNCTION notify_outbox() RETURNS trigger AS $$
BEGIN
    PERFORM pg_notify('outbox_jobs', NEW.kind);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER outbox_jobs_notify AFTER INSERT ON outbox_jobs
    FOR EACH ROW EXECUTE FUNCTION notify_outbox();
