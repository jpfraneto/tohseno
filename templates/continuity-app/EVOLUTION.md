# Application evolution index

This file is a safe, owner-reviewable index from intent to manifest and release.
It is not the canonical store for exact owner requests.

Do not paste customer prompts, contact details, capabilities, credentials,
message bodies, production data, private continuity content, encryption keys,
operator tokens, payment secrets, or model chain-of-thought here. Preserve exact
owner intent only in an owner-approved private repository or encrypted store.

## Current state

- Application: One Quiet Mark (illustrative)
- Current manifest revision: illustrative template only
- Current release revision: none
- Production status: not implemented or deployed

Replace this section only with verified facts. A `READY` order or an agent's
completed response is not release evidence.

## Revision entries

Add one entry before changing an existing application. Rollback is another entry;
do not erase the revision being rolled back.

### Revision `<stable revision ID>`

- Parent revision: `<stable revision ID or none>`
- Status: `received | distilled | proposed | approved | refused | applied | verified | released | superseded`
- Recorded at: `<RFC 3339 timestamp>`
- Private source retained: `<yes/no; no path, capability, or secret reference>`
- Intent summary: `<sanitized statement of the desired change>`
- Manifest before: `<schema version and safe revision/digest reference>`
- Manifest after: `<schema version and safe revision/digest reference or none>`
- Unsupported requirements: `<safe list or none>`
- Open decisions: `<safe list or none>`
- Owner approval: `<required/not required and safe evidence reference>`
- Source commit: `<commit or none>`
- Verification: `<commands/results or none>`
- Deployment artifact: `<safe image/build/release ID or none>`
- Rollback/ejection impact: `<safe summary>`

An integrity digest identifies exact private bytes but does not authorize access.
Do not publish a private prompt digest automatically: even a digest may correlate
records or make a low-entropy request guessable. If a private system records one,
keep it inside the same owner-controlled privacy boundary.
