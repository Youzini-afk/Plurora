# @plurora/contract-sdk

Generated TypeScript bindings for the Plurora public contract.

```ts
import { attach, fromHttpRpc } from "@plurora/contract-sdk";

const client = attach(fromHttpRpc("http://localhost:8787/rpc"));
const info = await client.hostInfo({});
```

Every generated method calls one canonical wire ID. To require an exact Host
contract and Protocol selection before subsequent calls:

```ts
await client.negotiateHost({
  profile: "plurora.contract.default/v1",
  versions: [{ layer: "host", version: "0.1.0" }],
  protocols: [],
});
```

Negotiation fails when the transport cannot carry the selection; it is never
silently ignored. Regenerate the package from the public schemas with:

```bash
bash scripts/regen-sdks.sh
```
