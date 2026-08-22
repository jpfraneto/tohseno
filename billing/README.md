# Billing release boundary

This directory intentionally contains no signing secret and no active public
verification key. A release authorized for billing must contain exactly
`verification-key-p256.txt`: one base64url, compressed SEC1 P-256 public key.
The corresponding PKCS#8 private key belongs only in the website operator's
secret manager. If the public file is absent, Studio refuses to begin checkout.

See `docs/runbooks/BILLING_0_9_9.md`.
