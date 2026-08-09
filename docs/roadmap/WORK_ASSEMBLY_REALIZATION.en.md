# Work, Assembly, Installation, and Realization Implementation Design

> [English](./WORK_ASSEMBLY_REALIZATION.en.md) · [中文](./WORK_ASSEMBLY_REALIZATION.md)

> Status: candidate architecture and Codex execution brief. This is a one-time implementation plan for the development phase, not a Stable protocol commitment. Delete this document after all implementation phases finish, and converge durable semantics into architecture, spec, guide, and `ALPHA_STATUS` documents.

## 0. Direct instructions for the execution agent

Codex must use this document as the implementation brief. Before changing code, read these documents in order:

1. [`../CHARTER.en.md`](../CHARTER.en.md)
2. [`../architecture/VISION.en.md`](../architecture/VISION.en.md)
3. [`../architecture/ARCHITECTURE.en.md`](../architecture/ARCHITECTURE.en.md)
4. [`../architecture/CONSTITUTION_V2.en.md`](../architecture/CONSTITUTION_V2.en.md)
5. [`../architecture/CAPABILITY_PACKAGE.en.md`](../architecture/CAPABILITY_PACKAGE.en.md)
6. [`../product/PLATFORM_PRODUCT_MODEL.en.md`](../product/PLATFORM_PRODUCT_MODEL.en.md)
7. [`../ALPHA_STATUS.en.md`](../ALPHA_STATUS.en.md)
8. This document

Execution rules:

- Create `feature/work-assembly-realization` from the latest `main`. Do not accumulate a half-complete rewrite directly on `main`.
- Every Phase must end in a buildable, reviewable, independently recoverable commit, then be pushed to the remote branch immediately.
- Temporary breakage is acceptable inside a Phase. At the Phase boundary, the worktree must be clean and the target checks must pass.
- This is a pre-release destructive refactor. Do not retain Project, the old Composition model, old RPCs, old events, old directories, old readers, aliases, fallbacks, or migration code.
- Do not migrate development data under `~/.plurora/projects/`. New code recognizes only the new layout. Tests use fresh temporary data directories.
- Destructive contract work does not permit destructive filesystem behavior. Installation or Workspace removal must preserve existing path-containment checks, symlink defenses, and the rule that `linked_local` sources are never deleted.
- Never hand-edit generated schemas, OpenAPI, Rust SDK, or TypeScript SDK. Change the source registry and generators.
- Do not add private APIs, publisher priority, root shell access, or hidden authority for first-party Packages, the official Shell, or agents.
- Work, Assembly, Game, Library, and Realization are not Constitutional Substrate concepts. They belong to Candidate Protocols, the Host, or the official distribution.
- Do not modify YdlTavern. Do not work on releases, a marketplace, payments, DRM, or a general commercial entitlement system.
- Do not delegate architecture review to subagents. Bounded discovery is acceptable, but the current executor owns design interpretation, changes, review, and final verification.
- Run only bounded checks locally. Full Rust, Docker, Windows, Desktop, and Host operations acceptance belongs in GitHub CI.
- If context becomes insufficient, stop only at a complete Phase boundary and report the exact state. Never leave a large uncommitted partial migration.

Suggested commit subjects appear in each Phase. Unless repository facts make a requirement impossible, do not repeatedly ask about routine implementation details; choose the smallest design that satisfies this document's invariants.

Codex's first operational turn must not merely restate this document. Verify `git status`, the remote, latest `main`, current CI baseline, and relevant model locations; create the implementation branch; record the Phase 1 sources of truth and generation chain; then begin Phase 1. Code is authoritative for current implementation facts, while this document is authoritative for the target boundary. When an unforeseen conflict appears, preserve these invariants, choose the smallest reviewable implementation, and record the deviation in the Phase report.

Ready-to-paste Codex bootstrap instruction:

```text
Open the latest main in D:\project\Plurora. Read
`docs/roadmap/WORK_ASSEMBLY_REALIZATION.en.md` completely, including every prerequisite it lists.
Treat it as the target architecture and execution contract for this program; do not stop after replanning or summarizing it.
After confirming a clean worktree and the remote, create `feature/work-assembly-realization` and begin Phase 1 immediately.
For every Phase, run bounded local checks, commit and push independently, and wait for that commit's GitHub CI.
Fix a failing Phase before beginning the next one. Do not retain compatibility for Project, Composition, or Deployment,
do not modify YdlTavern, and do not give agents or first-party implementations private authority.
After every Phase succeeds, fast-forward main, delete the temporary plan and implementation branch,
and produce one final complete report.
```

Use this fixed Phase report shape:

```text
Phase and commit SHA
User capability and architectural boundary completed
Retired identity or debt removed
Bounded local checks
GitHub CI run and conclusion
Any deviation from this document and its reason
Input preconditions for the next Phase
```

## 1. The target system

Plurora should not grow into three loosely connected systems: a game engine, an automatic deployment platform, and a game launcher. Those use cases share one object and lifecycle chain:

```text
Mutable Source / Workspace
        ↓ build, resolve, encapsulate
Immutable WorkRevision
        ↓ references
Recursive AssemblyRevision
        ↓ resolve providers, authority, and state on one Host
InstallationRecord + AssemblyLock
        ↓ start
RunRecord
        ↓ when nodes require remote or durable machine resources
OperationalIntent
        ↓ compile against Target inventory
RealizationPlan
        ↓ approval and deterministic execution
RealizationRevision + EffectReceipts
```

The same model must accommodate:

- open games assembled from Rust, TypeScript, WASM, isolated processes, or remote services;
- reusable components or nested Assemblies extracted from complete works;
- ordinary open-source repositories that an agent helps understand and deploy;
- closed-source games that expose only a launch entry and save backup;
- closed-source binaries or remote services that participate through open protocols;
- works whose local client, remote server, storage, and model provider run on different Targets.

Plurora's distinctive value is not replacing Godot, Unity, Unreal, Docker, Kubernetes, Git, or stores. It unifies:

- what a work consists of;
- the public semantics that connect its parts;
- ownership of state;
- explicit authority;
- replaceable implementations;
- realization of the work across machines;
- which parts can be copied, exported, backed up, deployed, or remain opaque.

## 2. First principles

### 2.1 Definition, installation, execution, and machine realization are different objects

The following are not one object:

- **WorkRevision:** an immutable, portable definition of a work.
- **Installation:** a Host-local adoption of that work, with choices, authority, and user state.
- **Run:** a living execution with component instances, streams, leases, and health.
- **Realization:** the concrete mapping of Assembly nodes and bindings onto one or more Targets.

Updating a work does not overwrite user state. Stopping a Run does not delete an Installation. Stopping a Realization does not destroy the Work. Re-deployment does not re-interpret live source.

### 2.2 Identity is not location

Durable identity uses logical IDs and content digests, never:

- absolute local paths;
- temporary URLs;
- process IDs;
- Docker container names;
- Host database row IDs.

Paths, ports, containers, and processes exist only in local Installation, Run, Realization, and receipt records.

### 2.3 Source openness, composability, deployability, and portability are independent axes

Visible source does not imply a stable integration boundary. Closed source does not imply that composition is impossible.

```text
Source visibility
× Protocol participation
× State portability
× Rebuildability
× Redistribution rights
× Runtime trust class
```

Record these axes independently. The current `plurora_native / external_wrapped / external_workspace` enum must no longer carry all of them.

### 2.4 Semantics, control plane, and data plane are separate

- A **Protocol** defines meaning, lifecycle, errors, authority, and behavior.
- The **control plane** discovers, selects, authorizes, activates, stops, recovers, and audits.
- The **data plane** carries actual high-frequency data.

Per-frame game state, audio, GPU buffers, and large entity synchronization must not be forced through JSON-RPC. Public Capability calls remain suitable for control, configuration, saves, AI, assets, and lower-frequency operations. Bindings may negotiate WASM direct calls, IPC, shared memory, WebSocket, QUIC, engine-native bridges, or other transports for data-plane traffic.

### 2.5 Plans and effects are separate

An agent, planner, or UI can create a candidate plan, but a plan grants no authority. Execution follows:

```text
Intent → Plan / ChangeSet → PolicyDecision → Effect → Receipt
```

A deterministic Host control plane performs external effects. An agent never receives a permanent root shell and never improvises deployment semantics on every run.

### 2.6 Every mutable state has an explicit owner

Every state declaration answers:

- who owns it;
- whether its scope is Run, Installation, User, Shared, or External;
- whether it has a schema or is opaque;
- whether it can be backed up, exported, merged, or migrated;
- who performs migration when a component is replaced.

Updating a component must never silently overwrite user content.

### 2.7 Recursive composition is stronger than “host plus plugins”

An Assembly may contain Components or another Assembly, then re-export selected internal Ports as its own Ports. The containment graph is acyclic; runtime message flow may contain feedback loops when the adopted protocols permit them.

A complete backend, editor toolchain, or gameplay subsystem can therefore be encapsulated as a higher-level unit without exposing every internal node to consumers.

## 3. Layer ownership

| Concept | Owning layer | It is not |
|---|---|---|
| `ArtifactDescriptor`, authority, journal, effects, invoke, stream | Constitutional Substrate | game or deployment product semantics |
| Work / Assembly / Port / Binding / State Slot | Experimental Protocol Commons | substrate |
| Package Envelope / Component artifact | Components and distribution | the identity of a Work |
| Installation / Run / Exposure / binding lease | Host Control Plane | portable Work data |
| OperationalIntent | portable artifact referenced by a Work | a Docker backend request |
| Target inventory / RealizationPlan / RealizationRevision | Host Control Plane | substrate |
| Library, Affordance, Play/Edit/Deploy actions | official distribution | universal protocol law |
| Foreign launch adapters and store entitlement adapters | Component / Adapter | platform DRM |

Add two Experimental protocols:

```text
plurora.work       profile plurora.work/experimental/v1
plurora.assembly   profile plurora.assembly/experimental/v1
```

They constrain only participants that adopt those Profiles. Do not add them to the Stable Constitutional Substrate list.

## 4. Common vocabulary

### Work

The durable logical identity that creators and users recognize. `WorkId` is a naming identity, not a version or content identity.

### WorkRevision

An immutable, content-addressed revision of a work. It references an Assembly, content roots, entrypoints, rights, provenance, and optional OperationalIntent. Its digest is the exact revision identity.

### Workspace

A mutable authoring or import area. It may come from Git, a user directory, an agent-managed copy, or a generator. A Workspace is not a Work and never enters a portable runtime lock.

### Component

An independently identifiable implementation unit that provides behavior or static resources. Reuse the existing `ComponentDescriptor`, trust class, artifact digest, and protocol implementation foundations.

### Port

A typed connection that a Component or Assembly exports or imports. A Port expresses semantics and interaction category without fixing one transport.

### Binding

A decision that connects an export Port to an import Port. A Binding may be fixed at authoring, installation, launch, or runtime.

### Assembly

A recursive graph of Components and nested Assemblies, including nodes, bindings, exposed Ports, and state slots.

### AssemblyLock

The exact immutable resolution of an Assembly in a specific context. It pins node artifacts, behavior digests, protocol Profiles, providers, bindings, and content roots.

### Installation

A Host-local adoption of a WorkRevision. It owns the AssemblyLock, user choices, authority, secret policy, state bindings, and update source.

### Run

One execution of an Installation. An Installation has at most one default Run in the MVP, without making future parallel Runs impossible.

### Exposure

A record that explicitly exposes one Assembly export from an Installation to another Installation or principal. An Exposure has audience, lease, resource scope, and revocation.

### OperationalIntent

Portable requirements for execution: workloads, resources, connections, state, endpoints, health, update policy, and placement constraints. It does not contain concrete ports, process IDs, or container IDs.

### TargetInventory

A time-bounded Host observation of a Target's capabilities, capacity, topology, trust zone, and availability.

### RealizationPlan

An immutable result of compiling AssemblyLock + OperationalIntent against TargetInventory. It contains concrete placement, transport, build, launch, route, state, and authority requirements, but has not executed them.

### RealizationRevision

A concrete attempted or active machine realization. It references the Plan, parent revision, actual resources, receipts, health, and activation state.

## 5. Identity and content rules

### 5.1 ID types

Define and validate these types in the new crate:

```rust
WorkId              // namespace/name
AssemblyId          // namespace/name
NodeId              // local within one Assembly
PortId              // local within one Component or Assembly
StateSlotId         // local within one Assembly
InstallationId      // Host-local opaque ID
RunId               // Host-local opaque ID
ExposureId          // Host-local opaque ID
BindingId           // Host-local opaque ID
RealizationId       // Host-local opaque ID
```

Rules:

- Logical IDs and digests are different fields.
- `WorkId` and `AssemblyId` follow the same safe namespace/name grammar as Package IDs, but are not Package ID aliases.
- Local IDs allow ASCII letters, digits, `-`, `_`, and `.`, and reject path separators, `..`, and shell-special characters.
- Host-local IDs use random UUIDs or equivalent unpredictable IDs. Never derive them directly from titles, paths, or user input.
- Every portable cross-Host reference carries a complete `ArtifactDescriptor`, or at minimum a verifiable digest, not only a logical ID.

### 5.2 Canonical artifacts

Authoring files may use YAML. Work, Assembly, Lock, Intent, Rights, and Plan objects stored in ObjectStore use canonical JSON.

A content digest must not include:

- build-machine absolute paths;
- current time;
- random temporary IDs;
- unsorted maps;
- raw secrets;
- actual ports or process details.

Time, builder identity, source commit, signatures, and provenance attach through separate attestations or receipts so they do not contaminate a reproducible semantic digest.

### 5.3 Artifact types

Add at least:

```text
urn:plurora:work-revision:v1
urn:plurora:assembly-revision:v1
urn:plurora:assembly-lock:v1
urn:plurora:rights-declaration:v1
urn:plurora:transparency-declaration:v1
urn:plurora:operational-intent:v1
urn:plurora:realization-plan:v1
urn:plurora:foreign-capsule:v1
```

Unknown artifact types remain copyable, storable, and exportable. Unknown semantics prohibit execution and automatic authority, not preservation.

## 6. Normative MVP data model

Implementation may split modules differently, but must not hide these semantics in a generic `metadata` map.

### 6.1 WorkRevision

```rust
pub struct WorkRevision {
    pub schema: String,                       // plurora.work-revision.v1
    pub work_id: WorkId,
    pub title: String,
    pub description: String,
    pub assembly: ArtifactDescriptor,
    pub content_roots: Vec<ArtifactDescriptor>,
    pub entrypoints: Vec<WorkEntrypoint>,
    pub rights: Option<ArtifactDescriptor>,
    pub transparency: Option<ArtifactDescriptor>,
    pub operational_intent: Option<ArtifactDescriptor>,
    pub annotations: BTreeMap<String, Value>,
}

pub struct WorkEntrypoint {
    pub id: String,
    pub intent_uri: String,
    pub target: WorkEntrypointTarget,
    pub annotations: BTreeMap<String, Value>,
}

pub enum WorkEntrypointTarget {
    AssemblyPort { port_id: PortId },
    Surface { surface_id: String },
    ForeignLaunch { launch_id: String },
}
```

`intent_uri` is an extensible hint, not a substrate enum. The official Shell may understand Profile vocabulary such as `play`, `open`, `edit`, and `inspect`. Unknown intents remain renderable as generic actions.

### 6.2 PortDescriptor

```rust
pub struct PortDescriptor {
    pub port_id: PortId,
    pub contract: PortContract,
    pub interaction: InteractionModelId,
    pub role: PortRole,
    pub transport: TransportRequirements,
    pub annotations: BTreeMap<String, Value>,
}

pub struct PortContract {
    pub protocol_id: String,
    pub interface_id: String,
    pub version: String,              // exact on export; semver requirement on import
    pub profiles: Vec<String>,
}

#[serde(transparent)]
pub struct InteractionModelId(pub String);

pub enum PortRole {
    Import {
        multiplicity: PortMultiplicity,
        latest_binding_phase: BindingPhase,
        availability: AvailabilityPolicy,
        accepted_effects: Vec<EffectClass>,
    },
    Export {
        multiplicity: PortMultiplicity,
        effect_class: EffectClass,
    },
}

pub struct PortMultiplicity {
    pub min: u16,
    pub max: Option<u16>,
}

pub enum BindingPhase {
    Authoring,
    Installation,
    Launch,
    Runtime,
}

pub enum AvailabilityPolicy {
    Required,
    DegradedWithout,
    Optional,
}

pub enum EffectClass {
    Pure,
    DeterministicStateful,
    RecordedNondeterministic,
    ExternalEffecting,
    RealtimeBestEffort,
}
```

Initial known `InteractionModelId` values are `plurora.interaction.capability-unary/v1`, `capability-stream/v1`, `event-stream/v1`, `duplex-stream/v1`, `artifact/v1`, `snapshot/v1`, and `endpoint/v1`. The wire shape is an open namespaced string rather than a closed enum: unknown values survive faithful reading and transfer, but cannot bind or execute without a declared implementation or Adapter.

An Import's `latest_binding_phase` is the latest point at which selection must complete; fixing it earlier is allowed. Effect classes have no implicit total order. An Import explicitly lists accepted classes instead of numerically comparing semantics such as `realtime_best_effort` and `external_effecting`.

`TransportRequirements` expresses constraints such as same-process, local-only, ordered, reliable, latency class, large payload, or shared-memory allowed. It never stores a concrete socket path.

Existing `provides` declarations automatically project to export Capability Ports, and `consumes` declarations project to import Capability Ports. Explicit Ports group protocol interfaces and represent non-Capability data planes; existing Manifests do not duplicate identical declarations.

### 6.3 AssemblyRevision

```rust
pub struct AssemblyRevision {
    pub schema: String,                       // plurora.assembly-revision.v1
    pub assembly_id: AssemblyId,
    pub nodes: Vec<AssemblyNode>,
    pub bindings: Vec<AssemblyBinding>,
    pub exposed_ports: Vec<AssemblyPortExposure>,
    pub state_slots: Vec<StateSlotDescriptor>,
    pub annotations: BTreeMap<String, Value>,
}

pub struct AssemblyNode {
    pub node_id: NodeId,
    pub source: AssemblyNodeSource,
    pub ports: Vec<PortDescriptor>,
    pub configuration: Option<ArtifactDescriptor>,
    pub annotations: BTreeMap<String, Value>,
}

pub enum AssemblyNodeSource {
    Component { component: ArtifactDescriptor },
    Assembly { assembly: ArtifactDescriptor },
}

pub struct PortEndpoint {
    pub node_id: NodeId,
    pub port_id: PortId,
}

pub struct AssemblyBinding {
    pub binding_id: String,
    pub provider: PortEndpoint,
    pub consumer: PortEndpoint,
    pub phase: BindingPhase,
    pub transport_policy: TransportPolicy,
    pub annotations: BTreeMap<String, Value>,
}

pub struct AssemblyPortExposure {
    pub port_id: PortId,
    pub direction: PortDirection,
    pub target: PortEndpoint,
    pub annotations: BTreeMap<String, Value>,
}
```

Validation rules:

- node IDs, binding IDs, and exposed Port IDs are unique;
- Component-node Port contracts are part of canonical AssemblyRevision content; changing a version, profile, interaction, transport, or effect changes the Assembly, Work, and Lock digests. Nested Assemblies expose their own boundary Ports instead of duplicating inline Ports;
- the nested Assembly content closure is complete and the containment graph is acyclic;
- provider endpoints are exports and consumer endpoints are imports;
- protocol, interface, version, Profile, interaction, and multiplicity are compatible;
- the consumer explicitly accepts the provider effect class; there is no implicit risk ordering;
- authoring-time bindings resolve completely during Work packing;
- an unbound required import enters a WorkRevision only when its binding phase is later than authoring;
- an import cannot exceed its maximum cardinality;
- unknown interactions or transport requirements never silently downgrade.

### 6.4 StateSlotDescriptor

```rust
pub struct StateSlotDescriptor {
    pub state_slot_id: StateSlotId,
    pub owner_node_id: NodeId,
    pub schema_ref: Option<ArtifactDescriptor>,
    pub scope: StateScope,
    pub portability: StatePortability,
    pub migration_port: Option<PortEndpoint>,
    pub backup_policy: BackupPolicy,
    pub annotations: BTreeMap<String, Value>,
}

pub enum StateScope {
    Run,
    Installation,
    User,
    Shared,
    External,
}

pub enum StatePortability {
    Portable,
    OpaqueExportable,
    HostBound,
    ExternalAuthority,
}

pub enum BackupPolicy {
    Required,
    Allowed,
    Forbidden,
}
```

Rules:

- `Portable` requires a `schema_ref`.
- `HostBound` and `ExternalAuthority` state cannot be advertised by a Work export as portable content.
- Replacing a node that owns durable state requires a migration Port or an explicit state-reset decision when schema or behavior is incompatible.
- Raw filesystem paths never enter WorkRevision or AssemblyLock.

### 6.5 AssemblyLock

```rust
pub struct AssemblyLock {
    pub schema: String,                       // plurora.assembly-lock.v1
    pub assembly: ArtifactDescriptor,
    pub nodes: Vec<NodeLock>,
    pub bindings: Vec<BindingLock>,
    pub protocol_profiles: Vec<ProtocolProfilePin>,
    pub content_roots: Vec<ArtifactDescriptor>,
}
```

`NodeLock` pins node ID, component or nested Assembly artifact, behavior digest, and trust class. `BindingLock` pins provider, consumer, concrete provider Component, transport class, and phase.

Delete the current `CompositionLock` and replace it with `AssemblyLock`. Update World Bundle, lockfile, schemas, SDKs, conformance, and documentation together. Do not retain a reader or alias.

### 6.6 RightsDeclaration and TransparencyDeclaration

Rights declarations are auditable publisher or acquisition-source claims, not legal judgments. Each declaration may reference signatures or attestations.

```rust
pub enum RightDisposition {
    Allowed,
    Denied,
    RequiresEntitlement,
    Unspecified,
}

pub struct RightsDeclaration {
    pub license_expression: Option<String>,
    pub terms_uri: Option<String>,
    pub install: RightDisposition,
    pub execute: RightDisposition,
    pub backup: RightDisposition,
    pub export_state: RightDisposition,
    pub copy_across_hosts: RightDisposition,
    pub redistribute_artifacts: RightDisposition,
    pub modify: RightDisposition,
    pub derive: RightDisposition,
    pub modding: RightDisposition,
    pub dedicated_server: RightDisposition,
    pub entitlement_requirements: Vec<ProtocolRequirement>,
    pub evidence_refs: Vec<ArtifactDescriptor>,
}

pub struct TransparencyDeclaration {
    pub source_visibility: SourceVisibility,
    pub source_refs: Vec<ArtifactDescriptor>,
    pub reproducible_build_claim: ClaimStatus,
    pub sbom_refs: Vec<ArtifactDescriptor>,
    pub provenance_refs: Vec<ArtifactDescriptor>,
    pub signature_refs: Vec<ArtifactDescriptor>,
    pub telemetry_disclosures: Vec<String>,
    pub state_portability: StatePortability,
    pub evidence_refs: Vec<ArtifactDescriptor>,
}
```

The UI distinguishes a declaration, verified evidence, and boundaries actually enforced by the Host. Closed source is not an error state. Unknown rights, opaque state, missing provenance, and trusted-native execution remain visible. A Rights declaration is not a capability; it drives conservative Host and distribution policy. `Denied` or `Unspecified` blocks unattended copy, export, and redistribution by default. Any user override is a separate auditable product-policy decision, never presented as a platform legal judgment.

### 6.7 InstallationRecord

```rust
pub struct InstallationRecord {
    pub schema_version: u16,
    pub installation_id: InstallationId,
    pub work_revision: ArtifactDescriptor,
    pub assembly_lock: ArtifactDescriptor,
    pub display_name: String,
    pub source: AcquisitionRecord,
    pub state_bindings: Vec<StateBindingRecord>,
    pub secret_policy: InstallationSecretPolicy,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub status: InstallationStatus,
}
```

An Installation is a mutable Host-owned record and is not content-addressed by its own identity. It stores no raw secret, arbitrary absolute path, or replayable plaintext credential.

### 6.8 RunRecord

```rust
pub struct RunRecord {
    pub run_id: RunId,
    pub installation_id: InstallationId,
    pub context_id: Option<String>,
    pub status: RunStatus,
    pub node_instances: Vec<NodeInstanceRecord>,
    pub bindings: Vec<ActiveBindingRecord>,
    pub started_at: DateTime<Utc>,
    pub stopped_at: Option<DateTime<Utc>>,
    pub health: RunHealth,
}
```

`host.run.start` never implicitly deploys missing nodes. If a required node lacks a usable Realization or Binding, return structured gaps and actionable next steps.

### 6.9 Exposure and runtime Binding

```rust
pub struct ExposureRecord {
    pub exposure_id: ExposureId,
    pub installation_id: InstallationId,
    pub run_id: Option<RunId>,
    pub export_port: PortId,
    pub audience: Vec<ResourceSelector>,
    pub expires_at: Option<DateTime<Utc>>,
    pub status: ExposureStatus,
}

pub struct ActiveBindingRecord {
    pub binding_id: BindingId,
    pub consumer_installation_id: InstallationId,
    pub consumer_port: PortId,
    pub exposure_id: ExposureId,
    pub authority_handle_id: String,
    pub transport: SelectedTransport,
    pub expires_at: Option<DateTime<Utc>>,
}
```

Every cross-Installation connection uses an explicit Exposure. Candidate providers include only exports visible to the current principal and compatible with the requested Protocol. A Binding response returns an opaque ID; the Host injects the underlying capability handle into the Component and never exposes it to an untrusted UI.

### 6.10 OperationalIntent

```rust
pub struct OperationalIntent {
    pub schema: String,
    pub workloads: Vec<WorkloadIntent>,
    pub endpoints: Vec<EndpointIntent>,
    pub state: Vec<StatePlacementIntent>,
    pub placement: Vec<PlacementConstraint>,
    pub update_policy: UpdatePolicy,
    pub annotations: BTreeMap<String, Value>,
}
```

`WorkloadIntent` references an Assembly node and does not contain a generic Docker command. It describes:

- permitted execution classes;
- CPU, memory, GPU, duration, and concurrency needs;
- network, filesystem, and secret imports;
- replica and restart expectations;
- health interface;
- hard and soft placement constraints.

### 6.11 TargetInventory

Evolve the current `ExecutionTarget` from a closed enum into namespaced capability records:

```rust
pub struct TargetCapabilityRecord {
    pub capability_id: String,
    pub version: String,
    pub properties: BTreeMap<String, Value>,
    pub available: bool,
    pub evidence_refs: Vec<ArtifactDescriptor>,
}

pub struct TargetInventorySnapshot {
    pub target_id: String,
    pub observed_at: DateTime<Utc>,
    pub capabilities: Vec<TargetCapabilityRecord>,
    pub capacity: ResourceCapacity,
    pub labels: BTreeMap<String, String>,
    pub trust_zone: String,
    pub topology: Vec<TopologyRelation>,
}
```

Project existing `LocalExec`, `ArtifactTransfer`, and `Deployment` capabilities through an adapter into namespaced records. Do not keep two permanent systems; delete the old enum after migration.

### 6.12 RealizationPlan and RealizationRevision

```rust
pub struct RealizationPlan {
    pub schema: String,
    pub installation_id: InstallationId,
    pub work_revision: ArtifactDescriptor,
    pub assembly_lock: ArtifactDescriptor,
    pub operational_intent: ArtifactDescriptor,
    pub inventory_refs: Vec<ArtifactDescriptor>,
    pub placements: Vec<NodePlacement>,
    pub transports: Vec<TransportBindingPlan>,
    pub build_actions: Vec<BuildAction>,
    pub launch_actions: Vec<LaunchAction>,
    pub state_actions: Vec<StateAction>,
    pub endpoint_actions: Vec<EndpointAction>,
    pub preconditions: Vec<ChangePrecondition>,
    pub required_authority: Vec<String>,
    pub risk_summary: Vec<String>,
}

pub struct RealizationRevision {
    pub realization_id: RealizationId,
    pub installation_id: InstallationId,
    pub plan_ref: ArtifactDescriptor,
    pub parent_realization_id: Option<RealizationId>,
    pub status: RealizationStatus,
    pub actual_resources: Vec<RealizedResource>,
    pub receipts: Vec<ArtifactDescriptor>,
    pub health: RealizationHealth,
    pub created_at: DateTime<Utc>,
    pub activated_at: Option<DateTime<Utc>>,
}
```

The planner is pure: identical input references and policy produce identical Plan bytes. The executor accepts only a persisted Plan whose digest verifies and whose authority remains current.

### 6.13 Bounded MVP decisions

To prevent the execution agent from expanding the first version into a distributed-platform rewrite, the MVP fixes these boundaries:

- one Host owns mutable authority for an Installation; multi-Host cooperation uses Targets, Exposures, and Artifact transfer rather than a distributed-consensus database;
- one Installation has one active WorkRevision / AssemblyLock pointer and one default Run; parallel Runs do not enter the first contract;
- Resolver performs deterministic bounded resolution over explicit candidates and does not implement a global ecosystem SAT solver;
- authoring graph changes create a new Work or Assembly revision; runtime mutability is limited to Bindings explicitly declared as Runtime;
- initial data planes implement only current Capability, Artifact, and Endpoint primitives; other interactions remain faithfully describable but are not presented as supported;
- the first Realization backend reuses current Docker, local, and Agent capability rather than constructing a general scheduling cluster;
- catalog discovery is separate from Installation: browsing a Work grants no authority, downloads no executable artifact, and creates no Run.

### 6.14 Lifecycle, idempotency, and uncertain outcomes

Initial state machines:

```text
Installation
resolving → ready → updating → ready
     └────→ blocked / failed
ready → removing → removed

Run
starting → running ↔ degraded → stopping → stopped
    └────→ failed / interrupted

Exposure
active → expired / revoked

Binding
selected → active → expired / revoked / broken

RealizationRevision
planned → applying → active ↔ degraded → stopping → stopped
             └────→ failed / outcome_unknown / recovery_required
```

Rules:

- `removed`, `revoked`, `expired`, and `stopped` for a specific revision are terminal. Recovery creates a new record or revision and never rewrites historical terminal state.
- Every public method that can create durable mutation or an external effect accepts an `idempotency_key`. Same key plus same fingerprint replays the known result; same key plus different fingerprint returns conflict.
- A terminal event commits once. Repeated stop, revoke, and remove return idempotent results.
- cancellation, timeout, denial, partial completion, and success are distinct terminal states.
- When an effect was emitted but its result cannot be confirmed, enter `outcome_unknown` or `recovery_required`; never guess success or failure.
- Active Work/Lock, Run, and Realization pointers update through CAS or parent preconditions so stale callers cannot overwrite newer state.
- A failed Binding does not automatically switch providers. An explicit policy may produce a candidate rebind only when the new provider does not expand authority, effect, or rights risk; the actual switch still creates a new Binding record.

### 6.15 Run-bound and managed Realization

All execution ultimately has concrete placement, but the product must not present every local launch as “deployment.” The MVP distinguishes:

- **Run-bound Realization:** starts only installed, verified artifacts on the current Host or a preselected local Target; its lifecycle is tied to the Run; it builds no source, creates no public route, and changes no durable machine resource across Runs. `host.run.start` may deterministically generate and execute it within existing run authority.
- **Managed Realization:** includes building, remote Targets, persistent workloads, shared endpoints, public routes, cross-Run resources, or complex placement; it requires a persisted `RealizationPlan` followed by explicit `apply`.

Both produce receipts, health, and actual-resource records. `host.run.start` cannot silently promote a missing managed Realization into deployment; it returns a structured gap. RunRecord references the Realization revisions it uses instead of copying machine details.

## 7. Work authoring format

A source repository uses two explicit files instead of putting every concern into `project.yaml`:

```text
work.yaml
assembly.yaml
```

Example:

```yaml
# work.yaml
schema: plurora.work-source.v1
work:
  id: example/modular-simulation
  title: Modular Simulation
  description: A rule-driven simulation assembled from replaceable components.
  assembly: assembly.yaml
  entrypoints:
    - id: play
      intent_uri: plurora.shell.default/play
      surface_id: example/simulation-ui/play
  content:
    - content/
  rights: rights.yaml
  operational_intent: operation.yaml
```

```yaml
# assembly.yaml
schema: plurora.assembly-source.v1
assembly:
  id: example/modular-simulation-main
  nodes:
    - id: simulation
      component: packages/simulation/manifest.yaml#main
    - id: save
      component: packages/save/manifest.yaml#main
    - id: ui
      component: packages/ui/manifest.yaml#main
  bindings:
    - id: save-binding
      from: {node_id: save, port_id: save-export}
      to: {node_id: simulation, port_id: save-import}
  exposed_ports:
    - id: play
      direction: export
      target: {node_id: ui, port_id: play}
```

Port endpoints use explicit `{node_id, port_id}` objects on the source wire. Both local ID classes allow `.`, so the source format does not use an ambiguous `node.port` shorthand.

`plurora work pack`:

1. safely opens source files and rejects symlink escape, traversal, and oversized files;
2. reads Package Envelopes and Component Descriptors;
3. imports content into ObjectStore;
4. produces AssemblyRevision;
5. resolves authoring-time bindings;
6. produces WorkRevision;
7. prints digest, closure, and diagnostics without installing or running anything.

### 7.1 Normalize every source into a Work

The Host no longer maintains permanently separate installation ontologies for Packages, native projects, external repositories, and closed-source games. Before an item enters Library or Installation, normalize it as follows:

- source with `work.yaml`: pack normally into WorkRevision;
- Package Manifest only: synthesize a single-node Assembly and WorkRevision without changing the original Component identity;
- ordinary source repository: create a Workspace first and produce only inspection, BuildGraph, and Work candidates; visible source alone does not make it an executable Work;
- external URI, local executable, OCI image, or remote service: create a ForeignCapsule WorkRevision and keep the concrete location only in an Installation-local binding;
- content-only bundle: create a Work with no executable nodes and an Open, Compose, or Inspect entrypoint.

An uninstalled catalog item may have only a WorkRevision. Once it enters a Host's daily lifecycle, it has an Installation. Acquisition sources may remain diverse while installation, authority, state, Run, and Realization stop duplicating type branches.

### 7.2 Distribution envelopes do not own Work identity

WorkRevision is the root of an Artifact DAG and may be acquired through a local archive, Git, OCI artifact, Package/registry, or remote service. Acquisition coordinates belong in `AcquisitionRecord`, not in the Work content identity. Packages primarily distribute Components; Works primarily reference and compose Components. One archive may carry both, but neither becomes the permanent ontology of the other.

Offline sharing may define a Work Bundle that only transports a WorkRevision closure. It reuses ArtifactDescriptor, integrity, and provenance without creating a new substrate type or dependency on an official registry.

## 8. Resolver and recursive Assembly

### 8.1 Port compatibility

Compatibility requires all of the following:

- matching protocol ID;
- matching interface ID;
- export exact version satisfies the import requirement;
- export Profiles cover every required import Profile;
- interaction models are directly compatible or an explicit adapter exists;
- transport constraints intersect;
- provider effect class belongs to the explicit consumer and policy acceptance set;
- multiplicity remains within bounds.

Matching a capability string alone is insufficient.

### 8.2 Adapters

An Adapter is an ordinary Component. It explicitly imports one Port, exports another, and declares conversion behavior and effects. The resolver never hides automatic schema coercion.

### 8.3 Recursive flattening

The resolver may produce a flattened graph for execution, but preserves:

- original nested Assembly identity;
- node paths such as `backend/matchmaking/store`;
- exposed Port mappings at every boundary;
- provenance and lock references.

Execution may be flat. User-facing inspection and audit must retain encapsulation boundaries.

### 8.4 Binding phases

- `Authoring`: fixed during Work packing and included in WorkRevision.
- `Installation`: chosen by resolver, user, or policy and written into AssemblyLock.
- `Launch`: chosen before every Run. Preferences may be saved, but availability and authority are revalidated.
- `Runtime`: connected through Exposure and lease while running; disconnection follows the import's availability policy.

### 8.5 Powerbox

The official product Powerbox flow is:

```text
A Component requests an import
→ Host computes protocol-compatible, visible, authorized Exposures
→ Shell shows provider, origin, trust, scope, duration, data, and effect risk
→ user or explicit policy selects one
→ Host mints the least-authority handle
→ runtime injects the Binding
→ expiry, revocation, provider stop, or protocol drift invalidates it
```

When no candidate exists, show how to install, start, or implement a provider. When several exist, never choose by publisher priority.

### 8.6 Update, replacement, and rollback

A WorkRevision is never modified in place. Installation update resolves a candidate WorkRevision and candidate AssemblyLock, then produces a structured diff covering:

- Component artifact and behavior digest;
- Ports, Protocols/Profiles, and Bindings;
- content roots;
- Rights and Transparency;
- authority, network, filesystem, secret, and effect requests;
- StateSlot schema, owner, portability, and migration;
- OperationalIntent and impact on current Realizations.

Update executes through a ChangeSet: verify source/digest and current Installation preconditions, create required state snapshots, run explicit migration, validate the candidate Run or Realization, then atomically switch the active Work/Lock pointer. Failure preserves the old pointer; a successful switch retains bounded rollback information. New permissions, more restrictive rights, state reset, and opaque migration each require a separate visible decision.

A Component replacement activates only when Port contracts remain compatible, required Bindings can be satisfied, and durable state has been migrated or explicitly reset. Remote provider version drift, Exposure expiry, and authority revocation invalidate a Binding and never silently rewrite the Installation lock.

## 9. Capability control plane and high-frequency data plane

MVP support:

- `CapabilityUnary` and `CapabilityStream`: reuse current invoke, stream, cancel, and handles.
- `Endpoint`: reuse port leases, proxy, and authenticated tunnel, but bind them to a Realization and Port.
- `Artifact` and `Snapshot`: transfer ObjectStore descriptors instead of embedding large content in RPC.

Later transports:

- EventStream;
- DuplexStream;
- local IPC;
- WASM Component direct binding;
- shared memory or ring buffer;
- engine-native bridge.

Every transport preserves principal, authority, deadline, cancellation, effect, and audit semantics. A Rust trait or in-process pointer is never the public protocol.

## 10. Deployment is compilation, not an agent shell

### 10.1 Build discovery

For an ordinary open-source repository, an agent may produce:

- `SourceInspectionReport`;
- candidate `BuildGraph`;
- candidate `OperationalIntent`;
- risks, unknowns, and questions requiring a user decision.

These are Artifacts and cannot execute directly. They become inputs to WorkRevision or ChangeSet only after approval.

### 10.2 BuildGraph

BuildGraph identifies a builder through a provider Protocol, not an unrestricted shell string:

```text
source inputs
→ build node (builder protocol + typed parameter artifact)
→ verification node
→ immutable outputs
→ provenance / receipt
```

MVP shape:

```rust
pub struct BuildGraph {
    pub schema: String,
    pub source_inputs: Vec<ArtifactDescriptor>,
    pub nodes: Vec<BuildNode>,
    pub outputs: Vec<BuildOutputDeclaration>,
}

pub struct BuildNode {
    pub node_id: String,
    pub builder_port: PortContract,
    pub input_refs: Vec<ArtifactDescriptor>,
    pub parameter_ref: ArtifactDescriptor,
    pub network_policy: BuildNetworkPolicy,
    pub expected_outputs: Vec<String>,
    pub verification_requirements: Vec<ProtocolRequirement>,
}

pub struct BuildOutputDeclaration {
    pub output_id: String,
    pub artifact_type_uri: String,
    pub media_type: String,
    pub executable: bool,
    pub protocol_claims: Vec<ProtocolImplementationDeclaration>,
}
```

BuildGraph contains no raw shell script, secret value, or mutable Workspace path. A concrete builder adapter may translate typed parameters into fixed argv or a sandbox action. Actual source snapshots and outputs bind through artifact digests.

Initial builder adapters:

- existing Dockerfile builder;
- existing Nixpacks path;
- prebuilt artifact passthrough.

Cloud Native Buildpacks, Cargo, npm, Godot export, and WASM Component build can arrive later as providers. They are not substrate enums.

### 10.3 Planner / Builder / Verifier / Operator / Observer

Agent coordination is separated by authority:

| Role | May do | Must not do |
|---|---|---|
| Planner | read allowed source and state; draft Work, Assembly, Plan, and ChangeSet | perform machine effects |
| Builder | transform approved inputs into artifacts inside a constrained environment | change Installation authority |
| Verifier | run checks and produce evidence | activate a new revision |
| Operator | invoke Host effects from an approved RealizationPlan | change the Plan itself |
| Observer | read health, logs, and receipts; draft recovery advice | automatically gain deploy authority |

One model may serve several roles, but each invocation has a different principal, handle, budget, and effect boundary.

### 10.4 Migration of existing deployment capability

Keep the capability of current DeploymentRevision, Docker build/deploy, Target Agent operations, private preview, approval, reconcile, recover, and rollback, while changing ownership:

- Project ID becomes Installation ID;
- Docker-specific request becomes a backend action inside RealizationPlan;
- DeploymentRevision becomes RealizationRevision;
- the fixed target capability enum becomes TargetInventory;
- route, port, and container fields move to actual resources and receipts;
- verified ChangeSet, preview, approval, and verification references remain evidence.

The first executor may support only a single-workload Docker or Agent Realization, but the data model must not make Docker the permanent only form.

## 11. Foreign Work and closed-source entrypoints

### 11.1 Do not add a “closed-source project type”

Closed-source integration is derived from:

- source visibility;
- artifact availability;
- rights;
- protocol Ports;
- state portability;
- trust class;
- entitlement requirements.

### 11.2 ForeignCapsule

```rust
pub struct ForeignCapsuleDescriptor {
    pub capsule_id: String,
    pub launch_requirements: Vec<ForeignLaunchRequirement>,
    pub protocol_ports: Vec<PortDescriptor>,
    pub state_slots: Vec<StateSlotDescriptor>,
    pub rights: ArtifactDescriptor,
    pub transparency: ArtifactDescriptor,
}
```

A portable Work cannot embed an absolute path to a user's local executable. An Installation satisfies a launch requirement through a Host-local binding:

- external URI;
- installed local executable;
- managed binary artifact;
- OCI image;
- remote service;
- store or entitlement adapter.

### 11.3 Derived integration depth

The UI may derive these labels from facts:

- External Link;
- Managed Capsule;
- Protocol Participant;
- Composable Work.

Do not store this as a substrate level enum. A closed-source program that implements open save, health, mod, or lobby protocols can compose normally. An open-source program without a protocol may still be only a Foreign Capsule.

### 11.4 No platform DRM

Plurora invokes ordinary entitlement adapters and records allowed operations and failures. It does not copy an external store's ownership database or turn DRM into a substrate requirement.

## 12. Official product: an affordance-based Library

Home no longer treats every item as a Project card. The official Library reads Work, Installation, Run, Rights, Target, and current authority, then derives available actions:

```text
Open
Play
Edit
Fork
Compose
Install
Update
Run
Stop
Deploy
Expose
Connect
Inspect
Backup
Export
Remove
```

`LibraryAffordanceResolver` belongs in official client-core:

```rust
pub struct Affordance {
    pub action: String,
    pub available: bool,
    pub reason_code: Option<String>,
    pub risk: Option<String>,
    pub next_step: Option<String>,
}
```

It improves UX and does not replace Host authorization. Every actual request still fails closed.

Examples:

| Item | Possible actions |
|---|---|
| Plurora-native open game | Play, Edit, Fork, Compose, Deploy, Export |
| closed-source local game | Play, Inspect, Backup |
| open-source Web service | Edit, Run, Deploy, Expose |
| remote closed service | Connect, Inspect, Disconnect |
| shared map or rule pack | Open, Compose, Export |
| running multiplayer backend | Inspect, Expose, Stop, Rollback |

Simple mode shows primary actions. Advanced mode expands Work digests, Assembly graph, bindings, authority, rights, Realizations, and receipts.

## 13. Destructive Public Contract reset

### 13.1 Remove

Remove methods:

```text
host.project.list
host.project.get
host.project.start
host.project.stop
host.project.status
```

Remove events:

```text
host/project.installed
host/project.started
host/project.stopped
host/project.uninstalled
```

Delete `ProjectDescriptor`, `ProjectRegistry`, `ProjectState`, `ProjectType`, `project.yaml`, `~/.plurora/projects/`, Project CLI commands, Web Project DTOs, and corresponding schemas, SDK output, tests, and documentation.

Delete current `CompositionDescriptor`, `CompositionLock`, `composition.yaml`, `init-composition`, and `composition check`. Work and Assembly tooling replaces their functionality.

Do not retain aliases, data readers, directory fallback, or deprecation output.

### 13.2 Add Installation and Run methods

```text
host.installation.list
host.installation.get
host.installation.create
host.installation.update
host.installation.remove

host.run.list
host.run.get
host.run.start
host.run.stop
host.run.status
```

### 13.3 Add Exposure and Binding methods

```text
host.exposure.list
host.exposure.create
host.exposure.revoke

host.binding.list
host.binding.candidates
host.binding.select
host.binding.revoke
```

### 13.4 Add Realization methods

```text
host.realization.plan
host.realization.apply
host.realization.get
host.realization.list
host.realization.stop
host.realization.rollback
host.realization.reconcile
```

### 13.5 Add events

```text
host/installation.created
host/installation.updated
host/installation.removed

host/run.starting
host/run.started
host/run.stopping
host/run.stopped
host/run.failed

host/exposure.created
host/exposure.revoked
host/exposure.expired

host/binding.selected
host/binding.revoked
host/binding.expired

host/realization.planned
host/realization.applying
host/realization.active
host/realization.stopped
host/realization.failed
host/realization.rolled_back
host/realization.reconciled
```

Every ID appears once. Update `PlatformMethod`, registry, dispatcher, schema exporter, OpenAPI, Rust and TypeScript SDKs, Public Contract, event registry, identity gate, and conformance together. Do not create a dual stack.

### 13.6 Host actions and resource selectors

Reset Project-centered fixed authorization into auditable actions plus exact resource selectors:

| Action | Typical methods | Required resource |
|---|---|---|
| `observe` | list/get/status/candidates | visible Work, Installation, Run, Target, and Realization |
| `installation.manage` | Installation create/update/remove | Installation, plus acquisition/source scope during creation |
| `run` | Run start/stop | exact Installation and Run |
| `binding.manage` | Binding select/revoke | consumer Installation, import Port, and Exposure |
| `exposure.manage` | Exposure create/revoke | provider Installation, Run, and export Port |
| `realization.plan` | Realization plan | Installation and candidate Target |
| `realization.apply` | apply/stop/rollback/reconcile | Installation, Target, and Realization |
| `develop.propose` | Work/Workspace ChangeSet draft | exact Workspace or Installation |
| `develop.approve` | approve/reject | exact ChangeSet |
| `develop.execute` | apply/promote | exact ChangeSet, Workspace or Installation, and effect resources |

Resource kinds include at least `work`, `workspace`, `installation`, `run`, `target`, `exposure`, `binding`, and `realization`. A default device invitation still grants only `observe`. Wildcard selectors are explicit and visible; an omitted ID never becomes a wildcard. Long operations refresh grant, ancestor delegation, lease, and resource match before every external effect, including build, state migration, endpoint creation, activation, and rollback.

The root credential remains a Host maintenance entry and is never injected into a Component, Surface, or agent. Every agent role receives only its task's actions and exact resources.

### 13.7 Structured failure reasons

Public errors and diagnostics distinguish at least:

```text
work_invalid
assembly_cycle
artifact_missing
artifact_digest_mismatch
port_unresolved
port_incompatible
binding_ambiguous
binding_unavailable
binding_expired
unsupported_interaction
state_migration_required
state_reset_required
rights_blocked
entitlement_required
authority_denied
target_unsatisfied
plan_stale
plan_digest_mismatch
approval_required
outcome_unknown
recovery_required
unsupported_backend
```

Reason codes are stable and localizable and contain no exception text, absolute path, secret, or raw stderr. Detailed diagnostics use redacted evidence and references. `absent`, `forbidden`, `unsupported`, `stale`, and `unavailable` never collapse into the same empty list.

## 14. Host data layout and authority

New layout:

```text
~/.plurora/
├── objects/
├── installations/
│   └── <installation_id>/
│       ├── installation.json          # materialized projection, not sole authority
│       ├── assembly.lock.json
│       ├── secrets.dat
│       ├── state/
│       └── diagnostics/
├── workspaces/
│   └── <workspace_id>/
│       ├── workspace.json
│       └── source/
└── runtime/
    └── ...
```

Authority rules:

- WorkRevision, AssemblyRevision, Lock, Intent, and Plan live in ObjectStore.
- Durable Installation, Exposure, Binding, and Realization authority lives in the Host journal. JSON files are rebuildable projections.
- Active Run state lives in memory plus journal terminal records. After Host restart, incomplete Runs become interrupted; the Host never guesses success.
- User state lives in an Installation state root or external provider and never in a Work artifact.
- A linked-local Workspace path is a Host-local binding and never enters portable WorkRevision.

## 15. Code organization

### Current repository migration map

| Current source of truth | Target ownership |
|---|---|
| `crates/plurora-core/src/project.rs` | delete; portable definitions move to `plurora-work`, Host-mutable data becomes Installation |
| `crates/plurora-runtime/src/project_registry.rs` | journal and projection in `plurora-service/src/installations.rs` |
| `crates/plurora-core/src/component.rs::CompositionLock` | `plurora-work::AssemblyLock` |
| `crates/plurora-cli/src/cli.rs::CompositionDescriptor` and `commands/composition.rs` | `work.yaml` / `assembly.yaml` reader, resolver, and Work CLI |
| `crates/plurora-runtime/src/runtime/protocol/projects.rs` | Installation and Run protocol handlers |
| `crates/plurora-service/src/development.rs` | retain ChangeSet/evidence core; scope changes from Project to Installation/Workspace |
| `crates/plurora-service/src/lib.rs::DeploymentRevision` and deployment projection | split into `realization/` and become RealizationRevision |
| `crates/plurora-runtime/src/target_deployment.rs` | first Docker/local backend executor for Realization |
| `crates/plurora-runtime/src/runtime/local_exec.rs::ExecutionTarget` | TargetInventory source and adapter for current capabilities |
| `clients/web/src/routes/home/use-home-projects.ts` | Library query and Affordance resolver |
| `clients/web/src/routes/project-frame.tsx` | Work entry and Installation frame |
| `clients/web/src/lib/project-deployment.ts` and `client-core/project-target-context.ts` | Realization client and Installation/Run context |
| `CompositionLock` in World Bundle | AssemblyLock while retaining historical replay versus re-execution separation |

Move ownership and types before UI naming. Do not construct another parallel implementation inside `plurora-service/src/lib.rs`.

### New crate

```text
crates/plurora-work/
├── src/lib.rs
├── src/ids.rs
├── src/canonical.rs
├── src/port.rs
├── src/assembly.rs
├── src/work.rs
├── src/state.rs
├── src/rights.rs
├── src/operational.rs
├── src/lock.rs
├── src/source.rs
├── src/resolver.rs
└── src/diagnostic.rs
```

Requirements:

- depend only on `plurora-core` and pure data/parsing libraries;
- do not depend on runtime, service, Web, Docker, or local data directories;
- make parsing, canonicalization, validation, resolution, and planning helpers pure and unit-testable;
- do not place Host mutable records in this crate except shared wire DTOs when unavoidable.

### Runtime

```text
crates/plurora-runtime/src/component_ports.rs
crates/plurora-runtime/src/assembly_runtime.rs
crates/plurora-runtime/src/binding_runtime.rs
```

### Service

Do not further enlarge `plurora-service/src/lib.rs`:

```text
crates/plurora-service/src/installations.rs
crates/plurora-service/src/runs.rs
crates/plurora-service/src/exposures.rs
crates/plurora-service/src/bindings.rs
crates/plurora-service/src/realization/mod.rs
crates/plurora-service/src/realization/planner.rs
crates/plurora-service/src/realization/executor.rs
crates/plurora-service/src/realization/projection.rs
```

### CLI

```text
plurora work init|check|pack|inspect
plurora installation list|info|create|update|remove
plurora run list|info|start|stop
plurora binding candidates|select|list|revoke
plurora realization plan|apply|list|info|stop|rollback|reconcile
```

### Web

Evolve:

```text
Project card         → Library item
Project frame        → Work / Installation entry frame
Project console      → Installation workbench
Project deployment   → Realization panel
Project context      → Installation / Run / Realization context
```

The UI may still use “project” as ordinary user-facing language where it is clearer, but DTOs, methods, and persisted identities use the accurate model.

## 16. Implementation phases

### Phase 1 — Candidate model and canonical artifacts

Implement:

- new `plurora-work` crate and all pure types from sections 5–6;
- ID validation, canonical JSON, digesting, model validation, raw-secret and path redlines;
- Experimental protocol descriptors;
- schema exporter and SDK exposure for new top-level types;
- update the identity gate with the exact new top-level schema set without loosening method or event redlines;
- no Project or Composition runtime changes yet.

Checks:

- canonical bytes are stable across map order;
- Work and Assembly digests are reproducible;
- inclusion cycle, invalid Port, invalid state, raw secret, and raw path are rejected;
- unknown annotations survive round trips;
- `cargo test -p plurora-work` passes;
- two schema/SDK generations have identical hashes.

Commit:

```text
feat(work): define portable work and assembly model
```

### Phase 2 — Resolver, Work authoring, and Composition replacement

Implement:

- `work.yaml` and `assembly.yaml` source readers;
- Work normalization for Packages, ordinary source repositories, Foreign Capsules, and content-only sources;
- Package and Component artifact import;
- capability-to-Port projection;
- recursive resolver, Port compatibility, and adapter diagnostics;
- `plurora work init|check|pack|inspect`;
- `AssemblyLock`;
- convert current composition examples, creator templates, and World Bundle;
- delete CompositionDescriptor, CompositionLock, old CLI commands, and old documentation.

Checks:

- a nested Assembly can encapsulate and re-export a Port;
- replacement preserves content roots;
- unresolved installation/launch/runtime imports produce structured diagnostics;
- ambiguous providers are never selected by publisher;
- World Bundle uses AssemblyLock while preserving replay and branching semantics.

Commit:

```text
feat(assembly): resolve recursive work assemblies
```

### Phase 3 — Installation registry and Project backend replacement

Implement:

- Installation journal, projection, and filesystem layout;
- Installation create, update, and remove, including candidate Work/Lock diff, state snapshot/migration, and rollback pointer;
- separate Workspace from Installation;
- Install Lab emits WorkRevision plus Installation;
- new `host.installation.*` methods;
- CLI Installation commands;
- delete ProjectDescriptor, ProjectRegistry, Project methods, events, and schemas.

Safety:

- linked-local source is never deleted;
- managed Workspace containment remains enforced;
- keep/delete state decision is explicit;
- old projects directory is never read.

Commit:

```text
feat(host): replace projects with installations
```

### Phase 4 — Run lifecycle and official Library

Implement:

- RunRegistry, node activation, and fixed Installation bindings;
- `host.run.*` and new lifecycle events;
- Web Home becomes Library;
- Affordance resolver;
- entrypoint launch;
- Settings, storage, secrets, and failure UI use Installation ID;
- remove Project machine naming from UI DTOs and routes.

Checks:

- opening an item never implicitly deploys it;
- missing required Realization produces an actionable next step;
- closing a browser tab does not stop the Run;
- stop, failure, restart, and recovery are clear;
- mobile/PWA and device authority use exact Installation selectors.

Commit:

```text
feat(product): introduce installation library and run lifecycle
```

### Phase 5 — Exposure, Powerbox, and cross-Installation Binding

Implement:

- ExposureRegistry with lease and revocation;
- candidate calculation;
- Powerbox methods and Web chooser;
- runtime handle injection;
- invalidation on provider stop, revocation, expiry, or version drift;
- saved preferences are hints, with launch-time revalidation.

Checks:

- no ambient service discovery;
- consumer receives only the selected Port's least-authority handle;
- cross-Installation state is never shared automatically;
- first-party and third-party providers are equal;
- multiple candidates require explicit selection.

Commit:

```text
feat(binding): add scoped cross-installation powerbox
```

### Phase 6 — Realization compiler and deployment refactor

Implement:

- OperationalIntent, TargetInventory, and pure planner;
- `host.realization.*` methods;
- migrate current Docker and Agent executor, preview, approval, reconcile, recover, and rollback;
- DeploymentRevision becomes RealizationRevision;
- Project selector becomes Installation selector;
- Web deployment panel becomes Realization panel;
- CLI Realization commands.

Checks:

- Plan digest is stable;
- planner performs no effects;
- apply revalidates Plan digest, preconditions, approval, and authority immediately before effects;
- Host restart recovers or fails closed;
- local and Agent Targets use the same Plan and receipt contract;
- rollback does not read live Workspace;
- Docker remains only the first backend.

Commit:

```text
feat(realization): compile assemblies onto execution targets
```

### Phase 7 — Foreign Work, Rights, and closed-source entrypoints

Implement:

- Rights, Transparency, and ForeignCapsule;
- external URI, local executable binding, managed artifact, OCI image, remote service, and entitlement adapter;
- Library trust and rights disclosure;
- opaque state backup;
- dedicated server entrypoint;
- portable descriptors cannot contain local absolute executable paths.

Checks:

- open source without protocols may be a Capsule;
- closed source with protocols may participate in Binding;
- denied or unspecified rights prevent automatic copy and export;
- entitlement failure leaks no credential;
- platform has no DRM shortcut.

Commit:

```text
feat(foreign-work): support rights-aware opaque entries
```

### Phase 8 — First game kit suitable for continued creation

Rebuild existing playable-creation capability as a useful `modular-simulation` Work:

- Rust isolated simulation Component;
- static Web renderer;
- input Port;
- portable save provider;
- optional AI provider;
- inspector/editor Surface;
- local Run;
- optional remote server Realization;
- Work fork, Component replacement, and state migration.

Do not build a general 3D engine. Choose a rule-driven, state-rich simulation, board, strategy, or management starter that benefits from branching and composition without extreme per-frame requirements.

Add a minimal “promote to reusable component” flow: select an Assembly subgraph, compute external imports/exports/state, and generate a candidate nested Assembly plus diagnostics. The agent does not automatically publish it.

Commit:

```text
feat(creator): ship a modular simulation work kit
```

### Phase 9 — Convergence

- delete this document and its Chinese counterpart;
- delete the temporary active-program root `AGENTS.md`, or replace it only with durable general repository instructions;
- move durable semantics into architecture and spec;
- update Product Model, guides, `ALPHA_STATUS`, and `NEXT_STEPS`;
- remove all Project, Composition, and Deployment machine identities;
- ordinary English “project” prose may remain only where it no longer names an old DTO;
- complete generation and CI;
- fast-forward merge to `main`;
- delete the feature branch.

Commit:

```text
docs(platform): converge work assembly and realization model
```

## 17. Definition of done for every Phase

Before each Phase commit:

```text
cargo fmt --check
cargo metadata --no-deps
target crate tests
affected TypeScript tests and typecheck
python scripts/check-docs.py
python scripts/check-identity.py
git diff --check
generated-output clean check when relevant
```

Push, then observe GitHub CI. Large checks run only in CI:

```text
cargo check --workspace
cargo test --workspace --all-targets --locked
full conformance
Web typecheck/test/build/PWA
Desktop sidecar smoke
External project Host operations acceptance
Windows backup/restore
Docker verified artifact / realization smoke
```

When CI fails, fix the current Phase with an explicit fix commit before starting the next Phase.

## 18. Required quality coverage

### Model

- canonicalization;
- separation of logical ID and digest;
- nested closure;
- unknown artifact preservation;
- separation of rights/transparency claims and evidence.

### Resolver

- version, Profile, and interaction mismatch;
- inclusion cycle;
- cardinality;
- authoring, installation, launch, and runtime phases;
- adapter path;
- ambiguous provider;
- state migration requirement.

### Host

- Installation journal rehydration;
- interrupted Run after restart;
- Exposure lease and revocation;
- exact resource authority;
- linked-local source preservation;
- keep/delete state;
- no raw secrets.

### Realization

- deterministic Plan;
- stale inventory and preconditions;
- authority refresh before effects;
- partial effect and unknown outcome;
- replay without live source;
- rollback;
- local and Agent parity;
- Target capability mismatch.

### Product

- Affordance reason codes;
- combinations of closed/open and composable/opaque;
- simple mode and advanced details;
- mobile remote control;
- Host-switch cache isolation;
- failure recovery.

## 19. Security threats and resource budgets

### 19.1 Required threat coverage

| Threat | Required control |
|---|---|
| malicious Assembly creates recursion, reference, or candidate explosion | inclusion-cycle check, hard depth/node/Binding/candidate limits, iterative resolution, structured limit reason |
| dependency confusion or publisher impersonation | logical ID plus exact digest, acquisition/provenance separation, no publisher priority, explicit provider |
| Component falsely claims Protocol or trust guarantees | separate claim/evidence/enforced-boundary presentation, behavior conformance, actual trust class |
| cross-Installation Binding becomes a confused deputy | exact consumer/provider/Port/audience/lease binding, Host-only handle injection, invocation-time revalidation |
| stale inventory, Plan, or approval creates TOCTOU | inventory reference, Plan digest, parent/CAS precondition, authority refresh before every effect |
| source or logs prompt-inject an agent into execution | treat external text as untrusted data; agent emits candidate Artifacts/ChangeSets and has no ambient shell or secret |
| Foreign binary escapes or receives excessive access | explicit ForeignCapsule/trust class, OS boundary, least file/network/secret grants, no false composition guarantees |
| replacement Component reads or exfiltrates state | StateSlot owner, migration Port, no automatic sharing, outbound policy, and receipts |
| Rights or Transparency self-report is deceptive | separate declaration and evidence; visibly mark unverified state; declaration never becomes platform authority |
| Target impersonates capability or completion | authenticated Target identity, Host-observed inventory, lease epoch, verifier evidence, typed receipt |
| artifact/archive bomb or disk exhaustion | decoded-size, closure, object, transfer, expansion-ratio, and storage quotas checked before writing |
| errors or logs leak secrets and local machine details | reason codes, redaction, reference-based diagnostics, no raw stderr or absolute paths |

### 19.2 Initial hard limits

These are implementation safety limits, not permanent protocol maxima. A Profile may lower them; a caller cannot disable them. Exceeding a limit returns `work_too_complex` or `artifact_budget_exceeded`. A later implementation version may raise them:

```text
single YAML source descriptor             1 MiB
single canonical metadata artifact        4 MiB (large content uses separate references)
nodes per Assembly                        4,096
bindings per Assembly                     16,384
maximum nesting depth                     32
ports per node                            1,024
exposed ports / state slots               4,096 each
provider candidates per resolution        256
actions per RealizationPlan               4,096
diagnostics returned per operation        1,000 (plus a truncation count)
```

Resolver uses digest memoization and bounded concurrency and never recursively copies an entire nested graph. Large Artifact transfer is streamed with backpressure, cancellation, digest verification, and pre-write budget checks. Performance optimization never bypasses validation or authority.

## 20. Explicit non-goals

This program does not build:

- a general 3D or 2D rendering engine;
- platform-standard ECS, scene, entity, inventory, or quest ontology;
- a Kubernetes replacement;
- marketplace, payments, or recommendation systems;
- DRM or an official entitlement database;
- an arbitrary root-shell agent;
- per-frame JSON-RPC;
- a claim that any open-source repository is automatically composable;
- compatibility for old Project or Composition data;
- every possible Target backend in one program;
- additional `*-lab` Packages merely to increase feature count.

## 21. Final acceptance scenarios

### Scenario A — Compose an open game

1. Pack a WorkRevision from `work.yaml`.
2. Assembly contains a Rust simulation, Web UI, save provider, and optional AI provider.
3. Select a save provider at Installation time.
4. Start a Run.
5. Replace the simulation Component while preserving portable state through migration.
6. Encapsulate the internal save + sync subgraph as a nested Assembly and reuse it in another Work.

### Scenario B — Agent-assisted deployment of an open-source service

1. Import Git source into a Workspace.
2. Agent produces candidate SourceInspectionReport, BuildGraph, and OperationalIntent.
3. User approves a ChangeSet.
4. Builder produces immutable artifact and provenance.
5. Planner creates the same semantic RealizationPlan for either local or Agent Target.
6. Apply, health-check, restart, and rollback.
7. Replay never reads live source again.

### Scenario C — Closed-source local game

1. Import catalog metadata and Rights/Transparency declarations.
2. Installation binds a user-local executable or store adapter.
3. Library shows Play, Inspect, and Backup, but does not invent Edit or Compose.
4. Opaque save state can be backed up and restored.
5. When a dedicated-server artifact and rights permit, create a separate Realization.
6. When the program implements an open save, mod, or lobby Protocol, it appears in Powerbox candidates.

### Scenario D — Runtime capability across Installations

1. A running backend Installation explicitly exposes `game.save/v1`.
2. A second Work requests that capability through a launch-time import.
3. User selects provider and lease duration in Powerbox.
4. Host injects the least-authority handle.
5. Provider stop, grant revocation, or lease expiry invalidates the Binding.
6. Consumer degrades or stops according to availability policy and receives no other backend authority.

## 22. External design references

Borrow mature boundaries without copying their product ontology:

- WebAssembly Component Model and WIT worlds for explicit imports, exports, worlds, and recursive composition:
  - <https://component-model.bytecodealliance.org/design/worlds.html>
  - <https://component-model.bytecodealliance.org/design/components.html>
- OCI Content Descriptor for media type, digest, size, content-addressed graphs, and preservation of unknown types:
  - <https://github.com/opencontainers/image-spec/blob/main/descriptor.md>
- in-toto Attestation Statement for separation of immutable subjects and typed predicates:
  - <https://github.com/in-toto/attestation/blob/main/spec/v1/statement.md>
- Cloud Native Buildpacks lifecycle for separating detect/build/export phases from platform orchestration:
  - <https://buildpacks.io/docs/for-platform-operators/concepts/lifecycle/>

These standards inform adapters and data boundaries. Plurora does not require every Component to be WASM, every Artifact to live in an OCI registry, or every build to use Buildpacks.

## 23. One-sentence constraint

**Work defines the creation, Assembly defines its composition, Installation owns local choices and user state, Run represents a living instance, and Realization compiles it onto real machines; no layer may turn its own product ontology into Plurora's physical law.**
