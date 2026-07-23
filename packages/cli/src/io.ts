import { createInterface } from "node:readline/promises";

export interface CliIo {
  interactive: boolean;
  out(line?: string): void;
  error(line?: string): void;
  prompt(question: string): Promise<string>;
}

export function defaultIo(): CliIo {
  return {
    interactive: Boolean(process.stdin.isTTY && process.stdout.isTTY),
    out(line = "") {
      process.stdout.write(`${line}\n`);
    },
    error(line = "") {
      process.stderr.write(`${line}\n`);
    },
    async prompt(question: string): Promise<string> {
      const readline = createInterface({ input: process.stdin, output: process.stdout });
      try {
        return await readline.question(question);
      } finally {
        readline.close();
      }
    },
  };
}
