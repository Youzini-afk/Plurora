#![allow(dead_code)]

use crate::conformance::runner::{CaseFn, ConformanceCase};

mod core_runtime;
mod external_project_install;
mod official_experience_agent;
mod outbound_model_streaming;
mod storage_tdb;
mod subprocess_packages;

pub(super) fn case(
    id: &'static str,
    tags: &'static [&'static str],
    run: CaseFn,
) -> ConformanceCase {
    ConformanceCase { id, tags, run }
}

/// Build the full ordered list of conformance cases.
pub(super) fn build_cases() -> Vec<ConformanceCase> {
    let mut cases = Vec::new();
    cases.extend(core_runtime::core_cases());
    cases.extend(core_runtime::permissions_cases());
    cases.extend(subprocess_packages::subprocess_cases());
    cases.extend(core_runtime::host_cases());
    cases.extend(core_runtime::surfaces_cases());
    cases.extend(official_experience_agent::official_foundation_labs_cases());
    cases.extend(core_runtime::proposals_cases());
    cases.extend(core_runtime::asset_cases());
    cases.extend(core_runtime::session_fork_cases());
    cases.extend(core_runtime::projection_cases());
    cases.extend(core_runtime::substrate_cases());
    cases.extend(subprocess_packages::subprocess_error_cases_cases());
    cases.extend(core_runtime::protocol_cases());
    cases.extend(core_runtime::world_bundle_cases());
    cases.extend(subprocess_packages::package_check_reload_cases());
    cases.extend(core_runtime::hooks_cases());
    cases.extend(subprocess_packages::generated_packages_cases());
    cases.extend(core_runtime::composition_cases());
    cases.extend(subprocess_packages::inproc_cases());
    cases.extend(core_runtime::replacement_cases());
    cases.extend(core_runtime::secret_conformance_cases());
    cases.extend(outbound_model_streaming::network_cases());
    cases.extend(outbound_model_streaming::outbound_cases());
    cases.extend(outbound_model_streaming::live_http_outbound_cases());
    cases.extend(outbound_model_streaming::kernel_v1_outbound_execute_cases());
    cases.extend(outbound_model_streaming::y1_outbound_execute_profile_conformance_cases());
    cases.extend(outbound_model_streaming::y2_manifest_permissions_secret_refs_conformance_cases());
    cases.extend(outbound_model_streaming::y3_kernel_v1_outbound_stream_conformance_cases());
    cases.extend(outbound_model_streaming::streaming_cases());
    cases.extend(outbound_model_streaming::live_model_cases());
    cases.extend(outbound_model_streaming::live_model_providers_cases());
    cases.extend(outbound_model_streaming::live_model_quirks_cases());
    cases.extend(official_experience_agent::inference_local_cases());
    cases.extend(official_experience_agent::inference_playtest_cases());
    cases.extend(official_experience_agent::agentic_forge_phase_a_cases());
    cases.extend(official_experience_agent::agentic_forge_phase_b_cases());
    cases.extend(official_experience_agent::agentic_forge_phase_c_cases());
    cases.extend(official_experience_agent::agentic_forge_phase_d_cases());
    cases.extend(official_experience_agent::agentic_forge_phase_f_cases());
    cases.extend(official_experience_agent::experience_runtime_cases());
    cases.extend(official_experience_agent::playable_board_beta_1_cases());
    cases.extend(official_experience_agent::playable_board_beta_2_cases());
    cases.extend(official_experience_agent::experience_observability_beta_3_cases());
    cases.extend(official_experience_agent::memory_lab_beta_4_cases());
    cases.extend(official_experience_agent::creator_loop_beta_5_cases());
    cases.extend(external_project_install::project_intake_lab_external_project_operating_plane_alpha_e1_cases());
    cases.extend(external_project_install::project_intake_lab_e5_external_project_operating_plane_alpha_e5_adapte_cases());
    cases.extend(
        external_project_install::workspace_lab_external_project_operating_plane_alpha_e2_cases(),
    );
    cases.extend(external_project_install::workspace_lab_e3_external_project_operating_plane_alpha_e3_managed_wor_cases());
    cases
        .extend(external_project_install::integrity_lab_package_installation_foundation_i3_cases());
    cases.extend(external_project_install::secret_store_lab_round_10a_1_phase_b_cases());
    cases.extend(external_project_install::project_scoped_secrets_round_10a_2_wave_2c_cases());
    cases.extend(external_project_install::project_lifecycle_round_10a_2_wave_3_cases());
    cases.extend(official_experience_agent::sharing_lab_beta_6_cases());
    cases.extend(storage_tdb::storage_backend_neutrality_s1_cases());
    cases.extend(storage_tdb::storage_backend_neutrality_s1_postgresql_opt_in_cases());
    cases.extend(storage_tdb::storage_lab_storage_backend_neutrality_alpha_s2_cases());
    cases.extend(storage_tdb::storage_lab_s3_blob_asset_store_contract_proof_cases());
    cases.extend(
        storage_tdb::storage_lab_s4_projection_index_materialization_contract_proof_cases(),
    );
    cases.extend(storage_tdb::storage_lab_s5_retrieval_vector_multimodal_provider_contract_cases());
    cases
        .extend(external_project_install::git_tools_lab_package_installation_foundation_i2_cases());
    cases.extend(external_project_install::install_lab_package_installation_foundation_i4_cases());
    cases.extend(storage_tdb::real_tdb_rust_adapter_subprocess_proof_cases());
    cases
}
