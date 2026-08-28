# Legacy subscription billing release boundary

ADR 0025 retains this receipt verifier only for compatibility. Recurring Pro
billing is not the native product's purchase surface and never gates local/BYO
work. Prepaid managed creation balance has a separate append-only ledger and
runbook at `docs/runbooks/MANAGED_COMPUTE.md`.

This directory intentionally contains no signing secret and no active public
verification key. A release authorized for billing must contain exactly
`verification-key-p256.txt`: one base64url, compressed SEC1 P-256 public key.
The corresponding PKCS#8 private key belongs only in the website operator's
secret manager. If the public file is absent, Studio refuses to begin checkout.

See `docs/runbooks/BILLING_1_0_0.md`.
