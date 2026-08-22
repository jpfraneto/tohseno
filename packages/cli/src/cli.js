import { NPM_CLI_VERSION } from "./constants.js";

export const HELP = `TOHSENO ${NPM_CLI_VERSION}

Usage:
  tohseno                 Install if needed, then open TOHSENO
  tohseno install         Install the authorized native release
  tohseno open            Open the installed TOHSENO
  tohseno doctor          Check this Mac without changing it
  tohseno --version       Print the npm bootstrap version
  tohseno --help          Show this help

Native commands such as create, evolve, studio, service, and companion are
delegated to the verified installation.`;

export function parseCommand(args) {
  if (!args.length) return { kind: "start", args: [] };
  if (args.length === 1 && ["--help", "-h", "help"].includes(args[0])) return { kind: "help", args: [] };
  if (args.length === 1 && ["--version", "-V"].includes(args[0])) return { kind: "version", args: [] };
  if (args.length === 1 && ["install", "open", "doctor"].includes(args[0])) return { kind: args[0], args: [] };
  return { kind: "delegate", args };
}

export function redact(message) {
  return String(message)
    .replace(/([?&](?:token|claim|nonce|secret|key)=)[^&\s]+/gi, "$1[redacted]")
    .replace(/\b(?:sk|pk)_(?:live|test)_[A-Za-z0-9_-]+\b/g, "[redacted]");
}
