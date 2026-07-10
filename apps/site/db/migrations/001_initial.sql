CREATE TABLE IF NOT EXISTS submissions (
  id TEXT PRIMARY KEY,
  content_hash TEXT NOT NULL,
  encrypted_markdown TEXT NOT NULL,
  encrypted_contact TEXT NOT NULL,
  capability_token_hash TEXT NOT NULL UNIQUE,
  capability_expires_at TEXT NOT NULL,
  capability_revoked_at TEXT,
  operating_mode TEXT NOT NULL CHECK (operating_mode IN ('self-hosted', 'client-owned', 'anky-operated')),
  status TEXT NOT NULL,
  manifest_version TEXT NOT NULL,
  compiled_summary_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS submissions_status_created_idx ON submissions(status, created_at);

CREATE TABLE IF NOT EXISTS order_events (
  sequence INTEGER PRIMARY KEY AUTOINCREMENT,
  id TEXT NOT NULL UNIQUE,
  submission_id TEXT NOT NULL REFERENCES submissions(id) ON DELETE RESTRICT,
  previous_status TEXT NOT NULL,
  next_status TEXT NOT NULL,
  actor_type TEXT NOT NULL,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS order_events_submission_idx ON order_events(submission_id, sequence);

CREATE TRIGGER IF NOT EXISTS order_events_no_update
BEFORE UPDATE ON order_events
BEGIN
  SELECT RAISE(ABORT, 'order_events are append-only');
END;

CREATE TRIGGER IF NOT EXISTS order_events_no_delete
BEFORE DELETE ON order_events
BEGIN
  SELECT RAISE(ABORT, 'order_events are append-only');
END;

CREATE TABLE IF NOT EXISTS payments (
  id TEXT PRIMARY KEY,
  submission_id TEXT NOT NULL REFERENCES submissions(id) ON DELETE RESTRICT,
  provider TEXT NOT NULL,
  provider_reference TEXT,
  checkout_session_id TEXT NOT NULL UNIQUE,
  checkout_url TEXT,
  attempt INTEGER NOT NULL CHECK (attempt >= 1),
  amount INTEGER NOT NULL CHECK (amount >= 0),
  currency TEXT NOT NULL,
  status TEXT NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE (submission_id, attempt)
);

CREATE INDEX IF NOT EXISTS payments_submission_idx ON payments(submission_id, created_at);

CREATE TABLE IF NOT EXISTS payment_events (
  provider TEXT NOT NULL,
  provider_event_id TEXT NOT NULL,
  checkout_session_id TEXT NOT NULL,
  outcome TEXT NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY (provider, provider_event_id)
);

CREATE TABLE IF NOT EXISTS messages (
  id TEXT PRIMARY KEY,
  submission_id TEXT NOT NULL REFERENCES submissions(id) ON DELETE RESTRICT,
  direction TEXT NOT NULL CHECK (direction IN ('inbound', 'outbound')),
  channel TEXT NOT NULL CHECK (channel = 'email'),
  encrypted_body TEXT NOT NULL,
  provider_reference TEXT,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS messages_submission_idx ON messages(submission_id, created_at);
