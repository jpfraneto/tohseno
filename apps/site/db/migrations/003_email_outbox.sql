ALTER TABLE messages ADD COLUMN template TEXT;
ALTER TABLE messages ADD COLUMN status TEXT NOT NULL DEFAULT 'pending';
ALTER TABLE messages ADD COLUMN idempotency_key TEXT;
ALTER TABLE messages ADD COLUMN updated_at TEXT;

UPDATE messages
SET template = 'operator-status',
    status = CASE WHEN provider_reference IS NULL THEN 'failed' ELSE 'sent' END,
    idempotency_key = 'legacy-message:' || id,
    updated_at = created_at
WHERE template IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS messages_idempotency_idx
ON messages(idempotency_key)
WHERE idempotency_key IS NOT NULL;

CREATE INDEX IF NOT EXISTS messages_delivery_idx
ON messages(status, created_at);
