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
                "host.outbound.execute",
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
                "host.outbound.execute",
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
                "host.outbound.execute",
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
                "host.outbound.execute",
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
                "host.outbound.execute",
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
    use std::sync::Arc;

    use crate::{
        InMemoryEventStore, InMemoryObjectStore, InstallationAuthorityRefresh,
        InstallationAuthoritySubject, InstallationAuthorityValidator, InstallationControl,
        InstallationCreateRequest, InstallationListRequest, InstallationMutationResult,
        InstallationRemoveRequest, InstallationUpdateRequest, InstallationView,
        InstallationWorkSummary, ObjectStore, OpenSessionRequest, ProtocolContext,
        ProtocolResourceSelector, Runtime, RuntimeConfig,
    };
    use async_trait::async_trait;
    use plurora_core::ArtifactDescriptor;
    use plurora_work::{
        AcquisitionKind, AcquisitionRecord, ArtifactModel, InstallationId, InstallationRecord,
        InstallationSecretPolicy, InstallationStatus, RightDisposition, RightsDeclaration, WorkId,
        WorkRevision, ASSEMBLY_LOCK_TYPE_URI, ASSEMBLY_REVISION_TYPE_URI, WORK_REVISION_TYPE_URI,
    };

    #[derive(Clone)]
    struct FakeInstallationControl {
        installations: Vec<InstallationView>,
    }

    struct ExactWorkAuthority(WorkId);

    #[async_trait]
    impl InstallationAuthorityValidator for ExactWorkAuthority {
        async fn validate_current(
            &self,
            grant_id: &str,
            subject: &InstallationAuthoritySubject,
        ) -> anyhow::Result<()> {
            anyhow::ensure!(grant_id == "grant-exact-work", "unexpected test grant");
            anyhow::ensure!(
                subject == &InstallationAuthoritySubject::Work(self.0.clone()),
                "unexpected test authority subject"
            );
            Ok(())
        }
    }

    #[async_trait]
    impl InstallationControl for FakeInstallationControl {
        async fn list(
            &self,
            request: InstallationListRequest,
        ) -> anyhow::Result<Vec<InstallationView>> {
            Ok(self
                .installations
                .iter()
                .filter(|view| {
                    request
                        .status
                        .is_none_or(|status| view.record.status == status)
                })
                .cloned()
                .collect())
        }

        async fn get(
            &self,
            installation_id: &InstallationId,
        ) -> anyhow::Result<Option<InstallationView>> {
            Ok(self
                .installations
                .iter()
                .find(|view| &view.record.installation_id == installation_id)
                .cloned())
        }

        async fn create(
            &self,
            request: InstallationCreateRequest,
        ) -> anyhow::Result<InstallationMutationResult> {
            request.validate()?;
            let now = chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")?
                .with_timezone(&chrono::Utc);
            Ok(InstallationMutationResult {
                installation: InstallationView {
                    work_summary: work_summary(request.work_id.clone(), &request.display_name),
                    record: InstallationRecord {
                        schema_version: InstallationRecord::SCHEMA_VERSION,
                        installation_id: InstallationId::new(),
                        work_revision: request.work_revision,
                        assembly_lock: request.assembly_lock,
                        display_name: request.display_name,
                        source: request.source,
                        state_bindings: request.state_bindings,
                        secret_policy: request.secret_policy,
                        created_at: now,
                        updated_at: now,
                        status: InstallationStatus::Ready,
                    },
                    revision: 1,
                    rollback: None,
                },
                diff: None,
                receipts: Vec::new(),
                idempotent: false,
            })
        }

        async fn update(
            &self,
            _request: InstallationUpdateRequest,
        ) -> anyhow::Result<InstallationMutationResult> {
            anyhow::bail!("not used")
        }

        async fn remove(
            &self,
            _request: InstallationRemoveRequest,
        ) -> anyhow::Result<InstallationMutationResult> {
            anyhow::bail!("not used")
        }
    }

    fn artifact(kind: &str, byte: char) -> ArtifactDescriptor {
        ArtifactDescriptor {
            artifact_type_uri: kind.to_string(),
            media_type: "application/json".to_string(),
            digest: format!("sha256:{}", byte.to_string().repeat(64)),
            size_bytes: 1,
            references: Vec::new(),
            annotations: Default::default(),
        }
    }

    fn work_summary(work_id: WorkId, title: &str) -> InstallationWorkSummary {
        InstallationWorkSummary {
            work_id,
            title: title.to_string(),
            description: String::new(),
            content_roots: Vec::new(),
            entrypoints: Vec::new(),
            rights: None,
            transparency: None,
            rights_declaration: None,
            transparency_declaration: None,
            operational_intent: None,
            annotations: Default::default(),
        }
    }

    fn installation(id: InstallationId, title: &str, byte: char) -> InstallationView {
        let now = chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        InstallationView {
            work_summary: work_summary(
                WorkId::parse(format!("tests/installation-{byte}")).expect("valid Work id"),
                title,
            ),
            record: InstallationRecord {
                schema_version: InstallationRecord::SCHEMA_VERSION,
                installation_id: id,
                work_revision: artifact(WORK_REVISION_TYPE_URI, byte),
                assembly_lock: artifact(ASSEMBLY_LOCK_TYPE_URI, byte),
                display_name: title.to_string(),
                source: AcquisitionRecord {
                    kind: AcquisitionKind::WorkBundle,
                    source_ref: None,
                    provenance_refs: Vec::new(),
                    update_channel: None,
                },
                state_bindings: Vec::new(),
                secret_policy: InstallationSecretPolicy::default(),
                created_at: now,
                updated_at: now,
                status: InstallationStatus::Ready,
            },
            revision: 1,
            rollback: None,
        }
    }

    fn installation_device(installation_id: &str) -> ProtocolContext {
        ProtocolContext::host_device(
            "grant-installation-a",
            vec!["observe".into(), "run".into()],
            vec![ProtocolResourceSelector {
                owner: "host".into(),
                kind: "installation".into(),
                id: Some(installation_id.into()),
            }],
            Vec::new(),
            "test",
        )
    }

    #[tokio::test]
    async fn installation_device_sees_only_its_exact_installation() {
        let installation_a = InstallationId::new();
        let installation_b = InstallationId::new();
        let control = Arc::new(FakeInstallationControl {
            installations: vec![
                installation(installation_a.clone(), "Installation A", 'a'),
                installation(installation_b.clone(), "Installation B", 'b'),
            ],
        });
        let runtime = Runtime::new(
            Arc::new(InMemoryEventStore::default()),
            RuntimeConfig {
                installation_control: control,
                ..RuntimeConfig::default()
            },
        );
        let context = installation_device(installation_a.as_str());
        let listed = runtime
            .call_protocol(&context, "host.installation.list", serde_json::json!({}))
            .await
            .expect("list visible installations");
        let installations = listed.as_array().expect("installations array");
        assert_eq!(installations.len(), 1);
        assert_eq!(
            installations[0]["record"]["installation_id"],
            installation_a.as_str()
        );

        assert!(runtime
            .call_protocol(
                &context,
                "host.installation.get",
                serde_json::json!({"installation_id": installation_b}),
            )
            .await
            .is_err());

        let open_request = serde_json::to_value(OpenSessionRequest {
            labels: vec![format!("installation:{}", installation_a)],
            active_package_set: Vec::new(),
            metadata: serde_json::json!({"installation_id": installation_a}),
        })
        .expect("serialize open-session request");
        assert!(runtime
            .call_protocol(&context, "context.open", open_request.clone(),)
            .await
            .is_err());

        // Product Run authority is not substrate session administration.
        let opened = runtime
            .call_protocol(
                &ProtocolContext::host_admin("test"),
                "context.open",
                open_request,
            )
            .await
            .expect("Host administrator opens substrate context");
        let session_id = opened["id"].as_str().expect("session id");
        assert!(runtime
            .call_protocol(
                &context,
                "context.fork",
                serde_json::json!({
                    "parent_session_id": session_id,
                    "forked_from_sequence": 0,
                    "metadata": {"reason": "authority-test"}
                }),
            )
            .await
            .is_err());
        runtime
            .call_protocol(
                &ProtocolContext::host_admin("test"),
                "context.close",
                serde_json::json!({"session_id": session_id}),
            )
            .await
            .expect("Host administrator closes substrate context");
    }

    #[tokio::test]
    async fn installation_create_requires_idempotency_and_exact_work_authority() {
        let control = Arc::new(FakeInstallationControl {
            installations: Vec::new(),
        });
        let object_store = Arc::new(InMemoryObjectStore::new());
        let work_id = WorkId::parse("tests/installation-create").unwrap();
        let work_model = WorkRevision {
            schema: WorkRevision::SCHEMA.to_string(),
            work_id: work_id.clone(),
            title: "Example".to_string(),
            description: String::new(),
            assembly: artifact(ASSEMBLY_REVISION_TYPE_URI, 'e'),
            content_roots: Vec::new(),
            entrypoints: Vec::new(),
            rights: None,
            transparency: None,
            operational_intent: None,
            annotations: Default::default(),
        };
        let work = work_model.artifact_descriptor().unwrap();
        object_store
            .put(work_model.canonical_bytes().unwrap().into())
            .await
            .unwrap();
        let runtime = Runtime::new(
            Arc::new(InMemoryEventStore::default()),
            RuntimeConfig {
                installation_control: control,
                object_store,
                ..RuntimeConfig::default()
            },
        );
        let params = serde_json::json!({
            "work_id": work_id,
            "work_revision": work,
            "assembly_lock": artifact(ASSEMBLY_LOCK_TYPE_URI, 'd'),
            "display_name": "Example",
            "source": {"kind": "work_bundle"},
            "state_bindings": [],
            "secret_policy": {},
            "idempotency_key": ""
        });
        let blank = runtime
            .call_protocol(
                &ProtocolContext::host_dev("test"),
                "host.installation.create",
                params.clone(),
            )
            .await
            .expect_err("blank idempotency key must fail before control invocation");
        assert!(blank.message.contains("non-empty idempotency_key"));

        let mut valid = params;
        valid["idempotency_key"] = serde_json::json!("install-1");
        let denied = ProtocolContext::host_device(
            "grant-wrong-work",
            vec!["installation.manage".into()],
            vec![ProtocolResourceSelector {
                owner: "host".into(),
                kind: "work".into(),
                id: Some("tests/wrong-work".into()),
            }],
            Vec::new(),
            "test",
        );
        let error = runtime
            .call_protocol(&denied, "host.installation.create", valid.clone())
            .await
            .expect_err("create must require the exact Work identity");
        assert!(error.message.contains("exact Work"));

        let authorized = ProtocolContext::host_device(
            "grant-exact-work",
            vec!["installation.manage".into()],
            vec![ProtocolResourceSelector {
                owner: "host".into(),
                kind: "work".into(),
                id: Some(work_id.to_string()),
            }],
            Vec::new(),
            "test",
        )
        .with_verified_authority_expiry(Some(chrono::Utc::now().timestamp_millis() + 60_000))
        .with_installation_authority_refresh(InstallationAuthorityRefresh::new(Arc::new(
            ExactWorkAuthority(work_id),
        )));
        let created = runtime
            .call_protocol(&authorized, "host.installation.create", valid)
            .await
            .expect("an explicit test control receives an exactly authorized create");
        assert_eq!(created["installation"]["record"]["display_name"], "Example");
    }

    #[tokio::test]
    async fn installation_update_and_remove_require_the_exact_installation() {
        let installation_a = InstallationId::new();
        let installation_b = InstallationId::new();
        let runtime = Runtime::new(
            Arc::new(InMemoryEventStore::default()),
            RuntimeConfig {
                installation_control: Arc::new(FakeInstallationControl {
                    installations: vec![installation(
                        installation_b.clone(),
                        "Installation B",
                        'b',
                    )],
                }),
                ..RuntimeConfig::default()
            },
        );
        let context = ProtocolContext::host_device(
            "grant-installation-a-manage",
            vec!["installation.manage".into()],
            vec![ProtocolResourceSelector {
                owner: "host".into(),
                kind: "installation".into(),
                id: Some(installation_a.as_str().to_string()),
            }],
            Vec::new(),
            "test",
        );

        for (method, params) in [
            (
                "host.installation.update",
                serde_json::json!({
                    "installation_id": installation_b.as_str(),
                    "expected_revision": 1,
                    "work_revision": artifact(WORK_REVISION_TYPE_URI, 'c'),
                    "assembly_lock": artifact(ASSEMBLY_LOCK_TYPE_URI, 'd'),
                    "state_action": {"kind": "preserve"},
                    "idempotency_key": "update-1"
                }),
            ),
            (
                "host.installation.remove",
                serde_json::json!({
                    "installation_id": installation_b.as_str(),
                    "expected_revision": 1,
                    "state_disposition": "keep",
                    "idempotency_key": "remove-1"
                }),
            ),
        ] {
            let error = runtime
                .call_protocol(&context, method, params)
                .await
                .expect_err("a different exact installation must be denied");
            assert!(error.message.contains("exact installation"));
        }
    }

    #[tokio::test]
    async fn installation_backup_denied_by_rights_never_reaches_state_control() {
        let objects = Arc::new(InMemoryObjectStore::new());
        let rights = RightsDeclaration {
            license_expression: Some("LicenseRef-no-backup".to_string()),
            terms_uri: None,
            install: RightDisposition::Allowed,
            execute: RightDisposition::Allowed,
            backup: RightDisposition::Denied,
            export_state: RightDisposition::Denied,
            copy_across_hosts: RightDisposition::Denied,
            redistribute_artifacts: RightDisposition::Denied,
            modify: RightDisposition::Denied,
            derive: RightDisposition::Denied,
            modding: RightDisposition::Denied,
            dedicated_server: RightDisposition::Denied,
            entitlement_requirements: Vec::new(),
            evidence_refs: Vec::new(),
        };
        let rights_ref = rights.artifact_descriptor().unwrap();
        objects
            .put(rights.canonical_bytes().unwrap().into())
            .await
            .unwrap();
        let work = WorkRevision {
            schema: WorkRevision::SCHEMA.to_string(),
            work_id: WorkId::parse("tests/no-backup").unwrap(),
            title: "No backup".to_string(),
            description: String::new(),
            assembly: artifact(ASSEMBLY_REVISION_TYPE_URI, 'f'),
            content_roots: Vec::new(),
            entrypoints: Vec::new(),
            rights: Some(rights_ref),
            transparency: None,
            operational_intent: None,
            annotations: Default::default(),
        };
        let work_ref = work.artifact_descriptor().unwrap();
        objects
            .put(work.canonical_bytes().unwrap().into())
            .await
            .unwrap();
        let installation_id = InstallationId::new();
        let runtime = Runtime::new(
            Arc::new(InMemoryEventStore::default()),
            RuntimeConfig {
                object_store: objects,
                installation_control: Arc::new(FakeInstallationControl {
                    installations: vec![installation(installation_id.clone(), "No backup", 'f')],
                }),
                ..RuntimeConfig::default()
            },
        );
        let error = runtime
            .call_protocol(
                &ProtocolContext::host_dev("rights-test"),
                "host.installation.update",
                serde_json::json!({
                    "installation_id": installation_id,
                    "expected_revision": 1,
                    "work_revision": work_ref,
                    "assembly_lock": artifact(ASSEMBLY_LOCK_TYPE_URI, 'f'),
                    "state_action": {"kind": "backup"},
                    "idempotency_key": "backup-denied"
                }),
            )
            .await
            .expect_err("Denied backup Rights must fail before state control");
        assert_eq!(error.code, "runtime/error/rights_denied");
        assert!(!error.message.contains("No backup"));
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
    async fn exact_installation_device_cannot_enumerate_global_surface_catalogue() {
        let runtime = Runtime::new(
            Arc::new(InMemoryEventStore::default()),
            RuntimeConfig::default(),
        );
        let exact_id = InstallationId::new();
        let exact = installation_device(exact_id.as_str());
        assert!(runtime
            .call_protocol(&exact, "shell.contribution.list", serde_json::json!({}),)
            .await
            .is_err());
        for method in [
            "host.package.list",
            "capability.discover",
            "object.put",
            "object.get",
            "object.list",
            "projection.list",
        ] {
            let params = if method == "object.get" {
                serde_json::json!({"asset_id": "missing"})
            } else {
                serde_json::json!({})
            };
            assert!(
                runtime.call_protocol(&exact, method, params).await.is_err(),
                "exact-installation authority must not enumerate Host-global method {method}"
            );
        }

        let global = ProtocolContext::host_device(
            "grant-all-installations",
            vec!["observe".into()],
            vec![ProtocolResourceSelector {
                owner: "host".into(),
                kind: "installation".into(),
                id: None,
            }],
            Vec::new(),
            "test",
        );
        assert_eq!(
            runtime
                .call_protocol(&global, "shell.contribution.list", serde_json::json!({}),)
                .await
                .expect("all-installation device can enumerate the Host catalogue"),
            serde_json::json!([])
        );
    }

    #[tokio::test]
    async fn object_put_authority_distinguishes_exact_artifacts_from_ordinary_assets() {
        fn exact_params(scope: serde_json::Value) -> serde_json::Value {
            let bytes = b"scoped exact artifact";
            serde_json::json!({
                "mime": "application/octet-stream",
                "content": bytes.iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
                "metadata": {},
                "artifact": {
                    "descriptor": {
                        "artifact_type_uri": "urn:plurora:test-scoped-object:v1",
                        "media_type": "application/octet-stream",
                        "digest": crate::sha256_digest(bytes),
                        "size_bytes": bytes.len(),
                        "references": [],
                        "annotations": {}
                    },
                    "content_encoding": "hex",
                    "scope": scope
                }
            })
        }

        let runtime = Runtime::new(
            Arc::new(InMemoryEventStore::default()),
            RuntimeConfig::default(),
        );
        let work_id = WorkId::parse("tests/scoped-object").unwrap();
        let other_work_id = WorkId::parse("tests/other-object").unwrap();
        let installation_id = InstallationId::new();
        let other_installation_id = InstallationId::new();
        let exact = ProtocolContext::host_device(
            "grant-exact-object",
            vec!["installation.manage".into()],
            vec![
                ProtocolResourceSelector {
                    owner: "host".into(),
                    kind: "work".into(),
                    id: Some(work_id.to_string()),
                },
                ProtocolResourceSelector {
                    owner: "host".into(),
                    kind: "installation".into(),
                    id: Some(installation_id.to_string()),
                },
            ],
            Vec::new(),
            "test",
        );

        let create_scope = serde_json::json!({
            "kind": "installation_create",
            "work_id": work_id,
        });
        let created = runtime
            .call_protocol(&exact, "object.put", exact_params(create_scope))
            .await
            .expect("exact Work-scoped upload");
        assert!(created["asset"].is_null());
        assert!(runtime
            .call_protocol(
                &exact,
                "object.put",
                exact_params(serde_json::json!({
                    "kind": "installation_create",
                    "work_id": other_work_id,
                })),
            )
            .await
            .is_err());
        assert!(runtime
            .call_protocol(
                &exact,
                "object.put",
                exact_params(serde_json::json!({
                    "kind": "installation_update",
                    "installation_id": other_installation_id,
                    "work_id": work_id,
                })),
            )
            .await
            .is_err());

        let ordinary = serde_json::json!({
            "mime": "text/plain",
            "content": "ordinary Asset",
            "metadata": {}
        });
        assert!(runtime
            .call_protocol(&exact, "object.put", ordinary.clone())
            .await
            .is_err());
        let access_manager = ProtocolContext::host_device(
            "grant-assets",
            vec!["access_manage".into(), "observe".into()],
            vec![ProtocolResourceSelector {
                owner: "host".into(),
                kind: "installation".into(),
                id: None,
            }],
            Vec::new(),
            "test",
        );
        let stored = runtime
            .call_protocol(&access_manager, "object.put", ordinary)
            .await
            .expect("ordinary Asset upload");
        assert!(stored["asset"]["id"].is_string());
        let asset_id = stored["asset"]["id"].as_str().unwrap();
        let fetched = runtime
            .call_protocol(
                &access_manager,
                "object.get",
                serde_json::json!({"asset_id": asset_id}),
            )
            .await
            .expect("original ordinary Asset object.get request");
        assert_eq!(fetched["record"], stored["asset"]);
        assert_eq!(fetched["content"], "ordinary Asset");
        assert!(fetched
            .as_object()
            .is_some_and(|response| response.len() == 2));
        assert!(runtime
            .call_protocol(
                &access_manager,
                "object.get",
                serde_json::json!({"kind": "asset", "asset_id": asset_id}),
            )
            .await
            .is_err());
        assert_eq!(runtime.list_assets().await.len(), 1);
    }
}

#[cfg(test)]
mod surface_tests {
    use std::sync::Arc;

    use crate::{InMemoryEventStore, ProtocolContext, Runtime, RuntimeConfig};
    use plurora_core::PackageManifest;

    #[tokio::test]
    async fn resolve_bundle_uses_a_loaded_package_root_without_exposing_metadata() {
        let package_root = tempfile::tempdir().expect("package root");
        std::fs::create_dir(package_root.path().join("dist")).expect("dist dir");
        std::fs::write(
            package_root.path().join("dist/bundle.mjs"),
            "export const ok = true;",
        )
        .expect("write bundle");
        let manifest: PackageManifest = serde_yaml::from_str(
            r#"
schema_version: 1
id: example/surface-package
version: 0.1.0
entry:
  kind: surface_bundle
  bundle: dist/bundle.mjs
contributes:
  surfaces:
    - id: pkg/surface/entry
      version: 0.1.0
      slot: experience_entry
      title: Package Surface
      metadata:
        host_path: /secret/path
        requested_capabilities: [attacker/metadata_grant]
permissions: {}
"#,
        )
        .expect("manifest");

        let mut config = RuntimeConfig::default();
        config
            .package_roots
            .insert(manifest.id.clone(), package_root.path().to_path_buf());
        let runtime = Runtime::new(Arc::new(InMemoryEventStore::default()), config);
        runtime.load_package(manifest).await.expect("load package");
        let value = runtime
            .call_protocol(
                &ProtocolContext::host_dev("test"),
                "host.surface.bundle.resolve",
                serde_json::json!({ "surface_id": "pkg/surface/entry" }),
            )
            .await
            .expect("resolve bundle");

        assert!(
            value.get("metadata").is_none(),
            "resolve_bundle must not expose arbitrary package metadata: {value:?}"
        );
        assert_eq!(value["package_id"], "example/surface-package");
        assert!(value["bundle_url"]
            .as_str()
            .unwrap()
            .starts_with("/surface-bundles/packages/example/surface-package/dist/bundle.mjs?v="));
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
                "host.surface.bundle.resolve",
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
                "host.exec.start",
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
                "host.port.lease",
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
                "host.proxy.register",
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
                "host.port.lease",
                serde_json::json!({"target_id":"local","port_name":"web"}),
            )
            .await
            .expect("lease succeeds");
        let lease_id = lease["lease"]["id"].as_str().expect("lease id");

        let registered = runtime
            .call_protocol(
                &context,
                "host.proxy.register",
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
                "host.port.lease",
                serde_json::json!({"target_id":"local","port_name":"web"}),
            )
            .await;

        let error = result.expect_err("anonymous deployment hub call must be denied");
        assert_eq!(error.code, "runtime/error/permission_denied");
    }

    #[tokio::test]
    async fn proxy_register_requires_matching_port_name() {
        let runtime = runtime();
        let context = ProtocolContext::host_dev("in_process");

        let lease = runtime
            .call_protocol(
                &context,
                "host.port.lease",
                serde_json::json!({"target_id":"local","port_name":"web"}),
            )
            .await
            .expect("lease succeeds");
        let lease_id = lease["lease"]["id"].as_str().expect("lease id");

        let mismatch = runtime
            .call_protocol(
                &context,
                "host.proxy.register",
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
                "host.outbound.websocket.open",
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
                "host.outbound.websocket.open",
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
                "host.outbound.websocket.open",
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
                "host.outbound.execute",
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
                "host.outbound.execute",
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
                "host.outbound.execute",
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
                "host.outbound.stream",
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
                "host.outbound.stream",
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
                "capability.cancel",
                serde_json::json!({
                    "stream_id": stream_id,
                    "session_id": "platform_outbound_stream_example_z6-stream-cancel"
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
                "host.outbound.websocket.open",
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
        let _ = runtime.call_protocol(&context, "host.outbound.execute", serde_json::json!({
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
                "host.outbound.websocket.open",
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
