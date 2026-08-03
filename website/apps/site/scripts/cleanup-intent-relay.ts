import { loadConfig } from "../config.ts";
import { FilesystemRelayStorage } from "../src/relay-storage.ts";

const config = loadConfig();
if (!config.relay.enabled || !config.relay.claimInstallerReady || !config.relay.root) {
  throw new Error("intent relay cleanup requires the explicitly enabled, claim-ready relay configuration");
}

const storage = new FilesystemRelayStorage(config.relay.root, {
  maxRecords: config.relay.maxRecords,
  maxBytes: config.relay.maxBytes,
});
await storage.initialize();
const cleaned = await storage.cleanup(config.relay.maxRecords);
console.log(JSON.stringify({ event: "intent_relay_cleanup", cleaned }));
