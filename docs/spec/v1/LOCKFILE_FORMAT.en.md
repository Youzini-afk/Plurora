# Plurora Lockfile v1 Format

> [English](./LOCKFILE_FORMAT.en.md) · [中文](./LOCKFILE_FORMAT.md)

## Purpose

The Plurora lockfile makes profile package installations reproducible. A profile manifest describes what is desired; the lockfile records what was actually resolved: versions, sources, commits, content hashes, signature state, install paths, and the permissions granted by the user at install time.

When the same profile manifest is installed on another machine or at another time, the installer should prefer the pinned lockfile result. This avoids unexpected runtime state caused by branch movement, tag replacement, transitive dependency changes, or permission reinterpretation.

The lockfile is installer and host data, not a kernel protocol method. Kernel v1 remains content-free; package dependency resolution and fetching are implemented by the installation layer or by ordinary capability packages.

## Location

Default location:

```text
~/.plurora/profiles/<name>.lock.toml
```

`<name>` is the profile name. Implementations may support an explicit `--lockfile <path>`, but writes for the default profile should use this path.

## Filesystem layout

Plurora's state lives under a single base directory, resolved as:

1. `PLURORA_DATA_DIR` environment variable (explicit override)
2. `$XDG_DATA_HOME/plurora/` (XDG-compliant)
3. `~/.plurora/` (default)

Layout:

```text
<data_dir>/
├── store/                       # Immutable, content-addressed package store
│   ├── sha256-abc.../          # One directory per tree hash
│   └── sha256-def.../
├── profiles/                    # Per-user mutable
│   ├── default.yaml            # Profile autoload list
│   ├── default.lock.toml       # Lockfile
│   ├── alpha.yaml
│   └── alpha.lock.toml
├── keys/                        # GPG public keys (trust roots)
│   └── trusted-keys.asc
└── cache/
    └── git/                    # Git fetch cache
```

The store is content-addressed: installed trees are immutable, while successful
install/update/uninstall operations remove entries that are no longer referenced
by profiles or lockfiles. Store schema migration may clear old store contents
when hash rules change; profiles and lockfiles are preserved so updates can
rebuild the store.

Permissions: data directory is created with 0700 on Unix.

## Encoding

- Format: TOML.
- Time: RFC 3339 timestamp with a UTC offset.
- Hashes: algorithm-prefixed strings; v1 requires `sha256:<hex-or-encoded-digest>`.
- Enum values: snake_case.

## Schema version

The top-level `schema` field must be:

```toml
schema = "plurora.lock.v1"
```

Readers must reject unknown schemas unless an explicit migration flow is enabled. Within v1, only additive changes are allowed. Breaking changes must use a new namespace such as `plurora.lock.v2`.

## Top-level fields

### `schema`

String. Fixed to `plurora.lock.v1`. It lets the reader choose the correct parsing and validation rules.

### `profile`

String. The profile name pinned by this lockfile. It should match `<name>` in the path; implementations should warn or reject when it does not, to avoid applying the wrong lockfile.

### `generated_at`

Timestamp. When the lockfile was generated. It is for diagnostics and audit and does not participate in resolution decisions.

### `manifest_hash`

String. The canonical SHA-256 hash of the profile manifest when the lockfile was generated. Installers use it to detect profile manifest drift.

### `package`

Array. Locked package entries. In TOML, entries are encoded as `[[package]]`. An empty array means the profile currently has no external locked packages.

## LockEntry fields

Each `[[package]]` represents one resolved and installed, or installable, package.

### `id`

String. Package id. It must match the locked package manifest's `id`.

### `version`

String. The resolved package version. This is a lockfile result, not a constraint.

### `source`

Enum. Source kind:

- `internal`: built into Plurora or provided by the host; no fetch is needed.
- `git`: fetched from a Git remote.
- `local`: loaded from a local path, mainly for development.

### `url`

Optional string. Origin URL for Git sources. Usually omitted for non-Git sources.

### `ref`

Optional string. The requested tag, branch, or commit ref at install time. For branches, this is not the final pin; `commit` is the final pin.

### `commit`

Optional string. Resolved commit SHA for Git sources. Git sources should set it; internal and local sources may omit it.

### `tree_hash`

String. SHA-256 of the package tree at install time. Readers at least validate the `sha256:` prefix; installers should compute the real content hash before writing.

### `manifest_hash`

String. SHA-256 of the canonicalized package manifest. It detects manifest content changes within the same commit or path.

### `package_envelope_digest`

Optional string. Complete SHA-256 digest of the logical package envelope. Unlike `tree_hash`, it pins the relationship among the manifest, components, protocols, content roots, surfaces, and auxiliary artifacts.

### `component_pins`

Optional array. Each pin records an independent component ID, component artifact digest, behavior digest, and trust class. This lets the Assembly resolver reason about implementation identity without treating the package ID as the component ID.

### `protocol_profile_pins`

Optional array. Exact protocol ID, version, and profile selections implemented by the locked components.

### `content_roots`

Optional array of complete `ArtifactDescriptor` values. Content roots remain separate from component pins so that replacing an implementation does not implicitly replace immutable world/project/media data.

### `signed`

Boolean. Whether the source was GPG-signed and verified. This records install-time verification and does not grant extra authority by itself.

### `signed_by`

Optional string. Signing key fingerprint. Usually present when `signed = true`. Readers can compare it with manifest `requires[].minimum_signed_by` during audit.

### `installed_at_store`

String. Path in the immutable store. Implementations may use the Nix store, a Plurora CAS store, or a host-managed read-only directory.

### `granted_capabilities`

Array of strings. Capability permissions granted by the user at install time. Runtime must still perform normal permission and capability handle checks.

### `granted_network`

Array of strings. Network hosts or host patterns granted by the user at install time. This records the install-time grant and does not replace manifest `permissions.network`.

### `granted_secrets`

Array of strings. Secret refs granted by the user at install time. It records reference names only, never raw secrets.

### `requires`

Array. Resolved transitive dependency edges. In TOML, it may be encoded as `[[package.requires]]`.

## LockRequirement fields

### `id`

Dependency package id. It must point to another locked entry or to an internal package resolvable by the host.

### `constraint`

Original semver constraint. An empty string means any version. This field preserves the user or upstream manifest intent.

### `resolved_to`

Resolution result. Recommended format is `<package-id>@<version>`, though implementations may use another stable internal identifier if it maps to an entry in `package[]`.

## Transitive resolution

The installer starts from direct `requires` declarations in the profile manifest. Each package manifest may declare its own `requires`. After resolution, the lockfile should contain the complete closure and each `LockEntry.requires` should record that package's direct dependency edges.

This lets tools answer why a package was installed, which constraint caused the current version, and which downstream packages an upgrade may affect. The lockfile does not have to record every intermediate solver state, but it must be sufficient to reconstruct the final dependency graph.

## Drift detection

`plurora lockfile --check` will:

1. read the lockfile;
2. for each `LockEntry`:
   a. verify the store path exists;
   b. recompute `manifest_hash` and compare it with the lockfile;
   c. re-derive the package-envelope digest, component pins, protocol-profile pins, and content roots when present;
   d. recompute `tree_hash` and compare it with the lockfile;
3. report any drift.

Non-zero exit codes are for CI: drift = failure.

When reading a lockfile, the installer should canonicalize the current profile manifest again and compute its SHA-256. If it differs from the top-level `manifest_hash`, the profile manifest has drifted.

The default policy should fail closed and require the user to run install/update to regenerate the lockfile. Development mode may allow `--allow-drift`, but it must show a clear warning.

For each package, the installer may also recompute `tree_hash` and `manifest_hash`. A mismatch indicates store corruption, local path changes, or inconsistent fetch results, and the entry should be rejected.

## Compatibility

Within v1, changes are additive-only: new optional top-level fields, new optional LockEntry fields, new optional grant arrays, or new metadata that readers may ignore.

v1 must not delete fields, change requiredness, change hash semantics, rename enum values, or change `requires` graph semantics. Those breaking changes require a v2 schema namespace.

Readers should ignore unknown optional fields, but must not ignore an unknown `schema`. Writers should keep field order stable where possible to minimize review diffs.

## Example

```toml
schema = "plurora.lock.v1"
profile = "default"
generated_at = "2026-05-23T00:00:00Z"
manifest_hash = "sha256:profile"

[[package]]
id = "vendor/tool"
version = "1.2.3"
source = "git"
url = "https://example.com/vendor/tool.git"
ref = "v1.2.3"
commit = "0123456789abcdef0123456789abcdef01234567"
tree_hash = "sha256:tree"
manifest_hash = "sha256:manifest"
signed = true
signed_by = "0123456789ABCDEF0123456789ABCDEF01234567"
installed_at_store = "/store/vendor-tool"
granted_capabilities = ["model/live_call"]
granted_network = ["api.example.com"]
granted_secrets = ["secret_ref:env:API_KEY"]

[[package.requires]]
id = "plurora/core"
constraint = ">=1.0.0"
resolved_to = "plurora/core@1.0.0"
```
