import { loadCompanionRelayConfig } from "../config.ts";
import { CompanionRelayStorage } from "../src/storage.ts";

const config = loadCompanionRelayConfig();
if (!config.enabled || !config.root) {
  throw new Error("companion relay cleanup requires explicitly enabled configuration");
}

const storage = new CompanionRelayStorage(config.root, config.limits);
await storage.initialize();
const cleaned = await storage.cleanup(
  config.limits.pairingSessions + config.limits.mailboxes + config.limits.envelopes,
);
console.log(JSON.stringify({ event: "companion_relay_cleanup", cleaned }));
