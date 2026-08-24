import { afterEach, describe, expect, test } from "bun:test";
import { generateKeyPairSync } from "node:crypto";
import { mkdtempSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  loadCompanionRelayConfig,
  safeCompanionStartupSummary,
} from "../config.ts";
import {
  ApnsPushProvider,
  FakePushProvider,
  NoopPushProvider,
  createPushProvider,
} from "../src/push-provider.ts";

const roots: string[] = [];
afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

describe("companion relay configuration", () => {
  test("is disabled and content-free by default", () => {
    const config = loadCompanionRelayConfig({
      NODE_ENV: "test",
      BASE_URL: "http://127.0.0.1:3100",
      PORT: "3100",
    });
    expect(config.enabled).toBe(false);
    expect(config.push.mode).toBe("noop");
    expect(config.limits.pairingLifetimeMs).toBe(120_000);
    expect(config.limits.clockSkewMs).toBe(300_000);
    expect(safeCompanionStartupSummary(config)).toEqual({
      service: "tohseno-companion-relay",
      version: "1.0.0",
      environment: "test",
      host: "127.0.0.1",
      port: 3100,
      baseUrl: "http://127.0.0.1:3100",
      trustProxy: false,
      enabled: false,
      activationReady: false,
      pushMode: "noop",
    });
  });

  test("fails closed for production activation and malformed limits", () => {
    const base = {
      NODE_ENV: "production",
      PORT: "3100",
      BASE_URL: "https://relay.tohseno.com",
      COMPANION_RELAY_ENABLED: "true",
      COMPANION_RELAY_ROOT: "/relay",
    };
    expect(() => loadCompanionRelayConfig(base)).toThrow("ACTIVATION_READY");
    expect(() => loadCompanionRelayConfig({
      ...base,
      COMPANION_RELAY_ACTIVATION_READY: "true",
      BASE_URL: "http://relay.tohseno.com",
    })).toThrow("https");
    expect(() => loadCompanionRelayConfig({
      ...base,
      COMPANION_RELAY_ACTIVATION_READY: "true",
      COMPANION_RELAY_ROOT: "relative",
    })).toThrow("absolute");
    expect(() => loadCompanionRelayConfig({
      NODE_ENV: "test",
      BASE_URL: "http://127.0.0.1:3100",
      COMPANION_RELAY_CLOCK_SKEW_SECONDS: "901",
    })).toThrow("between 1 and 900");
    expect(() => loadCompanionRelayConfig({
      NODE_ENV: "test",
      BASE_URL: "http://127.0.0.1:3100",
      COMPANION_RELAY_MAX_ENVELOPE_BYTES: String(16 * 1024 * 1024 + 17),
    })).toThrow(String(16 * 1024 * 1024 + 16));
    expect(() => loadCompanionRelayConfig({
      NODE_ENV: "test",
      BASE_URL: "http://127.0.0.1:3100",
      COMPANION_RELAY_HEALTHCHECK_HOST: "https://healthcheck.railway.app",
    })).toThrow("HEALTHCHECK_HOST");
  });

  test("forbids fake push in production and requires every APNs credential", () => {
    const production = {
      NODE_ENV: "production",
      BASE_URL: "https://relay.tohseno.com",
      COMPANION_RELAY_ENABLED: "true",
      COMPANION_RELAY_ACTIVATION_READY: "true",
      COMPANION_RELAY_ROOT: "/relay",
    };
    expect(() => loadCompanionRelayConfig({
      ...production,
      COMPANION_RELAY_PUSH_MODE: "fake",
    })).toThrow("forbidden");
    expect(() => loadCompanionRelayConfig({
      ...production,
      COMPANION_RELAY_PUSH_MODE: "apns",
    })).toThrow("TEAM_ID");
    expect(() => loadCompanionRelayConfig({
      ...production,
      COMPANION_RELAY_PUSH_MODE: "apns",
      COMPANION_RELAY_APNS_TEAM_ID: "ABCDEFGHIJ",
      COMPANION_RELAY_APNS_KEY_ID: "K123456789",
      COMPANION_RELAY_APNS_TOPIC: "com.tohseno.companion",
      COMPANION_RELAY_APNS_PRIVATE_KEY_PATH: "relative.p8",
    })).toThrow("absolute");
  });
});

describe("push providers", () => {
  const registration = {
    schema: "tohseno.companion-push-registration/1" as const,
    mailboxId: "A".repeat(32),
    deviceId: "phone_device_0001",
    token: "ab".repeat(32),
    registeredAt: Date.now(),
  };

  test("noop and fake providers expose deterministic test behavior", async () => {
    const noop = new NoopPushProvider();
    await noop.sendWake(registration);
    expect(noop.mode).toBe("noop");

    const fake = new FakePushProvider();
    await fake.sendWake(registration);
    await fake.sendWake(registration);
    expect(fake.deliveryCount()).toBe(2);
  });

  test("APNs sends a content-free background reconciliation wake", async () => {
    const { privateKey } = generateKeyPairSync("ec", { namedCurve: "prime256v1" });
    const pem = privateKey.export({ format: "pem", type: "pkcs8" }).toString();
    const requests: Array<{ url: string; init?: RequestInit }> = [];
    const provider = new ApnsPushProvider({
      environment: "sandbox",
      teamId: "ABCDEFGHIJ",
      keyId: "K123456789",
      topic: "com.tohseno.companion",
      privateKey: pem,
      fetch: (async (url: string | URL | Request, init?: RequestInit) => {
        requests.push({ url: String(url), init });
        return new Response(null, { status: 200 });
      }) as typeof fetch,
      now: () => 1_786_742_400_000,
    });
    await provider.sendWake(registration);
    expect(requests).toHaveLength(1);
    expect(requests[0].url).toBe(`https://api.sandbox.push.apple.com/3/device/${registration.token}`);
    expect(requests[0].init?.headers).toMatchObject({
      "apns-collapse-id": "tohseno-mailbox-reconcile",
      "apns-expiration": "0",
      "apns-push-type": "background",
      "apns-priority": "5",
      "apns-topic": "com.tohseno.companion",
    });
    expect(JSON.parse(String(requests[0].init?.body))).toEqual({
      aps: { "content-available": 1 },
      reason: "mailbox_changed",
    });
  });

  test("APNs startup rejects symlinked and malformed private-key files", async () => {
    const root = mkdtempSync(join(tmpdir(), "tohseno-apns-test-"));
    roots.push(root);
    const real = join(root, "AuthKey.p8");
    const link = join(root, "AuthKey-link.p8");
    writeFileSync(real, "x".repeat(128), { mode: 0o600 });
    symlinkSync(real, link);
    const env = {
      NODE_ENV: "test",
      PORT: "3100",
      BASE_URL: "http://127.0.0.1:3100",
      COMPANION_RELAY_ENABLED: "true",
      COMPANION_RELAY_ROOT: join(root, "relay"),
      COMPANION_RELAY_PUSH_MODE: "apns",
      COMPANION_RELAY_APNS_TEAM_ID: "ABCDEFGHIJ",
      COMPANION_RELAY_APNS_KEY_ID: "K123456789",
      COMPANION_RELAY_APNS_TOPIC: "com.tohseno.companion",
      COMPANION_RELAY_APNS_PRIVATE_KEY_PATH: link,
    };
    await expect(createPushProvider(loadCompanionRelayConfig(env))).rejects.toThrow("regular file");
    await expect(createPushProvider(loadCompanionRelayConfig({
      ...env,
      COMPANION_RELAY_APNS_PRIVATE_KEY_PATH: real,
    }))).rejects.toThrow("malformed");
  });
});
