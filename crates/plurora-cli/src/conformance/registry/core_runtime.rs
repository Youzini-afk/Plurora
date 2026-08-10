use super::{case, ConformanceCase};

macro_rules! c {
    ($id:expr, [$($tag:expr),*], $func:path) => {
        case($id, &[$($tag),*], || Box::pin($func()))
    };
}

pub(super) fn core_cases() -> Vec<ConformanceCase> {
    vec![
        // --- core ---
        c!(
            "session.open_empty",
            ["runtime", "session"],
            crate::conformance::core::session_open
        ),
        c!(
            "event.append_authorized",
            ["runtime", "event"],
            crate::conformance::core::event_append_authorized
        ),
        c!(
            "event.append_without_permission_denied",
            ["runtime", "event"],
            crate::conformance::core::event_append_without_permission_denied
        ),
        c!(
            "event.platform_namespace_denied",
            ["runtime", "event"],
            crate::conformance::core::platform_event_namespace_denied
        ),
        c!(
            "event.read_without_permission_denied",
            ["runtime", "event"],
            crate::conformance::core::event_read_without_permission_denied
        ),
        c!(
            "event.closed_session_rejects_append",
            ["runtime", "event"],
            crate::conformance::core::closed_session_rejects_append
        ),
        c!(
            "event.range_replay",
            ["runtime", "event"],
            crate::conformance::core::event_range_replay
        ),
        c!(
            "capability.invoke_rust_inproc",
            ["runtime", "capability"],
            crate::conformance::core::capability_invoke
        ),
        c!(
            "capability.handle_invoke",
            ["runtime", "capability", "handle"],
            crate::conformance::core::capability_handle_invoke
        ),
        c!(
            "capability.handle_attenuate_invoke",
            ["runtime", "capability", "handle"],
            crate::conformance::core::capability_handle_attenuate_invoke
        ),
        c!(
            "capability.handle_revoke_blocks_invoke",
            ["runtime", "capability", "handle"],
            crate::conformance::core::capability_handle_revoke_blocks_invoke
        ),
        c!(
            "capability.auto_mint_legacy_invoke",
            ["runtime", "capability", "handle"],
            crate::conformance::core::capability_auto_mint_legacy_invoke
        ),
        c!(
            "capability.invoke_events_completed",
            ["runtime", "capability", "audit"],
            crate::conformance::core::capability_invoke_events_completed
        ),
        c!(
            "capability.invoke_events_failed",
            ["runtime", "capability", "audit"],
            crate::conformance::core::capability_invoke_events_failed
        ),
        c!(
            "package.audit_report",
            ["runtime", "audit"],
            crate::conformance::audit::package_audit_report
        ),
        c!(
            "deployment_hub.requires_host_principal",
            ["runtime", "deployment_hub", "permission"],
            crate::conformance::protocol::deployment_hub_requires_host_principal
        ),
        c!(
            "deployment_hub.port_lease_loopback",
            ["runtime", "deployment_hub", "port"],
            crate::conformance::protocol::deployment_hub_port_lease_loopback
        ),
        c!(
            "deployment_hub.proxy_requires_matching_lease_port",
            ["runtime", "deployment_hub", "proxy"],
            crate::conformance::protocol::deployment_hub_proxy_requires_matching_lease_port
        ),
        c!(
            "deployment_hub.exec_stop_receipt",
            ["runtime", "deployment_hub", "exec", "receipt"],
            crate::conformance::protocol::deployment_hub_exec_stop_receipt
        ),
        c!(
            "deployment_hub.exec_terminal_is_observed_once",
            ["runtime", "deployment_hub", "exec", "receipt", "rehydrate"],
            crate::conformance::protocol::deployment_hub_exec_terminal_is_observed_once
        ),
        c!(
            "deployment_hub.exec_denial_is_deduplicated",
            ["runtime", "deployment_hub", "exec", "receipt", "rehydrate"],
            crate::conformance::protocol::deployment_hub_exec_denial_is_deduplicated
        ),
        c!(
            "deployment.sqlite_rehydrate",
            ["deployment", "deployment_hub", "sqlite", "slow"],
            crate::conformance::protocol::deployment_sqlite_rehydrate
        ),
        c!(
            "deployment.reconcile_empty_cleans_stale",
            ["deployment", "deployment_hub", "reconcile"],
            crate::conformance::protocol::deployment_reconcile_empty_cleans_stale
        ),
        c!(
            "deployment.reconcile_promotes_live_container",
            ["deployment", "deployment_hub", "reconcile"],
            crate::conformance::protocol::deployment_reconcile_promotes_live_container
        ),
        c!(
            "deployment.reconcile_exec_always_failed",
            ["deployment", "deployment_hub", "reconcile"],
            crate::conformance::protocol::deployment_reconcile_exec_always_failed
        ),
        c!(
            "capability.ambiguous_provider_denied",
            ["runtime", "capability"],
            crate::conformance::core::ambiguous_provider_denied
        ),
        c!(
            "capability.explicit_provider_selected",
            ["runtime", "capability"],
            crate::conformance::core::explicit_provider_selected
        ),
        c!(
            "package.unload_removes_capability",
            ["runtime", "package"],
            crate::conformance::core::unload_removes_capability
        ),
        c!(
            "first_party.no_privilege",
            ["first_party"],
            crate::conformance::core::first_party_no_privilege
        ),
        c!(
            "schema.capability_input_rejects_invalid",
            ["runtime", "schema"],
            crate::conformance::core::capability_schema_rejects_invalid
        ),
        c!(
            "schema.event_payload_rejects_invalid",
            ["runtime", "schema"],
            crate::conformance::core::event_schema_rejects_invalid
        ),
    ]
}

pub(super) fn permissions_cases() -> Vec<ConformanceCase> {
    vec![
        // --- permissions ---
        c!(
            "protocol.structured_permission_error",
            ["protocol", "permission"],
            crate::conformance::permissions::structured_permission_error
        ),
        c!(
            "permission.grant_revoke_audit",
            ["runtime", "permission"],
            crate::conformance::permissions::permission_grant_revoke_audit
        ),
        c!(
            "permission.assistant_capability_grant",
            ["runtime", "permission"],
            crate::conformance::permissions::assistant_capability_grant
        ),
        c!(
            "principal.package_cannot_self_assert_writer",
            ["runtime", "permission"],
            crate::conformance::permissions::principal_cannot_self_assert_writer
        ),
        c!(
            "principal.package_cannot_self_assert_capability_caller",
            ["runtime", "permission"],
            crate::conformance::permissions::principal_cannot_self_assert_capability_caller
        ),
    ]
}

pub(super) fn host_cases() -> Vec<ConformanceCase> {
    vec![
        // --- host ---
        c!(
            "host.diagnostics",
            ["runtime", "host"],
            crate::conformance::core::host_diagnostics
        ),
        c!(
            "host.profile_autoload",
            ["runtime", "host", "slow"],
            crate::conformance::core::host_profile_autoload
        ),
    ]
}

pub(super) fn surfaces_cases() -> Vec<ConformanceCase> {
    vec![
        // --- surfaces ---
        c!(
            "surface.contribution_list",
            ["surface", "shell:plurora.shell.default/v1"],
            crate::conformance::surfaces::contribution_list
        ),
        c!(
            "surface.shell_descriptor_metadata_validation",
            [
                "surface",
                "manifest",
                "package",
                "shell:plurora.shell.default/v1"
            ],
            crate::conformance::surfaces::shell_descriptor_metadata_validation
        ),
    ]
}

pub(super) fn proposals_cases() -> Vec<ConformanceCase> {
    vec![
        // --- proposals ---
        c!(
            "proposal.lifecycle_apply",
            ["runtime", "proposal"],
            crate::conformance::proposals::lifecycle_apply
        ),
        c!(
            "proposal.reject_and_apply_denied",
            ["runtime", "proposal"],
            crate::conformance::proposals::reject_and_apply_denied
        ),
        c!(
            "proposal.authority_is_enforced",
            ["runtime", "proposal", "permission"],
            crate::conformance::proposals::authority_is_enforced
        ),
        c!(
            "proposal.preflight_failure_is_structured",
            ["runtime", "proposal", "receipt"],
            crate::conformance::proposals::preflight_failure_is_structured
        ),
    ]
}

pub(super) fn asset_cases() -> Vec<ConformanceCase> {
    vec![
        // --- asset ---
        c!(
            "asset.put_get_list",
            ["runtime", "asset"],
            crate::conformance::core::asset_put_get_list
        ),
        c!(
            "asset.legacy_fnv_migration",
            ["runtime", "asset", "migration"],
            crate::conformance::artifact_store::asset_legacy_fnv_migration
        ),
        c!(
            "object_store.portability_integrity",
            ["runtime", "asset", "object_store"],
            crate::conformance::artifact_store::object_store_portability_integrity
        ),
    ]
}

pub(super) fn session_fork_cases() -> Vec<ConformanceCase> {
    vec![
        // --- session fork ---
        c!(
            "session.fork_branch",
            ["runtime", "session"],
            crate::conformance::core::session_fork_branch
        ),
    ]
}

pub(super) fn projection_cases() -> Vec<ConformanceCase> {
    vec![
        // --- projection ---
        c!(
            "projection.rebuild",
            ["runtime", "projection"],
            crate::conformance::core::projection_rebuild
        ),
    ]
}

pub(super) fn substrate_cases() -> Vec<ConformanceCase> {
    vec![
        // --- substrate ---
        c!(
            "substrate.sqlite_rehydrate",
            ["substrate", "slow"],
            crate::conformance::substrate::sqlite_rehydrate
        ),
    ]
}

pub(super) fn protocol_cases() -> Vec<ConformanceCase> {
    vec![
        // --- protocol ---
        c!(
            "protocol.call_host_info",
            ["protocol"],
            crate::conformance::protocol::call_host_info
        ),
        c!(
            "protocol.commons_advertised",
            ["protocol", "commons", "registry", "protocol:registry"],
            crate::conformance::protocol::protocol_commons_advertised
        ),
        c!(
            "protocol.major_mismatch_rejected",
            [
                "protocol",
                "commons",
                "version",
                "compatibility",
                "protocol:plurora.change"
            ],
            crate::conformance::protocol::protocol_major_mismatch_rejected
        ),
        c!(
            "protocol.unknown_id_rejected",
            [
                "protocol",
                "commons",
                "identity",
                "negative",
                "protocol:plurora.change"
            ],
            crate::conformance::protocol::unknown_protocol_id_is_rejected
        ),
        c!(
            "protocol.reports_are_separate",
            [
                "protocol",
                "commons",
                "conformance",
                "protocol:plurora.change"
            ],
            crate::conformance::protocol::protocol_and_implementation_reports_are_separate
        ),
        c!(
            "protocol.single_method_identity",
            ["protocol", "identity", "protocol:plurora.contract"],
            crate::conformance::protocol::single_method_identity
        ),
        c!(
            "protocol.layered_namespace_smoke",
            [
                "protocol",
                "canonical",
                "cli-smoke",
                "protocol:plurora.contract"
            ],
            crate::conformance::protocol::layered_namespace_smoke
        ),
        c!(
            "protocol.unsupported_version_rejected",
            ["protocol", "version", "compatibility"],
            crate::conformance::protocol::unsupported_version_rejected
        ),
        c!(
            "protocol.no_silent_downgrade",
            ["protocol", "version", "compatibility"],
            crate::conformance::protocol::no_silent_downgrade
        ),
        c!(
            "protocol.call_capability_in_process",
            ["protocol"],
            crate::conformance::protocol::call_capability_in_process
        ),
        c!(
            "installation.protocol_crud_uses_service_registry",
            ["protocol", "installation", "service"],
            crate::conformance::protocol_installation::installation_crud_uses_service_registry
        ),
        c!(
            "installation.idempotency_replay_and_conflict",
            ["protocol", "installation", "idempotency"],
            crate::conformance::protocol_installation::installation_idempotency_replay_and_conflict
        ),
        c!(
            "installation.stale_revision_rejected",
            ["protocol", "installation", "concurrency"],
            crate::conformance::protocol_installation::installation_stale_revision_is_rejected
        ),
        c!(
            "installation.state_migration_explicit",
            ["protocol", "installation", "state", "migration"],
            crate::conformance::protocol_installation::installation_state_migration_is_explicit
        ),
        c!(
            "installation.remove_keep_delete_distinct",
            ["protocol", "installation", "state", "remove"],
            crate::conformance::protocol_installation::installation_remove_keep_and_delete_are_distinct
        ),
        c!(
            "installation.journal_restart_rehydrate",
            ["protocol", "installation", "journal", "rehydrate"],
            crate::conformance::protocol_installation::installation_journal_rehydrates_after_restart
        ),
        c!(
            "installation.exact_authority",
            ["protocol", "installation", "permission"],
            crate::conformance::protocol_installation::installation_authority_is_exact
        ),
        c!(
            "installation.lifecycle_events",
            ["protocol", "installation", "event"],
            crate::conformance::protocol_installation::installation_events_match_public_lifecycle
        ),
        c!(
            "installation.retired_methods_invalid_request",
            ["protocol", "installation", "identity", "negative"],
            crate::conformance::protocol_installation::retired_host_methods_are_not_aliases
        ),
        c!(
            "surface.resolve_via_dev_path",
            ["protocol", "surface"],
            crate::conformance::protocol_installation::surface_resolve_via_dev_path
        ),
        c!(
            "surface.resolve_unknown_fails",
            ["protocol", "surface"],
            crate::conformance::protocol_installation::surface_resolve_unknown_fails
        ),
        c!(
            "surface.resolve_admin_principal_required",
            ["protocol", "surface", "permission"],
            crate::conformance::protocol_installation::surface_resolve_admin_principal_required
        ),
    ]
}

pub(super) fn world_bundle_cases() -> Vec<ConformanceCase> {
    vec![
        c!(
            "world_bundle.reference_closure",
            [
                "protocol",
                "portability",
                "world_bundle",
                "protocol:plurora.world.bundle"
            ],
            crate::conformance::world_bundle::reference_closure
        ),
        c!(
            "world_bundle.cross_host_import",
            [
                "protocol",
                "portability",
                "world_bundle",
                "sqlite",
                "protocol:plurora.world.bundle"
            ],
            crate::conformance::world_bundle::cross_host_import
        ),
        c!(
            "world_bundle.offline_replay",
            [
                "protocol",
                "portability",
                "world_bundle",
                "receipt",
                "protocol:plurora.world.bundle"
            ],
            crate::conformance::world_bundle::offline_replay
        ),
        c!(
            "world_bundle.reexecution_branch",
            [
                "protocol",
                "portability",
                "world_bundle",
                "branch",
                "protocol:plurora.world.bundle"
            ],
            crate::conformance::world_bundle::reexecution_branch
        ),
        c!(
            "world_bundle.shell_independence",
            [
                "protocol",
                "portability",
                "world_bundle",
                "cli-smoke",
                "protocol:plurora.world.bundle",
                "shell:plurora.shell.default/v1"
            ],
            crate::conformance::world_bundle::shell_independence
        ),
    ]
}

pub(super) fn hooks_cases() -> Vec<ConformanceCase> {
    vec![
        // --- hooks ---
        c!(
            "hook.ordering_stable",
            ["runtime", "hook"],
            crate::conformance::hooks::ordering_stable
        ),
        c!(
            "hook.veto_blocks_event_append",
            ["runtime", "hook"],
            crate::conformance::hooks::veto_blocks_event_append
        ),
        c!(
            "hook.metadata_mutation_allowed",
            ["runtime", "hook"],
            crate::conformance::hooks::metadata_mutation_allowed
        ),
        c!(
            "hook.package_owned_handler",
            ["runtime", "hook"],
            crate::conformance::hooks::package_owned_handler
        ),
        c!(
            "hook.unload_removes_subscription",
            ["runtime", "hook"],
            crate::conformance::hooks::unload_removes_subscription
        ),
    ]
}

pub(super) fn work_cases() -> Vec<ConformanceCase> {
    vec![
        // --- Work / Assembly authoring ---
        c!(
            "work.nested_exposure",
            ["work", "assembly"],
            crate::conformance::generated::work_source_nested_exposure
        ),
        c!(
            "work.digest_is_deterministic",
            ["work", "assembly"],
            crate::conformance::generated::work_source_digest_is_deterministic
        ),
        c!(
            "assembly.component_identity_independent_of_package_envelope",
            ["work", "assembly", "phase7"],
            crate::conformance::generated::component_identity_independent_of_package_envelope
        ),
        c!(
            "assembly.component_replacement_preserves_content_roots",
            ["work", "assembly", "phase7"],
            crate::conformance::generated::component_replacement_preserves_content_roots
        ),
        c!(
            "work.contract_none_is_foreign_capsule",
            ["work", "assembly", "phase7"],
            crate::conformance::generated::contract_none_is_foreign_capsule
        ),
        c!(
            "first_party.asset_lab",
            ["first_party", "slow"],
            crate::conformance::first_party_labs::asset_lab
        ),
        c!(
            "first_party.projection_lab",
            ["first_party", "slow"],
            crate::conformance::first_party_labs::projection_lab
        ),
        c!(
            "first_party.playable_seed",
            ["first_party", "slow"],
            crate::conformance::first_party_labs::playable_seed
        ),
        c!(
            "first_party.persona_lab",
            ["first_party", "slow"],
            crate::conformance::first_party_labs::persona_lab
        ),
        c!(
            "first_party.knowledge_lab",
            ["first_party", "slow"],
            crate::conformance::first_party_labs::knowledge_lab
        ),
        c!(
            "first_party.context_lab",
            ["first_party", "slow"],
            crate::conformance::first_party_labs::context_lab
        ),
        c!(
            "first_party.text_transform_lab",
            ["first_party", "slow"],
            crate::conformance::first_party_labs::text_transform_lab
        ),
        c!(
            "first_party.model_connector_lab",
            ["first_party", "slow"],
            crate::conformance::first_party_labs::model_connector_lab
        ),
        c!(
            "first_party.model_provider_lab",
            ["first_party", "slow"],
            crate::conformance::first_party_labs::model_provider_lab
        ),
        c!(
            "first_party.model_provider_lab_invoke_core",
            ["first_party", "slow"],
            crate::conformance::first_party_labs::model_provider_lab_invoke_core
        ),
        c!(
            "first_party.model_provider_lab_normalize_stream",
            ["first_party", "slow"],
            crate::conformance::first_party_labs::model_provider_lab_normalize_stream
        ),
        c!(
            "first_party.model_routing_lab",
            ["first_party", "slow"],
            crate::conformance::first_party_labs::model_routing_lab
        ),
        c!(
            "first_party.pi_agent_runtime_lab",
            ["first_party", "agentic", "slow"],
            crate::conformance::first_party_labs::pi_agent_runtime_lab
        ),
        c!(
            "first_party.capability_tool_bridge_lab",
            ["first_party", "agentic", "slow"],
            crate::conformance::first_party_labs::capability_tool_bridge_lab
        ),
    ]
}

pub(super) fn replacement_cases() -> Vec<ConformanceCase> {
    vec![
        // --- replacement ---
        c!(
            "replacement.thirdparty_seed_surfaces",
            ["replacement"],
            crate::conformance::replacement::thirdparty_seed_surfaces
        ),
        c!(
            "replacement.thirdparty_seed_invocation",
            ["replacement"],
            crate::conformance::replacement::thirdparty_seed_invocation
        ),
        c!(
            "replacement.ambiguous_no_publisher_priority",
            ["replacement"],
            crate::conformance::replacement::ambiguous_no_publisher_priority
        ),
        c!(
            "replacement.work_thirdparty",
            ["replacement", "work"],
            crate::conformance::replacement::work_thirdparty
        ),
        c!(
            "replacement.thirdparty_agent_runtime_surfaces",
            ["replacement", "agentic"],
            crate::conformance::replacement::thirdparty_agent_runtime_surfaces
        ),
        c!(
            "replacement.thirdparty_agent_runtime_invocation",
            ["replacement", "agentic"],
            crate::conformance::replacement::thirdparty_agent_runtime_invocation
        ),
        c!(
            "replacement.work_agent_runtime_replacement",
            ["replacement", "agentic", "work"],
            crate::conformance::replacement::work_agent_runtime_replacement
        ),
    ]
}

pub(super) fn secret_conformance_cases() -> Vec<ConformanceCase> {
    vec![
        // --- secret conformance ---
        c!(
            "substrate.permission_grant_rehydrate",
            ["substrate", "secret"],
            crate::conformance::secret_conformance::permission_grant_rehydrate
        ),
        c!(
            "secret.ref_validation",
            ["secret"],
            crate::conformance::secret_conformance::secret_ref_validation
        ),
        c!(
            "secret.raw_blocked_in_proposal",
            ["secret"],
            crate::conformance::secret_conformance::raw_secret_blocked_in_proposal
        ),
        c!(
            "secret.effect_receipt_redacts_raw_fields",
            ["secret", "receipt", "runtime"],
            crate::conformance::secret_conformance::effect_receipt_redacts_raw_secret_fields
        ),
        c!(
            "secret.raw_blocked_in_asset_metadata",
            ["secret"],
            crate::conformance::secret_conformance::raw_secret_blocked_in_asset_metadata
        ),
        c!(
            "first_party.no_secret_bypass",
            ["first_party", "secret"],
            crate::conformance::secret_conformance::no_secret_bypass
        ),
        c!(
            "secret.env_resolver_allowed",
            ["secret"],
            crate::conformance::secret_conformance::env_resolver_allowed
        ),
        c!(
            "secret.env_resolver_denied",
            ["secret"],
            crate::conformance::secret_conformance::env_resolver_denied
        ),
        c!(
            "secret.env_resolver_missing_no_leak",
            ["secret"],
            crate::conformance::secret_conformance::env_resolver_missing_no_leak
        ),
    ]
}
