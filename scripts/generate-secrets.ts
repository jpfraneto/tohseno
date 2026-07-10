const DATA_KEY_BYTES = 32;
const OPERATOR_TOKEN_BYTES = 32;

export {};

function secureRandomBytes(length: number): Uint8Array {
  const bytes = new Uint8Array(length);
  crypto.getRandomValues(bytes);
  return bytes;
}

function base64(bytes: Uint8Array): string {
  return Buffer.from(bytes).toString("base64");
}

function base64Url(bytes: Uint8Array): string {
  return Buffer.from(bytes).toString("base64url");
}

const dataKey = base64(secureRandomBytes(DATA_KEY_BYTES));
const operatorToken = base64Url(secureRandomBytes(OPERATOR_TOKEN_BYTES));

// Intentionally print only to stdout. This command never creates or modifies a
// file; the operator decides which secret store receives these values.
process.stdout.write(
  `TOHSENO_DATA_KEY=${dataKey}\nTOHSENO_OPERATOR_TOKEN=${operatorToken}\n`,
);
