# Plurora TypeScript Agent Adapter SDK

Pure TypeScript agent adapter that lets capability packages expose themselves as tools, invoke through the public protocol, and produce trace / proposal artifacts — without depending on private runtime internals, `pi-coding-agent`, or any external agent framework.

**This is an SDK adapter, not an agent framework.** It does not ship a model, prompt, memory, or turn loop; it only handles capability ↔ tool mapping, protocol invocation, and observability artifact construction.

## Usage

```ts
import {
  createPluroraAgentAdapter,
  capabilityToTool,
  createCapabilityTool,
  invokeCapabilityTool,
  streamCapabilityTool,
  createTraceEvent,
  createProposalDraft,
  diagnosePermissions,
  diagnoseProvider,
  blockRawSecrets,
  runPluroraAgentAdapterSelfTest,
} from "./index";
```

### Create adapter

```ts
const adapter = createPluroraAgentAdapter({
  protocolClient: myProtocolClient,  // implements ProtocolClient.call(request)
  packageId: "my/agent-package",
  principal: "user:alice",           // optional
});
```

### Capability → Tool mapping

```ts
const tool = capabilityToTool({
  capability_id: "search/query",
  name: "Search",
  description: "Search documents",
  streamable: true,
  provider_package_ids: ["search/impl"],
  required_permissions: ["network:outbound"],
});
// tool.name === "Search", tool.streamable === true
```

### Tool invocation

```ts
const result = await invokeCapabilityTool(protocolClient, {
  tool,
  input: { q: "hello" },
});
// result.ok === true → result.output holds the return value
```

With multiple providers you must specify explicitly:

```ts
const result = await invokeCapabilityTool(protocolClient, {
  tool: ambiguousTool,   // provider_package_ids: ["pkg/a", "pkg/b"]
  input: {},
  provider_package_id: "pkg/a",  // required
});
```

### Streaming

```ts
const { request, frames } = streamCapabilityTool({
  tool: streamableTool,
  input: { prompt: "Tell a story" },
});

// frames is a faux stream frame constructor (no real SSE dependency)
frames.start({ init: true });
frames.chunk({ text: "Once upon" });
frames.chunk({ text: "a time" });
frames.end();
```

### Trace events

```ts
const trace = createTraceEvent("tool_call", "my/pkg", { tool: "search" }, {
  capability_id: "search/query",
  call_id: "call-1",
});
```

### Proposal drafts

```ts
const draft = createProposalDraft(
  "my/pkg",
  "Search documents",
  "Search relevant documents for the user query",
  [{ capability_id: "search/query", input: { q: "hello" } }],
  { risk_note: "Read-only operation", requires_confirmation: false },
);
```

### Provider / Permission diagnostics

```ts
const provDiag = diagnoseProvider(tool);
if (!provDiag.unambiguous) {
  console.warn(provDiag.ambiguity_reason);
}

const permDiag = diagnosePermissions(tool);
if (!permDiag.satisfied) {
  console.warn("Missing permissions:", permDiag.missing);
}
```

### Raw secret scanning

```ts
const scan = blockRawSecrets({ api_key: "sk-abc..." });
if (scan.has_raw_secrets) {
  throw new Error("Payload contains raw secrets: " + scan.flagged_fields.join(", "));
}

// secret_ref form passes safely
blockRawSecrets({ api_key: "secret_ref:env:MY_KEY" }).has_raw_secrets; // false
```

### Self-test

```ts
const results = runPluroraAgentAdapterSelfTest();
const failed = results.filter(r => !r.passed);
if (failed.length > 0) {
  for (const f of failed) console.error(`FAIL: ${f.name} — ${f.detail}`);
}
```

## API reference

| Export | Kind | Description |
|---|---|---|
| `ProtocolClient` | interface | `call(request)` protocol client interface |
| `ProtocolRequest` / `ProtocolResponse` | interface | Protocol request / response |
| `CapabilityDescriptor` | interface | Capability descriptor |
| `CapabilityTool` | interface | Tool representation of a capability |
| `ToolCall` / `ToolResult` | interface | Invocation request / result |
| `AgentTraceEvent` | interface | Agent trace event |
| `AgentProposalDraft` | interface | Proposal draft artifact |
| `StreamRequest` / `StreamFrameAdapter` / `StreamAdapterFrame` | interface | Stream request / frame builder |
| `PermissionDiagnostics` / `ProviderDiagnostics` / `RawSecretScanResult` | interface | Diagnostic results |
| `createPluroraAgentAdapter` | function | Create adapter |
| `capabilityToTool` | function | Descriptor → tool |
| `createCapabilityTool` | function | Create tool from fields |
| `invokeCapabilityTool` | function | Invoke tool through protocol |
| `streamCapabilityTool` | function | Stream variant (returns request + frame builder) |
| `createTraceEvent` | function | Create trace event |
| `createProposalDraft` | function | Create proposal draft |
| `diagnosePermissions` | function | Permission diagnostics |
| `diagnoseProvider` | function | Provider diagnostics |
| `blockRawSecrets` | function | Raw secret scanner |
| `runPluroraAgentAdapterSelfTest` | function | Pure-TS self-test |

## Design constraints

- **No import** from `clients/web`, `clients/web/private`, or any runtime private module
- **No dependency** on `pi-coding-agent` or external agent framework
- **No external dependencies** — pure TypeScript
- **No** `platform.agent.*`, `platform.model.*`, `platform.prompt.*`, `platform.memory.*` methods
- **No** `any` type — prefers `unknown`-safe patterns
- **Rejects** ambiguous provider calls unless explicit `provider_package_id` is given
- **Blocks** raw secrets: reuses `secret_ref` pattern but does not import private runtime
