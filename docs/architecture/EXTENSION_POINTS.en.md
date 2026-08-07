# Extension Points

> [English](./EXTENSION_POINTS.en.md) · [中文](./EXTENSION_POINTS.md)

An extension point is a named interception point in the current public-contract runtime. Package Manifests declare subscriptions; the runtime owns registration, stable ordering, unload cleanup, and the bounded dispatch behavior implemented for that point.

Extension points are not private APIs and do not grant authority. A subscriber remains a Package participant subject to Manifest, permission, and runtime boundaries.

## Descriptor and subscription shape

A Manifest may declare an `ExtensionPointDescriptor` with:

- immutable `id` and `version`;
- `payload_schema`;
- `timing` (`sync` or `async`);
- `modifiable`;
- `short_circuit`.

A `HookSubscription` declares:

- `extension_point`;
- `handler`;
- `timing`;
- integer `precedence`.

Subscriptions are ordered by precedence, then subscriber Package ID, then handler name. Unloading a Package removes its subscriptions.

## Core points invoked today

The current runtime invokes exactly four built-in points:

| Point | Current behavior |
|---|---|
| `journal/before_append` | Awaited before persistence; may veto; the current dispatcher can return amended metadata. |
| `journal/after_append` | Awaited after persistence; receives the stored envelope; return value is ignored. |
| `capability/before_invoke` | Awaited before provider resolution/execution; may veto; the current dispatcher can return amended input. |
| `capability/after_invoke` | Awaited after a successful invocation; receives the invocation result; return value is ignored. |

`protocol.extension.list` returns these four IDs. `protocol.extension.describe` is reserved but not yet dispatched.

## Current implementation boundary

The registry, deterministic ordering, veto reporting, metadata/input mutation path, and unload cleanup are implemented and covered by runtime/conformance checks.

Arbitrary Package hook-handler execution, independent asynchronous delivery, deadline/quota enforcement per handler, failure audit, descriptor version negotiation, and generic dispatch of Package-declared extension points are not complete. Manifest declaration support must not be mistaken for a fully operational generic extension bus.

## Package-owned extension semantics

A Package may publish descriptor data under its own Package ID namespace. Shared semantics that need interoperability should be owned by an explicit Protocol. The runtime must not infer meaning from the ID or grant first-party implementations special routing.

Illustrative descriptor:

```yaml
contributes:
  extension_points:
    - id: someorg/conversation/before_step
      version: 1.0.0
      payload_schema: {}
      timing: sync
      modifiable: true
      short_circuit: true
```

Until generic Package-owned dispatch is implemented, this declaration is discoverable contract data rather than proof that arbitrary runtime calls are emitted for the point.

## Stability

Adding a built-in point expands the public runtime boundary and therefore requires a clear owner, payload schema, authority model, terminal/error behavior, and conformance proof. Product convenience alone is insufficient.
