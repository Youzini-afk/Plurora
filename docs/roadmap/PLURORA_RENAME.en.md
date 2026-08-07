# Plurora Destructive Rename Plan

> [English](./PLURORA_RENAME.en.md) · [中文](./PLURORA_RENAME.md)

> Status: temporary implementation plan. Delete this document when the rename is complete and move durable naming rules into architecture, contract, guide, and status documentation.
>
> Decision: the project name becomes **Plurora**. The repository is still in development, so the rename is a one-time breaking identity reset with no Yggdrasil compatibility layer, data migration, command aliases, protocol aliases, or old-format readers.

## Goal

This is not a UI-title replacement. It resets every public identity of the project:

- brand, repository, executables, installers, and release artifacts;
- Rust crates, TypeScript packages, SDKs, and source directories;
- environment variables, data directories, temporary names, containers, and Web storage keys;
- Package publisher namespaces, Protocol/Profile IDs, URNs, media types, lockfile schemas, and JSON Schema `$id` values;
- RPC methods, event kinds, generated SDKs, and Contract Registry;
- Web, Desktop, PWA, Docker, CI, examples, fixtures, and documentation;
- icons, wordmarks, and visual metaphors still tied to the world tree.

After completion, Git history is the only place that needs to retain the old name. The working tree, build products, and newly created data contain only the new identity.

## Breaking rules

### No compatibility

Explicitly do not:

- keep `ygg`, `yg`, or another old CLI entry;
- read `YGG_*` environment variables;
- probe or migrate `~/.yggdrasil`;
- read `yggdrasil.lock.v1`, `urn:yggdrasil:*`, `application/vnd.yggdrasil.*`, or old bundles;
- publish forwarding packages for `@yggdrasil/*`, `ygg-*`, or `yg-*` names;
- alias old RPC methods, event kinds, headers, Web storage keys, or file extensions;
- retain deprecated or legacy-adapter lifecycles;
- provide a `contract migrate` command for old IDs;
- document continued use of the old name.

Old development data is deleted. Old clients, Packages, SDKs, and exports are disposable development artifacts.

### Do not perform a blind global replacement

A breaking reset does not mean replacing every `ygg` token with `plurora`. Separate:

1. **brand identity**, which uses `Plurora / plurora / PLURORA`;
2. **architectural meaning**, which uses owner namespaces such as `host`, `context`, `journal`, `capability`, `authority`, `object`, `change`, `projection`, and `shell`.

RPC methods and event kinds should not all gain a `plurora.` prefix merely because the product has a brand. Globally unique Protocol, URN, media-type, publisher, and schema identities use `plurora`; domain operations use semantic owner names.

## Target naming map

### Product and repository

| Current | Target |
|---|---|
| `Yggdrasil` | `Plurora` |
| `Youzini-afk/Yggdrasil` | `Youzini-afk/Plurora` |
| `D:\project\Yggdrasil\Yggdrasil` | `D:\project\Plurora` |
| `Yggdrasil Host` | `Plurora Host` |
| `Yggdrasil Desktop` | `Plurora Desktop` |
| `Yggdrasil Web` | `Plurora Web` |
| `Yggdrasil Protocol Commons` | `Plurora Protocol Commons` |

GitHub may keep an automatic redirect for the old repository URL. That is GitHub behavior, not a compatibility route maintained by code or documentation.

### Rust workspace

| Current | Target directory / package |
|---|---|
| `crates/ygg-core` / `ygg-core` | `crates/plurora-core` / `plurora-core` |
| `crates/ygg-runtime` / `ygg-runtime` | `crates/plurora-runtime` / `plurora-runtime` |
| `crates/ygg-service` / `ygg-service` | `crates/plurora-service` / `plurora-service` |
| `crates/ygg-cli` / `ygg-cli` | `crates/plurora-cli` / `plurora-cli` |
| `ygg-desktop` | `plurora-desktop` |
| `ygg-tdb-rust-adapter` | `plurora-tdb-rust-adapter` |
| `yg-kernel-sdk` | `plurora-contract-sdk` |
| Rust imports such as `ygg_core` | corresponding `plurora_core` imports |

The generated public SDK becomes a Contract SDK rather than retaining the outdated `kernel-sdk` name.

### CLI and sidecar

| Current | Target |
|---|---|
| binary `ygg` | binary `plurora` |
| documented `yg` commands | `plurora` |
| `ygg-host` sidecar | `plurora-host` |
| `YGG_HOST_LISTEN_ADDR=` | `PLURORA_HOST_LISTEN_ADDR=` |
| `ygg host ...` / `yg ...` | `plurora host ...` / `plurora ...` |

Publish one public command only: `plurora`.

### TypeScript and npm

| Current | Target |
|---|---|
| `@yggdrasil/web` | `@plurora/web` |
| `ygg-desktop` | private `@plurora/desktop` |
| `@yggdrasil/kernel-sdk` | `@plurora/contract-sdk` |
| `@yggdrasil/subprocess` | `@plurora/subprocess` |
| `@yggdrasil/agentic-forge` | `@plurora/agentic-forge` |
| `@yggdrasil/experience-runtime` | `@plurora/experience-runtime` |
| `@yggdrasil/inference-capability` | `@plurora/inference-capability` |
| `sdk/typescript/ygg-agent-adapter` | `sdk/typescript/agent-adapter` / `@plurora/agent-adapter` |
| generated `KernelClient` | `PluroraClient` |
| generated `KernelMethods` | `PlatformMethods` |
| generated `KernelEvent` | `PlatformEvent` |

Reserve the npm scope and planned public crate names before implementation. Do not publish forwarding packages under old names.

### Data, environment, and local state

| Current | Target |
|---|---|
| `YGG_*` | `PLURORA_*` |
| `YGG_DATA_DIR` | `PLURORA_DATA_DIR` |
| `YGG_HOST_URL` | `PLURORA_HOST_URL` |
| `YGG_HTTP_ACCESS_TOKEN` | `PLURORA_HTTP_ACCESS_TOKEN` |
| `YGG_TARGET_AGENT_*` | `PLURORA_TARGET_AGENT_*` |
| `XDG_DATA_HOME/yggdrasil` | `XDG_DATA_HOME/plurora` |
| `~/.yggdrasil` | `~/.plurora` |
| `.yggdrasil-nixpacks` | `.plurora-nixpacks` |
| `ygg-*` temp/container/target names | `plurora-*` |

The new program only creates and reads Plurora paths. Developers delete old directories manually; the program does not import, move, or merge them.

### Desktop, PWA, and Web runtime

| Current | Target |
|---|---|
| Tauri `productName: Yggdrasil` | `Plurora` |
| `com.yggdrasil.desktop` | `io.github.youzini-afk.plurora` |
| window title | `Plurora` |
| sidecar `ygg-host` | `plurora-host` |
| `__YGG_RUNTIME__` | `__PLURORA_RUNTIME__` |
| `ygg-language` | `plurora-language` |
| `ygg-recently-opened` | `plurora-recently-opened` |
| `ygg-*` CSS classes and DOM IDs | `plurora-*` |
| `.ygg-change.json` | `.plurora-change.json` |
| `.ygg-world.json` | `.plurora-world.json` |
| world-tree icon files | new Plurora icon files and artwork |

The world-tree artwork is replaced, not merely renamed.

### Package publisher namespace

Replace first-party `official/*` IDs with `plurora/*`:

```text
official/install-lab        -> plurora/install-lab
official/package-lab        -> plurora/package-lab
official/model-provider-lab -> plurora/model-provider-lab
...
```

`plurora/*` identifies the publisher and grants no authority, routing priority, or hidden capability. Update every Manifest, profile, composition, fixture, template, test, and document in the same campaign.

### Globally unique machine identity

| Current | Target |
|---|---|
| `urn:yggdrasil:*` | `urn:plurora:*` |
| `application/vnd.yggdrasil.*` | `application/vnd.plurora.*` |
| `yggdrasil.lock.v1` | `plurora.lock.v1` |
| `ygg.contract.default/v1` | `plurora.contract.default/v1` |
| `ygg.shell.default/v1` | `plurora.shell.default/v1` |
| `ygg.change` | `plurora.change` |
| `ygg.change/default/v1` | `plurora.change/default/v1` |
| `ygg.world.bundle` | `plurora.world.bundle` |
| `ygg.runtime.*` | `plurora.runtime.*` |
| `x-ygg-*` | `x-plurora-*` |
| `ygg.*` WebSocket subprotocol | `plurora.*` |

Use domain-independent URNs for JSON Schema `$id` values:

```text
urn:plurora:schema:method:host.info:v1
urn:plurora:schema:event:host/project.started:v1
urn:plurora:schema:type:effect-receipt:v1
```

## Public contract reset

The rename removes the unpublished compatibility layer instead of reproducing it under a new brand.

### Remove

Remove:

- the `kernel.v1` legacy profile;
- all `kernel.v1.*` method aliases;
- all `kernel/v1/*` event kinds;
- `LEGACY_CONTRACT_PROFILE`;
- `ContractMaturity::LegacyAdapter` and alias lifecycle metadata;
- alias diagnostics such as `ygg.contract.alias.legacy_adapter`;
- generated legacy wrappers;
- the `contract migrate` command and implementation;
- the ad-hoc `GET /kernel/v1/host.info` route;
- tests and conformance cases that exist only to compare aliases;
- support-window and legacy-adapter documentation.

The Contract Registry remains, but records one canonical method, owner, version, Profile, maturity, and schema without aliases.

### Canonical method namespaces

Regenerate all 80 methods under owner-based IDs. Contract/Profile metadata owns versioning; method strings do not contain a brand or version.

| Current category | Target ID shape |
|---|---|
| Session | `context.open`, `context.close`, `context.fork`, `context.branch.list`, `context.get`, `context.list` |
| Event | `journal.append`, `journal.list`, `journal.subscribe` |
| Package | `host.package.load|unload|restart|logs|list|status|describe` |
| Project | `host.project.*` |
| Target / exec / port / proxy | `host.target.*`, `host.exec.*`, `host.port.*`, `host.proxy.*` |
| Capability | `capability.discover|describe|invoke|stream|cancel` |
| Capability handles | `authority.handle.attenuate|revoke|list` |
| Grants | `authority.grant.create|revoke|list`, `authority.decision.list` |
| Extension / hook | `protocol.extension.list|describe`, `protocol.hook.list` |
| Asset | `object.put|get|list` |
| Projection | `projection.register|rebuild|get|list` |
| Host identity | `host.info|ping|diagnostics`, `identity.current` |
| Package audit | `host.package.audit` |
| Proposal adapter | `change.proposal.*` |
| Surface | `host.surface.bundle.resolve`, `shell.contribution.list|describe` |
| Outbound | `host.outbound.audit|execute|stream|websocket.open|send|close` |

Create one machine-readable 80-entry old-to-new table before implementation. Dispatcher, exporter, OpenAPI, SDKs, and tests consume that source rather than duplicating mappings.

### Canonical event namespaces

Rename all 59 events to owner-based paths while keeping version in `schema_version`:

```text
kernel/v1/session.opened       -> context/opened
kernel/v1/package.ready        -> host/package.ready
kernel/v1/project.started      -> host/project.started
kernel/v1/asset.put            -> object/put
kernel/v1/proposal.approved    -> change/proposal.approved
kernel/v1/capability.completed -> capability/completed
kernel/v1/permission.denied    -> authority/denied
kernel/v1/outbound.request     -> host/outbound.request
kernel/v1/exec.started         -> host/exec.started
kernel/v1/deployment.health    -> host/deployment.health
```

Generate event filenames, constants, documentation, and SDK unions from one registry.

### Remove kernel-centric internal names

Rename internal types according to responsibility:

| Current | Target |
|---|---|
| `KernelMethod` | `PlatformMethod` |
| `KernelSession` | `SessionRecord` |
| `KernelEvent` | `EventEnvelope` or `PlatformEvent` by layer |
| `KernelEnv` | `ComponentEnv` |
| generated `KernelClient` | `PluroraClient` |
| `KernelOutboundStreamResponse` | `OutboundStreamResponse` |
| `KERNEL_PACKAGE_ID` | `PLATFORM_RUNTIME_ID` or a more specific name |

The rename changes identity and public boundaries without rewriting business state machines.

## Implementation sequence

Perform all work on `rename/plurora`. The branch may be temporarily broken; `main` receives only the complete green Plurora state.

### 1. Establish the rename map and red-line checker

Create one machine-readable map covering paths, Cargo, npm, CLI, environment, data, protocols, schemas, methods, events, publishers, Web identity, Docker, CI, and release names. Add a checker that initially permits only the explicit pending inventory and ends in zero-tolerance mode.

### 2. Rename the build graph

Use `git mv` for crates, SDKs, and adapters. Update workspace dependencies, Cargo names and imports, npm packages and lockfiles, codegen output directories, sidecar staging, Docker, release scripts, CI commands, cache keys, and artifact names.

This step ends with working `cargo metadata --no-deps`, npm dependency resolution, and codegen path discovery.

### 3. Reset contracts, schemas, and SDKs

Change `KernelMethod` to `PlatformMethod`, establish 80 method IDs and 59 event kinds, delete alias machinery and `contract migrate`, replace global identifiers, regenerate JSON Schema/OpenAPI/Rust SDK/TypeScript SDK, update subprocess reverse dispatch, the Web client, and conformance.

Generated files are rebuilt from exporter sources rather than hand-edited.

### 4. Rename the Package ecosystem

Change all first-party `official/*` identities to `plurora/*` across Manifests, capabilities, dependencies, profiles, compositions, catalogs, fixtures, templates, lockfiles, tests, and docs. Add no `official/*` aliases.

### 5. Rename Host, Web, Desktop, and visual identity

Change environment variables, data paths, backup names, targets, containers, labels, headers, WebSocket subprotocols, Web globals/storage/CSS/DOM IDs/download extensions, PWA metadata, Tauri identity, sidecar names, installers, icons, wordmarks, and About content. Smoke test only with a fresh data directory.

### 6. Rename documentation, repository, and external references

Update all repository documentation and comments. Then rename the GitHub repository and local directory, update `origin` and `safe.directory`, review Actions/releases/container registry/badges, and reserve registry names.

Do not mechanically rename Yggdrasil-Tavern or YdlTavern to PluroraTavern. Select a separate product name. Remove the old repository name from primary Plurora navigation until that independent rename is complete.

### 7. Final cleanup and merge

Delete old icons, generated files, SDK directories, empty paths, and temporary rename allowances. Regenerate lockfiles, schemas, SDKs, OpenAPI, and performance baselines. Move durable naming rules into permanent docs, delete this plan, and merge only a complete Plurora tree.

## Acceptance red lines

### Zero old identity

Outside `.git` history and dependency caches, all of these must return zero results:

```text
Yggdrasil
yggdrasil
@yggdrasil
YGG_
__YGG_RUNTIME__
~/.yggdrasil
yggdrasil.lock
urn:yggdrasil
application/vnd.yggdrasil
ygg.contract / ygg.shell / ygg.change / ygg.world
ygg- / ygg_ / yg-kernel-sdk
binary ygg
CLI text yg
kernel.v1
kernel/v1
LegacyAdapter
legacyKernelV1
legacy_kernel_v1
contract migrate
official/
```

Historical-name discussion is not retained in current documentation or the changelog; Git history already records it.

### Build and generation

Pass:

```text
cargo metadata --no-deps
cargo check --workspace
cargo test --workspace
schema export and validation
generated SDK clean check
Contract/OpenAPI ID uniqueness
Web npm check/test/build
Desktop sidecar build/smoke
Docker build and fresh-data startup
full conformance
Host operations acceptance
backup/restore on a fresh Plurora data directory
scripts/check-docs.py
git diff --check
```

Large Rust, Docker, Windows, and Desktop gates remain in GitHub CI; local checks stay bounded.

### Clean-room behavior

With an empty environment:

```text
PLURORA_DATA_DIR=<empty>
plurora host serve
plurora install ...
plurora project list/start/stop
Plurora Web/PWA connects
Plurora Desktop starts plurora-host
schema and SDK clients invoke canonical IDs only
```

The program never reads `YGG_*` or `.yggdrasil`, offers no migration prompt, and performs no silent fallback.

## Commit structure

Suggested reviewable commits on the branch:

```text
chore(rename): establish Plurora identity map
refactor(rename): rename Rust and Node workspace
refactor(contract): reset public method and event identities
refactor(ecosystem): rename first-party package namespace
feat(brand): replace Web Desktop and Host identity
docs(rename): complete Plurora documentation
chore(rename): enforce zero Yggdrasil residue
```

Do not add aliases just to keep intermediate commits green.

## Out of scope

- no new product features;
- no business-state-machine rewrite;
- no formal release;
- no user-data migrator;
- no compatibility tests for old Packages, clients, or protocols;
- no dependence on the occupied `.com` for machine identity;
- no rushed rename of the Tavern product before it has its own name.

## Definition of done

The rename is complete when:

1. users, developers, Package authors, and automation see only Plurora;
2. builds produce only Plurora binaries, packages, installers, directories, and artifacts;
3. the public contract has one canonical ID set with no Yggdrasil or `kernel.v1` dual stack;
4. new Hosts read only `PLURORA_*` and `~/.plurora`;
5. schemas, SDKs, OpenAPI, templates, and conformance share the same new identity;
6. the repository carries no compatibility debt for an unreleased user base;
7. this temporary plan is deleted and permanent docs describe only the current Plurora state.