# Local development

The local path exercises the complete intake, status, mock-payment, capsule, and operator workflow without network access or paid services.

## Prerequisites

- Bun installed and available as `bun`.
- Git.
- A checkout of this repository.

Do not use npm, pnpm, or yarn. No frontend build tool or separate database server is required.

## First setup

```sh
bun install
cp .env.example .env
bun run generate-secrets
```

`generate-secrets` prints a base64 32-byte `TOHSENO_DATA_KEY` and a strong `TOHSENO_OPERATOR_TOKEN`. Copy both values into `.env`; the command deliberately does not edit a file. Do not commit `.env` or paste those values into issues, logs, screenshots, or shell history shared with others.

For a self-contained local run, use values equivalent to:

```text
NODE_ENV=development
PORT=3000
BASE_URL=http://localhost:3000
DATABASE_PATH=./data/tohseno.sqlite
TRUST_PROXY=false
PAYMENTS_MODE=mock
EMAIL_MODE=console
```

Mock payment mode is development/test behavior and the server refuses it when `NODE_ENV=production`. Console email reports safe delivery metadata rather than printing recipient addresses or message bodies. Use `disabled` instead if no email side effect is wanted.

Apply migrations and start the server:

```sh
bun run migrate
bun run dev
```

Open `http://localhost:3000`. The normal HTML form remains the simplest end-to-end test.

## Useful commands

| Command | Purpose |
|---|---|
| `bun run dev` | Start the server with development watching/reload behavior |
| `bun run start` | Start the server without the development wrapper |
| `bun run migrate` | Apply pending raw SQLite migrations |
| `bun run test` | Run all network-independent tests |
| `bun run typecheck` | Check strict TypeScript |
| `bun run check` | Run the full safe pre-commit validation |
| `bun run generate-secrets` | Print fresh data/operator secrets without writing them |
| `bun run operator -- list` | List safe submission metadata through the operator API |

The Makefile exposes concise equivalents for common commands.

## HTTP smoke test

Check public routes without private data:

```sh
curl -i http://localhost:3000/healthz
curl -i http://localhost:3000/
curl -i http://localhost:3000/privacy
```

For an intake smoke test, use synthetic Markdown and an example-domain address. A normal form submission is preferred because it also checks progressive enhancement and redirects. If using `curl`, follow the exact content type accepted by `POST /api/submissions` and keep the response private: it contains a bearer status URL that must not be pasted into logs or tickets.

Verify that a random capability is indistinguishable from expired/revoked credentials:

```sh
curl -i http://localhost:3000/c/not-a-real-capability
```

It should return `404` without explaining which capability check failed.

## Operator CLI

The CLI calls the same narrow authenticated API intended for a future operator agent:

```sh
bun run operator -- list
bun run operator -- show <submission-id>
bun run operator -- transition <submission-id> <next-state>
bun run operator -- summary <submission-id> <json-file>
bun run operator -- message <submission-id> <text-file>
bun run operator -- retry-email <submission-id>
bun run operator -- revoke-capability <submission-id>
```

Set the CLI base URL and operator token through the documented environment configuration. The CLI refuses remote plaintext HTTP, credential-bearing/malformed base origins, redirects, and long-hanging requests. `show` first reads safe operational detail, then calls the explicit private `inspect-source` boundary. Decrypting Markdown records a safe audit event; do not redirect its output into tracked files.

Summary JSON must contain only safe compiled/operational fields. Message bodies are encrypted in storage. Safe operator detail includes notification template/status/provider reference, allowing `retry-email` to drain pending/failed intents without exposing recipient or body. Transition metadata is size-bounded and rejects sensitive field names; it must never include private documents, email addresses, capabilities, credentials, or arbitrary message text.

## Local database handling

The default database lives at `./data/tohseno.sqlite`. SQLite may create `-wal` and `-shm` siblings. All are local state and ignored by Git.

Tests create temporary databases and keys. Do not aim tests at a development or production path. To reset local development, stop the server and remove the local database files only if their synthetic contents are disposable; there is deliberately no repository command that destroys them automatically.

To inspect the privacy invariant, search the database file and WAL for your synthetic input and raw capability. The automated suite does this against a temporary database. Never run a plaintext search using real customer data as a shell argument because the shell/process history can become another disclosure.

## Before committing

```sh
bun run check
git diff --check
git status --short
```

Confirm that `.env`, `data/`, `*.sqlite*`, credentials, private capsules, and synthetic outputs are not staged. Tests must remain offline and use fake payment/email providers.
