# TOHSENO 0.9.0 Verification Matrix

Run ID: `20260816T134315Z`

Starting source: `d1ae6db3e0b3042b03ca667ac9db6dbedac4f840` on `main`, with the uncommitted inventory captured in `dist/verification/20260816T134315Z/phase0-git.log`.

This matrix is the claim ledger for the 0.9.0 adversarial verification transaction. A source review can identify an implementation location, but it cannot by itself move a runtime claim to `PASS`. Every row records the required fields: status, source implementation, independent test, manual or physical evidence, evidence path, defects found, fix commit, and final conclusion.

| ID | Claim | Status | Source implementation | Independent test | Manual or physical evidence | Evidence path | Defects found | Fix commit | Final conclusion |
|---|---|---|---|---|---|---|---|---|---|
| ARCH-01 | CLI, Studio, and Companion use one application service. | UNVERIFIED | Pending audit | Pending | N/A | Pending | None recorded | Pending | Pending |
| ARCH-02 | No frontend invokes another frontend. | UNVERIFIED | Pending audit | Pending | N/A | Pending | None recorded | Pending | Pending |
| ARCH-03 | Mobile creation does not shell out to the CLI. | UNVERIFIED | Pending audit | Pending | N/A | Pending | None recorded | Pending | Pending |
| ARCH-04 | Public node remains separate from private Companion transport. | UNVERIFIED | Pending audit | Pending | N/A | Pending | None recorded | Pending | Pending |
| CLI-01 | `create` performs factory birth. | UNVERIFIED | Pending audit | Pending | Authentic installed-candidate birth required | Pending | None recorded | Pending | Pending |
| CLI-02 | `evolve` performs exact-base evolution. | UNVERIFIED | Pending audit | Pending | Physical Companion evolution required | Pending | None recorded | Pending | Pending |
| CLI-03 | `init` and `record` preserve recording-layer compatibility. | UNVERIFIED | Pending audit | Pending | N/A | Pending | None recorded | Pending | Pending |
| CLI-04 | JSON stdout remains machine-safe. | UNVERIFIED | Pending audit | Pending | N/A | Pending | None recorded | Pending | Pending |
| CLI-05 | No-intention `create` opens the intended Studio route. | UNVERIFIED | Pending audit | Pending | Browser evidence required | Pending | None recorded | Pending | Pending |
| SVC-01 | Service survives the initiating Terminal. | UNVERIFIED | Pending audit | Pending | Real launchd lifecycle required | Pending | None recorded | Pending | Pending |
| SVC-02 | launchd owns and restarts the process. | UNVERIFIED | Pending audit | Pending | Real launchd lifecycle required | Pending | None recorded | Pending | Pending |
| SVC-03 | Explicit stop does not create a restart loop. | UNVERIFIED | Pending audit | Pending | Real launchd lifecycle required | Pending | None recorded | Pending | Pending |
| SVC-04 | Only loopback interfaces are bound. | UNVERIFIED | Pending audit | Pending | Listener inspection required | Pending | None recorded | Pending | Pending |
| SVC-05 | Real Keychain storage is used. | UNVERIFIED | Pending audit | Pending | Real Keychain lifecycle required | Pending | None recorded | Pending | Pending |
| SVC-06 | Update rollback preserves state. | UNVERIFIED | Pending audit | Pending | Isolated A/B installer journey required | Pending | None recorded | Pending | Pending |
| PAIR-01 | Pairing invitations are signed. | UNVERIFIED | Pending audit | Pending | Rendered invitation verification required | Pending | None recorded | Pending | Pending |
| PAIR-02 | Pairing invitations expire. | UNVERIFIED | Pending audit | Pending | Visible and protocol expiry required | Pending | None recorded | Pending | Pending |
| PAIR-03 | Pairing invitations are one-use. | UNVERIFIED | Pending audit | Pending | Real replay rejection required | Pending | None recorded | Pending | Pending |
| PAIR-04 | Arbitrary scanned relay origins are rejected. | UNVERIFIED | Pending audit | Pending | Real client parsing required | Pending | None recorded | Pending | Pending |
| PAIR-05 | Capability grants are explicit and revocable. | UNVERIFIED | Pending audit | Pending | Physical post-revocation rejection required | Pending | None recorded | Pending | Pending |
| PAIR-06 | Rendered Studio QR is independently decodable. | UNVERIFIED | Pending audit | Independent QR decoder required | Studio screenshot required | Pending | None recorded | Pending | Pending |
| SYNC-01 | Rust, Swift, and Bun agree on canonical bytes. | UNVERIFIED | Pending audit | Cross-language vectors and live handshake required | Swift client evidence required | Pending | None recorded | Pending | Pending |
| SYNC-02 | Relay stores only opaque encrypted payloads. | UNVERIFIED | Pending audit | Persisted-storage canary scan required | N/A | Pending | None recorded | Pending | Pending |
| SYNC-03 | Offline outbox survives process and app relaunch. | UNVERIFIED | Pending audit | Rust/relay/Swift restart test required | Physical app relaunch required | Pending | None recorded | Pending | Pending |
| SYNC-04 | Duplicate delivery is exactly once. | UNVERIFIED | Pending audit | Live/catch-up duplicate test required | Canonical Mac-side effect required | Pending | None recorded | Pending | Pending |
| SYNC-05 | Cursors reconcile missed events. | UNVERIFIED | Pending audit | Gap and snapshot-fallback test required | Real-device reconciliation required | Pending | None recorded | Pending | Pending |
| SYNC-06 | Revoked devices are rejected immediately. | UNVERIFIED | Pending audit | Post-revocation command test required | Physical Companion rejection required | Pending | None recorded | Pending | Pending |
| SYNC-07 | Encrypted icon and reference transport is bounded. | UNVERIFIED | Pending audit | Boundary/decompression tests required | Real icon decode where available | Pending | None recorded | Pending | Pending |
| SHOT-01 | Feedback binds an exact Version. | UNVERIFIED | Pending audit | Exact-Version command test required | Physical feedback required | Pending | None recorded | Pending | Pending |
| SHOT-02 | Stale evolution is rejected. | UNVERIFIED | Pending audit | Stale-base adversarial test required | Physical duplicate/stale attempt required | Pending | None recorded | Pending | Pending |
| SHOT-03 | Mobile evolution uses the shared factory. | UNVERIFIED | Pending audit | Journal-to-engine trace required | Authentic physical evolution required | Pending | None recorded | Pending | Pending |
| SHOT-04 | Mobile creation uses the shared factory. | UNVERIFIED | Pending audit | Journal-to-engine trace required | Authentic physical creation required | Pending | None recorded | Pending | Pending |
| SHOT-05 | Accepted state requires the real acceptance gates. | UNVERIFIED | Pending audit | Fail-closed gate tests required | Physical install and launch required | Pending | None recorded | Pending | Pending |
| IOS-01 | CompanionKit builds and passes shared vectors. | UNVERIFIED | Pending audit | Swift build/tests required | N/A | Pending | None recorded | Pending | Pending |
| IOS-02 | Conformance app runs in Simulator. | UNVERIFIED | Pending audit | Xcode build/UI verification required | Simulator screenshot required | Pending | None recorded | Pending | Pending |
| IOS-03 | Conformance app installs on a physical iPhone. | UNVERIFIED | Pending audit | Signed device build/install required | Physical iPhone required | Pending | None recorded | Pending | Pending |
| IOS-04 | Real camera pairing succeeds. | UNVERIFIED | Pending audit | Real Studio-to-camera handshake required | Human camera scan required | Pending | None recorded | Pending | Pending |
| IOS-05 | Real-device Keychain identity survives relaunch. | UNVERIFIED | Pending audit | Terminate/relaunch identity comparison required | Physical iPhone required | Pending | None recorded | Pending | Pending |
| IOS-06 | Real-device synchronization works after interruption. | UNVERIFIED | Pending audit | Relay/network interruption required | Physical iPhone required | Pending | None recorded | Pending | Pending |
| REL-01 | Clean deterministic artifacts are produced. | UNVERIFIED | Pending audit | Two-build byte comparison required | N/A | Pending | None recorded | Pending | Pending |
| REL-02 | Installer works from those exact artifacts. | UNVERIFIED | Pending audit | Isolated installer test required | N/A | Pending | None recorded | Pending | Pending |
| REL-03 | Persistent service works after installed-shell exit. | UNVERIFIED | Pending audit | Installed launchd lifecycle required | Real launchd evidence required | Pending | None recorded | Pending | Pending |
| REL-04 | Update and rollback work. | UNVERIFIED | Pending audit | A/B and failed-health rollback required | N/A | Pending | None recorded | Pending | Pending |
| REL-05 | Uninstall preserves user data. | UNVERIFIED | Pending audit | Isolated uninstall attack tests required | N/A | Pending | None recorded | Pending | Pending |
| REL-06 | Release metadata agrees byte-for-byte. | UNVERIFIED | Pending audit | Manifest/checksum/readiness comparison required | N/A | Pending | None recorded | Pending | Pending |
| JOURNAL-01 | Commands are durable before semantic execution. | UNVERIFIED | Pending audit | Failpoint crash-boundary tests required | N/A | Pending | None recorded | Pending | Pending |
| JOURNAL-02 | Crash recovery cannot duplicate feedback, Versions, Shots, or executions. | UNVERIFIED | Pending audit | Boundary failpoint matrix required | N/A | Pending | None recorded | Pending | Pending |
| RECORD-01 | Existing recording-only folders are recognized without mutation or promotion. | UNVERIFIED | Pending audit | Byte-level compatibility fixture required | Companion snapshot inspection required | Pending | None recorded | Pending | Pending |
| HTTP-01 | Local HTTP rejects non-loopback, hostile Host/Origin, CSRF, method, path, and size attacks. | UNVERIFIED | Pending audit | Adversarial HTTP suite required | Runtime listener inspection required | Pending | None recorded | Pending | Pending |
| INSTALL-01 | Installer rejects symlink, ownership, tamper, and interruption attacks. | UNVERIFIED | Pending audit | Adversarial installer suite required | N/A | Pending | None recorded | Pending | Pending |
| PRIV-01 | Plaintext canaries never enter relay-visible storage or operational logs. | UNVERIFIED | Pending audit | Real synchronization and repository-wide canary scan required | Physical sync path required | Pending | None recorded | Pending | Pending |
| DOC-01 | Governed and operational documentation gives one authority-consistent account of 0.9.0. | UNVERIFIED | Pending audit | Documentation consistency audit required | N/A | Pending | None recorded | Pending | Pending |
| SEC-01 | No unresolved critical or high-severity security defect remains. | UNVERIFIED | Pending audit | Source/runtime/dependency review required | Real lifecycle evidence required | Pending | None recorded | Pending | Pending |
| FACTORY-01 | Installed 0.9.0 candidate completes an authentic-harness birth. | UNVERIFIED | Pending audit | Public installed CLI smoke required | Physical delivery required | Pending | None recorded | Pending | Pending |
| FACTORY-02 | Mobile feedback and exact-base evolution produce exactly one accepted Version 0002. | UNVERIFIED | Pending audit | Authentic evolution and duplicate replay required | Physical Companion required | Pending | None recorded | Pending | Pending |
| FACTORY-03 | Mobile creation produces exactly one new accepted Shot. | UNVERIFIED | Pending audit | Authentic creation and duplicate replay required | Physical Companion required | Pending | None recorded | Pending | Pending |

## Status law

- `PASS`: the source path is understood and the independent runtime or byte-level evidence required by the claim succeeded.
- `FAIL`: direct evidence contradicts the claim; a fix and regression evidence are required before reconsideration.
- `BLOCKED`: required external authority, credential, physical action, or unavailable infrastructure prevented the test; absence of proof is not converted to a pass.
- `UNVERIFIED`: the audit or test has not yet been completed.
