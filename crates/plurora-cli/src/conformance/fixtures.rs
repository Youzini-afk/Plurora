use std::sync::Arc;

use plurora_core::{
    CapabilityDescriptor, CapabilityPermissions, EntryDescriptor, EventPermissions,
    HookSubscription, HookTiming, PackageContributions, PackageEntry, PackageManifest,
    PermissionSet, SandboxPolicy,
};
use plurora_runtime::{InMemoryEventStore, Runtime, RuntimeConfig};
use serde_json::json;

use crate::commands::demo;

pub(crate) fn runtime() -> (Arc<InMemoryEventStore>, Runtime<InMemoryEventStore>) {
    let store = Arc::new(InMemoryEventStore::default());
    let runtime = Runtime::new(store.clone(), RuntimeConfig::default());
    (store, runtime)
}

pub(crate) fn event_package(id: &str, read: bool, append: bool) -> PackageManifest {
    PackageManifest {
        id: id.to_string(),
        permissions: PermissionSet {
            events: EventPermissions { read, append },
            ..PermissionSet::default()
        },
        ..demo::demo_event_writer_manifest()
    }
}

pub(crate) fn hook_package(
    id: &str,
    extension_point: &str,
    handler: &str,
    precedence: i32,
) -> PackageManifest {
    PackageManifest {
        id: id.to_string(),
        contributes: PackageContributions {
            hooks: vec![HookSubscription {
                extension_point: extension_point.to_string(),
                handler: handler.to_string(),
                timing: HookTiming::Sync,
                precedence,
            }],
            ..PackageContributions::default()
        },
        ..demo::demo_event_writer_manifest()
    }
}

pub(crate) fn hook_handler_package(
    id: &str,
    extension_point: &str,
    handler: &str,
) -> PackageManifest {
    PackageManifest {
        schema_version: 1,
        id: id.to_string(),
        version: "0.1.0".to_string(),
        display_name: None,
        description: None,
        author: None,
        license: None,
        entry: EntryDescriptor::v1(PackageEntry::RustInproc {
            crate_ref: "example-hook-inproc".to_string(),
            symbol: "register".to_string(),
            abi_version: 1,
        }),
        provides: vec![CapabilityDescriptor {
            id: handler.to_string(),
            version: "0.1.0".to_string(),
            input_schema: serde_json::Value::Null,
            output_schema: serde_json::Value::Null,
            streaming: false,
            side_effects: Vec::new(),
            description: None,
        }],
        consumes: Vec::new(),
        requires: Vec::new(),
        contributes: PackageContributions {
            hooks: vec![HookSubscription {
                extension_point: extension_point.to_string(),
                handler: handler.to_string(),
                timing: HookTiming::Sync,
                precedence: 0,
            }],
            ..PackageContributions::default()
        },
        permissions: PermissionSet::default(),
        sandbox_policy: SandboxPolicy::default(),
    }
}

pub(crate) fn echo_package(id: &str, capability_id: &str) -> PackageManifest {
    schema_echo_package(
        id,
        capability_id,
        serde_json::Value::Null,
        serde_json::Value::Null,
    )
}

pub(crate) fn schema_echo_package(
    id: &str,
    capability_id: &str,
    input_schema: serde_json::Value,
    output_schema: serde_json::Value,
) -> PackageManifest {
    PackageManifest {
        schema_version: 1,
        id: id.to_string(),
        version: "0.1.0".to_string(),
        display_name: None,
        description: None,
        author: None,
        license: None,
        entry: EntryDescriptor::v1(PackageEntry::RustInproc {
            crate_ref: "example-echo-rust-inproc".to_string(),
            symbol: "register".to_string(),
            abi_version: 1,
        }),
        provides: vec![CapabilityDescriptor {
            id: capability_id.to_string(),
            version: "0.1.0".to_string(),
            input_schema,
            output_schema,
            streaming: false,
            side_effects: Vec::new(),
            description: None,
        }],
        consumes: Vec::new(),
        requires: Vec::new(),
        contributes: PackageContributions::default(),
        permissions: PermissionSet {
            capabilities: CapabilityPermissions {
                invoke: vec!["*".to_string()],
            },
            ..PermissionSet::default()
        },
        sandbox_policy: SandboxPolicy::default(),
    }
}

pub(crate) fn event_schema_package() -> PackageManifest {
    PackageManifest {
        id: "example/schema-writer".to_string(),
        contributes: PackageContributions {
            schemas: vec![plurora_core::SchemaContribution {
                id: "example/schema-writer/event.checked".to_string(),
                schema: json!({"type": "object", "required": ["ok"]}),
            }],
            hooks: Vec::new(),
            extension_points: Vec::new(),
            surfaces: Vec::new(),
        },
        permissions: PermissionSet {
            events: EventPermissions {
                read: false,
                append: true,
            },
            ..PermissionSet::default()
        },
        ..demo::demo_event_writer_manifest()
    }
}
