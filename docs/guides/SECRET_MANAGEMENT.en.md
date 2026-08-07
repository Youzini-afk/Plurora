# Secret Management

> [English](./SECRET_MANAGEMENT.en.md) · [中文](./SECRET_MANAGEMENT.md)

Plurora references secrets through `secret_ref`. The host resolves those references into real values only while executing a capability call. Packages never receive raw secrets. This guide explains the four resolver paths, the security model, and how to move from environment variables to the local encrypted store or project-level store.

## Design principles

- Packages use `secret_ref` values and do not touch raw values.
- The host resolves only at capability-call time; raw values do not enter events, audits, proposals, or logs.
- The env, store, project, and future vault types are implemented by separate resolvers.
- Missing, denied, or malformed references fail closed.
- Error messages do not leak values.
- A `secret_ref` is runtime authority input, not a container for raw secrets.
- Official packages have no special privilege; they also need ordinary manifest declarations for secret use.

## `secret_ref` format

Canonical format:

```text
secret_ref:<vault>:<key>
```

Currently supported:

```text
secret_ref:env:OPENAI_API_KEY
secret_ref:store:OPENAI_API_KEY
secret_ref:project:OPENAI_API_KEY
```

Compatibility prefixes still parse:

```text
secretRef:env:OPENAI_API_KEY
secret-ref:env:OPENAI_API_KEY
host:env:OPENAI_API_KEY
```

New docs and new packages should prefer `secret_ref:<vault>:<key>`.

## Four resolver paths

### `secret_ref:env:NAME` — environment variable

Reads the `$NAME` environment variable. An allowlist controls which env names may be resolved.

- Use for: development, CI, Docker deployments.
- Advantages: no extra storage; export once before start.
- Tradeoffs: must be set again on every start; shell history can capture values; some process launch forms expose values in `ps` output.

Example:

```bash
export OPENAI_API_KEY=sk-...
```

Manifests or profiles store only the reference:

```yaml
secret_refs:
  - secret_ref:env:OPENAI_API_KEY
```

The host must allowlist `OPENAI_API_KEY`. If it is not allowed, resolution fails and no outbound call is attempted.

### `secret_ref:store:NAME` — local encrypted store

Reads `~/.plurora/secrets.dat` encrypted with age. The master key lives in `~/.plurora/secret-store.key` (0600) or in the system keyring.

- Use for: desktop use, long-lived local use, product-grade UX.
- Advantages: users paste once in the UI; the value is encrypted on disk and available on the next start.
- Tradeoffs: requires `official/secret-store-lab` to be loaded.

Example:

```yaml
secret_refs:
  - secret_ref:store:OPENAI_API_KEY
```

The host uses `StoreSecretResolver` during capability execution to read and decrypt the store. The package still sees only `secret_ref:store:OPENAI_API_KEY`.


### `secret_ref:project:NAME` — project-level encrypted store

Reads `~/.plurora/projects/<project_id>/secrets.dat` for the current project. The project store uses the same age encryption model and master key as the platform store, but the data file is per project.

Resolution path:

1. Find the active project from the current `ProtocolContext.session_id`.
2. Read that project's `secrets.dat`.
3. If `NAME` exists, return the project value.
4. If it is missing and `secret_policy.fallback_to_platform: true`, fall back to platform `secret_ref:store:NAME`.
5. If `NAME` is in `secret_policy.require_per_project`, platform fallback is blocked.
6. If still missing, fail closed.

- Use for: project-specific provider keys or configuration that should be visible as project state.
- Advantages: one project can override a provider key without affecting other projects.
- Tradeoffs: requires project context; without an active project/session it must fail.

Example:

```yaml
secret_refs:
  - secret_ref:project:OPENAI_API_KEY

secret_policy:
  fallback_to_platform: true
  require_per_project: []
```

### `secret_ref:vault:KEY` — remote vault (future)

Reserved for future HashiCorp Vault / AWS Secrets Manager / Doppler integrations.

- Current state: syntax reserved, no implementation.
- Later: provided as an independent capability package.
- Behavior: still fail closed, avoid value leaks in errors, and audit references only.

## Which path to use

| Scenario | Recommended path |
|---|---|
| Local development | env |
| CI / automation | env |
| Desktop product | store |
| Plurora project default | project (with policy-based store fallback) |
| One project must use its own key | project + `require_per_project` |
| Docker single-service deployment | env |
| Shared multi-user deployment | env (export per user) |
| Team-shared secret source | future vault |

General rule:

- One-shot automation: use env.
- Long-lived desktop UX: use store.
- Project-level override of platform configuration: use project.
- Team rotation and central policy: wait for a vault capability.

## Using the store

### Through the UI (recommended)

YdlTavern's API Connections drawer supports paste + save:

1. Choose a provider (OpenAI / Anthropic / Gemini, etc.).
2. Paste the API key.
3. Press save.
4. The UI calls `official/secret-store-lab/put_secret`.
5. The UI sets the profile `secretRef` to `secret_ref:store:OPENAI_API_KEY`.
6. Later calls carry only the reference, never the raw key.

If the store is unavailable, the env path still works as a fallback. For installed projects, a profile can use `secret_ref:project:*`; when the project store is missing a value, it falls back to the platform store according to `secret_policy`.

### Through the command line

```bash
# Exercise the capability through conformance to verify availability.
plurora conformance --case secret_store
```

Future releases will add `plurora secret put / list / delete` commands for direct store management.

### Through the protocol

Any capability package can invoke:

```json
{
  "method": "capability.invoke",
  "params": {
    "capability_id": "official/secret-store-lab/put_secret",
    "input": { "name": "OPENAI_API_KEY", "value": "sk-..." }
  }
}
```

The raw `value` exists only for the invocation moment and is immediately encrypted on disk.

### Read behavior

The public protocol has no `get_secret`. Capability packages cannot request raw values. Reads happen only inside host executors:

1. The package declares and passes `secret_ref:store:NAME`.
2. The host checks manifest, handle, and network authority.
3. `StoreSecretResolver` decrypts the local store.
4. The executor injects the value into a provider header or adapter.
5. Audit records only `secret_ref:store:NAME`.

## Encryption details

- Algorithm: age (rage), authenticated encryption, X25519 identity.
- File format: age-encrypted JSON `{ schema, secrets: { name: value } }`.
- Schema: `plurora.secret-store.v1`.
- Store file: `~/.plurora/secrets.dat`.
- Master key file: `~/.plurora/secret-store.key`.
- File permissions: Unix 0600.
- Writes: atomic (tmp + rename).
- Name limit: ASCII letters/digits plus underscore and hyphen, 1..=128 characters.
- Value limit: UTF-8, <= 16 KiB.

These limits keep the store auditable and avoid treating arbitrary large payloads as secrets.

## Master key source

The host tries, in order:

1. OS keyring through the `keyring` crate. The current build does not enable it because of dbus system dependencies, so it falls through to step 2.
2. `~/.plurora/secret-store.key` with 0600 permissions.
3. Generate a new key on first use and persist it to the file.

OS keyring integration is deferred until CI and cross-platform builds have stable system dependencies.

## Security properties

- **No public `get_secret`**: packages cannot read other packages' secrets; only the host `SecretResolver` reads values.
- **Fail closed**: missing refs, missing stores, and missing entries return errors.
- **Errors do not leak values**: messages contain the ref name, not the value.
- **Store input is redacted in audit**: the `put_secret` call is audited, but the `value` field is redacted.
- **No plaintext store on disk**: values are not written into logs or backups; the store payload is age-encrypted.
- **No implicit network**: resolving a secret does not network; outbound still requires network authority.
- **Resolver separation**: env, store, and future vault resolvers can be composed, but each resolver handles only its own vault.

## Migrating from env to store

If you previously used `secret_ref:env:OPENAI_API_KEY`:

1. Open the YdlTavern API Connections drawer.
2. Paste the key you previously exported.
3. Press save.
4. The UI switches the profile to `secret_ref:store:OPENAI_API_KEY`.
5. Optionally unset the environment variable.

The env path remains available, and the platform store path remains available. Existing platform secrets keep working unchanged. A project can gradually switch a profile to `secret_ref:project:NAME`; if the project store has no value, policy can fall back to the platform store. The paths do not conflict, and different profiles can use different resolvers for the same provider.

## Errors and diagnostics

Common failures:

| Error | Meaning | Fix |
|---|---|---|
| malformed secret ref | invalid reference format | use `secret_ref:<vault>:<key>` |
| resolver denied | unsupported or disallowed vault | check allowlist / resolver config |
| missing env var | environment variable is absent | export it or migrate to store |
| missing store entry | store has no entry for the name | save through UI or capability |
| missing project context | project ref used without active project | call from a project session, or use store/env |
| project secret required | policy requires project-level config | write the secret in project settings |
| decrypt failed | store and key file do not match | check data directory and permissions |

Diagnostics must not include raw values. To check existence, use boolean or name-level capabilities such as `has_secret` / `list_secrets`; they do not return the secret value.

## Relationship to package installation

Package installation records which `secret_ref` authorities the user consented to. Install does not ask for raw secrets and does not write raw secrets into the lockfile.

The default install flow does not read a secret just because a package declares it. Reads happen only during capability invocation.

## Relationship to model calls

Model provider packages should accept `secret_ref`, for example:

```json
{
  "provider": "openai",
  "credential": "secret_ref:store:OPENAI_API_KEY"
}
```

The provider adapter constructs the request shape; the host outbound executor resolves and injects the header at the last moment. Responses, audits, and stream frames continue to contain only references.

## How project scope works

The scope for `secret_ref:project:*` comes from the project session, not from a `projectId` string the surface claims:

1. Home Play or `plurora project start` calls `host.project.start`.
2. The host creates or reuses a project session and writes `session.metadata.project_id`.
3. `clients/web` injects `session_id` into the surface as `initialProps.sessionId`.
4. Later surface RPCs automatically carry `session_id`.
5. Host dispatch sets `ProtocolContext.session_id`.
6. Before outbound dispatch resolves secrets, it reads `metadata.project_id` from that session.
7. The runtime sets the `ACTIVE_PROJECT_SCOPE` task-local with a `ProjectScopeContext`.
8. `ProjectStoreSecretResolver` first reads `~/.plurora/projects/<id>/secrets.dat`.
9. If the entry is missing, the project's `secret_policy` decides whether platform fallback is allowed.
10. When fallback is allowed, resolution reads `secret_ref:store:NAME`; when fallback is disabled or `require_per_project` matches, it fails closed.

The resolution order is therefore: project store → fallback policy → platform store. For the full real model call path, see [`REAL_MODEL_END_TO_END.md`](REAL_MODEL_END_TO_END.en.md); for where the project session comes from, see [`PROJECT_MODEL.md`](PROJECT_MODEL.en.md).

## Implementation locations

- `crates/plurora-core/src/secret_ref.rs` — `secret_ref` parsing and validation.
- `crates/plurora-core/src/paths.rs` — filesystem paths (`secret_store_path` / `secret_store_key_path`).
- `crates/plurora-runtime/src/secret.rs` — `HostSecretResolver` / `EnvSecretResolver` / `StoreSecretResolver` / `ProjectSecretResolver` / `CompositeSecretResolver`.
- `crates/plurora-runtime/src/secret_store.rs` — shared encrypted file load/save.
- `crates/plurora-runtime/src/inproc/secret_store_lab.rs` — capability implementation.
- `packages/official/secret-store-lab/manifest.yaml` — package manifest.

## Current limits

- OS keyring integration is deferred; the default path uses the local key file.
- `plurora secret put / list / delete` CLI is deferred.
- Remote vault resolvers are not implemented.
- The store is a local user-level store, not a team-shared vault.
- The project store is soft isolation, not a multi-tenant security boundary.

These limits do not change the core boundary: packages hold references, the host resolves, and errors fail closed.
