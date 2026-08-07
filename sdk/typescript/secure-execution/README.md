# Plurora TypeScript secure-execution helpers

Thin helpers for capability packages that need secret references, network
declarations, outbound audit/redaction, and streaming frame client support.
These wrap only the public kernel protocol and types — no private internals,
no protocol bypass.

## Usage

```ts
import {
  secretRef,
  NetworkDeclaration,
  OutboundAuditHelper,
  StreamFrameClient,
} from "./index";
```

### Secret references

```ts
// Create a secret reference (never embed raw secrets in payloads)
const ref = secretRef("env", "MY_API_KEY");
// → "secret_ref:env:MY_API_KEY"

// Alternative prefix forms
const altRef = secretRef("vault", "prod/openai", "secretRef");
// → "secretRef:vault:prod/openai"

// Validate a secret reference
isValidSecretRef("secret_ref:env:KEY"); // true
isValidSecretRef("sk-abc123");           // false
```

### Network declarations

```ts
const decl = new NetworkDeclaration({
  host: "api.example.com",
  methods: ["GET", "POST"],
  purpose: "model inference",
});
decl.toManifestEntry();
// → { host: "api.example.com", methods: ["GET", "POST"], purpose: "model inference" }
```

### Outbound audit helper

```ts
const audit = new OutboundAuditHelper({
  packageId: "example/my-package",
  capabilityId: "example/my-package/fetch",
});

// Build an audit-safe request payload (no raw secrets)
const payload = audit.buildRequestPayload({
  destinationHost: "api.example.com",
  method: "POST",
  secretRefsUsed: [secretRef("env", "MY_KEY")],
  purpose: "model inference",
});
// payload contains only references — never raw secrets
```

### Stream frame client

```ts
const client = new StreamFrameClient();

// Simulate a streaming lifecycle (no real inference)
const startFrame = client.start("example/stream/echo", {});
const chunk1 = client.chunk({ text: "faux token 1" });
const chunk2 = client.chunk({ text: "faux token 2" });
const endFrame = client.end();
// Frames carry invocation_id, stream_id, sequence, redaction_state
```

See `sdk/typescript/subprocess` for the base subprocess SDK.
