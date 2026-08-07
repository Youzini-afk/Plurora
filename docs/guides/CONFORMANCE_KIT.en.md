# Conformance Kit

> [English](./CONFORMANCE_KIT.en.md) · [中文](./CONFORMANCE_KIT.md)

The conformance kit lets third-party capability packages verify kernel v1 compliance before they are handed to a host, marketplace, CI gate, or user. It checks the platform boundary, not package content semantics.

## Purpose

Plurora requires official and third-party packages to use the same contract. The kit provides repeatable local validation: whether the manifest parses, the entry can run, bindings are correct, capability declarations line up, permissions are least-authority, audit is visible, and fixture calls are stable.

Passing the kit does not mean the package is high quality or content-correct. It means the package participates in the platform according to v1.

## Basic usage

```bash
plurora conformance package --contract v1 --path <package>
```

Common options:

```bash
plurora conformance package --contract v1 --path <package> --format json
plurora conformance package --contract v1 --path <package> --static-only
```

- `--format json`: machine-readable output for CI.
- `--static-only`: manifest/schema/declaration checks only; do not start the package.
- `--contract v1`: validate the v1 contract. Path B packages still use this flag; checks are skipped according to `entry.contract: "none"`.

## Report statuses

| Status | Meaning |
|---|---|
| PASS | The check passed. |
| FAIL | The check failed; compliance percentage drops and CI should usually fail. |
| SKIP | The check does not apply, for example capability binding on Path B. |
| WARNING | Not a contract violation, but risky or worth improving, such as broad declarations. |

## The 8 acceptance checks

### 1. Manifest parse

Parse `manifest.yaml` or equivalent package description and validate the basic shape of id, version, entry, capabilities, permissions, surface contributions, schemas, hooks, and extension points. Failures usually mean a wrong package root, invalid YAML, missing required fields, or schema mismatch.

### 2. Contract mode

Check `entry.contract`. `"v1"` means Path A: the package accepts capability handles, permission enforcement, and audit. `"none"` means Path B: the package is self-contained and receives no v1 bindings. Missing values default to Path A for safe new-package behavior.

### 3. Entry support

Confirm the entry kind is supported by the current host. `subprocess` and `rust_inproc` are the main current execution paths. `wasm` and `remote` are first-class manifest forms, but execution remains to be completed in a later iteration. If host policy denies the entry, the check fails.

### 4. Bindings / handshake

Path A subprocess packages must complete JSON-RPC stdio handshake and declare received bindings. Rust in-process packages must initialize through `ComponentEnv`. Path B skips v1 binding checks, but still has to prove it can start without requiring kernel capabilities.

### 5. Capability declarations

Check `provides` and `consumes`. Providers must have stable id, version, input/output schema, streaming flag, and side-effect description. Consumer declarations must map to handle ceilings. Ambiguous providers, missing schemas, and illegal namespaces fail.

### 6. Permission declarations

Check `events.append/read`, `capabilities.invoke`, `permissions.network`, `permissions.secret_refs`, and related declarations against package behavior. Undeclared use fails. Over-broad declarations may warn. Path B packages cannot use these declarations to gain v1 authority.

### 7. Audit visibility

Verify package lifecycle, capability calls, outbound requests, permission denial, or Path B contract mode are visible to host audit. Path A should produce declared-vs-used reports. Path B events should include `contract_mode: "none"` so operators can distinguish the mode.

### 8. Fixture invocation

Invoke non-streaming capabilities with deterministic fixture input and validate schema, permission, output shape, and no raw-secret leakage. Streaming capabilities get a lightweight lifecycle smoke. `--static-only` skips this check.

## Compliance percentage

The report computes PASS over applicable checks. SKIP is not counted in the denominator. WARNING is not failure, but should be reviewed before release.

Example: a Path A package with 8 PASS checks is 100%. A Path B package can also be 100% when self-contained checks pass and inapplicable binding/capability/permission checks are skipped.

## CI integration

Recommended package-repo command:

```bash
plurora conformance package --contract v1 --path . --format json > conformance.json
```

PR gates should fail when:

- any FAIL appears;
- compliance percentage drops below the project threshold;
- a new WARNING is not explicitly accepted;
- the JSON report shows broadened permissions without review.

In a monorepo, run the command for every package manifest. For Path B packages, still run the kit so lifecycle observability is verified.

## Common failures

- `manifest_not_found`: `--path` is wrong or the package has no manifest.
- `entry_not_supported`: host policy does not allow the entry.
- `handshake_failed`: subprocess did not output correct JSON-RPC handshake, or stdout is polluted by logs.
- `binding_missing`: package declared needed authority but received no handle.
- `schema_invalid`: input/output schema does not match the v1 schema subset.
- `permission_denied`: fixture call attempted undeclared authority.
- `raw_secret_detected`: payload, metadata, or output contains an obvious raw secret.

## Custom checks

A future iteration will extract the kit as an embeddable library. For now, run custom checks as separate CI steps and use conformance JSON as input. Examples: require every network declaration to include `purpose`, or require package ids to match an organization prefix.

Custom checks must not replace the official kit. They may tighten project rules, never loosen the v1 contract.

## Relationship to SDKs

Generated SDKs use the same `docs/spec/v1/schemas/` source. If SDK generation succeeds but conformance fails, the package implementation or manifest is inconsistent with the contract. If conformance passes but SDK types are missing, generated SDK artifacts need refresh.

## JSON report shape

`--format json` emits stable fields for CI and dashboards:

```json
{
  "package_id": "example/echo",
  "contract": "v1",
  "compliance_percent": 100,
  "checks": [
    { "id": "manifest.parse", "status": "PASS" },
    { "id": "bindings.handshake", "status": "PASS" }
  ]
}
```

Fields may expand additively. CI should ignore unknown fields and depend only on stable fields such as `status`, `id`, and `compliance_percent`.

## Static vs dynamic checks

Static checks do not start the package: manifest parse, contract mode, entry support, capability declarations, and permission declarations. Dynamic checks start the package and validate handshake, bindings, fixture invocation, and audit visibility.

Run static checks as fast PR lint. Run the full kit on main branches or release gates.

## Release advice

Before release, save conformance JSON as an artifact. The artifact should record kit version, schema hash, package manifest hash, and run time. If compatibility issues appear later, this helps distinguish package changes, schema changes, and host behavior changes.

## Monorepo example

```bash
for manifest in packages/*/manifest.yaml; do
  dir=$(dirname "$manifest")
  plurora conformance package --contract v1 --path "$dir" --format json
done
```

Do not merge all package reports into a single unstructured log. Keep them per package so permission or schema failures are easy to locate.

## Relationship to effect audit

The conformance kit proves that a package obeys the contract in test scenarios. Effect audit proves which authority a running package actually used. They complement each other: CI relies on the kit, operators rely on audit, and release review should inspect both.

## Version compatibility

Kit versions follow additive v1 schema evolution. New checks should enter as WARNING or SKIP by default to avoid surprising existing packages. When a check becomes a release gate, roadmap or changelog should say so explicitly.

## Path A and Path B

| Item | Path A (`v1`) | Path B (`none`) |
|---|---|---|
| Manifest permission declarations | Effective and enforced | Not used to gain v1 authority |
| Bindings | Required | Skipped |
| Capability invoke | Through handles | Not applicable |
| Audit | declared vs used | lifecycle + `contract_mode: none` |
| Kit target | correct authority and callable | self-contained and observable |

## References

- [`../spec/KERNEL_V1_CONTRACT.md`](../spec/KERNEL_V1_CONTRACT.en.md)
- [`CAPABILITY_HANDLES.md`](CAPABILITY_HANDLES.en.md)
- [`PATH_B_SELF_CONTAINED.md`](PATH_B_SELF_CONTAINED.en.md)
