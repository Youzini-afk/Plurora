use super::{case, ConformanceCase};

macro_rules! c {
    ($id:expr, [$($tag:expr),*], $func:path) => {
        case($id, &[$($tag),*], || Box::pin($func()))
    };
}

pub(super) fn project_intake_lab_external_project_operating_plane_alpha_e1_cases(
) -> Vec<ConformanceCase> {
    vec![
        // --- project-intake-lab (External Project Operating Plane Alpha E1) ---
        c!(
            "project_intake.contract_shape",
            [
                "first_party",
                "external_project",
                "project_intake",
                "no_execution"
            ],
            crate::conformance::project_intake_lab::project_intake_contract
        ),
        c!(
            "project_intake.source_classification",
            [
                "first_party",
                "external_project",
                "project_intake",
                "no_execution"
            ],
            crate::conformance::project_intake_lab::project_intake_source_classification
        ),
        c!(
            "project_intake.stack_detection_npm_lifecycle",
            [
                "first_party",
                "external_project",
                "project_intake",
                "no_execution"
            ],
            crate::conformance::project_intake_lab::project_intake_stack_detection
        ),
        c!(
            "project_intake.workspace_plan_no_execution",
            [
                "first_party",
                "external_project",
                "project_intake",
                "no_execution"
            ],
            crate::conformance::project_intake_lab::project_intake_workspace_plan
        ),
        c!(
            "project_intake.local_path_rejection",
            [
                "first_party",
                "external_project",
                "project_intake",
                "no_execution",
                "secret"
            ],
            crate::conformance::project_intake_lab::project_intake_local_path_rejection
        ),
        c!(
            "project_intake.adapter_plan_no_execution",
            [
                "first_party",
                "external_project",
                "project_intake",
                "no_execution"
            ],
            crate::conformance::project_intake_lab::project_intake_adapter_plan
        ),
        c!(
            "project_intake.no_forbidden_namespace",
            [
                "first_party",
                "external_project",
                "project_intake",
                "no_execution",
                "protocol"
            ],
            crate::conformance::project_intake_lab::project_intake_no_forbidden_namespace
        ),
        c!(
            "project_intake.no_raw_secrets",
            [
                "first_party",
                "external_project",
                "project_intake",
                "no_execution",
                "secret"
            ],
            crate::conformance::project_intake_lab::project_intake_no_raw_secrets
        ),
    ]
}

pub(super) fn project_intake_lab_e5_external_project_operating_plane_alpha_e5_adapte_cases(
) -> Vec<ConformanceCase> {
    vec![
    // --- project-intake-lab E5 (External Project Operating Plane Alpha E5: Adapter/Wrapper Generation Proof) ---
    c!(
        "project_intake.adapter_manifest_preview_no_write",
        [
            "first_party",
            "external_project",
            "project_intake",
            "no_execution"
        ],
        crate::conformance::project_intake_lab::project_intake_adapter_manifest_preview_no_write
    ),
    c!(
        "project_intake.rejects_first_party_adapter_id",
        [
            "first_party",
            "external_project",
            "project_intake",
            "no_execution",
            "secret"
        ],
        crate::conformance::project_intake_lab::project_intake_rejects_first_party_adapter_id
    ),
    c!(
        "project_intake.rejects_path_traversal_adapter_id",
        [
            "first_party",
            "external_project",
            "project_intake",
            "no_execution"
        ],
        crate::conformance::project_intake_lab::project_intake_rejects_path_traversal_adapter_id
    ),
    c!(
        "project_intake.capability_namespace_mismatch_rejected",
        [
            "first_party",
            "external_project",
            "project_intake",
            "no_execution",
            "protocol"
        ],
        crate::conformance::project_intake_lab::project_intake_capability_namespace_mismatch_rejected
    ),
    c!(
        "project_intake.wrapper_preview_no_execution",
        [
            "first_party",
            "external_project",
            "project_intake",
            "no_execution"
        ],
        crate::conformance::project_intake_lab::project_intake_wrapper_preview_no_execution
    ),
    c!(
        "project_intake.fixture_preview_redacted",
        [
            "first_party",
            "external_project",
            "project_intake",
            "no_execution",
            "secret"
        ],
        crate::conformance::project_intake_lab::project_intake_fixture_preview_redacted
    ),
    c!(
        "project_intake.readiness_checklist_ok",
        [
            "first_party",
            "external_project",
            "project_intake",
            "no_execution"
        ],
        crate::conformance::project_intake_lab::project_intake_readiness_checklist_ok
    ),
    c!(
        "project_intake.e5_no_forbidden_namespace_no_raw_secret",
        [
            "first_party",
            "external_project",
            "project_intake",
            "no_execution",
            "secret",
            "protocol"
        ],
        crate::conformance::project_intake_lab::project_intake_e5_no_forbidden_namespace_no_raw_secret
    ),
    ]
}

pub(super) fn workspace_lab_external_project_operating_plane_alpha_e2_cases() -> Vec<ConformanceCase>
{
    vec![
        // --- workspace-lab (External Project Operating Plane Alpha E2) ---
        c!(
            "workspace_lab.contract_shape",
            [
                "first_party",
                "external_project",
                "workspace_lab",
                "policy",
                "no_execution"
            ],
            crate::conformance::workspace_lab::workspace_lab_contract
        ),
        c!(
            "workspace_lab.action_taxonomy_deny_default",
            [
                "first_party",
                "external_project",
                "workspace_lab",
                "policy",
                "no_execution"
            ],
            crate::conformance::workspace_lab::workspace_lab_action_deny_default
        ),
        c!(
            "workspace_lab.policy_mismatch_fail_closed",
            [
                "first_party",
                "external_project",
                "workspace_lab",
                "policy",
                "no_execution"
            ],
            crate::conformance::workspace_lab::workspace_lab_policy_mismatch
        ),
        c!(
            "workspace_lab.raw_secret_blocked",
            [
                "first_party",
                "external_project",
                "workspace_lab",
                "policy",
                "no_execution",
                "secret"
            ],
            crate::conformance::workspace_lab::workspace_lab_raw_secret_blocked
        ),
        c!(
            "workspace_lab.audit_redacted",
            [
                "first_party",
                "external_project",
                "workspace_lab",
                "policy",
                "no_execution"
            ],
            crate::conformance::workspace_lab::workspace_lab_audit_redacted
        ),
        c!(
            "workspace_lab.no_forbidden_namespace",
            [
                "first_party",
                "external_project",
                "workspace_lab",
                "policy",
                "no_execution",
                "protocol"
            ],
            crate::conformance::workspace_lab::workspace_lab_no_forbidden_namespace
        ),
        c!(
            "workspace_lab.no_execution",
            [
                "first_party",
                "external_project",
                "workspace_lab",
                "policy",
                "no_execution"
            ],
            crate::conformance::workspace_lab::workspace_lab_no_execution
        ),
    ]
}

pub(super) fn workspace_lab_e3_external_project_operating_plane_alpha_e3_managed_wor_cases(
) -> Vec<ConformanceCase> {
    vec![
        // --- workspace-lab E3 (External Project Operating Plane Alpha E3: Managed Workspace Deterministic Proof) ---
        c!(
            "workspace_lab.fixture_workspace_creation",
            [
                "first_party",
                "external_project",
                "workspace_lab",
                "managed_workspace",
                "no_execution",
                "fixture"
            ],
            crate::conformance::workspace_lab::workspace_lab_fixture_workspace_creation
        ),
        c!(
            "workspace_lab.inspect_read_no_filesystem",
            [
                "first_party",
                "external_project",
                "workspace_lab",
                "managed_workspace",
                "no_execution",
                "fixture"
            ],
            crate::conformance::workspace_lab::workspace_lab_inspect_read_no_filesystem
        ),
        c!(
            "workspace_lab.run_plan_requires_approval",
            [
                "first_party",
                "external_project",
                "workspace_lab",
                "managed_workspace",
                "no_execution",
                "fixture"
            ],
            crate::conformance::workspace_lab::workspace_lab_run_plan_requires_approval
        ),
        c!(
            "workspace_lab.fixture_process_result_redacted",
            [
                "first_party",
                "external_project",
                "workspace_lab",
                "managed_workspace",
                "no_execution",
                "fixture"
            ],
            crate::conformance::workspace_lab::workspace_lab_fixture_process_result_redacted
        ),
        c!(
            "workspace_lab.entrypoint_discovery",
            [
                "first_party",
                "external_project",
                "workspace_lab",
                "managed_workspace",
                "no_execution",
                "fixture"
            ],
            crate::conformance::workspace_lab::workspace_lab_entrypoint_discovery
        ),
        c!(
            "workspace_lab.patch_draft_proposal",
            [
                "first_party",
                "external_project",
                "workspace_lab",
                "managed_workspace",
                "no_execution",
                "fixture"
            ],
            crate::conformance::workspace_lab::workspace_lab_patch_draft_proposal
        ),
        c!(
            "workspace_lab.e3_raw_secret_no_forbidden_namespace",
            [
                "first_party",
                "external_project",
                "workspace_lab",
                "managed_workspace",
                "no_execution",
                "fixture",
                "secret",
                "protocol"
            ],
            crate::conformance::workspace_lab::workspace_lab_e3_raw_secret_no_forbidden_namespace
        ),
    ]
}

pub(super) fn integrity_lab_package_installation_foundation_i3_cases() -> Vec<ConformanceCase> {
    vec![
        // --- integrity-lab (Package Installation Foundation I3) ---
        c!(
            "integrity.tree_hash_deterministic",
            ["first_party", "integrity", "package_install"],
            crate::conformance::integrity_tools::tree_hash_deterministic
        ),
        c!(
            "integrity.tree_hash_excludes_metadata",
            ["first_party", "integrity", "package_install"],
            crate::conformance::integrity_tools::tree_hash_excludes_metadata
        ),
        c!(
            "integrity.manifest_hash_yaml_json_equivalent",
            ["first_party", "integrity", "package_install"],
            crate::conformance::integrity_tools::manifest_hash_yaml_json_equivalent
        ),
        c!(
            "integrity.gpg_verify_valid_signature",
            ["first_party", "integrity", "package_install"],
            crate::conformance::integrity_tools::gpg_verify_valid_signature
        ),
        c!(
            "integrity.gpg_verify_wrong_key_fails",
            ["first_party", "integrity", "package_install"],
            crate::conformance::integrity_tools::gpg_verify_wrong_key_fails
        ),
        c!(
            "integrity.gpg_verify_invalid_signature_no_panic",
            ["first_party", "integrity", "package_install"],
            crate::conformance::integrity_tools::gpg_verify_invalid_signature_no_panic
        ),
        c!(
            "integrity.fingerprint_extraction_consistent",
            ["first_party", "integrity", "package_install"],
            crate::conformance::integrity_tools::fingerprint_extraction_consistent
        ),
    ]
}

pub(super) fn secret_store_lab_round_10a_1_phase_b_cases() -> Vec<ConformanceCase> {
    vec![
        // --- secret-store-lab (Round 10A.1 Phase B) ---
        c!(
            "secret_store.put_then_has_succeeds",
            ["first_party", "secret_store", "secret"],
            crate::conformance::secret_store::put_then_has_succeeds
        ),
        c!(
            "secret_store.list_returns_names_not_values",
            ["first_party", "secret_store", "secret"],
            crate::conformance::secret_store::list_returns_names_not_values
        ),
        c!(
            "secret_store.delete_removes",
            ["first_party", "secret_store", "secret"],
            crate::conformance::secret_store::delete_removes
        ),
        c!(
            "secret_store.put_invalid_name_rejected",
            ["first_party", "secret_store", "secret"],
            crate::conformance::secret_store::put_invalid_name_rejected
        ),
        c!(
            "secret_store.put_oversized_value_rejected",
            ["first_party", "secret_store", "secret"],
            crate::conformance::secret_store::put_oversized_value_rejected
        ),
        c!(
            "secret_store.health_reports_layout",
            ["first_party", "secret_store", "secret"],
            crate::conformance::secret_store::health_reports_layout
        ),
        c!(
            "secret_store_resolver.resolves_existing",
            ["first_party", "secret_store", "secret"],
            crate::conformance::secret_store::resolver_resolves_existing
        ),
        c!(
            "secret_store_resolver.missing_name_fails_closed",
            ["first_party", "secret_store", "secret"],
            crate::conformance::secret_store::resolver_missing_name_fails_closed
        ),
        c!(
            "secret_store_resolver.non_store_ref_rejected",
            ["first_party", "secret_store", "secret"],
            crate::conformance::secret_store::resolver_non_store_ref_rejected
        ),
        c!(
            "secret_store_resolver.error_does_not_leak_value",
            ["first_party", "secret_store", "secret"],
            crate::conformance::secret_store::resolver_error_does_not_leak_value
        ),
        c!(
            "secret_store_resolver.host_profile_installs_composite_resolver",
            ["first_party", "secret_store", "secret", "host"],
            crate::conformance::secret_store::host_profile_installs_composite_resolver
        ),
    ]
}

pub(super) fn project_scoped_secrets_round_10a_2_wave_2c_cases() -> Vec<ConformanceCase> {
    vec![
        // --- project-scoped secrets (Round 10A.2 Wave 2C) ---
        c!(
            "project_secret.put_then_resolve_via_project_ref",
            ["project", "secret"],
            crate::conformance::project_secret::put_then_resolve_via_project_ref
        ),
        c!(
            "project_secret.fallback_to_platform_when_missing",
            ["project", "secret"],
            crate::conformance::project_secret::fallback_to_platform_when_missing
        ),
        c!(
            "project_secret.no_fallback_when_disabled",
            ["project", "secret"],
            crate::conformance::project_secret::no_fallback_when_disabled
        ),
        c!(
            "project_secret.require_per_project_blocks_fallback",
            ["project", "secret"],
            crate::conformance::project_secret::require_per_project_blocks_fallback
        ),
        c!(
            "project_secret.isolation_between_projects",
            ["project", "secret"],
            crate::conformance::project_secret::isolation_between_projects
        ),
        c!(
            "project_secret.no_session_context_fails_closed",
            ["project", "secret", "outbound"],
            crate::conformance::project_secret::no_session_context_fails_closed
        ),
        c!(
            "project_secret.list_returns_names_not_values",
            ["project", "secret"],
            crate::conformance::project_secret::list_returns_names_not_values
        ),
    ]
}

pub(super) fn project_lifecycle_round_10a_2_wave_3_cases() -> Vec<ConformanceCase> {
    vec![
        // --- project lifecycle (Round 10A.2 Wave 3) ---
        c!(
            "project.detect_native_yaml",
            ["project", "install"],
            crate::conformance::project_lifecycle::detect_native_yaml
        ),
        c!(
            "project.detect_no_yaml",
            ["project", "install"],
            crate::conformance::project_lifecycle::detect_no_yaml
        ),
        c!(
            "project.detect_invalid_yaml_rejected",
            ["project", "install"],
            crate::conformance::project_lifecycle::detect_invalid_yaml_rejected
        ),
        c!(
            "project.register_creates_project_dir",
            ["project", "install"],
            crate::conformance::project_lifecycle::register_creates_project_dir
        ),
        c!(
            "project.list_returns_registered",
            ["project"],
            crate::conformance::project_lifecycle::list_returns_registered
        ),
        c!(
            "project.state_transitions",
            ["project"],
            crate::conformance::project_lifecycle::state_transitions
        ),
        c!(
            "project.archive_keeps_data",
            ["project", "uninstall"],
            crate::conformance::project_lifecycle::archive_keeps_data
        ),
    ]
}

pub(super) fn git_tools_lab_package_installation_foundation_i2_cases() -> Vec<ConformanceCase> {
    vec![
        // --- git-tools-lab (Package Installation Foundation I2) ---
        c!(
            "git_tools.url_validation_https_only",
            ["first_party", "git_tools", "install"],
            crate::conformance::git_tools::url_validation_https_only
        ),
        c!(
            "git_tools.url_validation_no_userinfo",
            ["first_party", "git_tools", "install", "secret"],
            crate::conformance::git_tools::url_validation_no_userinfo
        ),
        c!(
            "git_tools.path_validation_absolute",
            ["first_party", "git_tools", "install"],
            crate::conformance::git_tools::path_validation_absolute
        ),
        c!(
            "git_tools.path_validation_no_traversal",
            ["first_party", "git_tools", "install"],
            crate::conformance::git_tools::path_validation_no_traversal
        ),
        c!(
            "git_tools.read_signed_tag_unsigned",
            ["first_party", "git_tools", "install", "fixture"],
            crate::conformance::git_tools::read_signed_tag_unsigned
        ),
    ]
}

pub(super) fn install_lab_package_installation_foundation_i4_cases() -> Vec<ConformanceCase> {
    vec![
        // --- install-lab (Package Installation Foundation I4) ---
        c!(
            "install_lab.resolve_plan_local_source",
            ["first_party", "install", "package_install", "fixture"],
            crate::conformance::install_lab::resolve_plan_local_source
        ),
        c!(
            "install_lab.project_root_install_registers_surface_dist",
            [
                "first_party",
                "install",
                "package_install",
                "project",
                "surface"
            ],
            crate::conformance::install_lab::project_root_install_registers_surface_dist
        ),
        c!(
            "install_lab.resolve_plan_runs_conformance",
            ["first_party", "install", "package_install", "fixture"],
            crate::conformance::install_lab::resolve_plan_runs_conformance
        ),
        c!(
            "install_lab.resolve_plan_blocks_when_strict",
            ["first_party", "install", "package_install", "fixture"],
            crate::conformance::install_lab::resolve_plan_blocks_when_strict
        ),
        c!(
            "install_lab.strict_conformance_blocks",
            ["first_party", "install", "package_install", "fixture"],
            crate::conformance::install_lab::strict_conformance_blocks
        ),
        c!(
            "install_lab.lenient_conformance_warns_not_blocks",
            ["first_party", "install", "package_install", "fixture"],
            crate::conformance::install_lab::lenient_conformance_warns_not_blocks
        ),
        c!(
            "install_lab.transitive_conformance_propagates",
            ["first_party", "install", "package_install", "fixture"],
            crate::conformance::install_lab::transitive_conformance_propagates
        ),
        c!(
            "install_lab.resolve_plan_with_transitive",
            ["first_party", "install", "package_install", "fixture"],
            crate::conformance::install_lab::resolve_plan_with_transitive
        ),
        c!(
            "install_lab.resolve_plan_cycle_detection",
            ["first_party", "install", "package_install", "fixture"],
            crate::conformance::install_lab::resolve_plan_cycle_detection
        ),
        c!(
            "install_lab.execute_plan_local",
            ["first_party", "install", "package_install", "fixture"],
            crate::conformance::install_lab::execute_plan_local
        ),
        c!(
            "install_lab.execute_plan_consent_mismatch",
            ["first_party", "install", "package_install", "fixture"],
            crate::conformance::install_lab::execute_plan_consent_mismatch
        ),
        c!(
            "install_lab.uninstall_removes_from_profile",
            ["first_party", "install", "package_install", "fixture"],
            crate::conformance::install_lab::uninstall_removes_from_profile
        ),
        c!(
            "install_lab.list_installed_reflects_lockfile",
            ["first_party", "install", "package_install", "fixture"],
            crate::conformance::install_lab::list_installed_reflects_lockfile
        ),
        c!(
            "install_lab.check_lockfile_drift_detection",
            ["first_party", "install", "package_install", "fixture"],
            crate::conformance::install_lab::check_lockfile_drift_detection
        ),
        c!(
            "install_lab.check_for_updates_local_dangling_unsupported",
            [
                "first_party",
                "install",
                "package_install",
                "fixture",
                "update"
            ],
            crate::conformance::install_lab::check_for_updates_local_dangling_unsupported
        ),
        c!(
            "install_lab.check_for_updates_external_project_not_applicable",
            [
                "first_party",
                "install",
                "package_install",
                "fixture",
                "update"
            ],
            crate::conformance::install_lab::check_for_updates_external_project_not_applicable
        ),
        c!(
            "install_lab.update_project_local_replaces_dist_and_lockfile",
            [
                "first_party",
                "install",
                "package_install",
                "fixture",
                "update"
            ],
            crate::conformance::install_lab::update_project_local_replaces_dist_and_lockfile
        ),
        c!(
            "install_lab.update_project_local_current_noop",
            [
                "first_party",
                "install",
                "package_install",
                "fixture",
                "update"
            ],
            crate::conformance::install_lab::update_project_local_current_noop
        ),
        c!(
            "install_lab.update_project_local_force_reinstalls_current",
            [
                "first_party",
                "install",
                "package_install",
                "fixture",
                "update"
            ],
            crate::conformance::install_lab::update_project_local_force_reinstalls_current
        ),
        c!(
            "install_lab.update_project_external_not_applicable",
            [
                "first_party",
                "install",
                "package_install",
                "fixture",
                "update"
            ],
            crate::conformance::install_lab::update_project_external_not_applicable
        ),
        c!(
            "install_lab.update_project_permission_drift_blocks_before_mutation",
            [
                "first_party",
                "install",
                "package_install",
                "fixture",
                "update"
            ],
            crate::conformance::install_lab::update_project_permission_drift_blocks_before_mutation
        ),
        c!(
            "install.real_github_smoke",
            ["install", "real-network", "opt-in"],
            crate::conformance::install_real_smoke::real_github_smoke
        ),
    ]
}
