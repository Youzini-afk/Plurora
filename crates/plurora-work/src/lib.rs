pub mod assembly;
pub mod canonical;
pub mod diagnostic;
pub mod host;
pub mod ids;
pub mod lock;
pub mod operational;
pub mod port;
pub mod realization;
pub mod rights;
pub mod state;
pub mod work;

pub use assembly::{
    validate_assembly_closure, AssemblyBinding, AssemblyNode, AssemblyNodeSource,
    AssemblyPortExposure, AssemblyRevision, ASSEMBLY_REVISION_TYPE_URI, MAX_ASSEMBLY_BINDINGS,
    MAX_ASSEMBLY_DEPTH, MAX_ASSEMBLY_NODES, MAX_EXPOSED_PORTS, MAX_PORTS_PER_NODE, MAX_STATE_SLOTS,
};
pub use canonical::{
    canonical_digest, canonical_json_bytes, validate_artifact_descriptor, validate_canonical_model,
    validate_descriptor_type, validate_portable_model, validate_portable_value, ArtifactModel,
    CANONICAL_JSON_MEDIA_TYPE, MAX_CANONICAL_METADATA_BYTES,
};
pub use diagnostic::{DiagnosticCode, ModelError, ModelResult, WorkDiagnostic};
pub use host::{
    AcquisitionKind, AcquisitionRecord, ActiveBindingRecord, ExposureRecord, ExposureStatus,
    HealthStatus, InstallationRecord, InstallationSecretPolicy, InstallationStatus,
    NodeInstanceRecord, NodeInstanceStatus, ResourceSelector, RunHealth, RunRecord, RunStatus,
    StateBindingKind, StateBindingRecord,
};
pub use ids::{
    AssemblyId, BindingId, ExposureId, InstallationId, NodeId, PortId, RealizationId, RunId,
    StateSlotId, WorkId,
};
pub use lock::{AssemblyLock, BindingLock, NodeLock, ASSEMBLY_LOCK_TYPE_URI};
pub use operational::{
    AccessMode, EndpointIntent, EndpointVisibility, FilesystemImport, NetworkImport,
    OperationalIntent, PlacementConstraint, PlacementConstraintKind, ReplicaPolicy,
    ResourceCapacity, ResourceRequirements, RestartPolicy, SecretImport, StateDurability,
    StatePlacementIntent, TargetCapabilityRecord, TargetInventorySnapshot, TopologyRelation,
    UpdatePolicy, UpdateStrategy, WorkloadImports, WorkloadIntent, OPERATIONAL_INTENT_TYPE_URI,
    TARGET_INVENTORY_TYPE_URI,
};
pub use port::{
    check_port_compatibility, check_transport_policy_compatibility, AvailabilityPolicy,
    BindingPhase, EffectClass, InteractionModelId, PortContract, PortDescriptor, PortDirection,
    PortEndpoint, PortMultiplicity, PortRole, SelectedTransport, TransportPolicy,
    TransportRequirements, INTERACTION_ARTIFACT, INTERACTION_CAPABILITY_STREAM,
    INTERACTION_CAPABILITY_UNARY, INTERACTION_DUPLEX_STREAM, INTERACTION_ENDPOINT,
    INTERACTION_EVENT_STREAM, INTERACTION_SNAPSHOT, KNOWN_INTERACTION_MODELS,
};
pub use realization::{
    BuildAction, EndpointAction, LaunchAction, NodePlacement, RealizationHealth, RealizationPlan,
    RealizationRevision, RealizationStatus, RealizedResource, StateAction, StateActionKind,
    TransportBindingPlan, MAX_REALIZATION_PLAN_ACTIONS, REALIZATION_PLAN_TYPE_URI,
};
pub use rights::{
    ClaimStatus, ProtocolRequirement, RightDisposition, RightsDeclaration, SourceVisibility,
    TransparencyDeclaration, RIGHTS_DECLARATION_TYPE_URI, TRANSPARENCY_DECLARATION_TYPE_URI,
};
pub use state::{
    validate_state_replacement, BackupPolicy, StatePortability, StateScope, StateSlotDescriptor,
};
pub use work::{WorkEntrypoint, WorkEntrypointTarget, WorkRevision, WORK_REVISION_TYPE_URI};

pub use plurora_core::ArtifactDescriptor;

pub const FOREIGN_CAPSULE_TYPE_URI: &str = "urn:plurora:foreign-capsule:v1";

pub const WORK_ARTIFACT_TYPE_URIS: &[&str] = &[
    WORK_REVISION_TYPE_URI,
    ASSEMBLY_REVISION_TYPE_URI,
    ASSEMBLY_LOCK_TYPE_URI,
    RIGHTS_DECLARATION_TYPE_URI,
    TRANSPARENCY_DECLARATION_TYPE_URI,
    OPERATIONAL_INTENT_TYPE_URI,
    TARGET_INVENTORY_TYPE_URI,
    REALIZATION_PLAN_TYPE_URI,
    FOREIGN_CAPSULE_TYPE_URI,
];
