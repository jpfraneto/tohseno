export type ReferenceNodeRoute =
  | "health"
  | "openapi"
  | "submit-record"
  | "shot-projection"
  | "shot-records"
  | "unmatched";

export type ReferenceNodeMethod = "GET" | "POST" | "OTHER";

export interface ReferenceNodeRequestLog {
  event: "request";
  method: ReferenceNodeMethod;
  route: ReferenceNodeRoute;
  status: number;
  durationMs: number;
  code?: string;
}

export function methodClass(method: string): ReferenceNodeMethod {
  if (method === "GET" || method === "POST") return method;
  return "OTHER";
}

export function semanticRoute(pathname: string): ReferenceNodeRoute {
  if (pathname === "/healthz") return "health";
  if (pathname === "/openapi.json") return "openapi";
  if (pathname === "/v1/records") return "submit-record";
  const segments = pathname.split("/");
  if (
    segments.length === 5 &&
    segments[0] === "" &&
    segments[1] === "v1" &&
    segments[2] === "shots" &&
    segments[3] !== "" &&
    segments[4] === ""
  ) {
    return "shot-projection";
  }
  if (
    segments.length === 4 &&
    segments[0] === "" &&
    segments[1] === "v1" &&
    segments[2] === "shots" &&
    segments[3] !== ""
  ) {
    return "shot-projection";
  }
  if (
    segments.length === 5 &&
    segments[0] === "" &&
    segments[1] === "v1" &&
    segments[2] === "shots" &&
    segments[3] !== "" &&
    segments[4] === "records"
  ) {
    return "shot-records";
  }
  return "unmatched";
}
