# Capability Handles

> [English](./CAPABILITY_HANDLES.en.md) · [中文](./CAPABILITY_HANDLES.md)

Capability handles are the runtime authority model for kernel v1. Manifest strings say what a package may request at most; handles say what the kernel actually grants at a point in time.

## What handles are

Handles are kernel-minted, unforgeable, scoped, revocable, and expirable. A package sees an opaque id plus minimal metadata; it cannot construct equivalent authority itself.

Handles cover:

- invoking a capability provider;
- reading or appending an event range;
- using outbound network primitives;
- resolving declared `secret_ref`s;
- other v1 authority exposed by the host.

## Why not string permissions

String-only permissions mix declaration and actual authority. A package can declare `capabilities.invoke`, but the runtime still needs to know: which session, which provider, which method, which host, when it expires, and whether it has been revoked.

The handle pattern is common in capability-security systems, including:

- WASI preview2 resource handles;
- Cloudflare Workers Durable Object / service bindings;
- SES / object-capability attenuated references;
- browser platform handles that cannot be forged directly.

Plurora uses it to make least authority, attenuation, revocation, and audit the normal path.

## String declaration vs runtime authority

| Layer | Meaning | Mutability |
|---|---|---|
| Manifest capability / permission strings | Authority ceiling and review input | Fixed at package publish time |
| Host policy | Local maximum allowed by the host | Host configuration |
| Runtime handle | Current usable authority | Can attenuate, revoke, expire |

Calls use the kernel handle as the source of truth. If the manifest declaration is insufficient, no handle is minted. If host policy denies it, no handle is minted.

## Handle fields

| Field | Meaning |
|---|---|
| `id` | Unforgeable kernel-minted identifier. |
| `cap_type` | Authority type: invoke, event, outbound, secret, host, etc. |
| `cap_version` | Semantic version of the handle model. |
| `scope` | Package, session, capability, provider, host, or resource scope. |
| `constraints` | Limits: methods, hosts, schemas, counts, bytes, deadlines, metadata. |
| `lease` | Expiry time, refresh policy, or one-shot policy. |
| `provenance` | Source: manifest declaration, host grant, attenuated parent, audit reason. |
| `parent` | Optional parent handle for attenuation trees and revocation propagation. |

## Lifecycle

### 1. Mint at package load

When a Path A package loads, the kernel reads manifest, host policy, and profile. Declarations that pass are converted into initial handles. Path B packages do not receive v1 handles.

### 2. Inject through bindings

Handles are injected through bindings:

- subprocess: the `package.handshake` `bindings` dictionary;
- rust_inproc: `KernelEnv`;
- wasm: future WIT resource imports;
- remote: future SPIFFE + Biscuit token exchange.

SDKs wrap these handles as `kernelClient` methods, so package authors usually do not write protocol fields by hand.

### 3. Attenuate

A package or host can derive a narrower child handle: shorter lease, smaller session scope, fewer methods, fewer hosts, lower call count. A child can never be stronger than its parent.

### 4. Use

When a package calls `kernel.v1.capability.invoke`, outbound methods, or event methods, the runtime checks handle id, caller, scope, constraints, lease, and revoke state. Failures fail closed and write audit.

### 5. Revoke

`kernel.v1.cap.revoke(handle)` makes a handle invalid immediately. Revocation can affect just one handle or a subtree. Package unload revokes live handles held by that package.

### 6. Expire

When a leased handle expires, it can no longer be used. The package needs a new handle through host grant, manifest reload, or an explicit refresh path.

## How packages use handles

Package authors normally use an SDK:

```ts
const result = await kernelClient.invoke("provider/capability", input)
```

The SDK selects an appropriate handle from bindings and places it in protocol context. If no handle exists, the call fails instead of falling back to anonymous host authority.

The low-level protocol still permits explicit handle ids for non-TypeScript SDKs, tests, and other language bindings.

## Subprocess bindings

Subprocess packages handshake after start. The handshake includes package identity, contract mode, SDK capability, and available bindings. Example:

```json
{
  "contract": "v1",
  "bindings": {
    "invoke": [{ "id": "cap_...", "scope": { "package_id": "demo/echo" } }],
    "outbound": [],
    "events": []
  }
}
```

stdout remains reserved for JSON-RPC frames; stderr can be captured as package logs.

## Rust in-process bindings

Rust in-process packages receive handles through `KernelEnv`. The host catalog binds manifest entries to in-process provider traits. In-process providers missing from the catalog are rejected at load time.

## Outbound and secrets

Network and secrets use the same model:

- manifest declares `permissions.network` and `permissions.secret_refs`;
- host policy decides whether to allow them;
- the kernel mints outbound / secret handles;
- calls check host, method, scheme, and `secret_ref` declarations;
- audit writes references and redaction state, never raw secrets.

## How effect audit consumes handles

`kernel.v1.audit.package` and `plurora audit --package <id>` merge three data sets:

1. declared: manifest capabilities, permissions, network, and secret_refs;
2. granted: handles minted, attenuated, revoked, and expired by the kernel;
3. used: audit events for capability invocation, outbound, event read/write, and secret resolution.

Reports flag:

- declared but unused;
- used but undeclared;
- granted but never used;
- revoked/expired handle use attempts;
- wider-than-needed declarations;
- Path B `contract_mode: "none"`.

## Design rules

- Least authority by default.
- No package-id privilege.
- String declarations are not authority.
- Authority must be observable, revocable, and expirable.
- All denials fail closed.
- Audit records must not contain raw secrets or content semantics.

## FAQ

### Do handles break cross-language SDKs?

No. A handle is an opaque id plus JSON metadata. Any language can store it and send it back.

### Can a package pass a handle to another package?

Not by default. Handles are caller-package-bound. Future Powerbox work can add explicit delegation, but it must mint new provenance and audit chain.

### Do Path B packages have handles?

No v1 capability bindings. A Path B package can run as a self-contained process, but it cannot gain kernel authority through manifest declarations.

### Does revoke affect in-flight calls?

New calls must fail. In-flight streaming calls follow cancel/termination policy and emit lifecycle events.

## Operator checklist

- Check that manifest permissions correspond to necessary features.
- Run `plurora audit --package <id>` for declared vs used authority.
- Periodically inspect live handle counts for long-running packages.
- Revoke handles that are no longer needed instead of waiting for package unload.
- Use shorter leases for network and secret handles.
- Use attenuated child handles for high-risk calls.
- Run the conformance kit in CI to catch newly undeclared use.

## Package-author checklist

- Declare only capabilities and permissions that are actually needed.
- Do not store handle ids in config files as long-term credentials.
- Do not pass handles to another package or user script.
- Surface permission errors as understandable diagnostics.
- Treat missing handles as normal for optional features.
- Use SDK bindings instead of hand-written spoofed package ids.

## Minimal example flow

1. Manifest declares consumption of `example/echo.invoke`.
2. Host policy allows that provider.
3. Package load mints an invoke handle.
4. Subprocess handshake receives bindings.
5. SDK uses the handle to call the provider.
6. Kernel writes `capability.invoked` and `capability.completed`.
7. `plurora audit` shows declared, granted, and used authority match.

## References

- [`../spec/KERNEL_V1_CONTRACT.md`](../spec/KERNEL_V1_CONTRACT.en.md)
- [`CONFORMANCE_KIT.md`](CONFORMANCE_KIT.en.md)
- [`PATH_B_SELF_CONTAINED.md`](PATH_B_SELF_CONTAINED.en.md)
