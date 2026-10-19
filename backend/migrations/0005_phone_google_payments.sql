-- Phone verification, Google sign-in, M-Pesa payments (PesaTide) and
-- flat Kenyan-shilling pricing.

-- ── Users: username sign-in, verified phone, linked Google account ───────
ALTER TABLE users
    ADD COLUMN username          CITEXT UNIQUE CHECK (username ~ '^[a-z0-9][a-z0-9._-]{2,31}$'),
    ADD COLUMN phone             TEXT UNIQUE CHECK (phone ~ '^\+[1-9][0-9]{7,14}$'),
    ADD COLUMN phone_verified_at TIMESTAMPTZ,
    ADD COLUMN google_sub        TEXT UNIQUE CHECK (length(google_sub) <= 255);

-- Six-digit SMS codes, stored hashed, single use, short-lived.
CREATE TABLE phone_codes (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id      UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    phone        TEXT        NOT NULL,
    code_hash    BYTEA       NOT NULL,
    attempts     INTEGER     NOT NULL DEFAULT 0,
    expires_at   TIMESTAMPTZ NOT NULL,
    consumed_at  TIMESTAMPTZ,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX phone_codes_user_idx ON phone_codes (user_id, created_at DESC);

-- ── Payments (M-Pesa STK push through PesaTide) ──────────────────────────
CREATE TABLE payments (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id     UUID        NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    invoice_id          UUID        NOT NULL REFERENCES invoices(id) ON DELETE CASCADE,
    provider            TEXT        NOT NULL DEFAULT 'pesatide' CHECK (provider IN ('pesatide')),
    reference           TEXT        NOT NULL UNIQUE,
    provider_id         TEXT        UNIQUE,
    phone               TEXT        NOT NULL,
    amount_cents        BIGINT      NOT NULL CHECK (amount_cents > 0),
    currency            TEXT        NOT NULL CHECK (currency ~ '^[A-Z]{3}$'),
    status              TEXT        NOT NULL DEFAULT 'pending'
                        CHECK (status IN ('pending', 'processing', 'succeeded', 'failed', 'cancelled', 'expired')),
    receipt             TEXT        CHECK (length(receipt) <= 64),
    failure_reason      TEXT        CHECK (length(failure_reason) <= 300),
    initiated_by        UUID        REFERENCES users(id) ON DELETE SET NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at        TIMESTAMPTZ
);
CREATE INDEX payments_invoice_idx ON payments (invoice_id, created_at DESC);
CREATE INDEX payments_open_idx ON payments (created_at) WHERE status IN ('pending', 'processing');

-- ── Flat monthly pricing in Kenyan shillings (amounts in cents) ──────────
-- Individual developers: Starter KES 120, Standard KES 200.
-- Institutions: Premium KES 1,200 for the whole institution.
UPDATE plans SET name = 'Starter', price_monthly_cents = 12000, price_per_mailbox_cents = 0,
    description = 'For a single app: invoices, receipts and alerts from your own domain.'
WHERE id = 'developer_plus';
UPDATE plans SET name = 'Standard', price_monthly_cents = 20000, price_per_mailbox_cents = 0,
    description = 'Higher volume, several domains and a teammate for growing products.'
WHERE id = 'developer_scale';
UPDATE plans SET name = 'Premium', price_monthly_cents = 120000, price_per_mailbox_cents = 0,
    description = 'Work email for your whole institution: mailboxes, webmail, phone setup and the sending API. One flat price.'
WHERE id = 'pro';
-- The earlier per-mailbox institution tier is retired for new customers.
UPDATE plans SET is_public = false, self_service = false, price_per_mailbox_cents = 0 WHERE id = 'starter';
UPDATE plans SET price_per_mailbox_cents = 0;
