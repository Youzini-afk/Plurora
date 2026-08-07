# @plurora/contract-sdk

Generated TypeScript bindings for the Plurora kernel contract.

```ts
import { attach, fromHttpRpc } from "@plurora/contract-sdk";

const client = attach(fromHttpRpc("http://localhost:8080/rpc"));
const info = await client.hostInfo({});
```

`hostInfo()` uses the layered canonical route. Explicit legacy wire wrappers are generated when
an alias exists, for example `legacyKernelV1HostInfo({})`. To require an exact host contract before
subsequent calls:

```ts
await client.negotiateHost({
  profile: "plurora.contract.default/v1",
  versions: [{ layer: "host", version: "0.1.0" }],
});
```

Negotiation fails if the transport cannot carry the selection; it is never silently ignored.
HTTP and stdio transports queue top-level Contract Registry diagnostics across concurrent and
non-diagnostic responses. After a call through an explicit legacy wrapper, use
`client.drainContractDiagnostics()` to read and clear migration warnings without changing the
method result. Legacy Adapter diagnostics mean the old wire ID remains an identity compatibility
route but receives no new field semantics.

The generated types come from `docs/spec/v1/schemas/`. Regenerate with:

```bash
bash scripts/regen-sdks.sh
```

This package can be consumed from npm or via a workspace path reference:

```json
{ "dependencies": { "@plurora/contract-sdk": "file:../plurora/sdk/typescript/contract-sdk" } }
```
