# Real Model Calls End to End

> [English](./REAL_MODEL_END_TO_END.en.md) · [中文](./REAL_MODEL_END_TO_END.md)

Real model calls still use ordinary Package capabilities, Host secret resolution, and outbound executors. An Installation contributes Host-local policy and secret scope only. It does not let Work, Packages, or first-party code bypass authority, audit, or redaction.

## Call chain

```text
Work / Assembly
  └─ Component import Port → model provider capability
        ↓ resolver + AssemblyLock
Installation
  ├─ active WorkRevision / AssemblyLock
  ├─ InstallationSecretPolicy
  └─ secret_ref:installation:*
        ↓ Host-verified Installation context
capability.invoke / capability.stream
        ↓ manifest + handle + schema + effect checks
provider adapter
        ↓ host.outbound.execute / stream / websocket
Host executor resolves secret at the last moment
        ↓ HTTPS / WSS
terminal EffectReceipt + redacted audit
```

Web Installation detail does not automatically create a Run. Real calls must run inside the context created by an explicit successful `host.run.start`, which activates only an installed, verified, uniquely matching local implementation. Missing, ambiguous, unsupported-backend, or machine-resource requirements return structured gaps instead of implicit build/deploy. Closing a tab does not stop the Run; explicit `host.run.stop` stops only that Run.

## Configure secrets

Platform-shared value:

```text
secret_ref:store:OPENAI_API_KEY
```

Installation-local value:

```text
secret_ref:installation:OPENAI_API_KEY
```

The Installation record must allow the full reference:

```yaml
secret_policy:
  allow_platform_fallback: false
  allowed_secret_refs:
    - secret_ref:installation:OPENAI_API_KEY
```

`InstallationStoreSecretResolver` checks the exact allowlist, then reads `~/.plurora/installations/<installation-id>/secrets.dat`. A missing local value resolves the matching `secret_ref:store:*` only when `allow_platform_fallback` is true.

## Provider Package

A provider is an ordinary Package:

- its manifest declares the capability, network host, method, purpose, and required `secret_ref`;
- its Component export Port declares protocol/interface/version/profile, interaction, transport, and effect class;
- first-party publisher identity gets no routing priority, and multiple compatible providers remain ambiguous;
- raw keys never enter Package input/output schemas.

The adapter constructs the request shape but cannot access the network directly. Real network effects cross `host.outbound.*`, which rechecks current authority and policy before execution.

## Execution and audit

1. `capability.invoke` or stream validates the caller handle, schema, and provider binding.
2. The Host establishes secret scope from a verified Installation context.
3. The outbound executor resolves the secret and injects the header at the last moment.
4. Audit records only Package / capability / destination / method / purpose / secret ref / redaction state.
5. Success, denial, error, cancellation, and timeout produce distinct terminal evidence.

Events, logs, stream frames, proposals, and receipts never contain raw request bodies, provider responses, prompts, or secret values.

## Default network boundary

- HTTP is HTTPS-only and WebSocket is WSS-only.
- Redirects fail closed.
- Missing manifest destination / method / purpose declarations deny the request.
- Live executors are disabled by default; ordinary conformance uses fake executors.
- Secret resolution itself performs no network access.

## Common failures

| Diagnostic | Cause | Fix |
|---|---|---|
| no active installation scope | Call lacks a Host-verified Installation context | Call through a Host path bound to the Installation |
| reference is not allowed | Ref is absent from `allowed_secret_refs` | Update Installation policy |
| installation entry absent | Local value is missing and fallback is disabled | Write the Installation store or explicitly enable fallback |
| outbound denied | Manifest/handle/policy denies destination | Correct declarations and reapprove |
| binding ambiguous | Multiple providers are equally compatible | Bind an explicit provider in Work/Assembly |
| run gap | Run start lacks a matching local implementation, binding, or satisfiable target | Show `reason_code` and `next_step`, repair the Installation/local Package, and retry `host.run.start` |

See [`SECRET_MANAGEMENT.md`](SECRET_MANAGEMENT.en.md) for resolver details and [`INSTALLATION_MODEL.md`](INSTALLATION_MODEL.en.md) for Work / Installation boundaries.
