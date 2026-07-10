# Key rotation considerations

TOHSENO encrypts source Markdown, contact details, and messages with AES-256-GCM under `TOHSENO_DATA_KEY`. Rotation is a data migration, not an environment-variable refresh.

## Current key model

- The environment supplies one base64-encoded 32-byte data key.
- Every encrypted value receives an independent random nonce.
- Every envelope authenticates its submission/message field context as AES-GCM additional data, preventing cross-record ciphertext substitution.
- The stored envelope carries a format/algorithm version so a future reader can select migration behavior.
- Authentication failure is treated as unreadable/tampered data.
- The database does not store the raw key.

The initial envelope does not constitute a complete key-management service. Unless implementation explicitly records a key identifier and supports multiple readers, replacing `TOHSENO_DATA_KEY` immediately makes existing ciphertext unreadable.

## Do not rotate by replacement

Do not:

1. generate a new key;
2. overwrite the production environment variable;
3. restart and assume old rows will migrate lazily.

That sequence strands existing data and can cause new rows to be written under a different key while old rows remain inaccessible.

## Safe future rotation plan

A production rotation needs an implemented and tested migration tool with this sequence:

1. Inventory every encrypted column and backup: submissions, messages, and future encrypted fields.
2. Add explicit key identifiers to new envelopes or a deployment-side keyring mapping.
3. Configure the old key read-only and the new key for writes, without logging either.
4. Stop or serialize writes for the migration, or use a transactionally safe per-row version strategy.
5. For each value, decrypt/authenticate with the old key and re-encrypt with a fresh nonce under the new key.
6. Verify counts, envelope versions, and decryption on a restored production-like backup using synthetic/plaintext-known fixtures—not real content printed to logs.
7. Switch readers to the new key and monitor authentication failures.
8. Keep the old key in restricted rollback custody for a defined period covering backups.
9. Retire old-key access only after all live data and required backups have a documented retention/migration outcome.

The repository does not yet provide that production re-encryption command. Until it does, treat data-key rotation as planned manual engineering work and do not claim automated rotation.

## Backup coupling

A database backup and its key version are a pair. Record which key identifier can read each backup without placing the key inside an unaudited archive or repository. Test isolated restoration. Key destruction can be a form of cryptographic erasure only when copies, backups, runtime memory, processors, and required record retention have been analyzed; do not promise deletion merely by losing a key.

## Other secret rotations

`TOHSENO_OPERATOR_TOKEN` is independent of encrypted data. Rotating it invalidates operator clients but does not require database re-encryption. Update the server and CLI through a coordinated secret-manager change; avoid a window with no authorized operator unless planned.

Stripe, Resend, infrastructure, and webhook secrets follow their provider rotation procedures. Use overlapping credentials only where the provider supports it, verify new credentials, then revoke old ones. They never belong inside the data-encryption envelope.

Capability tokens are per-submission bearer credentials, not global keys. A compromised capability should be revoked. A future replacement token requires storing its new one-way hash and delivering the raw value exactly once; changing `TOHSENO_DATA_KEY` does not revoke it.

## Lost or suspected-compromised key

If the data key is lost, stop writes and restore from approved secret custody; generating a new key does not recover existing ciphertext. If compromise is suspected, restrict access, preserve safe audit evidence, stop or isolate the service as appropriate, assess database/backups/runtime access, and perform a reviewed migration. Do not decrypt customer content merely to “check” exposure.
