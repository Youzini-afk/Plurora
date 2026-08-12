use super::{case, ConformanceCase};

macro_rules! c {
    ($id:expr, [$($tag:expr),*], $func:path) => {
        case($id, &[$($tag),*], || Box::pin($func()))
    };
}

pub(super) fn source_intake_lab_external_source_operating_plane_alpha_e1_cases(
) -> Vec<ConformanceCase> {
    vec![
        // --- source-intake-lab (External Source Operating Plane Alpha E1) ---
        c!(
            "source_intake.contract_shape",
            [
                "first_party",
                "external_source",
                "source_intake",
                "no_execution"
            ],
            crate::conformance::source_intake_lab::source_intake_contract
        ),
        c!(
            "source_intake.source_classification",
            [
                "first_party",
                "external_source",
                "source_intake",
                "no_execution"
            ],
            crate::conformance::source_intake_lab::source_intake_source_classification
        ),
        c!(
            "source_intake.stack_detection_npm_lifecycle",
            [
                "first_party",
                "external_source",
                "source_intake",
                "no_execution"
            ],
            crate::conformance::source_intake_lab::source_intake_stack_detection
        ),
        c!(
            "source_intake.workspace_plan_no_execution",
            [
                "first_party",
                "external_source",
                "source_intake",
                "no_execution"
            ],
            crate::conformance::source_intake_lab::source_intake_workspace_plan
        ),
        c!(
            "source_intake.local_path_rejection",
            [
                "first_party",
                "external_source",
                "source_intake",
                "no_execution",
                "secret"
            ],
            crate::conformance::source_intake_lab::source_intake_local_path_rejection
        ),
        c!(
            "source_intake.adapter_plan_no_execution",
            [
                "first_party",
                "external_source",
                "source_intake",
                "no_execution"
            ],
            crate::conformance::source_intake_lab::source_intake_adapter_plan
        ),
        c!(
            "source_intake.no_forbidden_namespace",
            [
                "first_party",
                "external_source",
                "source_intake",
                "no_execution",
                "protocol"
            ],
            crate::conformance::source_intake_lab::source_intake_no_forbidden_namespace
        ),
        c!(
            "source_intake.no_raw_secrets",
            [
                "first_party",
                "external_source",
                "source_intake",
                "no_execution",
                "secret"
            ],
            crate::conformance::source_intake_lab::source_intake_no_raw_secrets
        ),
    ]
}

pub(super) fn source_intake_lab_e5_external_source_operating_plane_alpha_e5_adapte_cases(
) -> Vec<ConformanceCase> {
    vec![
        // --- source-intake-lab E5 (External Source Operating Plane Alpha E5: Adapter/Wrapper Generation Proof) ---
        c!(
            "source_intake.adapter_manifest_preview_no_write",
            [
                "first_party",
                "external_source",
                "source_intake",
                "no_execution"
            ],
            crate::conformance::source_intake_lab::source_intake_adapter_manifest_preview_no_write
        ),
        c!(
            "source_intake.rejects_first_party_adapter_id",
            [
                "first_party",
                "external_source",
                "source_intake",
                "no_execution",
                "secret"
            ],
            crate::conformance::source_intake_lab::source_intake_rejects_first_party_adapter_id
        ),
        c!(
            "source_intake.rejects_path_traversal_adapter_id",
            [
                "first_party",
                "external_source",
                "source_intake",
                "no_execution"
            ],
            crate::conformance::source_intake_lab::source_intake_rejects_path_traversal_adapter_id
        ),
        c!(
        "source_intake.capability_namespace_mismatch_rejected",
        [
            "first_party",
            "external_source",
            "source_intake",
            "no_execution",
            "protocol"
        ],
        crate::conformance::source_intake_lab::source_intake_capability_namespace_mismatch_rejected
    ),
        c!(
            "source_intake.wrapper_preview_no_execution",
            [
                "first_party",
                "external_source",
                "source_intake",
                "no_execution"
            ],
            crate::conformance::source_intake_lab::source_intake_wrapper_preview_no_execution
        ),
        c!(
            "source_intake.fixture_preview_redacted",
            [
                "first_party",
                "external_source",
                "source_intake",
                "no_execution",
                "secret"
            ],
            crate::conformance::source_intake_lab::source_intake_fixture_preview_redacted
        ),
        c!(
            "source_intake.readiness_checklist_ok",
            [
                "first_party",
                "external_source",
                "source_intake",
                "no_execution"
            ],
            crate::conformance::source_intake_lab::source_intake_readiness_checklist_ok
        ),
        c!(
        "source_intake.e5_no_forbidden_namespace_no_raw_secret",
        [
            "first_party",
            "external_source",
            "source_intake",
            "no_execution",
            "secret",
            "protocol"
        ],
        crate::conformance::source_intake_lab::source_intake_e5_no_forbidden_namespace_no_raw_secret
    ),
    ]
}

pub(super) fn workspace_lab_external_source_operating_plane_alpha_e2_cases() -> Vec<ConformanceCase>
{
    vec![
        // --- workspace-lab (External Source Operating Plane Alpha E2) ---
        c!(
            "workspace_lab.contract_shape",
            [
                "first_party",
                "external_source",
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
                "external_source",
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
                "external_source",
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
                "external_source",
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
                "external_source",
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
                "external_source",
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
                "external_source",
                "workspace_lab",
                "policy",
                "no_execution"
            ],
            crate::conformance::workspace_lab::workspace_lab_no_execution
        ),
    ]
}

pub(super) fn workspace_lab_e3_external_source_operating_plane_alpha_e3_managed_wor_cases(
) -> Vec<ConformanceCase> {
    vec![
        // --- workspace-lab E3 (External Source Operating Plane Alpha E3: Managed Workspace Deterministic Proof) ---
        c!(
            "workspace_lab.fixture_workspace_creation",
            [
                "first_party",
                "external_source",
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
                "external_source",
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
                "external_source",
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
                "external_source",
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
                "external_source",
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
                "external_source",
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
                "external_source",
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

pub(super) fn installation_scoped_secret_cases() -> Vec<ConformanceCase> {
    vec![
        // --- Installation-scoped secret policy and runtime resolution ---
        c!(
            "installation_secret.put_resolve_owned_path",
            ["installation", "secret"],
            crate::conformance::installation_secret::put_resolve_uses_installation_owned_path
        ),
        c!(
            "installation_secret.policy_fallback_and_retired_scheme",
            ["installation", "secret", "policy", "negative"],
            crate::conformance::installation_secret::policy_controls_fallback_and_rejects_retired_scheme
        ),
        c!(
            "installation_secret.isolation_and_list_redaction",
            ["installation", "secret", "redaction"],
            crate::conformance::installation_secret::installation_secret_isolation_and_list_redaction
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
        // --- install-lab (Work candidate and Installation hand-off) ---
        c!(
            "install_lab.detect_source_work_package_foreign",
            ["first_party", "installation", "source", "fixture"],
            crate::conformance::install_lab::detect_source_classifies_work_package_and_foreign
        ),
        c!(
            "install_lab.resolve_plan_local_package",
            ["first_party", "installation", "package", "fixture"],
            crate::conformance::install_lab::resolve_plan_local_package
        ),
        c!(
            "install_lab.resolve_plan_runs_conformance",
            ["first_party", "installation", "package", "fixture"],
            crate::conformance::install_lab::resolve_plan_runs_conformance
        ),
        c!(
            "install_lab.invalid_manifest_rejected_strict",
            ["first_party", "installation", "package", "fixture"],
            crate::conformance::install_lab::invalid_manifest_is_rejected_in_strict_mode
        ),
        c!(
            "install_lab.invalid_manifest_rejected_lenient",
            ["first_party", "installation", "package", "fixture"],
            crate::conformance::install_lab::invalid_manifest_is_rejected_in_lenient_mode
        ),
        c!(
            "install_lab.invalid_transitive_manifest_rejected",
            ["first_party", "installation", "package", "fixture"],
            crate::conformance::install_lab::invalid_transitive_manifest_is_rejected
        ),
        c!(
            "install_lab.resolve_plan_with_transitive",
            ["first_party", "installation", "package", "fixture"],
            crate::conformance::install_lab::resolve_plan_with_transitive
        ),
        c!(
            "install_lab.resolve_plan_cycle_detection",
            ["first_party", "installation", "package", "fixture"],
            crate::conformance::install_lab::resolve_plan_cycle_detection
        ),
        c!(
            "install_lab.execute_emits_installation_candidate",
            ["first_party", "installation", "package", "fixture"],
            crate::conformance::install_lab::execute_plan_emits_installation_candidate
        ),
        c!(
            "install_lab.execute_persists_candidate_objects",
            ["first_party", "installation", "object_store", "fixture"],
            crate::conformance::install_lab::execute_plan_persists_candidate_objects
        ),
        c!(
            "install_lab.execute_consent_mismatch",
            ["first_party", "installation", "policy", "fixture"],
            crate::conformance::install_lab::execute_plan_consent_mismatch
        ),
        c!(
            "install_lab.execute_source_drift_rejected",
            ["first_party", "installation", "integrity", "fixture"],
            crate::conformance::install_lab::execute_plan_rejects_source_drift
        ),
        c!(
            "install_lab.work_source_requires_pack",
            ["first_party", "installation", "work", "fixture"],
            crate::conformance::install_lab::work_source_requires_pack
        ),
        c!(
            "install_lab.external_intake_managed_workspace",
            ["first_party", "installation", "workspace", "fixture"],
            crate::conformance::install_lab::external_intake_creates_managed_workspace
        ),
        c!(
            "install_lab.external_intake_linked_local_preserved",
            ["first_party", "installation", "workspace", "fixture"],
            crate::conformance::install_lab::linked_local_intake_preserves_user_source
        ),
        c!(
            "install_lab.external_workspace_installation_candidate",
            ["first_party", "installation", "workspace", "fixture"],
            crate::conformance::install_lab::external_workspace_emits_installation_candidate
        ),
        c!(
            "install.real_github_smoke",
            ["install", "real-network", "opt-in"],
            crate::conformance::install_real_smoke::real_github_smoke
        ),
    ]
}
