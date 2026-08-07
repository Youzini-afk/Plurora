# Package Installation

> [English](./PACKAGE_INSTALLATION.en.md) · [中文](./PACKAGE_INSTALLATION.md)

Plurora's installation system lets users install capability packages and projects from GitHub or local paths while keeping the result reproducible, auditable, and reversible.
This guide covers the install flow, native/external project detection, manifest fields, lockfiles, filesystem conventions, and CLI usage.

## Goals

- Let ordinary users install a capability package with one command.
- Let authors declare package dependencies instead of copying profile fragments.
- Make install results reproducible through a lockfile.
- Require user consent for every newly expanded authority.
- Fail safe by default and avoid half-written profiles.
- Let uninstall/update remove profile references and collect unreferenced content-addressed stores.

## Design principles

- The kernel does not know git.
- Git is handled by `plurora/git-tools-lab` as a capability package over the `host.outbound.execute` boundary.
- Install orchestration lives in `plurora/install-lab`, not in the kernel.
- Default deny: HTTPS-only; reject `ssh://`, `git://`, and `file://`.
- Default deny: URLs must not contain username/password.
- Integrity: every package records commit, tree hash, and manifest hash.
- Signatures (optional): GPG signed-tag verification with key allowlists.
- Consistency: lockfile plus immutable content-addressed storage.
- Auditability: user-granted capabilities, network, and secrets are recorded in the lockfile.
- Consent: newly added or expanded authority prompts the user.
- No first-party privilege: installer, git tools, and integrity tools load through ordinary manifests.

## User flow

### Install

```bash
# Simple case
plurora install github.com/user/plurora-package

# Native project (repository root has project.yaml)
plurora install github.com/Youzini-afk/Plurora-Tavern

# Local native-project dogfood
plurora install ../YdlTavern --data-dir <data-dir> --profile <profile> -y

# Pinned version (recommended)
plurora install github.com/user/plurora-package#v1.2.0

# Local path (development)
plurora install ./packages/my-package

# Require a signed tag (release/controlled environments)
plurora install <url> --require-signed

# Non-interactive (CI)
plurora install <url> --yes

# Strict conformance gating
plurora install <url> --strict

# External project strategy
plurora install github.com/user/external-app --wrap-as-adapter
plurora install github.com/user/external-app --workspace-only
```

### Other commands

```bash
plurora list-installed [--profile <name>]
plurora project list
plurora project info <id>
plurora project status <id>
plurora project start <id>
plurora project stop <id>
plurora uninstall <package-id-or-project-id> [--profile <name>]
plurora update [<package-id>|--project-id <id>] [--check-only]  # Check/update through install-lab
plurora lockfile [--check]         # Verify lockfile and store consistency
```

### Profile and data dir

The default profile is `default`.
Use `--profile <name>` to operate on a different profile.
Use `--data-dir <path>` to override the data directory for tests and CI.

```bash
plurora install ./packages/dev --profile alpha --data-dir /tmp/plurora-alpha --yes
plurora list-installed --profile alpha --data-dir /tmp/plurora-alpha
```

Install-related flags:

- `--require-signed`: require a verifiable signed Git tag; signatures are not mandatory by default.
- `--strict`: block install on conformance failure; the default warns and continues.
- `--yes`: non-interactive approval for consent prompts.
- `--profile <name>`: choose the profile to update.
- `--data-dir <path>`: override the `~/.plurora` data directory for tests and CI.
- `--wrap-as-adapter`: for an external project, generate/use an adapter package.
- `--workspace-only`: for an external project, register it only as an agent workspace, without wrapping.


## Native vs external project detection

`plurora install <url>` first checks whether the source root contains `project.yaml`.

| Detection result | Behavior |
|---|---|
| Valid `project.yaml` with `project.type: plurora_native` | Install as a native Plurora project, copy the source into the store, resolve nested package manifests, write profile autoload entries, register in `ProjectRegistry`, write `~/.plurora/projects/<id>/`, copy project dist, and show a Home project card. |
| Present but invalid `project.yaml` | Fail closed and require descriptor fixes. |
| No `project.yaml` | Enter the external-project wizard. |

A native project's `project.yaml` references the package manifests it needs and declares `entry_surface_id`. That surface should be contributed by one of those packages, usually with `slot: experience_entry`.

The installed native-project path is: source → content-addressed store → nested manifests/profile autoload → project registry → project dist → internal `/surface-bundles/projects/<project_id>/...` → an authorized short-lived `/surface-assets/<lease>/...` handle. A package `surface_bundle` is a static/non-executing browser entry that describes the bundle, styles, fonts, and mount export for the iframe. It does not use the wasm sentinel and is not treated as an executable package entry.

See [`PROJECT_MODEL.md`](PROJECT_MODEL.en.md).

## External project wizard

An external project is a repository not written for Plurora. The installer shows detected language, package manager, entry points, and lifecycle risks, then asks the user to choose:

1. **Wrap with adapter**: generate an adapter package and connect the external project as a controlled capability or surface. Best for long-lived use.
2. **Workspace only**: register only as an agent workspace, with no wrapper. Best for temporary analysis, modification, or migration.
3. **Cancel**: do not install.

Without a TTY and without explicit flags, the default is `workspace-only`, so adapter code is not generated implicitly.

### `--wrap-as-adapter`

Forces the wrapping path. The installer uses external-project intake / adapter planning to produce an adapter manifest preview, and writes the project record after consent. The adapter is still an ordinary capability package with no kernel privilege.

### `--workspace-only`

Forces the workspace path. Plurora records source, workspace path, detection metadata, and future agent action policy. It does not claim the external project has become a Plurora capability package.

## Manifest `requires`

Packages declare dependencies in `manifest.yaml`:

```yaml
requires:
  - id: "plurora/composition-lab"
    source:
      kind: internal
    version: ">=1.0.0"

  - id: "third-party/cool-tool"
    source:
      kind: git
      url: "https://github.com/user/cool-tool"
      ref: "v1.2.0"
    version: ">=1.0.0"
    minimum_signed_by: ["FA9C5BC2..."]

  - id: "local/dev-helper"
    source:
      kind: local
      path: "../dev-helper"
```

Fields:

- `id`: package id; must match the resolved `manifest.id`.
- `source`: one of `internal`, `git`, or `local`.
- `version`: semantic version constraint such as `""`, `">=1.0.0"`, `"^2.1"`, or `"=1.2.3"`.
- `minimum_signed_by`: optional GPG fingerprint allowlist; requires a matching signature.

`requires` is install data. It does not grant runtime authority.
Runtime authority still comes from `permissions`, bindings, and capability handles.
`consumes` declares capability needs; `requires` declares package dependencies.

## Lockfile

Lockfile location:

```text
~/.plurora/profiles/<name>.lock.toml
```

See [`../spec/v1/LOCKFILE_FORMAT.md`](../spec/v1/LOCKFILE_FORMAT.en.md).

The lockfile records:

- profile name and manifest hash;
- each installed package id, version, source, ref, and commit;
- `manifest_hash` and `tree_hash`;
- store path;
- signature status and signing fingerprint;
- granted capabilities, network, and secrets;
- resolved direct dependency edges.

This lets tools answer:

- where a package came from;
- why the package is installed;
- whether the install still matches the lockfile;
- which downstream packages an update affects;
- which permissions the user has already approved.

## Filesystem layout

```text
~/.plurora/
├── store/              # Immutable content-addressed storage
│   ├── sha256-abc.../
│   └── sha256-def.../
├── profiles/           # Mutable profiles + lockfiles
│   ├── default.yaml
│   ├── default.lock.toml
│   └── alpha.yaml
├── keys/               # Trusted GPG public keys
│   └── trusted-keys.asc
└── cache/git/          # Git fetch cache
```

Data directory precedence:

1. `PLURORA_DATA_DIR`;
2. a Plurora directory under `XDG_DATA_HOME`;
3. `~/.plurora`.

CLI `--data-dir` has the highest precedence and is intended for tests, CI, and one-off demos.

## Detailed install flow

```text
plurora install github.com/user/repo#v1.0
            ↓
1. URL parsing (parse_install_url)
            ↓
2. Load existing lockfile (if present)
            ↓
3. install-lab.resolve_plan
   ├─ git-tools-lab.resolve_ref → commit_sha
   ├─ git-tools-lab.fetch_tree → temporary directory
   ├─ git-tools-lab.read_signed_tag → pgp_signature
   ├─ integrity-lab.compute_manifest_hash
   ├─ integrity-lab.compute_tree_hash
   ├─ integrity-lab.verify_gpg_signature (when signed)
   ├─ plurora-core::conformance::run_checks (static)
   └─ recursive manifest.requires (cycle detection)
            ↓
4. Show plan (human readable + signature state + integrity hashes)
            ↓
5. Consent prompt (new/expanded authority)
   ├─ TTY: interactive dialoguer prompt
   ├─ --yes: auto-approve
   └─ no TTY and no --yes: error
            ↓
6. install-lab.execute_plan
    ├─ verify consent covers planned authority
    ├─ fetch again into staging
    ├─ atomic rename into store
    ├─ for native projects, resolve nested manifests and copy project dist
    ├─ update profile YAML (atomic, autoloading package manifests)
    ├─ write the ProjectRegistry record
    └─ write lockfile (atomic)
            ↓
7. after host serve loads the profile, keep `/surface-bundles/projects/<project_id>/...` protected and issue a short-lived `/surface-assets/<lease>/...` handle to the sandbox after project authorization
            ↓
8. Done
```

## Security model

### HTTPS-only

Git URLs accept HTTPS by default only.
`ssh://`, `git://`, and `file://` are rejected.
URLs containing username/password are rejected so credentials cannot enter logs, audit, or lockfiles.

### Path validation

`fetch_tree` requires an absolute `dest_dir` with no `..` components.
Tree writing rejects dangerous entries such as `.git`, path separators, and parent-directory references.

### Atomic writes

All profile, lockfile, and store writes use tmp + rename.
A crash may leave a temporary directory, but store, profile, and lockfile should not be half-written.

### Content-addressed store and schema

`~/.plurora/store/` is content-addressed.
Once written, content is not mutated.
`tree_hash` covers `dist/` inside the package/project tree, so a browser surface bundle-only change gets a new hash. The store has a schema marker; when hash rules change, host layout initialization clears old store contents while preserving profiles and lockfiles, so the next update rebuilds the store.

Uninstall, install replacement, and project update collect orphaned store directories after profile / lockfile writes succeed. Content still referenced by another lockfile/profile is kept.

### Default safety baseline

The default behavior matches the technical baseline of package managers such as cargo/npm/pip: HTTPS-only, atomic writes, and content hashing are always on; signature verification and conformance gating are explicit opt-ins.

- HTTPS-only and URL credential rejection are always enabled.
- Content hashes (tree hash / manifest hash) are always recorded.
- Profile, lockfile, and store writes always use atomic writes.
- Signature verification is enabled with `--require-signed`.
- Conformance blocking is enabled with `--strict`.

### Signature verification

Git packages are not required to have GPG signed tags by default, but signature state is still recorded when present.
`minimum_signed_by` requires a specific fingerprint.
`--require-signed` requires a verifiable signature and fits release, controlled, or organizational policy environments.
The integrity tool uses `sequoia-openpgp` and supports common RSA / Ed25519 signing material.

### Conformance gating

Static v1 conformance checks run before install.
The default is warning-only: failures appear in the install plan but do not block installation.
`--strict` promotes conformance failures into install blockers for CI, releases, or organizational policy.

### API keys and secrets

Install records only the `secret_ref` authority the user consented to. It does not collect raw API keys.
For API key management, see [`SECRET_MANAGEMENT.md`](SECRET_MANAGEMENT.en.md): projects should prefer `secret_ref:project:*` (with policy fallback to `store`), desktop platform defaults should prefer `secret_ref:store:*`, and development/CI can keep using `secret_ref:env:*`.

### Consent audit

The lockfile fields `granted_capabilities`, `granted_network`, and `granted_secrets` record what the user approved.
Future installs or updates compare against existing grants and prompt only for new or expanded authority.

## Uninstall

```bash
plurora uninstall fixture/pkg-local
```

Uninstall will:

1. remove the package from profile YAML;
2. remove the corresponding lockfile entry;
3. keep store content;
4. atomically write profile and lockfile.

Uninstall does not delete dependencies still referenced by other packages.
Future dependency reverse lookup can warn when another package still needs the target package.

## Update

```bash
plurora update
plurora update third-party/cool-tool
plurora update --project-id my-project__abc12345 --check-only
```

CLI update routes through `plurora/install-lab/update_project`; `--check-only` calls `plurora/install-lab/check_for_updates`.
Update checks upstream refs, resolves a new plan, and reruns integrity, signature, conformance, and consent checks.
If authority does not change, the user does not repeat old approvals.
If network, secret, or capability authority expands, new consent is required.
Native project updates refresh lockfiles, profiles, the project descriptor, the project registry, and project dist; failures roll back from a snapshot. External workspace content itself is not updated by install-lab; adapter-package updates are follow-up polish.

## Drift detection

```bash
plurora lockfile --check
```

This command will:

1. read the lockfile;
2. verify each `LockEntry` store path exists;
3. recompute `manifest_hash` and compare it with the lockfile;
4. recompute `tree_hash` and compare it with the lockfile;
5. report any drift.

Non-zero exit codes are for CI: drift means failure.

## Implementation references

- `crates/plurora-core/src/manifest.rs` (`PackageDependency`, `DependencySource`)
- `crates/plurora-core/src/lockfile.rs` (`Lockfile`, `LockEntry`)
- `crates/plurora-core/src/paths.rs` (filesystem layout)
- `crates/plurora-core/src/conformance.rs` (reusable static checks)
- `crates/plurora-runtime/src/inproc/install_lab.rs` (orchestrator)
- `crates/plurora-runtime/src/inproc/git_tools_lab.rs` (gix-based git)
- `crates/plurora-runtime/src/inproc/integrity_lab.rs` (sequoia GPG + sha256)
- `crates/plurora-cli/src/commands/install.rs` (CLI entry)
- `crates/plurora-cli/src/install/consent.rs` (consent prompts)
- `crates/plurora-cli/src/install/url_parser.rs` (URL parsing)

## Conformance coverage

The current install foundation covers:

- git URL and path rejection;
- signed-tag fixture;
- tree hash, manifest hash, GPG verify, and fingerprint;
- resolve plan, execute plan, uninstall, list, lockfile drift, store GC, and store schema migration;
- `check_for_updates` / `update_project`, covering local project dist refresh, current noop, force reinstall, permission-drift blocking, and external not-applicable;
- transitive dependencies and cycle detection;
- conformance gating, strict blocking, lenient warning, and transitive propagation;
- `install.real_github_smoke`, the opt-in real GitHub smoke.

Default conformance does not use the network.
The real GitHub smoke requires explicit opt-in:

```bash
PLURORA_GIT_INSTALL_REAL_TESTS=1 cargo run -p plurora-cli -- conformance --case install.real_github_smoke
```

## Limits

- Sigstore keyless verification: deferred (no git-tag convention yet).
- Tauri UI install flow: deferred (CLI only).
- Central marketplace: not planned (against platform philosophy).
- Auto-update daemon: deferred (`plurora update` remains manual).
- Binary package distribution: deferred (source/git only).
- Cross-profile package sharing semantics: deferred.
- Standalone `plurora gc` command: not needed yet; install/update/uninstall already collect orphaned store entries automatically.

## Recommended practice

- Publish packages with immutable tags; avoid asking users to install floating branches.
- Use signed tags for GitHub packages.
- Pin upstream refs in `requires` and use clear version constraints.
- Run `plurora lockfile --check` in CI.
- Local development can use plain `plurora install <url>`; release or controlled environments can add `--require-signed` and `--strict` as needed.
- For Plurora-native experiences, prefer a root `project.yaml` over publishing only loose package manifests.
- Describe new network and secret authority with clear purposes so users can consent.
