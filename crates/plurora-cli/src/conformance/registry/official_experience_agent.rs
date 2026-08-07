use super::{case, ConformanceCase};

macro_rules! c {
    ($id:expr, [$($tag:expr),*], $func:path) => {
        case($id, &[$($tag),*], || Box::pin($func()))
    };
}

pub(super) fn official_foundation_labs_cases() -> Vec<ConformanceCase> {
    vec![
        // --- official foundation / labs ---
        c!(
            "official.foundation_packages",
            ["official", "slow"],
            crate::conformance::official_foundation::foundation_packages
        ),
        c!(
            "official.assistant_lab_proposal",
            ["official", "slow"],
            crate::conformance::official_labs::assistant_lab_proposal
        ),
        c!(
            "play_creation.blank_loop",
            ["official", "slow"],
            crate::conformance::official_play_creation::blank_play_creation_loop
        ),
        c!(
            "deployment_hub.local_exec_default_deny_all",
            ["deployment_hub", "exec", "permission"],
            crate::conformance::official_labs::deployment_hub_local_exec_default_deny_all
        ),
        c!(
            "docker_runtime_lab.contract_and_plan",
            ["deployment_hub", "docker", "official"],
            crate::conformance::official_labs::docker_runtime_lab_contract_and_plan
        ),
        c!(
            "docker_runtime_lab.blocks_dangerous_spec",
            ["deployment_hub", "docker", "official", "security"],
            crate::conformance::official_labs::docker_runtime_lab_blocks_dangerous_spec
        ),
        c!(
            "docker_runtime_lab.build_image_blocks_secret_and_non_dockerfile",
            ["deployment_hub", "docker", "official", "security", "build"],
            crate::conformance::official_labs::docker_runtime_lab_build_image_blocks_secret_and_non_dockerfile
        ),
    ]
}

pub(super) fn inference_local_cases() -> Vec<ConformanceCase> {
    vec![
        // --- inference local ---
        c!(
            "official.inference_local_lab_describe_capabilities",
            ["official", "slow"],
            crate::conformance::inference_local::inference_local_lab_describe_capabilities
        ),
        c!(
            "official.inference_local_lab_invoke",
            ["official", "slow"],
            crate::conformance::inference_local::inference_local_lab_invoke
        ),
        c!(
            "official.inference_local_lab_invoke_rejects_http",
            ["official", "slow"],
            crate::conformance::inference_local::inference_local_lab_invoke_rejects_http
        ),
        c!(
            "official.inference_local_lab_stream",
            ["official", "slow"],
            crate::conformance::inference_local::inference_local_lab_stream
        ),
        c!(
            "official.inference_local_lab_explain_error",
            ["official", "slow"],
            crate::conformance::inference_local::inference_local_lab_explain_error
        ),
    ]
}

pub(super) fn inference_playtest_cases() -> Vec<ConformanceCase> {
    vec![
        // --- inference playtest ---
        c!(
            "official.inference_playtest_lab_draft",
            ["official", "slow"],
            crate::conformance::inference_playtest::inference_playtest_draft
        ),
        c!(
            "official.inference_playtest_lab_inspect",
            ["official", "slow"],
            crate::conformance::inference_playtest::inference_playtest_inspect
        ),
        c!(
            "official.inference_playtest_lab_reject_apply_denied",
            ["official", "slow"],
            crate::conformance::inference_playtest::inference_playtest_reject_apply_denied
        ),
        c!(
            "official.inference_playtest_lab_apply_and_branch",
            ["official", "slow"],
            crate::conformance::inference_playtest::inference_playtest_apply_and_branch
        ),
        c!(
            "official.inference_playtest_lab_no_chat_kernel_terms",
            ["official", "slow"],
            crate::conformance::inference_playtest::inference_playtest_no_chat_kernel_terms
        ),
    ]
}

pub(super) fn agentic_forge_phase_a_cases() -> Vec<ConformanceCase> {
    vec![
        // --- agentic forge Phase A ---
        c!(
            "agentic_forge.describe_contract",
            ["agentic"],
            crate::conformance::agentic_forge::agentic_forge_describe_contract
        ),
        c!(
            "agentic_forge.start_run_plan_graph_working_state",
            ["agentic"],
            crate::conformance::agentic_forge::agentic_forge_start_run
        ),
        c!(
            "agentic_forge.inspect_cancel_summarize",
            ["agentic"],
            crate::conformance::agentic_forge::agentic_forge_inspect_cancel_summarize
        ),
        c!(
            "agentic_forge.raw_secret_blocked",
            ["agentic", "secret"],
            crate::conformance::agentic_forge::agentic_forge_raw_secret_blocked
        ),
        c!(
            "agentic_forge.no_kernel_agent_namespace",
            ["agentic"],
            crate::conformance::agentic_forge::agentic_forge_no_kernel_agent_namespace
        ),
    ]
}

pub(super) fn agentic_forge_phase_b_cases() -> Vec<ConformanceCase> {
    vec![
        // --- agentic forge Phase B ---
        c!(
            "agentic_forge.create_candidate_branch_aware",
            ["agentic"],
            crate::conformance::agentic_forge::agentic_forge_create_candidate
        ),
        c!(
            "agentic_forge.compare_candidate_stale_detection",
            ["agentic"],
            crate::conformance::agentic_forge::agentic_forge_compare_candidate
        ),
        c!(
            "agentic_forge.draft_promote_proposal_no_mutation",
            ["agentic"],
            crate::conformance::agentic_forge::agentic_forge_draft_promote_proposal
        ),
        c!(
            "agentic_forge.stale_promote_blocked",
            ["agentic"],
            crate::conformance::agentic_forge::agentic_forge_stale_promote_blocked
        ),
        c!(
            "agentic_forge.archive_candidate_target_unchanged",
            ["agentic"],
            crate::conformance::agentic_forge::agentic_forge_archive_candidate
        ),
    ]
}

pub(super) fn agentic_forge_phase_c_cases() -> Vec<ConformanceCase> {
    vec![
        // --- agentic forge Phase C ---
        c!(
            "agentic_forge.inference_node_deterministic_candidate_seed",
            ["agentic"],
            crate::conformance::agentic_forge::agentic_forge_inference_node_deterministic
        ),
        c!(
            "agentic_forge.replay_match_mismatch_flagged",
            ["agentic"],
            crate::conformance::agentic_forge::agentic_forge_replay_match_mismatch
        ),
        c!(
            "agentic_forge.inference_output_privilege_escalation_rejected",
            ["agentic"],
            crate::conformance::agentic_forge::agentic_forge_inference_output_validation
        ),
        c!(
            "agentic_forge.cloud_adapter_needs_host_policy_no_network",
            ["agentic", "network"],
            crate::conformance::agentic_forge::agentic_forge_cloud_adapter_no_network
        ),
        c!(
            "agentic_forge.inference_failure_taxonomy_recovery_hints",
            ["agentic"],
            crate::conformance::agentic_forge::agentic_forge_inference_failure_taxonomy
        ),
    ]
}

pub(super) fn agentic_forge_phase_d_cases() -> Vec<ConformanceCase> {
    vec![
        // --- agentic forge Phase D ---
        c!(
            "agentic_forge.explain_tool_call_scoped_no_ambient_authority",
            ["agentic"],
            crate::conformance::agentic_forge::agentic_forge_explain_tool_call_scoped
        ),
        c!(
            "agentic_forge.record_observation_untrusted_large_output_redaction",
            ["agentic"],
            crate::conformance::agentic_forge::agentic_forge_record_observation_untrusted
        ),
        c!(
            "agentic_forge.tool_risk_injection_exfiltration_outbound",
            ["agentic", "network"],
            crate::conformance::agentic_forge::agentic_forge_tool_risk_categories
        ),
        c!(
            "agentic_forge.replay_tool_plan_mismatch_flagged",
            ["agentic"],
            crate::conformance::agentic_forge::agentic_forge_replay_tool_mismatch
        ),
        c!(
            "agentic_forge.plan_toolchain_requires_explicit_provider_nested_delegation_blocked",
            ["agentic"],
            crate::conformance::agentic_forge::agentic_forge_plan_toolchain_requires_provider
        ),
    ]
}

pub(super) fn agentic_forge_phase_f_cases() -> Vec<ConformanceCase> {
    vec![
        // --- agentic forge Phase F ---
        c!(
            "agentic_forge.thirdparty_replacement_shape_no_official_priority",
            ["agentic", "replacement"],
            crate::conformance::agentic_forge::agentic_forge_thirdparty_replacement_shape
        ),
        c!(
            "agentic_forge.no_official_priority_ordinary_package",
            ["agentic", "official"],
            crate::conformance::agentic_forge::agentic_forge_no_official_priority
        ),
        c!(
            "agentic_forge.hostile_injection_secret_blocked_cross_package",
            ["agentic", "secret"],
            crate::conformance::agentic_forge::agentic_forge_hostile_injection_secret_blocked
        ),
        c!(
            "agentic_forge.budget_deadline_contract_cancellation_consistent",
            ["agentic"],
            crate::conformance::agentic_forge::agentic_forge_budget_deadline_contract
        ),
        c!(
            "agentic_forge.cross_package_replay_mismatch_flagged",
            ["agentic"],
            crate::conformance::agentic_forge::agentic_forge_cross_package_replay_consistency
        ),
    ]
}

pub(super) fn experience_runtime_cases() -> Vec<ConformanceCase> {
    vec![
        // --- experience runtime ---
        c!(
            "experience_runtime.describe_contract_shape",
            ["experience"],
            crate::conformance::experience_runtime::experience_runtime_describe_contract
        ),
        c!(
            "experience_runtime.checkpoint_recovery_shape",
            ["experience"],
            crate::conformance::experience_runtime::experience_runtime_checkpoint_shape
        ),
        c!(
            "experience_runtime.recovery_shape",
            ["experience"],
            crate::conformance::experience_runtime::experience_runtime_recovery_shape
        ),
        c!(
            "experience_runtime.no_kernel_experience_namespace",
            ["experience"],
            crate::conformance::experience_runtime::experience_runtime_no_kernel_namespace
        ),
        c!(
            "experience_runtime.template_generation",
            ["experience", "generated"],
            crate::conformance::experience_runtime::experience_runtime_template_generation
        ),
        c!(
            "experience_runtime.bind_agent_run_shape",
            ["experience", "agentic"],
            crate::conformance::experience_runtime::experience_runtime_bind_agent_run
        ),
    ]
}

pub(super) fn playable_board_beta_1_cases() -> Vec<ConformanceCase> {
    vec![
    // --- playable board Beta 1 ---
    c!(
        "playable_board.describe_contract_shape",
        ["experience"],
        crate::conformance::playable_creation_board::playable_board_describe_contract
    ),
    c!(
        "playable_board.launch_and_player_actions",
        ["experience"],
        crate::conformance::playable_creation_board::playable_board_launch_and_player_actions
    ),
    c!(
        "playable_board.checkpoint_recovery_shape",
        ["experience"],
        crate::conformance::playable_creation_board::playable_board_checkpoint_recovery
    ),
    c!(
        "playable_board.request_change_no_chat",
        ["experience"],
        crate::conformance::playable_creation_board::playable_board_request_change_no_chat
    ),
    c!(
        "playable_board.bind_agent_run_scoped",
        ["experience", "agentic"],
        crate::conformance::playable_creation_board::playable_board_bind_agent_run_scoped
    ),
    c!(
        "playable_board.candidate_proposal_no_target_mutation",
        ["experience"],
        crate::conformance::playable_creation_board::playable_board_candidate_proposal_no_target_mutation
    ),
    c!(
        "playable_board.reject_approve_fork_proof",
        ["experience"],
        crate::conformance::playable_creation_board::playable_board_reject_approve_fork_proof
    ),
    c!(
        "playable_board.thirdparty_no_official_priority",
        ["experience", "replacement"],
        crate::conformance::playable_creation_board::playable_board_thirdparty_no_official_priority
    ),
    c!(
        "playable_board.no_forbidden_namespace",
        ["experience"],
        crate::conformance::playable_creation_board::playable_board_no_forbidden_namespace
    ),
    c!(
        "playable_board.no_raw_secrets",
        ["experience", "secret"],
        crate::conformance::playable_creation_board::playable_board_no_raw_secrets
    ),
    ]
}

pub(super) fn playable_board_beta_2_cases() -> Vec<ConformanceCase> {
    vec![
        // --- playable board Beta 2 ---
        c!(
            "playable_board.content_address_stable",
            ["experience"],
            crate::conformance::playable_creation_board::playable_board_content_address_stable
        ),
        c!(
            "playable_board.checkpoint_metadata",
            ["experience"],
            crate::conformance::playable_creation_board::playable_board_checkpoint_metadata
        ),
        c!(
            "playable_board.provenance_graph",
            ["experience"],
            crate::conformance::playable_creation_board::playable_board_provenance_graph
        ),
        c!(
            "playable_board.state_diff_preview",
            ["experience"],
            crate::conformance::playable_creation_board::playable_board_state_diff_preview
        ),
        c!(
            "playable_board.describe_asset_provenance",
            ["experience"],
            crate::conformance::playable_creation_board::playable_board_describe_asset_provenance
        ),
        c!(
            "playable_board.beta2_no_raw_secrets",
            ["experience", "secret"],
            crate::conformance::playable_creation_board::playable_board_beta2_no_raw_secrets
        ),
        c!(
            "official.asset_lab_content_address",
            ["official", "slow"],
            crate::conformance::official_labs::asset_lab_content_address
        ),
        c!(
            "official.asset_lab_provenance_graph",
            ["official", "slow"],
            crate::conformance::official_labs::asset_lab_provenance_graph
        ),
        c!(
            "official.projection_lab_state_snapshot",
            ["official", "slow"],
            crate::conformance::official_labs::projection_lab_state_snapshot
        ),
    ]
}

pub(super) fn experience_observability_beta_3_cases() -> Vec<ConformanceCase> {
    vec![
    // --- experience observability Beta 3 ---
    c!(
        "experience_observability.contract_shape",
        ["experience"],
        crate::conformance::experience_observability::experience_observability_contract
    ),
    c!(
        "experience_observability.session_health",
        ["experience"],
        crate::conformance::experience_observability::experience_observability_session_health
    ),
    c!(
        "experience_observability.package_health",
        ["experience"],
        crate::conformance::experience_observability::experience_observability_package_health
    ),
    c!(
        "experience_observability.agent_run_health",
        ["experience", "agentic"],
        crate::conformance::experience_observability::experience_observability_agent_run_health
    ),
    c!(
        "experience_observability.proposal_causality",
        ["experience"],
        crate::conformance::experience_observability::experience_observability_proposal_causality
    ),
    c!(
        "experience_observability.cost_latency_summary",
        ["experience"],
        crate::conformance::experience_observability::experience_observability_cost_latency
    ),
    c!(
        "experience_observability.failure_breadcrumbs",
        ["experience"],
        crate::conformance::experience_observability::experience_observability_failure_breadcrumbs
    ),
    c!(
        "experience_observability.guardrail_audit_summary",
        ["experience"],
        crate::conformance::experience_observability::experience_observability_guardrail_summary
    ),
    c!(
        "experience_observability.no_forbidden_namespace",
        ["experience"],
        crate::conformance::experience_observability::experience_observability_no_forbidden_namespace
    ),
    c!(
        "experience_observability.no_raw_secrets",
        ["experience", "secret"],
        crate::conformance::experience_observability::experience_observability_no_raw_secrets
    ),
    ]
}

pub(super) fn memory_lab_beta_4_cases() -> Vec<ConformanceCase> {
    vec![
        // --- memory lab Beta 4 ---
        c!(
            "memory_lab.contract_shape",
            ["memory"],
            crate::conformance::memory_lab::memory_lab_contract
        ),
        c!(
            "memory_lab.record_memory",
            ["memory"],
            crate::conformance::memory_lab::memory_lab_record_memory
        ),
        c!(
            "memory_lab.retrieve_memory",
            ["memory"],
            crate::conformance::memory_lab::memory_lab_retrieve_memory
        ),
        c!(
            "memory_lab.trace_retrieval",
            ["memory"],
            crate::conformance::memory_lab::memory_lab_trace_retrieval
        ),
        c!(
            "memory_lab.draft_update_proposal_only",
            ["memory"],
            crate::conformance::memory_lab::memory_lab_draft_update
        ),
        c!(
            "memory_lab.correction_proposal_gated",
            ["memory"],
            crate::conformance::memory_lab::memory_lab_correction
        ),
        c!(
            "memory_lab.forget_redaction_plan",
            ["memory"],
            crate::conformance::memory_lab::memory_lab_forget_redaction
        ),
        c!(
            "memory_lab.branch_view",
            ["memory"],
            crate::conformance::memory_lab::memory_lab_branch_view
        ),
        c!(
            "memory_lab.no_forbidden_namespace",
            ["memory"],
            crate::conformance::memory_lab::memory_lab_no_forbidden_namespace
        ),
        c!(
            "memory_lab.no_raw_secrets",
            ["memory", "secret"],
            crate::conformance::memory_lab::memory_lab_no_raw_secrets
        ),
    ]
}

pub(super) fn creator_loop_beta_5_cases() -> Vec<ConformanceCase> {
    vec![
        // --- creator loop Beta 5 ---
        c!(
            "creator_loop.playable_board_template",
            ["experience", "generated"],
            crate::conformance::creator_loop::creator_loop_playable_board_template
        ),
        c!(
            "creator_loop.playable_experience_template",
            ["experience", "generated"],
            crate::conformance::creator_loop::creator_loop_playable_experience_template
        ),
        c!(
            "creator_loop.experience_surface_warnings",
            ["experience", "generated"],
            crate::conformance::creator_loop::creator_loop_experience_surface_warnings
        ),
        c!(
            "creator_loop.missing_checkpoint_warning",
            ["experience", "generated"],
            crate::conformance::creator_loop::creator_loop_missing_checkpoint_warning
        ),
        c!(
            "creator_loop.dangerous_permissions_warning",
            ["experience", "generated"],
            crate::conformance::creator_loop::creator_loop_dangerous_permissions_warning
        ),
        c!(
            "creator_loop.network_nondeterministic_hint",
            ["experience", "generated", "network"],
            crate::conformance::creator_loop::creator_loop_network_nondeterministic_hint
        ),
        c!(
            "creator_loop.composition_experience_diagnostics",
            ["experience", "composition", "generated"],
            crate::conformance::creator_loop::creator_loop_composition_experience_diagnostics
        ),
        c!(
            "creator_loop.walkthrough_reference",
            ["experience", "generated"],
            crate::conformance::creator_loop::creator_loop_walkthrough_reference
        ),
        c!(
            "creator_loop.thirdparty_no_privilege",
            ["experience", "replacement", "generated"],
            crate::conformance::creator_loop::creator_loop_thirdparty_no_privilege
        ),
    ]
}

pub(super) fn sharing_lab_beta_6_cases() -> Vec<ConformanceCase> {
    vec![
        // --- sharing lab Beta 6 ---
        c!(
            "sharing_lab.contract_shape",
            ["sharing"],
            crate::conformance::sharing_lab::sharing_contract
        ),
        c!(
            "sharing_lab.export_composition_bundle",
            ["sharing", "composition"],
            crate::conformance::sharing_lab::sharing_export_bundle
        ),
        c!(
            "sharing_lab.import_composition_bundle",
            ["sharing", "composition"],
            crate::conformance::sharing_lab::sharing_import_bundle
        ),
        c!(
            "sharing_lab.branch_session_bundle",
            ["sharing"],
            crate::conformance::sharing_lab::sharing_branch_session_bundle
        ),
        c!(
            "sharing_lab.package_set_lockfile",
            ["sharing"],
            crate::conformance::sharing_lab::sharing_package_set_lockfile
        ),
        c!(
            "sharing_lab.compatibility_report",
            ["sharing"],
            crate::conformance::sharing_lab::sharing_compatibility_report
        ),
        c!(
            "sharing_lab.ai_disclosure_bundle",
            ["sharing"],
            crate::conformance::sharing_lab::sharing_ai_disclosure_bundle
        ),
        c!(
            "sharing_lab.read_only_share_manifest",
            ["sharing"],
            crate::conformance::sharing_lab::sharing_read_only_manifest
        ),
        c!(
            "sharing_lab.async_fork_share_plan",
            ["sharing"],
            crate::conformance::sharing_lab::sharing_async_fork_plan
        ),
        c!(
            "sharing_lab.no_marketplace_no_raw_secrets",
            ["sharing", "secret"],
            crate::conformance::sharing_lab::sharing_no_marketplace_no_raw_secrets
        ),
    ]
}
