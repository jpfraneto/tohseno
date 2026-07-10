export {};

const args = process.argv.slice(2).filter((argument, index) => !(argument === "--" && index === 0));
const [command, submissionId, value] = args;
const operatorToken = process.env.TOHSENO_OPERATOR_TOKEN;

function operatorBaseUrl(raw: string): string {
  const parsed = new URL(raw);
  const loopback = new Set(["localhost", "127.0.0.1", "[::1]"]).has(parsed.hostname);
  if (parsed.protocol !== "https:" && !(parsed.protocol === "http:" && loopback)) {
    throw new Error("BASE_URL must use HTTPS except for a loopback development server");
  }
  if (parsed.username || parsed.password || parsed.pathname !== "/" || parsed.search || parsed.hash) {
    throw new Error("BASE_URL must be a bare origin without credentials, path, query, or fragment");
  }
  return parsed.origin;
}

let baseUrl: string;
try {
  baseUrl = operatorBaseUrl(process.env.BASE_URL ?? "http://localhost:3000");
} catch (error) {
  console.error(error instanceof Error ? error.message : "BASE_URL is invalid");
  process.exit(1);
}

if (!operatorToken) {
  console.error("TOHSENO_OPERATOR_TOKEN is required");
  process.exit(1);
}

function usage(): never {
  console.error(`Usage:
  bun run operator -- list
  bun run operator -- show <submission-id>
  bun run operator -- transition <submission-id> <next-state>
  bun run operator -- summary <submission-id> <json-file>
  bun run operator -- message <submission-id> <text-file>
  bun run operator -- retry-email <submission-id>
  bun run operator -- revoke-capability <submission-id>`);
  process.exit(1);
}

async function request(path: string, init: RequestInit = {}): Promise<unknown> {
  const headers = new Headers(init.headers);
  headers.set("Authorization", `Bearer ${operatorToken}`);
  if (init.body !== undefined) headers.set("Content-Type", "application/json");
  const response = await fetch(new URL(path, baseUrl), {
    ...init,
    headers,
    redirect: "error",
    signal: init.signal ?? AbortSignal.timeout(10_000),
  });
  const data: unknown = await response.json().catch(() => ({ error: `HTTP ${response.status}` }));
  if (!response.ok) {
    const message = typeof data === "object" && data !== null && "error" in data && typeof data.error === "string"
      ? data.error
      : `HTTP ${response.status}`;
    throw new Error(message);
  }
  return data;
}

async function readRequiredFile(path: string | undefined): Promise<string> {
  if (!path) usage();
  const file = Bun.file(path);
  if (!await file.exists()) throw new Error(`File does not exist: ${path}`);
  return file.text();
}

let result: unknown;
try {
  switch (command) {
    case "list":
      result = await request("/api/operator/submissions");
      break;
    case "show":
      if (!submissionId || value) usage();
      result = {
        ...await request(`/api/operator/submissions/${encodeURIComponent(submissionId)}`) as Record<string, unknown>,
        ...await request(`/api/operator/submissions/${encodeURIComponent(submissionId)}/inspect-source`, {
          method: "POST",
          body: "{}",
        }) as Record<string, unknown>,
      };
      break;
    case "transition": {
      const nextState = value;
      if (!submissionId || !nextState || args.length !== 3) usage();
      result = await request(`/api/operator/submissions/${encodeURIComponent(submissionId)}/transition`, {
        method: "POST",
        body: JSON.stringify({ nextStatus: nextState }),
      });
      break;
    }
    case "summary": {
      if (!submissionId || !value || args.length !== 3) usage();
      const summary: unknown = JSON.parse(await readRequiredFile(value));
      result = await request(`/api/operator/submissions/${encodeURIComponent(submissionId)}/summary`, {
        method: "POST",
        body: JSON.stringify({ summary }),
      });
      break;
    }
    case "message": {
      if (!submissionId || !value || args.length !== 3) usage();
      const message = await readRequiredFile(value);
      result = await request(`/api/operator/submissions/${encodeURIComponent(submissionId)}/message`, {
        method: "POST",
        body: JSON.stringify({ message }),
      });
      break;
    }
    case "revoke-capability":
      if (!submissionId || value) usage();
      result = await request(`/api/operator/submissions/${encodeURIComponent(submissionId)}/revoke-capability`, {
        method: "POST",
        body: "{}",
      });
      break;
    case "retry-email":
      if (!submissionId || value) usage();
      result = await request(`/api/operator/submissions/${encodeURIComponent(submissionId)}/retry-email`, {
        method: "POST",
        body: "{}",
      });
      break;
    default:
      usage();
  }
  console.log(JSON.stringify(result, null, 2));
} catch (error) {
  console.error(error instanceof Error ? error.message : "Operator command failed");
  process.exit(1);
}
