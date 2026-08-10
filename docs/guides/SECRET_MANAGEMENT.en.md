# Secret Management

> [English](./SECRET_MANAGEMENT.en.md) · [中文](./SECRET_MANAGEMENT.md)

Plurora references secrets through `secret_ref`. The host resolves those references into real values only while executing a capability call. Packages never receive raw secrets. This guide explains the four resolver paths, the security model, and how to move from environment variables to platform or Installation-level encrypted storage.

## Design principles

- Packages use `secret_ref` values and do not touch raw values.
- The host resolves only at capability-call time; raw values do not enter events, audits, proposals, or logs.
- The env, store, installation, and future vault types are implemented by separate resolvers.
- Missing, denied, or malformed references fail closed.
- Error messages do not leak values.
- A `secret_ref` is runtime authority input, not a container for raw secrets.
- First-party Packages have no special privilege; they also need ordinary manifest declarations for secret use.

## `secret_ref` format

Canonical format:

```text
secret_ref:<vault>:<key>
```

Currently supported:

```text
secret_ref:env:OPENAI_API_KEY
secret_ref:store:OPENAI_API_KEY
secret_ref:installation:OPENAI_API_KEY
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
- Tradeoffs: requires `plurora/secret-store-lab` to be loaded.

Example:

```yaml
secret_refs:
  - secret_ref:store:OPENAI_API_KEY
```

The host uses `StoreSecretResolver` during capability execution to read and decrypt the store. The package still sees only `secret_ref:store:OPENAI_API_KEY`.


### `secret_ref:installation:NAME` — Installation-level encrypted store

Reads `~/.plurora/installations/<installation-id>/secrets.dat` for the current Installation. The Installation store uses the same age encryption model and master key as the platform store, but the data file is isolated per Installation.

Resolution path:

1. Obtain the active `installation_id` from a Host-verified call context.
2. Require the exact full ref in `secret_policy.allowed_secret_refs`.
3. Read that Installation's `secrets.dat`.
4. If `NAME` exists, return the Installation value.
5. If it is missing and `secret_policy.allow_platform_fallback: true`, fall back to platform `secret_ref:store:NAME`.
6. If still missing, fail closed.

- Use for: Installation-specific provider keys or configuration that should be visible as Installation state.
- Advantages: one Installation can override a provider key without affecting others.
- Tradeoffs: requires a Host-verified Installation context; without one it must fail.

Example:

```yaml
secret_refs:
  - secret_ref:installation:OPENAI_API_KEY

secret_policy:
  allow_platform_fallback: true
  allowed_secret_refs:
    - secret_ref:installation:OPENAI_API_KEY
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
| Local override for installed Work | installation (with policy-based store fallback) |
| One Installation must use its own key | installation + `allow_platform_fallback: false` |
| Docker single-service deployment | env |
| Shared multi-user deployment | env (export per user) |
| Team-shared secret source | future vault |

General rule:

- One-shot automation: use env.
- Long-lived desktop UX: use store.
- Installation-level override of platform configuration: use installation.
- Team rotation and central policy: wait for a vault capability.

## Using the store

### Through the UI (recommended)

YdlTavern's API Connections drawer supports paste + save:

1. Choose a provider (OpenAI / Anthropic / Gemini, etc.).
2. Paste the API key.
3. Press save.
4. The UI calls `plurora/secret-store-lab/put_secret`.
5. The UI sets the profile `secretRef` to `secret_ref:store:OPENAI_API_KEY`.
6. Later calls carry only the reference, never the raw key.

If the store is unavailable, env remains an independent path. Installed Work can use `secret_ref:installation:*`; `secret_policy` explicitly controls whether a missing Installation value may fall back to the platform store.

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
    "capability_id": "plurora/secret-store-lab/put_secret",
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

The env and platform-store paths remain available. An Installation can explicitly use `secret_ref:installation:NAME`; a missing local value falls back only when policy permits it. The three paths do not widen each other's authority.

## Errors and diagnostics

Common failures:

| Error | Meaning | Fix |
|---|---|---|
| malformed secret ref | invalid reference format | use `secret_ref:<vault>:<key>` |
| resolver denied | unsupported or disallowed vault | check allowlist / resolver config |
| missing env var | environment variable is absent | export it or migrate to store |
| missing store entry | store has no entry for the name | save through UI or capability |
| missing installation context | installation ref used without an active Installation | call through a Host-bound Installation, or use store/env |
| installation secret not allowed | ref is absent from the policy allowlist | update Installation secret policy, then retry |
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

## How Installation scope works

The scope for `secret_ref:installation:*` comes from a Host-verified Installation context, not an arbitrary string or path supplied by a surface:

1. Host protocol / runtime dispatch verifies the caller's authority for the exact Installation.
2. The Host obtains `InstallationSecretPolicy` and the secret-store path from the Installation projection.
3. Runtime sets `ACTIVE_INSTALLATION_SCOPE` with the typed InstallationId, policy, and verified store path.
4. `InstallationStoreSecretResolver` requires the full ref to appear exactly in `allowed_secret_refs`.
5. The resolver first reads `~/.plurora/installations/<id>/secrets.dat`.
6. Only when the value is absent and `allow_platform_fallback` is true does it resolve `secret_ref:store:NAME`; otherwise it fails closed.

The resolution order is therefore: exact allowlist → Installation store → fallback policy → platform store. See [`INSTALLATION_MODEL.md`](INSTALLATION_MODEL.en.md) for the Installation boundary.

## Implementation locations

- `crates/plurora-core/src/secret_ref.rs` — `secret_ref` parsing and validation.
- `crates/plurora-core/src/paths.rs` — filesystem paths (`secret_store_path` / `secret_store_key_path`).
- `crates/plurora-runtime/src/secret.rs` — `HostSecretResolver` / `EnvSecretResolver` / `StoreSecretResolver` / `CompositeSecretResolver`.
- `crates/plurora-runtime/src/installation_secret.rs` — `InstallationStoreSecretResolver` and verified scope.
- `crates/plurora-runtime/src/secret_store.rs` — shared encrypted file load/save.
- `crates/plurora-runtime/src/inproc/secret_store_lab.rs` — capability implementation.
- `packages/plurora/secret-store-lab/manifest.yaml` — package manifest.

## Current limits

- OS keyring integration is deferred; the default path uses the local key file.
- `plurora secret put / list / delete` CLI is deferred.
- Remote vault resolvers are not implemented.
- The store is a local user-level store, not a team-shared vault.
- Installation stores are selected by exact Host resource authority, but are still not a team-shared vault.

These limits do not change the core boundary: packages hold references, the host resolves, and errors fail closed.
