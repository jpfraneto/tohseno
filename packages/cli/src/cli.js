import { NPM_CLI_VERSION } from "./constants.js";

export const HELP = `TOHSENO ${NPM_CLI_VERSION}

Usage:
  tohseno                 Show where to start
  tohseno init [path]     Connect an existing Xcode app, one step at a time
  tohseno deploy          Ship the connected app after Companion approval
  tohseno open            Open the installed TOHSENO
  tohseno doctor          Check this Mac without changing it
  tohseno --version       Print the npm CLI version
  tohseno --help          Show this help

The command runtime is downloaded only when a real command needs it, then its
exact bytes and Apple Developer ID signature are verified before execution.`;

export const GUIDE = `TOHSENO CLI ${NPM_CLI_VERSION} is installed.

Start with an existing Xcode project:
  cd /path/to/YourApp
  tohseno init
  tohseno deploy

\`tohseno init\` walks through the real setup one line at a time.`;

export function parseCommand(args) {
  if (!args.length) return { kind: "guide", args: [] };
  if (args.length === 1 && ["--help", "-h", "help"].includes(args[0])) return { kind: "help", args: [] };
  if (args.length === 1 && ["--version", "-V"].includes(args[0])) return { kind: "version", args: [] };
  if (args.length === 1 && ["open", "doctor"].includes(args[0])) return { kind: args[0], args: [] };
  return { kind: "delegate", args };
}

export function redact(message) {
  return String(message)
    .replace(/([?&](?:token|claim|nonce|secret|key)=)[^&\s]+/gi, "$1[redacted]")
    .replace(/\b(?:sk|pk)_(?:live|test)_[A-Za-z0-9_-]+\b/g, "[redacted]");
}
