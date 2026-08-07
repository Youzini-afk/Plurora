// ---------------------------------------------------------------------------
// Y2: Dispatch enforcement unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod y2_tests {
    use std::sync::Arc;

    use crate::{
        FakeOutboundExecutor, InMemoryEventStore, OutboundExecutorConfig, ProtocolContext, Runtime,
        RuntimeConfig,
    };
    use plurora_core::{
        CapabilityDescriptor, EntryDescriptor, NetworkDeclaration, NetworkPermissions,
        PackageContributions, PackageEntry, PackageManifest, PermissionSet, SandboxPolicy,
    };

    /// Helper: create a runtime with a FakeOutboundExecutor.
    fn runtime_with_fake() -> (
        Arc<InMemoryEventStore>,
        Runtime<InMemoryEventStore>,
        Arc<FakeOutboundExecutor>,
    ) {
        let store = Arc::new(InMemoryEventStore::default());
        let fake = Arc::new(FakeOutboundExecutor::new());
        let config = RuntimeConfig {
            outbound_executor: OutboundExecutorConfig::Custom(fake.clone()),
            ..RuntimeConfig::default()
        };
        let runtime = Runtime::new(store.clone(), config);
        (store, runtime, fake)
    }

    /// Helper: create a package manifest with network and secret_refs permissions.
    fn package_with_secret_refs(id: &str, secret_refs: Vec<String>) -> PackageManifest {
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
                id: format!("{id}/fetch"),
                version: "0.1.0".to_string(),
                input_schema: serde_json::Value::Null,
                output_schema: serde_json::Value::Null,
                streaming: false,
                side_effects: vec!["network".to_string()],
                description: None,
            }],
            consumes: Vec::new(),
            requires: Vec::new(),
            contributes: PackageContributions::default(),
            permissions: PermissionSet {
                network: NetworkPermissions {
                    declarations: vec![NetworkDeclaration {
                        host: "api.openai.com".to_string(),
                        methods: vec!["POST".to_string()],
                        purpose: Some("test".to_string()),
                    }],
                    hosts: vec![],
                },
                secret_refs,
                ..PermissionSet::default()
            },
            sandbox_policy: SandboxPolicy::default(),
        }
    }

    /// Y2: Undeclared secret_ref in secret_headers is rejected.
    #[tokio::test]
    async fn outbound_execute_secret_ref_undeclared_fails() {
        let (_store, runtime, fake) = runtime_with_fake();
        // Package declares one secret_ref but request uses a different one
        runtime
            .load_package(package_with_secret_refs(
                "example/y2-undeclared",
                vec!["secret_ref:env:DECLARED_KEY".to_string()],
            ))
            .await
            .expect("load package");

        let context = ProtocolContext::package("example/y2-undeclared", "in_process");
        let result = runtime
            .call_protocol(
                &context,
                "kernel.v1.outbound.execute",
                serde_json::json!({
                    "capability_id": "example/y2-undeclared/fetch",
                    "destination_host": "api.openai.com",
                    "method": "POST",
                    "secret_headers": {
                        "Authorization": {
                            "secret_ref": "secret_ref:env:UNDECLARED_KEY",
                            "scheme": "bearer"
                        }
                    }
                }),
            )
            .await;

        assert!(result.is_err(), "undeclared secret_ref should be denied");
        let err_msg = format!("{:?}", result.unwrap_err());
        assert!(
            err_msg.contains("not declared"),
            "error should mention undeclared secret_ref, got: {err_msg}"
        );
        assert_eq!(
            fake.call_count(),
            0,
            "executor should not be called for undeclared secret_ref"
        );
    }

    /// Y2: Declared secret_ref is allowed to proceed.
    #[tokio::test]
    async fn outbound_execute_secret_ref_declared_resolves() {
        let (_store, runtime, _fake) = runtime_with_fake();
        runtime
            .load_package(package_with_secret_refs(
                "example/y2-declared",
                vec!["secret_ref:env:MY_API_KEY".to_string()],
            ))
            .await
            .expect("load package");

        let context = ProtocolContext::package("example/y2-declared", "in_process");

        // Note: secret resolution will fail (no resolver configured), but
        // the Y2 check happens BEFORE resolution. The error should be from
        // the resolver, not from the undeclared check.
        let result = runtime
            .call_protocol(
                &context,
                "kernel.v1.outbound.execute",
                serde_json::json!({
                    "capability_id": "example/y2-declared/fetch",
                    "destination_host": "api.openai.com",
                    "method": "POST",
                    "secret_headers": {
                        "Authorization": {
                            "secret_ref": "secret_ref:env:MY_API_KEY",
                            "scheme": "bearer"
                        }
                    }
                }),
            )
            .await;

        // The Y2 declaration check passes, but secret resolution may fail
        // (DenyAllSecretResolver is the default). The key point is we
        // should NOT get the "not declared" error.
        if let Err(e) = &result {
            let err_msg = format!("{:?}", e);
            assert!(
                !err_msg.contains("not declared"),
                "declared secret_ref should not produce 'not declared' error, got: {err_msg}"
            );
        }
        // Executor may or may not be called depending on resolver success,
        // but the Y2 check should not block it.
    }

    /// Y2: Request without secret_headers skips the manifest check.
    #[tokio::test]
    async fn outbound_execute_no_secret_headers_no_check_required() {
        let (_store, runtime, fake) = runtime_with_fake();
        // Package has no secret_refs declared, but also doesn't use any
        runtime
            .load_package(package_with_secret_refs("example/y2-no-secret", vec![]))
            .await
            .expect("load package");

        let context = ProtocolContext::package("example/y2-no-secret", "in_process");
        let result = runtime
            .call_protocol(
                &context,
                "kernel.v1.outbound.execute",
                serde_json::json!({
                    "capability_id": "example/y2-no-secret/fetch",
                    "destination_host": "api.openai.com",
                    "method": "POST",
                }),
            )
            .await;

        // Should succeed (fake executor returns ok)
        assert!(
            result.is_ok(),
            "request without secret_headers should succeed, got: {:?}",
            result.err()
        );
        assert_eq!(fake.call_count(), 1, "executor should be called");
    }

    /// Y2: Multiple secret_refs must all be declared.
    #[tokio::test]
    async fn outbound_execute_multiple_secret_refs_all_must_be_declared() {
        let (_store, runtime, fake) = runtime_with_fake();
        // Declare only one of two needed refs
        runtime
            .load_package(package_with_secret_refs(
                "example/y2-multi",
                vec!["secret_ref:env:KEY_A".to_string()],
            ))
            .await
            .expect("load package");

        let context = ProtocolContext::package("example/y2-multi", "in_process");
        let result = runtime
            .call_protocol(
                &context,
                "kernel.v1.outbound.execute",
                serde_json::json!({
                    "capability_id": "example/y2-multi/fetch",
                    "destination_host": "api.openai.com",
                    "method": "POST",
                    "secret_refs": ["secret_ref:env:KEY_A", "secret_ref:env:KEY_B"],
                }),
            )
            .await;

        assert!(
            result.is_err(),
            "undeclared second secret_ref should be denied"
        );
        let err_msg = format!("{:?}", result.unwrap_err());
        assert!(
            err_msg.contains("not declared"),
            "error should mention undeclared secret_ref, got: {err_msg}"
        );
        assert_eq!(
            fake.call_count(),
            0,
            "executor should not be called when any secret_ref is undeclared"
        );
    }

    /// Y2: Top-level secret_refs also require manifest declaration.
    #[tokio::test]
    async fn outbound_execute_top_level_secret_ref_undeclared_fails() {
        let (_store, runtime, fake) = runtime_with_fake();
        runtime
            .load_package(package_with_secret_refs("example/y2-toplevel", vec![]))
            .await
            .expect("load package");

        let context = ProtocolContext::package("example/y2-toplevel", "in_process");
        let result = runtime
            .call_protocol(
                &context,
                "kernel.v1.outbound.execute",
                serde_json::json!({
                    "capability_id": "example/y2-toplevel/fetch",
                    "destination_host": "api.openai.com",
                    "method": "POST",
                    "secret_refs": ["secret_ref:env:UNDECLARED"],
                }),
            )
            .await;

        assert!(
            result.is_err(),
            "top-level undeclared secret_ref should be denied"
        );
        let err_msg = format!("{:?}", result.unwrap_err());
        assert!(
            err_msg.contains("not declared"),
            "error should mention undeclared, got: {err_msg}"
        );
        assert_eq!(fake.call_count(), 0, "executor should not be called");
    }
}

#[cfg(test)]
mod host_resource_authority_tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use crate::{
        InMemoryEventStore, ProjectRegistry, ProtocolContext, ProtocolResourceSelector, Runtime,
        RuntimeConfig,
    };
    use plurora_core::project::{
        ProjectDescriptor, ProjectId, ProjectInner, ProjectType, SecretPolicy,
    };

    fn project(id: &str, title: &str) -> ProjectDescriptor {
        ProjectDescriptor {
            schema_version: 1,
            project: ProjectInner {
                id: ProjectId::new(id).expect("valid project id"),
                title: title.to_string(),
                description: String::new(),
                project_type: ProjectType::PluroraNative,
                icon: None,
                entry_surface_id: Some("test/surface/main".to_string()),
                packages: vec!["packages/test/manifest.yaml".to_string()],
                optional_packages: Vec::new(),
                required_surfaces: Vec::new(),
                required_capabilities: Vec::new(),
                secret_policy: SecretPolicy::default(),
                external: None,
                metadata: BTreeMap::new(),
            },
        }
    }

    fn project_device(project_id: &str) -> ProtocolContext {
        ProtocolContext::host_device(
            "grant-project-a",
            vec!["observe".into(), "project_operate".into()],
            vec![ProtocolResourceSelector {
                owner: "host".into(),
                kind: "project".into(),
                id: Some(project_id.into()),
            }],
            Vec::new(),
            "test",
        )
    }

    #[tokio::test]
    async fn project_device_cannot_list_or_open_another_project() {
        let project_a = "authority_project_a__abc12345";
        let project_b = "authority_project_b__abc12345";
        let registry = Arc::new(ProjectRegistry::new());
        registry
            .register(project(project_a, "Project A"))
            .expect("register A");
        registry
            .register(project(project_b, "Project B"))
            .expect("register B");
        let runtime = Runtime::new(
            Arc::new(InMemoryEventStore::default()),
            RuntimeConfig {
                project_registry: registry,
                ..RuntimeConfig::default()
            },
        );
        let context = project_device(project_a);
        let listed = runtime
            .call_protocol(&context, "host.project.list", serde_json::json!({}))
            .await
            .expect("list allowed projects");
        let projects = listed["projects"].as_array().expect("projects array");
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0]["id"], project_a);

        assert!(runtime
            .call_protocol(
                &context,
                "host.project.get",
                serde_json::json!({"project_id": project_b}),
            )
            .await
            .is_err());

        let started = runtime
            .call_protocol(
                &context,
                "host.project.start",
                serde_json::json!({"project_id": project_a}),
            )
            .await
            .expect("start allowed project");
        let session_id = started["session_id"].as_str().expect("session id");
        let session = runtime
            .get_session(session_id)
            .await
            .expect("session exists");
        assert_eq!(session.metadata["project_id"], project_a);

        let forked = runtime
            .call_protocol(
                &context,
                "kernel.v1.session.fork",
                serde_json::json!({
                    "parent_session_id": session_id,
                    "forked_from_sequence": 0,
                    "metadata": {"reason": "authority-test"}
                }),
            )
            .await
            .expect("fork allowed project session");
        let child_session_id = forked["child_session_id"]
            .as_str()
            .expect("child session id");
        let child = runtime
            .get_session(child_session_id)
            .await
            .expect("forked session exists");
        assert_eq!(child.metadata["project_id"], project_a);
        runtime
            .call_protocol(
                &context,
                "kernel.v1.session.get",
                serde_json::json!({"session_id": child_session_id}),
            )
            .await
            .expect("forked session retains the project authority binding");
    }

    #[tokio::test]
    async fn target_device_list_is_filtered_structurally() {
        let runtime = Runtime::new(
            Arc::new(InMemoryEventStore::default()),
            RuntimeConfig::default(),
        );
        let denied = ProtocolContext::host_device(
            "grant-target-other",
            vec!["observe".into()],
            vec![ProtocolResourceSelector {
                owner: "host".into(),
                kind: "target".into(),
                id: Some("local-copy".into()),
            }],
            Vec::new(),
            "test",
        );
        let listed = runtime
            .call_protocol(&denied, "host.target.list", serde_json::json!({}))
            .await
            .expect("target list");
        assert_eq!(listed, serde_json::json!([]));
        assert!(runtime
            .call_protocol(
                &denied,
                "host.target.status",
                serde_json::json!({"target_id": "local"}),
            )
            .await
            .is_err());
    }

    #[tokio::test]
    async fn exact_project_device_cannot_enumerate_global_surface_catalogue() {
        let runtime = Runtime::new(
            Arc::new(InMemoryEventStore::default()),
            RuntimeConfig::default(),
        );
        let exact = project_device("authority_project_a__abc12345");
        assert!(runtime
            .call_protocol(
                &exact,
                "kernel.v1.surface.contribution.list",
                serde_json::json!({}),
            )
            .await
            .is_err());
        for method in [
            "kernel.v1.package.list",
            "kernel.v1.capability.discover",
            "kernel.v1.asset.list",
            "kernel.v1.projection.list",
        ] {
            assert!(
                runtime
                    .call_protocol(&exact, method, serde_json::json!({}))
                    .await
                    .is_err(),
                "exact-project authority must not enumerate Host-global method {method}"
            );
        }

        let global = ProtocolContext::host_device(
            "grant-all-projects",
            vec!["observe".into()],
            vec![ProtocolResourceSelector {
                owner: "host".into(),
                kind: "project".into(),
                id: None,
            }],
            Vec::new(),
            "test",
        );
        assert_eq!(
            runtime
                .call_protocol(
                    &global,
                    "kernel.v1.surface.contribution.list",
                    serde_json::json!({}),
                )
                .await
                .expect("all-project device can enumerate the Host catalogue"),
            serde_json::json!([])
        );
    }
}

#[cfg(test)]
mod surface_tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use crate::{InMemoryEventStore, ProjectRegistry, ProtocolContext, Runtime, RuntimeConfig};
    use plurora_core::project::{
        ProjectDescriptor, ProjectId, ProjectInner, ProjectType, SecretPolicy,
    };

    #[tokio::test]
    async fn resolve_bundle_does_not_return_project_metadata() {
        let registry = Arc::new(ProjectRegistry::new());
        let mut metadata = BTreeMap::new();
        metadata.insert(
            "requested_capabilities".to_string(),
            serde_json::json!(["attacker/metadata_grant"]),
        );
        metadata.insert("host_path".to_string(), serde_json::json!("/secret/path"));

        registry
            .register(ProjectDescriptor {
                schema_version: 1,
                project: ProjectInner {
                    id: ProjectId::new("surface_meta_test__abc12345").unwrap(),
                    title: "Surface metadata test".to_string(),
                    description: String::new(),
                    project_type: ProjectType::PluroraNative,
                    icon: None,
                    entry_surface_id: Some("pkg/surface/entry".to_string()),
                    packages: vec!["packages/pkg/manifest.yaml".to_string()],
                    optional_packages: Vec::new(),
                    required_surfaces: Vec::new(),
                    required_capabilities: Vec::new(),
                    secret_policy: SecretPolicy::default(),
                    external: None,
                    metadata,
                },
            })
            .expect("register project");

        let runtime = Runtime::new(
            Arc::new(InMemoryEventStore::default()),
            RuntimeConfig {
                project_registry: registry,
                ..RuntimeConfig::default()
            },
        );
        let value = runtime
            .call_protocol(
                &ProtocolContext::host_dev("test"),
                "kernel.v1.surface.resolve_bundle",
                serde_json::json!({ "surface_id": "pkg/surface/entry" }),
            )
            .await
            .expect("resolve bundle");

        assert!(
            value.get("metadata").is_none(),
            "resolve_bundle must not expose arbitrary project metadata: {value:?}"
        );
    }

    #[tokio::test]
    async fn resolve_bundle_fingerprints_dev_bundle() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("bundle.mjs"), "export const ok = true;")
            .expect("write bundle");

        let mut config = RuntimeConfig::default();
        config.surface_dev_paths.insert(
            "example".to_string(),
            dir.path().to_string_lossy().to_string(),
        );
        let runtime = Runtime::new(Arc::new(InMemoryEventStore::default()), config);
        let value = runtime
            .call_protocol(
                &ProtocolContext::host_dev("test"),
                "kernel.v1.surface.resolve_bundle",
                serde_json::json!({ "surface_id": "example/surface" }),
            )
            .await
            .expect("resolve bundle");

        let fingerprint = value["bundle_fingerprint"]
            .as_str()
            .expect("bundle fingerprint");
        assert_eq!(fingerprint.len(), 16);
        assert!(fingerprint.chars().all(|ch| ch.is_ascii_hexdigit()));
        assert_eq!(
            value["bundle_url"].as_str().expect("bundle url"),
            format!("/surface-bundles/example/bundle.mjs?v={fingerprint}")
        );
    }
}

#[cfg(test)]
mod deployment_hub_tests {
    use std::sync::Arc;

    use crate::{InMemoryEventStore, ProtocolContext, ProtocolPrincipal, Runtime, RuntimeConfig};

    fn runtime() -> Runtime<InMemoryEventStore> {
        Runtime::new(
            Arc::new(InMemoryEventStore::default()),
            RuntimeConfig::default(),
        )
    }

    #[tokio::test]
    async fn default_exec_start_is_denied() {
        let runtime = runtime();
        let context = ProtocolContext::host_dev("in_process");

        let result = runtime
            .call_protocol(
                &context,
                "kernel.v1.exec.start",
                serde_json::json!({
                    "target_id": "local",
                    "command": {"program": "definitely-not-started", "args": []}
                }),
            )
            .await
            .expect("exec.start dispatch succeeds with denied response");

        assert_eq!(result["status"]["kind"], "denied");
        assert!(result["exec_id"].is_null());
        assert!(result["error"]
            .as_str()
            .unwrap_or_default()
            .contains("denied"));
    }

    #[tokio::test]
    async fn port_lease_is_loopback_only() {
        let runtime = runtime();
        let context = ProtocolContext::host_dev("in_process");

        let result = runtime
            .call_protocol(
                &context,
                "kernel.v1.port.lease",
                serde_json::json!({
                    "target_id": "local",
                    "port_name": "web",
                    "requested_port": 39123
                }),
            )
            .await
            .expect("port lease succeeds");

        assert_eq!(result["lease"]["host"], "127.0.0.1");
        assert_eq!(result["lease"]["bind"], "loopback_only");
        assert_eq!(result["lease"]["status"], "active");
    }

    #[tokio::test]
    async fn proxy_register_requires_existing_active_port_lease() {
        let runtime = runtime();
        let context = ProtocolContext::host_dev("in_process");

        let missing = runtime
            .call_protocol(
                &context,
                "kernel.v1.proxy.register",
                serde_json::json!({
                    "upstream": {"port_lease_id": "missing", "port_name": "web"},
                    "protocol": "http"
                }),
            )
            .await;
        assert!(missing.is_err(), "missing lease must be denied");

        let lease = runtime
            .call_protocol(
                &context,
                "kernel.v1.port.lease",
                serde_json::json!({"target_id":"local","port_name":"web"}),
            )
            .await
            .expect("lease succeeds");
        let lease_id = lease["lease"]["id"].as_str().expect("lease id");

        let registered = runtime
            .call_protocol(
                &context,
                "kernel.v1.proxy.register",
                serde_json::json!({
                    "upstream": {"port_lease_id": lease_id, "port_name": "web"},
                    "protocol": "http"
                }),
            )
            .await
            .expect("active lease can be proxied");
        assert_eq!(registered["route"]["status"], "active");
        assert_eq!(registered["route"]["ready"], false);
        assert_eq!(registered["route"]["upstream"]["port_lease_id"], lease_id);
    }

    #[tokio::test]
    async fn deployment_hub_methods_require_host_principal() {
        let runtime = runtime();
        let context = ProtocolContext {
            principal: ProtocolPrincipal::Anonymous,
            transport: "in_process".to_string(),
            authority: None,
            host_operation: None,
            session_id: None,
            correlation_id: None,
            parent_invocation_id: None,
        };

        let result = runtime
            .call_protocol(
                &context,
                "kernel.v1.port.lease",
                serde_json::json!({"target_id":"local","port_name":"web"}),
            )
            .await;

        let error = result.expect_err("anonymous deployment hub call must be denied");
        assert_eq!(error.code, "kernel/v1/error/permission_denied");
    }

    #[tokio::test]
    async fn proxy_register_requires_matching_port_name() {
        let runtime = runtime();
        let context = ProtocolContext::host_dev("in_process");

        let lease = runtime
            .call_protocol(
                &context,
                "kernel.v1.port.lease",
                serde_json::json!({"target_id":"local","port_name":"web"}),
            )
            .await
            .expect("lease succeeds");
        let lease_id = lease["lease"]["id"].as_str().expect("lease id");

        let mismatch = runtime
            .call_protocol(
                &context,
                "kernel.v1.proxy.register",
                serde_json::json!({
                    "upstream": {"port_lease_id": lease_id, "port_name": "admin"},
                    "protocol": "http"
                }),
            )
            .await;

        assert!(mismatch.is_err(), "mismatched port_name must be denied");
    }
}

#[cfg(test)]
mod z_websocket_tests {
    use std::sync::Arc;

    use crate::{
        EventStore, FakeOutboundExecutor, FakeWebSocketExecutor, InMemoryEventStore,
        OutboundExecutePolicyConfig, OutboundExecutorConfig, OutboundExecutorResponse,
        ProtocolContext, Runtime, RuntimeConfig,
    };
    use plurora_core::{
        CapabilityDescriptor, EntryDescriptor, NetworkDeclaration, NetworkPermissions,
        PackageContributions, PackageEntry, PackageManifest, PermissionSet, SandboxPolicy,
    };

    fn runtime_with_fake_ws() -> (
        Arc<InMemoryEventStore>,
        Runtime<InMemoryEventStore>,
        Arc<FakeWebSocketExecutor>,
    ) {
        let store = Arc::new(InMemoryEventStore::default());
        let fake = Arc::new(FakeWebSocketExecutor::new());
        let config = RuntimeConfig {
            outbound_websocket_executor: fake.clone(),
            ..RuntimeConfig::default()
        };
        let runtime = Runtime::new(store.clone(), config);
        (store, runtime, fake)
    }

    fn package_ws(id: &str, secret_refs: Vec<String>) -> PackageManifest {
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
                id: format!("{id}/ws"),
                version: "0.1.0".to_string(),
                input_schema: serde_json::Value::Null,
                output_schema: serde_json::Value::Null,
                streaming: true,
                side_effects: vec!["network".to_string()],
                description: None,
            }],
            consumes: Vec::new(),
            requires: Vec::new(),
            contributes: PackageContributions::default(),
            permissions: PermissionSet {
                network: NetworkPermissions {
                    declarations: vec![NetworkDeclaration {
                        host: "api.example.com".to_string(),
                        methods: vec!["WEBSOCKET".to_string()],
                        purpose: Some("test websocket".to_string()),
                    }],
                    hosts: vec![],
                },
                secret_refs,
                ..PermissionSet::default()
            },
            sandbox_policy: SandboxPolicy::default(),
        }
    }

    #[tokio::test]
    async fn dispatch_outbound_websocket_open_namespace_enforced() {
        let (_store, runtime, _fake) = runtime_with_fake_ws();
        runtime
            .load_package(package_ws("example/ws-ns", vec![]))
            .await
            .expect("load package");
        let context = ProtocolContext::package("example/ws-ns", "in_process");
        let result = runtime
            .call_protocol(
                &context,
                "kernel.v1.outbound.websocket.open",
                serde_json::json!({
                    "capability_id": "other/pkg/ws",
                    "destination_host": "api.example.com"
                }),
            )
            .await;
        assert!(result.is_err());
        assert!(format!("{:?}", result.unwrap_err()).contains("namespace"));
    }

    #[tokio::test]
    async fn dispatch_outbound_websocket_open_secret_ref_undeclared_fails() {
        let (_store, runtime, _fake) = runtime_with_fake_ws();
        runtime
            .load_package(package_ws("example/ws-secret", vec![]))
            .await
            .expect("load package");
        let context = ProtocolContext::package("example/ws-secret", "in_process");
        let result = runtime
            .call_protocol(
                &context,
                "kernel.v1.outbound.websocket.open",
                serde_json::json!({
                    "capability_id": "example/ws-secret/ws",
                    "destination_host": "api.example.com",
                    "secret_refs": ["secret_ref:env:MISSING"]
                }),
            )
            .await;
        assert!(result.is_err());
        assert!(format!("{:?}", result.unwrap_err()).contains("not declared"));
    }

    #[tokio::test]
    async fn dispatch_outbound_websocket_open_with_fake_executor_emits_opened() {
        let (store, runtime, _fake) = runtime_with_fake_ws();
        runtime
            .load_package(package_ws("example/ws-ok", vec![]))
            .await
            .expect("load package");
        let context = ProtocolContext::package("example/ws-ok", "in_process");
        let result = runtime
            .call_protocol(
                &context,
                "kernel.v1.outbound.websocket.open",
                serde_json::json!({
                    "capability_id": "example/ws-ok/ws",
                    "destination_host": "api.example.com",
                    "subprotocols": ["json"]
                }),
            )
            .await
            .expect("open websocket");
        let connection_id = result
            .get("connection_id")
            .and_then(serde_json::Value::as_str)
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        let events = store
            .list_kind_prefix(plurora_core::EVENT_OUTBOUND_WEBSOCKET_OPENED)
            .await
            .unwrap();
        assert!(events.iter().any(|event| event
            .payload
            .get("connection_id")
            .and_then(serde_json::Value::as_str)
            == Some(connection_id)));
    }

    fn runtime_with_fake_execute(
        fake: Arc<FakeOutboundExecutor>,
    ) -> (Arc<InMemoryEventStore>, Runtime<InMemoryEventStore>) {
        let store = Arc::new(InMemoryEventStore::default());
        let config = RuntimeConfig {
            outbound_executor: OutboundExecutorConfig::Custom(fake),
            outbound_execute_policy: OutboundExecutePolicyConfig {
                enabled: true,
                allowed_hosts: vec!["api.example.com".to_string()],
                https_only: true,
                timeout_ms: 30_000,
                allow_redirects: false,
                allow_insecure_loopback_for_tests: false,
            },
            ..RuntimeConfig::default()
        };
        (store.clone(), Runtime::new(store, config))
    }

    async fn wait_for_event(store: &InMemoryEventStore, kind: &str) -> serde_json::Value {
        for _ in 0..40 {
            let events = store.list_kind_prefix(kind).await.unwrap();
            if let Some(event) = events.last() {
                return event.payload.clone();
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        panic!("event {kind} not found");
    }

    #[tokio::test]
    async fn outbound_execute_emits_completed_event_on_success() {
        let fake = Arc::new(FakeOutboundExecutor::new());
        let (store, runtime) = runtime_with_fake_execute(fake);
        runtime
            .load_package(package_ws("example/z6-exec-ok", vec![]))
            .await
            .unwrap();
        let context = ProtocolContext::package("example/z6-exec-ok", "in_process");
        let _ = runtime
            .call_protocol(
                &context,
                "kernel.v1.outbound.execute",
                serde_json::json!({
                    "capability_id": "example/z6-exec-ok/ws",
                    "destination_host": "api.example.com",
                    "method": "WEBSOCKET"
                }),
            )
            .await
            .unwrap();
        let payload = wait_for_event(&store, plurora_core::EVENT_OUTBOUND_EXECUTE_COMPLETED).await;
        assert_eq!(payload["status"], "ok");
        assert_eq!(payload["executor_kind"], "fake");
    }

    #[tokio::test]
    async fn outbound_execute_emits_completed_event_on_error() {
        let fake = Arc::new(FakeOutboundExecutor::with_fixture(
            "api.example.com",
            "WEBSOCKET",
            None,
            OutboundExecutorResponse {
                status: "error".to_string(),
                status_code: Some(500),
                headers_shape: None,
                body_shape: None,
                provider_request_id: None,
                usage: serde_json::Value::Null,
                cost: serde_json::Value::Null,
                redaction_state: plurora_core::RedactionState::Redacted,
                network_performed: false,
                executor_kind: crate::ExecutorKind::Fake,
            },
        ));
        let (store, runtime) = runtime_with_fake_execute(fake);
        runtime
            .load_package(package_ws("example/z6-exec-error", vec![]))
            .await
            .unwrap();
        let context = ProtocolContext::package("example/z6-exec-error", "in_process");
        let _ = runtime
            .call_protocol(
                &context,
                "kernel.v1.outbound.execute",
                serde_json::json!({
                    "capability_id": "example/z6-exec-error/ws",
                    "destination_host": "api.example.com",
                    "method": "WEBSOCKET"
                }),
            )
            .await
            .unwrap();
        let payload = wait_for_event(&store, plurora_core::EVENT_OUTBOUND_EXECUTE_COMPLETED).await;
        assert_eq!(payload["status"], "error");
    }

    #[tokio::test]
    async fn outbound_execute_emits_completed_event_on_denied() {
        let fake = Arc::new(FakeOutboundExecutor::new());
        let (store, runtime) = runtime_with_fake_execute(fake);
        runtime
            .load_package(package_ws("example/z6-exec-denied", vec![]))
            .await
            .unwrap();
        let context = ProtocolContext::package("example/z6-exec-denied", "in_process");
        let result = runtime
            .call_protocol(
                &context,
                "kernel.v1.outbound.execute",
                serde_json::json!({
                    "capability_id": "example/z6-exec-denied/ws",
                    "destination_host": "denied.example.com",
                    "method": "WEBSOCKET"
                }),
            )
            .await;
        assert!(result.is_err());
        let payload = wait_for_event(&store, plurora_core::EVENT_OUTBOUND_EXECUTE_COMPLETED).await;
        assert_eq!(payload["status"], "denied");
    }

    #[tokio::test]
    async fn outbound_stream_emits_completed_event_on_ended() {
        let fake = Arc::new(FakeOutboundExecutor::new());
        let (store, runtime) = runtime_with_fake_execute(fake);
        runtime
            .load_package(package_ws("example/z6-stream-ended", vec![]))
            .await
            .unwrap();
        let context = ProtocolContext::package("example/z6-stream-ended", "in_process");
        let _ = runtime
            .call_protocol(
                &context,
                "kernel.v1.outbound.stream",
                serde_json::json!({
                    "capability_id": "example/z6-stream-ended/ws",
                    "destination_host": "api.example.com",
                    "method": "WEBSOCKET",
                    "stream_format": "sse"
                }),
            )
            .await
            .unwrap();
        let payload = wait_for_event(&store, plurora_core::EVENT_OUTBOUND_STREAM_COMPLETED).await;
        assert_eq!(payload["status"], "ok");
        assert_eq!(payload["final_termination"], "ended");
    }

    #[tokio::test]
    async fn outbound_stream_emits_completed_event_on_cancelled() {
        let fake = Arc::new(FakeOutboundExecutor::new());
        let (store, runtime) = runtime_with_fake_execute(fake);
        runtime
            .load_package(package_ws("example/z6-stream-cancel", vec![]))
            .await
            .unwrap();
        let context = ProtocolContext::package("example/z6-stream-cancel", "in_process");
        let response = runtime
            .call_protocol(
                &context,
                "kernel.v1.outbound.stream",
                serde_json::json!({
                    "capability_id": "example/z6-stream-cancel/ws",
                    "destination_host": "api.example.com",
                    "method": "WEBSOCKET",
                    "stream_format": "sse"
                }),
            )
            .await
            .unwrap();
        let stream_id = response["stream_id"].as_str().unwrap();
        runtime
            .call_protocol(
                &context,
                "kernel.v1.capability.cancel",
                serde_json::json!({
                    "stream_id": stream_id,
                    "session_id": "kernel_outbound_stream_example_z6-stream-cancel"
                }),
            )
            .await
            .unwrap();
        let payload = wait_for_event(&store, plurora_core::EVENT_OUTBOUND_STREAM_COMPLETED).await;
        assert_eq!(payload["status"], "cancelled");
        assert_eq!(payload["final_termination"], "cancelled");
    }

    #[tokio::test]
    async fn outbound_websocket_emits_completed_event_on_close() {
        let fake = Arc::new(FakeWebSocketExecutor::with_canned_inbound_frames(vec![
            crate::OutboundWebSocketFrame::Text("hello".to_string()),
        ]));
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Runtime::new(
            store.clone(),
            RuntimeConfig {
                outbound_websocket_executor: fake,
                ..RuntimeConfig::default()
            },
        );
        runtime
            .load_package(package_ws("example/z6-ws-close", vec![]))
            .await
            .unwrap();
        let context = ProtocolContext::package("example/z6-ws-close", "in_process");
        let _ = runtime
            .call_protocol(
                &context,
                "kernel.v1.outbound.websocket.open",
                serde_json::json!({
                    "capability_id": "example/z6-ws-close/ws",
                    "destination_host": "api.example.com"
                }),
            )
            .await
            .unwrap();
        let payload =
            wait_for_event(&store, plurora_core::EVENT_OUTBOUND_WEBSOCKET_COMPLETED).await;
        assert_eq!(payload["package_id"], "example/z6-ws-close");
        assert_eq!(payload["total_frames_in"], 1);
    }

    #[tokio::test]
    async fn outbound_completion_event_has_no_secrets_and_redaction_state_set() {
        let env_name = format!("PLURORA_Z6_SECRET_{}", std::process::id());
        std::env::set_var(&env_name, "super-secret-value");
        struct Guard(String);
        impl Drop for Guard {
            fn drop(&mut self) {
                std::env::remove_var(&self.0);
            }
        }
        let _guard = Guard(env_name.clone());
        let fake = Arc::new(FakeOutboundExecutor::new());
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Runtime::new(
            store.clone(),
            RuntimeConfig {
                outbound_executor: OutboundExecutorConfig::Custom(fake),
                outbound_execute_policy: OutboundExecutePolicyConfig {
                    enabled: true,
                    allowed_hosts: vec!["api.example.com".to_string()],
                    https_only: true,
                    timeout_ms: 30_000,
                    allow_redirects: false,
                    allow_insecure_loopback_for_tests: false,
                },
                secret_resolver: crate::SecretResolverConfig::with_resolver(Arc::new(
                    crate::EnvSecretResolver::from_iter(vec![env_name.clone()]),
                )),
                ..RuntimeConfig::default()
            },
        );
        let secret_ref = format!("secret_ref:env:{env_name}");
        runtime
            .load_package(package_ws("example/z6-secret", vec![secret_ref.clone()]))
            .await
            .unwrap();
        let context = ProtocolContext::package("example/z6-secret", "in_process");
        let _ = runtime.call_protocol(&context, "kernel.v1.outbound.execute", serde_json::json!({
            "capability_id": "example/z6-secret/ws",
            "destination_host": "api.example.com",
            "method": "WEBSOCKET",
            "secret_headers": {"Authorization": {"secret_ref": secret_ref, "scheme": "bearer"}}
        })).await.unwrap();
        let payload = wait_for_event(&store, plurora_core::EVENT_OUTBOUND_EXECUTE_COMPLETED).await;
        let text = serde_json::to_string(&payload).unwrap();
        assert!(text.contains("secret_ref:env:"));
        assert!(!text.contains("super-secret-value"));
        assert_eq!(payload["redaction_state"], "redacted");
    }

    #[tokio::test]
    async fn outbound_completion_event_no_payload_in_websocket() {
        let fake = Arc::new(FakeWebSocketExecutor::with_canned_inbound_frames(vec![
            crate::OutboundWebSocketFrame::Text("raw-frame-payload".to_string()),
        ]));
        let store = Arc::new(InMemoryEventStore::default());
        let runtime = Runtime::new(
            store.clone(),
            RuntimeConfig {
                outbound_websocket_executor: fake,
                ..RuntimeConfig::default()
            },
        );
        runtime
            .load_package(package_ws("example/z6-ws-scrub", vec![]))
            .await
            .unwrap();
        let context = ProtocolContext::package("example/z6-ws-scrub", "in_process");
        let _ = runtime
            .call_protocol(
                &context,
                "kernel.v1.outbound.websocket.open",
                serde_json::json!({
                    "capability_id": "example/z6-ws-scrub/ws",
                    "destination_host": "api.example.com"
                }),
            )
            .await
            .unwrap();
        let payload =
            wait_for_event(&store, plurora_core::EVENT_OUTBOUND_WEBSOCKET_COMPLETED).await;
        assert!(payload.get("payload").is_none());
        assert!(payload.get("body").is_none());
        assert!(payload.get("data").is_none());
        assert!(!serde_json::to_string(&payload)
            .unwrap()
            .contains("raw-frame-payload"));
    }
}
