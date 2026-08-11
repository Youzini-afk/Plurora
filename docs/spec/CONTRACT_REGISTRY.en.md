# Contract Registry and Explicit Negotiation (Experimental)

> [English](./CONTRACT_REGISTRY.en.md) · [中文](./CONTRACT_REGISTRY.md)

This document describes the executable registry for Plurora's public contract. The registry is the single source of truth for method identity, ownership, maturity, schemas, implementation status, streaming behavior, and explicit contract negotiation.

The current pre-release registry exposes exactly one wire ID for every method. It does not resolve alternate IDs, run request/response adapters, or publish parallel compatibility surfaces.

## One resolution boundary

Before a permission gate or handler runs, every transport uses the same sequence:

1. validate the optional contract selection;
2. resolve the requested method by exact registry ID;
3. attach the Host-established principal and transport context;
4. dispatch the single `PlatformMethod` handler;
5. return the common result or structured error envelope.

HTTP RPC, Host stdio, in-process calls, and subprocess reverse stdio share this boundary. A missing or unknown ID fails before business dispatch.

## Registry shape

Registry version `0.1.0` publishes 99 `ContractMethod` records. Each record contains:

- `id` — the only public wire ID;
- `owner_layer` — `substrate`, `host`, `protocol`, or `shell`;
- `maturity` — `experimental`, `candidate`, or `stable`;
- request and response schema URIs;
- `introduced_in`;
- implementation status;
- streaming metadata.

The first ID segment declares ownership:

| Prefix | Owner | Examples |
|---|---|---|
| `context`, `journal`, `capability`, `authority`, `object`, `identity` | Substrate | `context.open`, `journal.append`, `authority.handle.revoke` |
| `host` | Host | `host.installation.list`, `host.outbound.execute` |
| `protocol`, `change`, `projection` | Protocol | `protocol.extension.list`, `change.proposal.apply` |
| `shell` | Shell | `shell.contribution.list` |

Package capability IDs remain Package-owned slash namespaces such as `org/package/capability`; they are not public-contract method IDs.

## Explicit negotiation

The RPC envelope may include a contract selection:

```json
{
  "id": "request-1",
  "method": "host.info",
  "params": {},
  "contract": {
    "profile": "plurora.contract.default/v1",
    "versions": [
      { "layer": "host", "version": "0.1.0" }
    ],
    "protocols": []
  }
}
```

- Omitting `contract` selects `plurora.contract.default/v1`.
- `plurora.contract.default/v1` requires the published substrate, Host, Protocol, and Shell layer versions.
- `plurora.shell.default/v1` requires the published Host, Protocol, and Shell layer versions.
- Explicit layer requirements must match exactly.
- Duplicate requirements, unknown profiles, layers outside the selected profile, and version mismatches fail closed.
- Explicit Protocol Commons selections are negotiated before method dispatch.
- Negotiation never silently falls back to another profile or version.

An unsatisfied selection returns `protocol/error/unsupported_contract` with a structured reason and does not invoke the requested handler.

## `host.info`

`host.info` publishes the registry version, default profile, layer and version descriptors, profiles, method descriptors, supported transports, and Protocol Commons descriptors. Clients must use this response for discovery rather than infer support from product branding or Package IDs.

## Schemas and SDKs

Every method schema carries `x-plurora-contract` metadata derived from the runtime registry. The generator:

- emits one TypeScript and one Rust method identity per wire ID;
- rejects duplicate wire IDs, generated function names, and OpenAPI operation IDs;
- keeps request/result types synchronized with the JSON Schemas;
- emits negotiation-capable clients only when the transport can carry contract selection.

Run the repository generator to update schemas, OpenAPI, and both generated SDKs together:

```sh
scripts/regen-sdks.sh
```

Generated artifacts are reviewed but not edited manually. A clean regeneration must be deterministic.

## Change discipline

Within the current v1 boundary, compatible evolution is additive. Removing or renaming a method, changing requiredness, or changing field meaning requires a new explicit version boundary. Pre-release destructive resets are performed as one coordinated repository change; the finished tree contains only the selected identity set.
