import { createPrivateKey, sign } from "node:crypto";
import type { KeyObject } from "node:crypto";
import { constants } from "node:fs";
import { open } from "node:fs/promises";
import type { CompanionRelayConfig } from "../config.ts";
import type { PushRegistration } from "./types.ts";

export interface PushProvider {
  readonly mode: "noop" | "fake" | "apns";
  sendWake(registration: PushRegistration): Promise<void>;
}

export class NoopPushProvider implements PushProvider {
  readonly mode = "noop" as const;
  async sendWake(_registration: PushRegistration): Promise<void> {}
}

export class FakePushProvider implements PushProvider {
  readonly mode = "fake" as const;
  private deliveries = 0;

  async sendWake(_registration: PushRegistration): Promise<void> {
    this.deliveries += 1;
  }

  deliveryCount(): number {
    return this.deliveries;
  }
}

interface ApnsProviderOptions {
  environment: "sandbox" | "production";
  teamId: string;
  keyId: string;
  topic: string;
  privateKey: string;
  fetch: typeof globalThis.fetch;
  now: () => number;
}

export class ApnsPushProvider implements PushProvider {
  readonly mode = "apns" as const;
  private cachedToken?: { value: string; issuedAt: number };
  private readonly privateKey: KeyObject;

  constructor(private readonly options: ApnsProviderOptions) {
    this.privateKey = createPrivateKey(options.privateKey);
    if (
      this.privateKey.asymmetricKeyType !== "ec" ||
      this.privateKey.asymmetricKeyDetails?.namedCurve !== "prime256v1"
    ) {
      throw new Error("APNs provider key must be a P-256 private key");
    }
  }

  async sendWake(registration: PushRegistration): Promise<void> {
    if (!/^[a-f0-9]{64,200}$/.test(registration.token) || registration.token.length % 2 !== 0) {
      throw new Error("APNs device token is not valid for provider delivery");
    }
    const origin = this.options.environment === "production"
      ? "https://api.push.apple.com"
      : "https://api.sandbox.push.apple.com";
    const response = await this.options.fetch(
      `${origin}/3/device/${registration.token}`,
      {
        method: "POST",
        redirect: "error",
        headers: {
          authorization: `bearer ${this.providerToken()}`,
          "apns-collapse-id": "tohseno-mailbox-reconcile",
          "apns-expiration": "0",
          "apns-push-type": "background",
          "apns-priority": "5",
          "apns-topic": this.options.topic,
          "content-type": "application/json",
        },
        body: JSON.stringify({
          aps: { "content-available": 1 },
          reason: "mailbox_changed",
        }),
        signal: AbortSignal.timeout(10_000),
      },
    );
    if (!response.ok) throw new Error(`APNs wake request failed with status ${response.status}`);
  }

  private providerToken(): string {
    const now = Math.floor(this.options.now() / 1000);
    if (
      this.cachedToken &&
      now >= this.cachedToken.issuedAt &&
      now - this.cachedToken.issuedAt < 50 * 60
    ) {
      return this.cachedToken.value;
    }
    const header = base64UrlJson({ alg: "ES256", kid: this.options.keyId });
    const claims = base64UrlJson({ iss: this.options.teamId, iat: now });
    const signingInput = `${header}.${claims}`;
    const signature = sign("sha256", Buffer.from(signingInput), {
      key: this.privateKey,
      dsaEncoding: "ieee-p1363",
    }).toString("base64url");
    const value = `${signingInput}.${signature}`;
    this.cachedToken = { value, issuedAt: now };
    return value;
  }
}

export async function createPushProvider(
  config: CompanionRelayConfig,
  options: { fetch?: typeof globalThis.fetch; now?: () => number } = {},
): Promise<PushProvider> {
  if (config.push.mode === "noop") return new NoopPushProvider();
  if (config.push.mode === "fake") return new FakePushProvider();

  const path = config.push.privateKeyPath!;
  let handle;
  try {
    handle = await open(path, constants.O_RDONLY | constants.O_NOFOLLOW);
  } catch {
    throw new Error("APNs private key must be a bounded regular file, not a symbolic link");
  }
  let privateKey: string;
  try {
    const details = await handle.stat();
    if (
      !details.isFile() ||
      details.size < 64 ||
      details.size > 16 * 1024 ||
      (details.mode & 0o077) !== 0 ||
      (typeof process.getuid === "function" && details.uid !== process.getuid())
    ) {
      throw new Error("APNs private key must be a private bounded regular file");
    }
    privateKey = await handle.readFile("utf8");
  } finally {
    await handle.close();
  }
  try {
    return new ApnsPushProvider({
      environment: config.push.environment,
      teamId: config.push.teamId!,
      keyId: config.push.keyId!,
      topic: config.push.topic!,
      privateKey,
      fetch: options.fetch ?? globalThis.fetch,
      now: options.now ?? (() => Date.now()),
    });
  } catch {
    throw new Error("APNs private key is malformed");
  }
}

function base64UrlJson(value: unknown): string {
  return Buffer.from(JSON.stringify(value)).toString("base64url");
}
