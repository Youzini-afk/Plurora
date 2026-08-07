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
                "official",
                "external_project",
                "project_intake",
                "no_execution"
            ],
            crate::conformance::project_intake_lab::project_intake_contract
        ),
        c!(
            "project_intake.source_classification",
            [
                "official",
                "external_project",
                "project_intake",
                "no_execution"
            ],
            crate::conformance::project_intake_lab::project_intake_source_classification
        ),
        c!(
            "project_intake.stack_detection_npm_lifecycle",
            [
                "official",
                "external_project",
                "project_intake",
                "no_execution"
            ],
            crate::conformance::project_intake_lab::project_intake_stack_detection
        ),
        c!(
            "project_intake.workspace_plan_no_execution",
            [
                "official",
                "external_project",
                "project_intake",
                "no_execution"
            ],
            crate::conformance::project_intake_lab::project_intake_workspace_plan
        ),
        c!(
            "project_intake.local_path_rejection",
            [
                "official",
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
                "official",
                "external_project",
                "project_intake",
                "no_execution"
            ],
            crate::conformance::project_intake_lab::project_intake_adapter_plan
        ),
        c!(
            "project_intake.no_forbidden_namespace",
            [
                "official",
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
                "official",
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
            "official",
            "external_project",
            "project_intake",
            "no_execution"
        ],
        crate::conformance::project_intake_lab::project_intake_adapter_manifest_preview_no_write
    ),
    c!(
        "project_intake.rejects_official_adapter_id",
        [
            "official",
            "external_project",
            "project_intake",
            "no_execution",
            "secret"
        ],
        crate::conformance::project_intake_lab::project_intake_rejects_official_adapter_id
    ),
    c!(
        "project_intake.rejects_path_traversal_adapter_id",
        [
            "official",
            "external_project",
            "project_intake",
            "no_execution"
        ],
        crate::conformance::project_intake_lab::project_intake_rejects_path_traversal_adapter_id
    ),
    c!(
        "project_intake.capability_namespace_mismatch_rejected",
        [
            "official",
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
            "official",
            "external_project",
            "project_intake",
            "no_execution"
        ],
        crate::conformance::project_intake_lab::project_intake_wrapper_preview_no_execution
    ),
    c!(
        "project_intake.fixture_preview_redacted",
        [
            "official",
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
            "official",
            "external_project",
            "project_intake",
            "no_execution"
        ],
        crate::conformance::project_intake_lab::project_intake_readiness_checklist_ok
    ),
    c!(
        "project_intake.e5_no_forbidden_namespace_no_raw_secret",
        [
            "official",
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
                "official",
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
                "official",
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
                "official",
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
                "official",
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
                "official",
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
                "official",
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
                "official",
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
                "official",
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
                "official",
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
                "official",
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
                "official",
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
                "official",
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
                "official",
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
                "official",
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
            ["official", "integrity", "package_install"],
            crate::conformance::integrity_tools::tree_hash_deterministic
        ),
        c!(
            "integrity.tree_hash_excludes_metadata",
            ["official", "integrity", "package_install"],
            crate::conformance::integrity_tools::tree_hash_excludes_metadata
        ),
        c!(
            "integrity.manifest_hash_yaml_json_equivalent",
            ["official", "integrity", "package_install"],
            crate::conformance::integrity_tools::manifest_hash_yaml_json_equivalent
        ),
        c!(
            "integrity.gpg_verify_valid_signature",
            ["official", "integrity", "package_install"],
            crate::conformance::integrity_tools::gpg_verify_valid_signature
        ),
        c!(
            "integrity.gpg_verify_wrong_key_fails",
            ["official", "integrity", "package_install"],
            crate::conformance::integrity_tools::gpg_verify_wrong_key_fails
        ),
        c!(
            "integrity.gpg_verify_invalid_signature_no_panic",
            ["official", "integrity", "package_install"],
            crate::conformance::integrity_tools::gpg_verify_invalid_signature_no_panic
        ),
        c!(
            "integrity.fingerprint_extraction_consistent",
            ["official", "integrity", "package_install"],
            crate::conformance::integrity_tools::fingerprint_extraction_consistent
        ),
    ]
}

pub(super) fn secret_store_lab_round_10a_1_phase_b_cases() -> Vec<ConformanceCase> {
    vec![
        // --- secret-store-lab (Round 10A.1 Phase B) ---
        c!(
            "secret_store.put_then_has_succeeds",
            ["official", "secret_store", "secret"],
            crate::conformance::secret_store::put_then_has_succeeds
        ),
        c!(
            "secret_store.list_returns_names_not_values",
            ["official", "secret_store", "secret"],
            crate::conformance::secret_store::list_returns_names_not_values
        ),
        c!(
            "secret_store.delete_removes",
            ["official", "secret_store", "secret"],
            crate::conformance::secret_store::delete_removes
        ),
        c!(
            "secret_store.put_invalid_name_rejected",
            ["official", "secret_store", "secret"],
            crate::conformance::secret_store::put_invalid_name_rejected
        ),
        c!(
            "secret_store.put_oversized_value_rejected",
            ["official", "secret_store", "secret"],
            crate::conformance::secret_store::put_oversized_value_rejected
        ),
        c!(
            "secret_store.health_reports_layout",
            ["official", "secret_store", "secret"],
            crate::conformance::secret_store::health_reports_layout
        ),
        c!(
            "secret_store_resolver.resolves_existing",
            ["official", "secret_store", "secret"],
            crate::conformance::secret_store::resolver_resolves_existing
        ),
        c!(
            "secret_store_resolver.missing_name_fails_closed",
            ["official", "secret_store", "secret"],
            crate::conformance::secret_store::resolver_missing_name_fails_closed
        ),
        c!(
            "secret_store_resolver.non_store_ref_rejected",
            ["official", "secret_store", "secret"],
            crate::conformance::secret_store::resolver_non_store_ref_rejected
        ),
        c!(
            "secret_store_resolver.error_does_not_leak_value",
            ["official", "secret_store", "secret"],
            crate::conformance::secret_store::resolver_error_does_not_leak_value
        ),
        c!(
            "secret_store_resolver.host_profile_installs_composite_resolver",
            ["official", "secret_store", "secret", "host"],
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
            ["official", "git_tools", "install"],
            crate::conformance::git_tools::url_validation_https_only
        ),
        c!(
            "git_tools.url_validation_no_userinfo",
            ["official", "git_tools", "install", "secret"],
            crate::conformance::git_tools::url_validation_no_userinfo
        ),
        c!(
            "git_tools.path_validation_absolute",
            ["official", "git_tools", "install"],
            crate::conformance::git_tools::path_validation_absolute
        ),
        c!(
            "git_tools.path_validation_no_traversal",
            ["official", "git_tools", "install"],
            crate::conformance::git_tools::path_validation_no_traversal
        ),
        c!(
            "git_tools.read_signed_tag_unsigned",
            ["official", "git_tools", "install", "fixture"],
            crate::conformance::git_tools::read_signed_tag_unsigned
        ),
    ]
}

pub(super) fn install_lab_package_installation_foundation_i4_cases() -> Vec<ConformanceCase> {
    vec![
        // --- install-lab (Package Installation Foundation I4) ---
        c!(
            "install_lab.resolve_plan_local_source",
            ["official", "install", "package_install", "fixture"],
            crate::conformance::install_lab::resolve_plan_local_source
        ),
        c!(
            "install_lab.project_root_install_registers_surface_dist",
            [
                "official",
                "install",
                "package_install",
                "project",
                "surface"
            ],
            crate::conformance::install_lab::project_root_install_registers_surface_dist
        ),
        c!(
            "install_lab.resolve_plan_runs_conformance",
            ["official", "install", "package_install", "fixture"],
            crate::conformance::install_lab::resolve_plan_runs_conformance
        ),
        c!(
            "install_lab.resolve_plan_blocks_when_strict",
            ["official", "install", "package_install", "fixture"],
            crate::conformance::install_lab::resolve_plan_blocks_when_strict
        ),
        c!(
            "install_lab.strict_conformance_blocks",
            ["official", "install", "package_install", "fixture"],
            crate::conformance::install_lab::strict_conformance_blocks
        ),
        c!(
            "install_lab.lenient_conformance_warns_not_blocks",
            ["official", "install", "package_install", "fixture"],
            crate::conformance::install_lab::lenient_conformance_warns_not_blocks
        ),
        c!(
            "install_lab.transitive_conformance_propagates",
            ["official", "install", "package_install", "fixture"],
            crate::conformance::install_lab::transitive_conformance_propagates
        ),
        c!(
            "install_lab.resolve_plan_with_transitive",
            ["official", "install", "package_install", "fixture"],
            crate::conformance::install_lab::resolve_plan_with_transitive
        ),
        c!(
            "install_lab.resolve_plan_cycle_detection",
            ["official", "install", "package_install", "fixture"],
            crate::conformance::install_lab::resolve_plan_cycle_detection
        ),
        c!(
            "install_lab.execute_plan_local",
            ["official", "install", "package_install", "fixture"],
            crate::conformance::install_lab::execute_plan_local
        ),
        c!(
            "install_lab.execute_plan_consent_mismatch",
            ["official", "install", "package_install", "fixture"],
            crate::conformance::install_lab::execute_plan_consent_mismatch
        ),
        c!(
            "install_lab.uninstall_removes_from_profile",
            ["official", "install", "package_install", "fixture"],
            crate::conformance::install_lab::uninstall_removes_from_profile
        ),
        c!(
            "install_lab.list_installed_reflects_lockfile",
            ["official", "install", "package_install", "fixture"],
            crate::conformance::install_lab::list_installed_reflects_lockfile
        ),
        c!(
            "install_lab.check_lockfile_drift_detection",
            ["official", "install", "package_install", "fixture"],
            crate::conformance::install_lab::check_lockfile_drift_detection
        ),
        c!(
            "install_lab.check_for_updates_local_dangling_unsupported",
            [
                "official",
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
                "official",
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
                "official",
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
                "official",
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
                "official",
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
                "official",
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
                "official",
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
