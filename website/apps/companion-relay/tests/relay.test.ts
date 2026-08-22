import { afterEach, describe, expect, test } from "bun:test";
import { createHash, randomBytes } from "node:crypto";
import {
  mkdtempSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { loadCompanionRelayConfig } from "../config.ts";
import { FakePushProvider } from "../src/push-provider.ts";
import { createCompanionRelayApplication } from "../server.ts";

const roots: string[] = [];
afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

class MutableClock {
  constructor(private value: number) {}
  now(): number { return this.value; }
  advance(milliseconds: number): void { this.value += milliseconds; }
}

function secret() {
  const capability = randomBytes(32).toString("base64url");
  return {
    capability,
    verifier: createHash("sha256").update(Buffer.from(capability, "base64url")).digest("hex"),
  };
}

function mailboxSecrets() {
  return {
    write: secret(),
    read: secret(),
    ack: secret(),
    revoke: secret(),
    push: secret(),
  };
}

type Fixture = Awaited<ReturnType<typeof fixture>>;

async function fixture(extra: Record<string, string> = {}, now = Date.now()) {
  const root = mkdtempSync(join(tmpdir(), "tohseno-companion-relay-test-"));
  roots.push(root);
  const clock = new MutableClock(now);
  const logs: Array<Record<string, unknown>> = [];
  const push = new FakePushProvider();
  const config = loadCompanionRelayConfig({
    NODE_ENV: "test",
    HOST: "127.0.0.1",
    PORT: "3100",
    BASE_URL: "http://127.0.0.1:3100",
    COMPANION_RELAY_ENABLED: "true",
    COMPANION_RELAY_ROOT: root,
    COMPANION_RELAY_PUSH_MODE: "fake",
    ...extra,
  });
  const application = await createCompanionRelayApplication({
    config,
    push,
    clock,
    log: (record) => logs.push(record),
    logError: (record) => logs.push(record),
  });
  return { root, clock, logs, push, config, application };
}

function request(path: string, init: RequestInit = {}): Request {
  return new Request(`http://127.0.0.1:3100${path}`, init);
}

function authorization(capability: string): Record<string, string> {
  return { Authorization: `Bearer ${capability}` };
}

function canonicalTimestamp(value: number): string {
  return new Date(Math.floor(value / 1000) * 1000)
    .toISOString()
    .replace(".000Z", "Z");
}

async function createMailbox(fx: Fixture, secrets = mailboxSecrets()) {
  const response = await fx.application.fetch(request("/v1/companion/mailboxes", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      schema: "tohseno.companion-mailbox-create/1",
      write_verifier: secrets.write.verifier,
      read_verifier: secrets.read.verifier,
      ack_verifier: secrets.ack.verifier,
      revoke_verifier: secrets.revoke.verifier,
      push_verifier: secrets.push.verifier,
    }),
  }));
  expect(response.status).toBe(201);
  const body = await response.json();
  return { id: body.mailbox_id as string, secrets };
}

function envelope(
  mailboxId: string,
  now: number,
  overrides: Partial<Record<string, unknown>> = {},
) {
  return {
    schema: "tohseno.companion-envelope/1",
    envelope_id: crypto.randomUUID(),
    mailbox_id: mailboxId,
    sender_device_id: "phone_device_0001",
    recipient_device_id: "studio_device_001",
    sender_sequence: 1,
    created_at: canonicalTimestamp(now),
    expires_at: canonicalTimestamp(now + 60 * 60 * 1000),
    ephemeral_public_key: Buffer.alloc(32, 1).toString("base64url"),
    nonce: Buffer.alloc(12, 2).toString("base64url"),
    ciphertext: Buffer.from(JSON.stringify({ body: "private companion content" })).toString("base64url"),
    signature: Buffer.alloc(64, 3).toString("base64url"),
    ...overrides,
  };
}

async function upload(fx: Fixture, mailbox: Awaited<ReturnType<typeof createMailbox>>, value: object) {
  return fx.application.fetch(request(`/v1/companion/mailboxes/${mailbox.id}/envelopes`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      ...authorization(mailbox.secrets.write.capability),
    },
    body: JSON.stringify(value),
  }));
}

describe("shared Rust relay contract", () => {
  test("accepts the checked-in Rust DTO fixtures without a parallel interpretation", async () => {
    const vectors = JSON.parse(readFileSync(join(
      import.meta.dir,
      "../../../../companion/test-vectors/companion-v1.json",
    ), "utf8")) as any;
    const relay = vectors.relay;
    const fx = await fixture({}, Date.parse("2026-08-15T12:00:00Z"));
    const capability = (byte: number) => Buffer.alloc(32, byte).toString("base64url");

    const pairing = await fx.application.fetch(request("/v1/companion/pairing-sessions", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(relay.pairing_create),
    }));
    expect(pairing.status).toBe(201);
    const pairingCreated = await pairing.json();
    expect(Object.keys(pairingCreated).sort()).toEqual(Object.keys(relay.pairing_created).sort());
    expect(pairingCreated).toMatchObject({
      schema: relay.pairing_created.schema,
      expires_at: relay.pairing_created.expires_at,
    });
    expect(pairingCreated.session_id).toMatch(/^[A-Za-z0-9_-]{32}$/);
    const opaquePairingResponse = Buffer.from("rust-swift-opaque-pairing-response");
    const pairingAccepted = await fx.application.fetch(request(
      `/v1/companion/pairing-sessions/${pairingCreated.session_id}/respond`,
      {
        method: "POST",
        headers: { "Content-Type": "application/octet-stream" },
        body: opaquePairingResponse,
      },
    ));
    expect(await pairingAccepted.json()).toEqual(relay.pairing_response_accepted);
    const pairingRecovered = await fx.application.fetch(request(
      `/v1/companion/pairing-sessions/${pairingCreated.session_id}`,
      { headers: authorization(capability(50)) },
    ));
    expect(Buffer.from(await pairingRecovered.arrayBuffer())).toEqual(opaquePairingResponse);

    const created = await fx.application.fetch(request("/v1/companion/mailboxes", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(relay.mailbox_create),
    }));
    expect(created.status).toBe(201);
    const mailboxCreated = await created.json();
    expect(Object.keys(mailboxCreated).sort()).toEqual(Object.keys(relay.mailbox_created).sort());
    expect(mailboxCreated).toMatchObject({
      schema: relay.mailbox_created.schema,
      created_at: relay.mailbox_created.created_at,
    });
    expect(mailboxCreated.mailbox_id).toMatch(/^[A-Za-z0-9_-]{32}$/);

    const pushRegistered = await fx.application.fetch(request("/v1/companion/push/register", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        ...authorization(capability(56)),
      },
      body: JSON.stringify({
        ...relay.push_register,
        mailbox_id: mailboxCreated.mailbox_id,
      }),
    }));
    expect(pushRegistered.status).toBe(201);

    const directEnvelope = {
      ...relay.direct_envelope,
      mailbox_id: mailboxCreated.mailbox_id,
    };
    const accepted = await fx.application.fetch(request(
      `/v1/companion/mailboxes/${mailboxCreated.mailbox_id}/envelopes`,
      {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          ...authorization(capability(52)),
        },
        body: JSON.stringify(directEnvelope),
      },
    ));
    expect(await accepted.json()).toEqual({ ...relay.envelope_accepted, cursor: 1 });

    const page = await fx.application.fetch(request(
      `/v1/companion/mailboxes/${mailboxCreated.mailbox_id}/envelopes?cursor=0`,
      { headers: authorization(capability(53)) },
    ));
    const pageBody = await page.json();
    expect(Object.keys(pageBody).sort()).toEqual(Object.keys(relay.mailbox_page).sort());
    expect(pageBody).toEqual({
      ...relay.mailbox_page,
      envelopes: [{ cursor: 1, envelope: directEnvelope }],
      next_cursor: 1,
      head_cursor: 1,
    });
    expect(pageBody.envelopes[0].envelope.created_at).toBe(
      relay.direct_envelope.created_at,
    );
    expect(relay.direct_envelope).toEqual(vectors.envelope.envelope);

    const acknowledged = await fx.application.fetch(request(
      `/v1/companion/mailboxes/${mailboxCreated.mailbox_id}/ack`,
      {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          ...authorization(capability(54)),
        },
        body: JSON.stringify({ ...relay.mailbox_ack, cursor: 1 }),
      },
    ));
    expect(await acknowledged.json()).toEqual({
      ...relay.mailbox_acknowledged,
      acknowledged_cursor: 1,
    });

    const pushRemoved = await fx.application.fetch(request(
      `/v1/companion/push/register/${relay.push_register.device_id}`,
      {
        method: "DELETE",
        headers: {
          "Content-Type": "application/json",
          ...authorization(capability(56)),
        },
        body: JSON.stringify({
          ...relay.push_unregister,
          mailbox_id: mailboxCreated.mailbox_id,
        }),
      },
    ));
    expect(pushRemoved.status).toBe(204);

    const revoked = await fx.application.fetch(request(
      `/v1/companion/mailboxes/${mailboxCreated.mailbox_id}`,
      {
        method: "DELETE",
        headers: authorization(capability(55)),
      },
    ));
    expect(await revoked.json()).toEqual(relay.mailbox_revoked);

    const health = await (await fx.application.fetch(request("/healthz"))).json();
    expect(health).toEqual({
      ...relay.health,
      service_version: "0.9.9",
      push_enabled: true,
    });
  });
});

describe("pairing rendezvous", () => {
  test("accepts one bounded opaque response, supports retry, expiry, and cancellation", async () => {
    const fx = await fixture();
    const read = secret();
    const cancel = secret();
    const expiresAt = fx.clock.now() + 90_000;
    const created = await fx.application.fetch(request("/v1/companion/pairing-sessions", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        schema: "tohseno.companion-pairing-session-create/1",
        expires_at: canonicalTimestamp(expiresAt),
        read_verifier: read.verifier,
        cancel_verifier: cancel.verifier,
      }),
    }));
    expect(created.status).toBe(201);
    expect(created.headers.get("Cache-Control")).toBe("no-store");
    const { session_id: id } = await created.json();
    const opaque = new TextEncoder().encode("opaque-pairing-proof-ciphertext");
    const respond = () => fx.application.fetch(request(`/v1/companion/pairing-sessions/${id}/respond`, {
      method: "POST",
      headers: { "Content-Type": "application/octet-stream" },
      body: opaque,
    }));
    expect((await respond()).status).toBe(201);
    expect(await (await respond()).json()).toMatchObject({ duplicate: true });
    const conflict = await fx.application.fetch(request(`/v1/companion/pairing-sessions/${id}/respond`, {
      method: "POST",
      headers: { "Content-Type": "application/octet-stream" },
      body: "different",
    }));
    expect(conflict.status).toBe(409);
    expect((await fx.application.fetch(request(`/v1/companion/pairing-sessions/${id}`, {
      headers: authorization(secret().capability),
    }))).status).toBe(401);
    const recovered = await fx.application.fetch(request(`/v1/companion/pairing-sessions/${id}`, {
      headers: authorization(read.capability),
    }));
    expect(new Uint8Array(await recovered.arrayBuffer())).toEqual(opaque);
    expect((await fx.application.fetch(request(`/v1/companion/pairing-sessions/${id}`, {
      method: "DELETE",
      headers: authorization(cancel.capability),
    }))).status).toBe(204);
    expect((await fx.application.fetch(request(`/v1/companion/pairing-sessions/${id}`, {
      headers: authorization(read.capability),
    }))).status).toBe(410);

    const expiringRead = secret();
    const expiringCancel = secret();
    const expiring = await fx.application.fetch(request("/v1/companion/pairing-sessions", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        schema: "tohseno.companion-pairing-session-create/1",
        expires_at: canonicalTimestamp(fx.clock.now() + 1_000),
        read_verifier: expiringRead.verifier,
        cancel_verifier: expiringCancel.verifier,
      }),
    }));
    const expiringId = (await expiring.json()).session_id;
    fx.clock.advance(1_001);
    expect((await fx.application.fetch(request(`/v1/companion/pairing-sessions/${expiringId}`, {
      headers: authorization(expiringRead.capability),
    }))).status).toBe(410);

    const serializedLogs = JSON.stringify(fx.logs);
    for (const prohibited of [id, read.capability, cancel.capability, "opaque-pairing-proof-ciphertext"]) {
      expect(serializedLogs).not.toContain(prohibited);
    }
  });

  test("enforces pairing body, schema, lifetime, and capacity bounds", async () => {
    const fx = await fixture({
      COMPANION_RELAY_MAX_PAIRING_SESSIONS: "1",
      COMPANION_RELAY_MAX_PAIRING_RESPONSE_BYTES: "32",
    });
    const firstRead = secret();
    const firstCancel = secret();
    const createBody = (read: ReturnType<typeof secret>, cancel: ReturnType<typeof secret>) => ({
      schema: "tohseno.companion-pairing-session-create/1",
      expires_at: canonicalTimestamp(fx.clock.now() + 60_000),
      read_verifier: read.verifier,
      cancel_verifier: cancel.verifier,
    });
    const first = await fx.application.fetch(request("/v1/companion/pairing-sessions", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(createBody(firstRead, firstCancel)),
    }));
    const id = (await first.json()).session_id;
    const second = await fx.application.fetch(request("/v1/companion/pairing-sessions", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(createBody(secret(), secret())),
    }));
    expect(second.status).toBe(503);
    const oversized = await fx.application.fetch(request(`/v1/companion/pairing-sessions/${id}/respond`, {
      method: "POST",
      headers: { "Content-Type": "application/octet-stream" },
      body: "x".repeat(33),
    }));
    expect(oversized.status).toBe(413);
    const tooLong = await fx.application.fetch(request("/v1/companion/pairing-sessions", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        ...createBody(secret(), secret()),
        expires_at: canonicalTimestamp(fx.clock.now() + fx.config.limits.pairingLifetimeMs + 1_000),
      }),
    }));
    expect(tooLong.status).toBe(400);
  });
});

describe("opaque mailbox delivery", () => {
  test("delivers offline pages, rejects replay, acknowledges, and recovers after restart", async () => {
    const fx = await fixture();
    const mailbox = await createMailbox(fx);
    const firstEnvelope = envelope(mailbox.id, fx.clock.now());
    const first = await upload(fx, mailbox, firstEnvelope);
    expect(first.status).toBe(201);
    expect(await first.json()).toMatchObject({ accepted: true, duplicate: false, cursor: 1 });
    const duplicate = await upload(fx, mailbox, firstEnvelope);
    expect(duplicate.status).toBe(200);
    expect(await duplicate.json()).toMatchObject({ duplicate: true, cursor: 1 });
    const replay = await upload(fx, mailbox, envelope(mailbox.id, fx.clock.now(), {
      sender_sequence: 1,
    }));
    expect(replay.status).toBe(409);
    const secondEnvelope = envelope(mailbox.id, fx.clock.now(), {
      sender_sequence: 2,
    });
    expect((await upload(fx, mailbox, secondEnvelope)).status).toBe(201);

    const page = await fx.application.fetch(request(`/v1/companion/mailboxes/${mailbox.id}/envelopes?cursor=0`, {
      headers: authorization(mailbox.secrets.read.capability),
    }));
    expect(page.status).toBe(200);
    const pageBody = await page.json();
    expect(pageBody.envelopes.map((item: { cursor: number }) => item.cursor)).toEqual([1, 2]);
    expect(pageBody.envelopes[0].envelope.ciphertext).toBe(firstEnvelope.ciphertext);
    expect(pageBody.envelopes[0].envelope.created_at).toBe(firstEnvelope.created_at);
    expect(pageBody.next_cursor).toBe(2);

    const ack = await fx.application.fetch(request(`/v1/companion/mailboxes/${mailbox.id}/ack`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        ...authorization(mailbox.secrets.ack.capability),
      },
      body: JSON.stringify({ schema: "tohseno.companion-mailbox-ack/1", cursor: 2 }),
    }));
    expect(await ack.json()).toMatchObject({ acknowledged_cursor: 2 });
    expect(readdirSync(join(fx.root, "mailboxes", mailbox.id, "envelopes"))).toEqual([]);
    const retryAck = await fx.application.fetch(request(`/v1/companion/mailboxes/${mailbox.id}/ack`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        ...authorization(mailbox.secrets.ack.capability),
      },
      body: JSON.stringify({ schema: "tohseno.companion-mailbox-ack/1", cursor: 2 }),
    }));
    expect(retryAck.status).toBe(200);
    const staleCursor = await fx.application.fetch(request(`/v1/companion/mailboxes/${mailbox.id}/envelopes?cursor=0`, {
      headers: authorization(mailbox.secrets.read.capability),
    }));
    expect(staleCursor.status).toBe(409);
    expect(await staleCursor.json()).toMatchObject({ reset_required: true, reset_before_cursor: 2 });

    const restarted = await createCompanionRelayApplication({
      config: fx.config,
      push: fx.push,
      clock: fx.clock,
      log: (record) => fx.logs.push(record),
      logError: (record) => fx.logs.push(record),
    });
    const retryAfterAck = await restarted.fetch(request(`/v1/companion/mailboxes/${mailbox.id}/envelopes`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        ...authorization(mailbox.secrets.write.capability),
      },
      body: JSON.stringify(firstEnvelope),
    }));
    expect(retryAfterAck.status).toBe(200);
    expect(await retryAfterAck.json()).toMatchObject({ duplicate: true, cursor: 1 });
  });

  test("finishes envelope cleanup after a committed acknowledgement survives a crash", async () => {
    const fx = await fixture();
    const mailbox = await createMailbox(fx);
    expect((await upload(fx, mailbox, envelope(mailbox.id, fx.clock.now()))).status).toBe(201);

    const mailboxRoot = join(fx.root, "mailboxes", mailbox.id);
    const metadataPath = join(mailboxRoot, "metadata.json");
    const metadata = JSON.parse(readFileSync(metadataPath, "utf8"));
    metadata.envelopes[0].discarded = true;
    metadata.acknowledgedCursor = 1;
    metadata.resetBeforeCursor = 1;
    writeFileSync(metadataPath, JSON.stringify(metadata), { mode: 0o600 });
    expect(readdirSync(join(mailboxRoot, "envelopes"))).toHaveLength(1);

    const restarted = await createCompanionRelayApplication({
      config: fx.config,
      push: fx.push,
      clock: fx.clock,
      log: (record) => fx.logs.push(record),
      logError: (record) => fx.logs.push(record),
    });
    const retry = await restarted.fetch(request(`/v1/companion/mailboxes/${mailbox.id}/ack`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        ...authorization(mailbox.secrets.ack.capability),
      },
      body: JSON.stringify({ schema: "tohseno.companion-mailbox-ack/1", cursor: 1 }),
    }));
    expect(retry.status).toBe(200);
    expect(await retry.json()).toMatchObject({ acknowledged_cursor: 1 });
    expect(readdirSync(join(mailboxRoot, "envelopes"))).toEqual([]);
  });

  test("streams live envelopes over SSE without polling", async () => {
    const fx = await fixture();
    const mailbox = await createMailbox(fx);
    const response = await fx.application.fetch(request(`/v1/companion/mailboxes/${mailbox.id}/live?cursor=0`, {
      headers: authorization(mailbox.secrets.read.capability),
    }));
    expect(response.status).toBe(200);
    expect(response.headers.get("Content-Type")).toContain("text/event-stream");
    const reader = response.body!.getReader();
    const initial = await reader.read();
    expect(new TextDecoder().decode(initial.value)).toContain("tohseno companion relay");
    const value = envelope(mailbox.id, fx.clock.now());
    expect((await upload(fx, mailbox, value)).status).toBe(201);
    const delivered = await reader.read();
    const text = new TextDecoder().decode(delivered.value);
    expect(text).toContain("event: envelope");
    expect(text).toContain("id: 1");
    expect(text).toContain(value.envelope_id);
    await reader.cancel();
    expect((await fx.application.storage!.metrics()).liveSubscribers).toBe(0);
  });

  test("registers content-free push, revokes immediately, and closes live delivery", async () => {
    const fx = await fixture();
    const mailbox = await createMailbox(fx);
    const deviceId = "phone_device_0001";
    const token = "ab".repeat(32);
    const registered = await fx.application.fetch(request("/v1/companion/push/register", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        ...authorization(mailbox.secrets.push.capability),
      },
      body: JSON.stringify({
        schema: "tohseno.companion-push-register/1",
        mailbox_id: mailbox.id,
        device_id: deviceId,
        apns_token: token,
      }),
    }));
    expect(registered.status).toBe(201);

    const live = await fx.application.fetch(request(`/v1/companion/mailboxes/${mailbox.id}/live`, {
      headers: authorization(mailbox.secrets.read.capability),
    }));
    const reader = live.body!.getReader();
    await reader.read();
    const value = envelope(mailbox.id, fx.clock.now());
    expect((await upload(fx, mailbox, value)).status).toBe(201);
    expect(fx.push.deliveryCount()).toBe(1);
    const delivered = await reader.read();
    expect(new TextDecoder().decode(delivered.value)).toContain("event: envelope");

    const unregistered = await fx.application.fetch(request(`/v1/companion/push/register/${deviceId}`, {
      method: "DELETE",
      headers: {
        "Content-Type": "application/json",
        ...authorization(mailbox.secrets.push.capability),
      },
      body: JSON.stringify({
        schema: "tohseno.companion-push-unregister/1",
        mailbox_id: mailbox.id,
      }),
    }));
    expect(unregistered.status).toBe(204);
    expect((await upload(fx, mailbox, envelope(mailbox.id, fx.clock.now(), {
      sender_sequence: 2,
    }))).status).toBe(201);
    expect(fx.push.deliveryCount()).toBe(1);
    expect(new TextDecoder().decode((await reader.read()).value)).toContain("event: envelope");

    const revoked = await fx.application.fetch(request(`/v1/companion/mailboxes/${mailbox.id}`, {
      method: "DELETE",
      headers: authorization(mailbox.secrets.revoke.capability),
    }));
    expect(revoked.status).toBe(200);
    expect(await revoked.json()).toMatchObject({ revoked: true, revocation_epoch: 1 });
    const revokedEvent = await reader.read();
    expect(new TextDecoder().decode(revokedEvent.value)).toContain("event: revoked");
    expect((await upload(fx, mailbox, envelope(mailbox.id, fx.clock.now(), {
      sender_sequence: 3,
    }))).status).toBe(410);
    expect((await fx.application.fetch(request(`/v1/companion/mailboxes/${mailbox.id}/envelopes`, {
      headers: authorization(mailbox.secrets.read.capability),
    }))).status).toBe(410);

    const serializedLogs = JSON.stringify(fx.logs);
    for (const prohibited of [
      mailbox.id,
      deviceId,
      token,
      mailbox.secrets.write.capability,
      value.ephemeral_public_key,
      value.nonce,
      value.ciphertext,
      "private companion content",
    ]) {
      expect(serializedLogs).not.toContain(prohibited);
    }
    expect(serializedLogs).toContain("push_wake_batch");
  });

  test("stores opaque ciphertext without interpreting private payload fields", async () => {
    const fx = await fixture();
    const mailbox = await createMailbox(fx);
    const privateValue = "marketing-note-that-the-relay-must-not-read";
    const value = envelope(mailbox.id, fx.clock.now(), {
      ciphertext: Buffer.from(JSON.stringify({
        schema: "tohseno.marketing-note/1",
        body: privateValue,
      })).toString("base64url"),
    });
    expect((await upload(fx, mailbox, value)).status).toBe(201);
    const metadata = readFileSync(join(fx.root, "mailboxes", mailbox.id, "metadata.json"), "utf8");
    expect(metadata).not.toContain(privateValue);
    expect(metadata).not.toContain("tohseno.marketing-note/1");
    const metrics = await (await fx.application.fetch(request("/metrics"))).json();
    expect(JSON.stringify(metrics)).not.toContain(privateValue);
    expect(JSON.stringify(fx.logs)).not.toContain(privateValue);
  });

  test("cleanup is bounded while sender replay watermarks survive retention", async () => {
    const fx = await fixture();
    const mailbox = await createMailbox(fx);
    for (let sequence = 1; sequence <= 3; sequence += 1) {
      expect((await upload(fx, mailbox, envelope(mailbox.id, fx.clock.now(), {
        sender_sequence: sequence,
        expires_at: canonicalTimestamp(fx.clock.now() + 1_000),
      }))).status).toBe(201);
    }
    fx.clock.advance(fx.config.limits.clockSkewMs + 2_000);
    expect(await fx.application.storage!.cleanup(1, fx.clock.now())).toBe(1);
    let metadata = JSON.parse(readFileSync(
      join(fx.root, "mailboxes", mailbox.id, "metadata.json"),
      "utf8",
    ));
    expect(metadata.envelopes).toHaveLength(2);
    expect(await fx.application.storage!.cleanup(10, fx.clock.now())).toBe(2);
    metadata = JSON.parse(readFileSync(
      join(fx.root, "mailboxes", mailbox.id, "metadata.json"),
      "utf8",
    ));
    expect(metadata.envelopes).toEqual([]);
    expect(metadata.senderHighWater.phone_device_0001).toBe(3);
    expect((await upload(fx, mailbox, envelope(mailbox.id, fx.clock.now(), {
      sender_sequence: 3,
    }))).status).toBe(409);
  });

  test("an envelope expiring between reads requires cursor reconciliation", async () => {
    const fx = await fixture();
    const mailbox = await createMailbox(fx);
    expect((await upload(fx, mailbox, envelope(mailbox.id, fx.clock.now(), {
      expires_at: canonicalTimestamp(fx.clock.now() + 1_000),
    }))).status).toBe(201);
    expect((await upload(fx, mailbox, envelope(mailbox.id, fx.clock.now(), {
      sender_sequence: 2,
    }))).status).toBe(201);

    fx.clock.advance(fx.config.limits.clockSkewMs + 1_001);
    const stale = await fx.application.fetch(request(
      `/v1/companion/mailboxes/${mailbox.id}/envelopes?cursor=0`,
      { headers: authorization(mailbox.secrets.read.capability) },
    ));
    expect(stale.status).toBe(409);
    expect(await stale.json()).toMatchObject({
      reset_required: true,
      reset_before_cursor: 1,
      head_cursor: 2,
    });

    const reconciled = await fx.application.fetch(request(
      `/v1/companion/mailboxes/${mailbox.id}/envelopes?cursor=1`,
      { headers: authorization(mailbox.secrets.read.capability) },
    ));
    expect(reconciled.status).toBe(200);
    expect(await reconciled.json()).toMatchObject({
      next_cursor: 2,
      head_cursor: 2,
      has_more: false,
    });
  });

  test("enforces expiry, capacity, content type, authority, and safe storage paths", async () => {
    const fx = await fixture({
      COMPANION_RELAY_MAX_ENVELOPES: "1",
      COMPANION_RELAY_MAX_ENVELOPE_BYTES: "2048",
    });
    const mailbox = await createMailbox(fx);
    const first = envelope(mailbox.id, fx.clock.now(), {
      expires_at: canonicalTimestamp(fx.clock.now() + 1_000),
    });
    expect((await upload(fx, mailbox, first)).status).toBe(201);
    const overCapacity = await upload(fx, mailbox, envelope(mailbox.id, fx.clock.now(), {
      sender_sequence: 2,
    }));
    expect(overCapacity.status).toBe(503);
    const wrongType = await fx.application.fetch(request(`/v1/companion/mailboxes/${mailbox.id}/envelopes`, {
      method: "POST",
      headers: {
        "Content-Type": "text/plain",
        ...authorization(mailbox.secrets.write.capability),
      },
      body: JSON.stringify(first),
    }));
    expect(wrongType.status).toBe(415);
    const extraField = await upload(fx, mailbox, {
      ...envelope(mailbox.id, fx.clock.now(), { sender_sequence: 2 }),
      plaintext_command: "must never be accepted outside ciphertext",
    });
    expect(extraField.status).toBe(400);
    const nonCanonicalTime = await upload(fx, mailbox, envelope(mailbox.id, fx.clock.now(), {
      sender_sequence: 2,
      created_at: canonicalTimestamp(fx.clock.now()).replace("Z", ".123Z"),
    }));
    expect(nonCanonicalTime.status).toBe(400);
    const malformedKey = await upload(fx, mailbox, envelope(mailbox.id, fx.clock.now(), {
      sender_sequence: 2,
      ephemeral_public_key: `${"A".repeat(42)}B`,
    }));
    expect(malformedKey.status).toBe(400);
    const wrongAuthority = await fx.application.fetch(new Request("http://evil.example/v1/companion", {
      method: "GET",
    }));
    expect(wrongAuthority.status).toBe(421);

    fx.clock.advance(fx.config.limits.clockSkewMs + 1_001);
    expect(await fx.application.storage!.cleanup(100, fx.clock.now())).toBeGreaterThan(0);
    const cleanedMetadata = readFileSync(
      join(fx.root, "mailboxes", mailbox.id, "metadata.json"),
      "utf8",
    );
    expect(cleanedMetadata).not.toContain(first.envelope_id);
    const retainedReplayWatermark = await upload(fx, mailbox, envelope(mailbox.id, fx.clock.now(), {
      sender_sequence: 1,
    }));
    expect(retainedReplayWatermark.status).toBe(409);
    expect((await upload(fx, mailbox, envelope(mailbox.id, fx.clock.now(), {
      sender_sequence: 2,
    }))).status).toBe(201);
    const reset = await fx.application.fetch(request(`/v1/companion/mailboxes/${mailbox.id}/envelopes?cursor=0`, {
      headers: authorization(mailbox.secrets.read.capability),
    }));
    expect(reset.status).toBe(409);

    const metadataPath = join(fx.root, "mailboxes", mailbox.id, "metadata.json");
    const saved = `${metadataPath}.saved`;
    renameSync(metadataPath, saved);
    symlinkSync(saved, metadataPath);
    const unsafe = await fx.application.fetch(request(`/v1/companion/mailboxes/${mailbox.id}/envelopes`, {
      headers: authorization(mailbox.secrets.read.capability),
    }));
    expect(unsafe.status).toBe(500);
    expect(await unsafe.json()).toMatchObject({ error_class: "unsafe_path" });
  });

  test("accepts one local-network hostname alias for a loopback development relay", async () => {
    const fx = await fixture({ NODE_ENV: "development" });

    const localNetwork = await fx.application.fetch(
      new Request("http://tohseno-mac.local:3100/healthz"),
    );
    expect(localNetwork.status).toBe(200);

    const privateAddress = await fx.application.fetch(
      new Request("http://172.20.10.3:3100/healthz"),
    );
    expect(privateAddress.status).toBe(200);

    const wrongPort = await fx.application.fetch(
      new Request("http://tohseno-mac.local:3101/healthz"),
    );
    expect(wrongPort.status).toBe(421);

    const externalHost = await fx.application.fetch(
      new Request("http://relay.example:3100/healthz"),
    );
    expect(externalHost.status).toBe(421);

    const publicAddress = await fx.application.fetch(
      new Request("http://8.8.8.8:3100/healthz"),
    );
    expect(publicAddress.status).toBe(421);
  });

  test("does not accept local-network authority aliases outside development", async () => {
    const fx = await fixture();
    const response = await fx.application.fetch(
      new Request("http://tohseno-mac.local:3100/healthz"),
    );
    expect(response.status).toBe(421);
  });

  test("allows only the configured hosting health probe on its alternate authority", async () => {
    const fx = await fixture({
      NODE_ENV: "production",
      BASE_URL: "https://companion.tohseno.com",
      TRUST_PROXY: "true",
      COMPANION_RELAY_ACTIVATION_READY: "true",
      COMPANION_RELAY_PUSH_MODE: "noop",
      COMPANION_RELAY_HEALTHCHECK_HOST: "healthcheck.railway.app",
    });

    const health = await fx.application.fetch(
      new Request("http://healthcheck.railway.app/healthz"),
    );
    expect(health.status).toBe(200);
    expect(await health.json()).toMatchObject({ ready: true });

    expect((await fx.application.fetch(
      new Request("http://healthcheck.railway.app/metrics"),
    )).status).toBe(421);
    expect((await fx.application.fetch(
      new Request("http://healthcheck.railway.app/healthz", { method: "POST" }),
    )).status).toBe(421);
    expect((await fx.application.fetch(
      new Request("http://other.railway.app/healthz"),
    )).status).toBe(421);
  });
});

describe("rate limits and operational surface", () => {
  test("applies independent source and global windows", async () => {
    const sourceLimited = await fixture({
      TRUST_PROXY: "true",
      BASE_URL: "http://relay.test",
      COMPANION_RELAY_SOURCE_RATE: "2",
      COMPANION_RELAY_GLOBAL_RATE: "20",
    });
    const proxied = (source: string) => new Request("http://127.0.0.1:3100/v1/companion", {
      headers: {
        "X-Forwarded-Host": "relay.test",
        "X-Forwarded-Proto": "http",
        "X-Forwarded-For": source,
      },
    });
    expect((await sourceLimited.application.fetch(proxied("192.0.2.1"))).status).toBe(200);
    expect((await sourceLimited.application.fetch(proxied("192.0.2.1"))).status).toBe(200);
    expect((await sourceLimited.application.fetch(proxied("192.0.2.1"))).status).toBe(429);
    expect((await sourceLimited.application.fetch(proxied("192.0.2.2"))).status).toBe(200);

    const globalLimited = await fixture({
      TRUST_PROXY: "true",
      BASE_URL: "http://relay.test",
      COMPANION_RELAY_SOURCE_RATE: "20",
      COMPANION_RELAY_GLOBAL_RATE: "2",
    });
    expect((await globalLimited.application.fetch(proxied("192.0.2.1"))).status).toBe(200);
    expect((await globalLimited.application.fetch(proxied("192.0.2.2"))).status).toBe(200);
    expect((await globalLimited.application.fetch(proxied("192.0.2.3"))).status).toBe(429);

    const direct = await fixture({
      COMPANION_RELAY_SOURCE_RATE: "2",
      COMPANION_RELAY_GLOBAL_RATE: "20",
    });
    expect((await direct.application.fetch(request("/v1/companion"), "192.0.2.10")).status).toBe(200);
    expect((await direct.application.fetch(request("/v1/companion"), "192.0.2.10")).status).toBe(200);
    expect((await direct.application.fetch(request("/v1/companion"), "192.0.2.10")).status).toBe(429);
    expect((await direct.application.fetch(request("/v1/companion"), "192.0.2.11")).status).toBe(200);
  });

  test("health, capacity metrics, and security headers expose no identifiers", async () => {
    const fx = await fixture();
    const mailbox = await createMailbox(fx);
    const health = await fx.application.fetch(request("/healthz"));
    expect(await health.json()).toEqual({
      schema: "tohseno.companion-relay-health/1",
      service_version: "0.9.9",
      ready: true,
      push_enabled: true,
      maximum_envelope_bytes: fx.config.limits.envelopeBytes,
      retention_seconds: fx.config.limits.retentionMs / 1000,
    });
    expect(health.headers.get("Cache-Control")).toBe("no-store");
    expect(health.headers.get("Access-Control-Allow-Origin")).toBeNull();
    expect(health.headers.get("Content-Security-Policy")).toContain("default-src 'none'");
    const metrics = await fx.application.fetch(request("/metrics"));
    const metricsText = await metrics.text();
    expect(metricsText).toContain('"mailboxes":1');
    expect(metricsText).not.toContain(mailbox.id);
    expect(metricsText).not.toContain(mailbox.secrets.read.capability);
    const notFound = await fx.application.fetch(request(`/v1/companion/private-looking-${mailbox.id}`));
    expect(notFound.status).toBe(404);
    expect(JSON.stringify(fx.logs)).not.toContain(mailbox.id);
  });

  test("disabled configuration fails closed while health remains honest", async () => {
    const config = loadCompanionRelayConfig({
      NODE_ENV: "test",
      BASE_URL: "http://127.0.0.1:3100",
      PORT: "3100",
    });
    const application = await createCompanionRelayApplication({
      config,
      log: () => {},
      logError: () => {},
    });
    expect((await application.fetch(request("/v1/companion"))).status).toBe(503);
    expect(await (await application.fetch(request("/healthz"))).json()).toMatchObject({
      ready: false,
      push_enabled: false,
    });
  });

  test("rejects a symlinked relay root", async () => {
    const parent = mkdtempSync(join(tmpdir(), "tohseno-companion-root-test-"));
    roots.push(parent);
    const real = join(parent, "real");
    const link = join(parent, "link");
    mkdirSync(real);
    symlinkSync(real, link);
    const config = loadCompanionRelayConfig({
      NODE_ENV: "test",
      PORT: "3100",
      BASE_URL: "http://127.0.0.1:3100",
      COMPANION_RELAY_ENABLED: "true",
      COMPANION_RELAY_ROOT: link,
    });
    await expect(createCompanionRelayApplication({ config })).rejects.toThrow("unsafe");
  });
});
