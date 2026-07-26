# Replaceable node client

`@tohseno/node-client` is the transport boundary for an optional public Shot
record index. It contains no TOHSENO hostname or default node. Every
`HttpNodeClient` must receive a caller-selected bare HTTP(S) origin:

```ts
import { HttpNodeClient } from "@tohseno/node-client";

const node = new HttpNodeClient("http://127.0.0.1:8787");
const result = await node.submit(signedPublicShotRecord);
const projection = await node.getProjection(result.projection.shotId);
const records = await node.getRecords(result.projection.shotId);
```

The client sends canonical signed-record JSON without a request envelope. It
omits browser credentials, refuses redirects, bounds response bytes, validates
closed response shapes, applies a bounded request timeout, and reports
content-free errors. A custom `fetch` implementation, smaller response limit,
and timeout can be injected for another runtime or test:

```ts
const node = new HttpNodeClient(origin, {
  fetch: isolatedFetch,
  maxResponseBytes: 512 * 1024,
  timeoutMs: 5_000,
});
```

The node is replaceable and non-authoritative. `getRecords` exports the signed
records in hash-chain order, but transport validation is not identity
verification. Consumers that rely on authenticity should independently run
the protocol verifier over every exported record and derive the projection with
the registry projector. No local Shot action or generated app runtime depends
on this client.
