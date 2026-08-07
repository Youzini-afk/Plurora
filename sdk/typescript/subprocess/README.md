# Plurora TypeScript subprocess SDK

Thin helper for JSON-RPC-over-stdio Component Packages. It handles the
subprocess handshake, capability invocation, streaming, cancellation, and
reverse calls to exact Plurora public-contract method IDs.

```ts
import { pluroraClient, serveSubprocessPackage } from "@plurora/subprocess";

serveSubprocessPackage({
  onInvoke: ({ input }) => input ?? null,
});

const info = await pluroraClient.sendRequest("host.info", {});
```

The Host fixes the reverse caller principal to the subprocess Package. The SDK
exposes no Host internals, compatibility aliases, or implicit authority.
