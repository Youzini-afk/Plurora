#![allow(unused_macros)]

use super::{case, ConformanceCase};

macro_rules! c {
    ($id:expr, [$($tag:expr),*], $func:path) => {
        case($id, &[$($tag),*], || Box::pin($func()))
    };
}

pub(super) fn storage_backend_neutrality_s1_cases() -> Vec<ConformanceCase> {
    vec![
        // --- storage backend neutrality S1 ---
        c!(
            "storage_backend.in_memory_event_store_contract_append_range",
            ["storage", "substrate"],
            crate::conformance::storage_backend::in_memory_event_store_contract_append_range
        ),
        c!(
            "storage_backend.sqlite_event_store_contract_append_range",
            ["storage", "substrate"],
            crate::conformance::storage_backend::sqlite_event_store_contract_append_range
        ),
        c!(
            "storage_backend.backend_parity_kind_prefix",
            ["storage", "substrate"],
            crate::conformance::storage_backend::backend_parity_kind_prefix
        ),
        c!(
            "storage_backend.backend_parity_concurrent_append",
            ["storage", "substrate"],
            crate::conformance::storage_backend::backend_parity_concurrent_append
        ),
        c!(
            "storage_backend.backend_parity_subscription",
            ["storage", "substrate"],
            crate::conformance::storage_backend::backend_parity_subscription
        ),
        c!(
            "storage_backend.rehydrate_parity",
            ["storage", "substrate"],
            crate::conformance::storage_backend::storage_backend_rehydrate_parity
        ),
    ]
}

pub(super) fn storage_backend_neutrality_s1_postgresql_opt_in_cases() -> Vec<ConformanceCase> {
    vec![
        // --- storage backend neutrality S1 — PostgreSQL (opt-in) ---
        #[cfg(feature = "postgres")]
        c!(
            "storage_backend.postgres_event_store_contract_append_range",
            ["storage", "substrate", "postgres"],
            crate::conformance::storage_backend::postgres_event_store_contract_append_range
        ),
        #[cfg(feature = "postgres")]
        c!(
            "storage_backend.postgres_backend_parity_kind_prefix",
            ["storage", "substrate", "postgres"],
            crate::conformance::storage_backend::postgres_backend_parity_kind_prefix
        ),
        #[cfg(feature = "postgres")]
        c!(
            "storage_backend.postgres_backend_parity_concurrent_append",
            ["storage", "substrate", "postgres"],
            crate::conformance::storage_backend::postgres_backend_parity_concurrent_append
        ),
        #[cfg(feature = "postgres")]
        c!(
            "storage_backend.postgres_backend_parity_subscription",
            ["storage", "substrate", "postgres"],
            crate::conformance::storage_backend::postgres_backend_parity_subscription
        ),
        #[cfg(feature = "postgres")]
        c!(
            "storage_backend.postgres_rehydrate_parity",
            ["storage", "substrate", "postgres"],
            crate::conformance::storage_backend::postgres_rehydrate_parity
        ),
    ]
}

pub(super) fn storage_lab_storage_backend_neutrality_alpha_s2_cases() -> Vec<ConformanceCase> {
    vec![
        // --- storage-lab (Storage Backend Neutrality Alpha S2) ---
        c!(
            "storage_lab.contract_shape_no_kernel_database_terms",
            ["storage_lab", "storage", "official"],
            crate::conformance::storage_lab::contract_shape_no_kernel_database_terms
        ),
        c!(
            "storage_lab.backend_classes_no_credentials",
            ["storage_lab", "storage"],
            crate::conformance::storage_lab::backend_classes_no_secret_config
        ),
        c!(
            "storage_lab.package_state_plan_scoped",
            ["storage_lab", "storage"],
            crate::conformance::storage_lab::package_state_plan_scoped
        ),
        c!(
            "storage_lab.put_document_preview_no_write",
            ["storage_lab", "storage"],
            crate::conformance::storage_lab::put_document_preview_no_write
        ),
        c!(
            "storage_lab.get_document_preview_no_read",
            ["storage_lab", "storage"],
            crate::conformance::storage_lab::get_document_preview_no_read
        ),
        c!(
            "storage_lab.query_prefix_preview_no_query_execution",
            ["storage_lab", "storage"],
            crate::conformance::storage_lab::query_prefix_preview_no_query_execution
        ),
        c!(
            "storage_lab.delete_tombstone_preview_no_delete",
            ["storage_lab", "storage"],
            crate::conformance::storage_lab::delete_tombstone_preview_no_delete
        ),
        c!(
            "storage_lab.export_snapshot_preview_redacted",
            ["storage_lab", "storage"],
            crate::conformance::storage_lab::export_snapshot_preview_redacted
        ),
        c!(
            "storage_lab.raw_secret_rejected",
            ["storage_lab", "storage", "secret"],
            crate::conformance::storage_lab::raw_secret_rejected
        ),
        c!(
            "storage_lab.unsafe_id_rejected",
            ["storage_lab", "storage"],
            crate::conformance::storage_lab::unsafe_id_rejected
        ),
    ]
}

pub(super) fn storage_lab_s3_blob_asset_store_contract_proof_cases() -> Vec<ConformanceCase> {
    vec![
        // --- storage-lab S3 (Blob / Asset Store Contract Proof) ---
        c!(
            "storage_lab.blob_contract_shape",
            ["storage_lab", "storage", "blob"],
            crate::conformance::storage_lab::blob_contract_shape
        ),
        c!(
            "storage_lab.put_blob_preview_content_address_deterministic",
            ["storage_lab", "storage", "blob"],
            crate::conformance::storage_lab::put_blob_preview_content_address_deterministic
        ),
        c!(
            "storage_lab.put_blob_preview_no_storage_no_content_event",
            ["storage_lab", "storage", "blob"],
            crate::conformance::storage_lab::put_blob_preview_no_storage_no_content_event
        ),
        c!(
            "storage_lab.get_blob_metadata_preview_no_content",
            ["storage_lab", "storage", "blob"],
            crate::conformance::storage_lab::get_blob_metadata_preview_no_content
        ),
        c!(
            "storage_lab.export_blob_manifest_refs_only",
            ["storage_lab", "storage", "blob"],
            crate::conformance::storage_lab::export_blob_manifest_refs_only
        ),
        c!(
            "storage_lab.blob_raw_secret_and_unsafe_id_rejected",
            ["storage_lab", "storage", "blob", "secret"],
            crate::conformance::storage_lab::blob_raw_secret_and_unsafe_id_rejected
        ),
    ]
}

pub(super) fn storage_lab_s4_projection_index_materialization_contract_proof_cases(
) -> Vec<ConformanceCase> {
    vec![
        // --- storage-lab S4 (Projection / Index Materialization Contract Proof) ---
        c!(
            "storage_lab.projection_contract_shape",
            ["storage_lab", "storage", "projection"],
            crate::conformance::storage_lab::projection_contract_shape
        ),
        c!(
            "storage_lab.projection_materialization_plan_only",
            ["storage_lab", "storage", "projection"],
            crate::conformance::storage_lab::projection_materialization_plan_only
        ),
        c!(
            "storage_lab.projection_query_preview_no_execution",
            ["storage_lab", "storage", "projection"],
            crate::conformance::storage_lab::projection_query_preview_no_execution
        ),
        c!(
            "storage_lab.projection_migration_plan_no_rewrite",
            ["storage_lab", "storage", "projection"],
            crate::conformance::storage_lab::projection_migration_plan_no_rewrite
        ),
        c!(
            "storage_lab.projection_rejects_raw_secret",
            ["storage_lab", "storage", "projection", "secret"],
            crate::conformance::storage_lab::projection_rejects_raw_secret
        ),
        c!(
            "storage_lab.projection_no_db_table_leakage",
            ["storage_lab", "storage", "projection"],
            crate::conformance::storage_lab::projection_no_db_table_leakage
        ),
    ]
}

pub(super) fn storage_lab_s5_retrieval_vector_multimodal_provider_contract_cases(
) -> Vec<ConformanceCase> {
    vec![
        // --- storage-lab S5 (Retrieval / Vector / Multimodal Provider Contract) ---
        c!(
            "storage_lab.retrieval_contract_shape",
            ["storage_lab", "storage", "retrieval"],
            crate::conformance::storage_lab::retrieval_contract_shape
        ),
        c!(
            "storage_lab.multimodal_index_plan_no_embedding_no_storage",
            ["storage_lab", "storage", "retrieval"],
            crate::conformance::storage_lab::multimodal_index_plan_no_embedding_no_storage
        ),
        c!(
        "storage_lab.multimodal_index_rejects_invalid_modality_or_too_many_refs",
        ["storage_lab", "storage", "retrieval"],
        crate::conformance::storage_lab::multimodal_index_rejects_invalid_modality_or_too_many_refs
    ),
        c!(
            "storage_lab.vector_search_plan_no_execution",
            ["storage_lab", "storage", "retrieval"],
            crate::conformance::storage_lab::vector_search_plan_no_execution
        ),
        c!(
            "storage_lab.backend_fit_mentions_tdb_future_only",
            ["storage_lab", "storage", "retrieval", "tdb"],
            crate::conformance::storage_lab::backend_fit_mentions_tdb_future_only
        ),
        c!(
            "storage_lab.retrieval_rejects_raw_secret",
            ["storage_lab", "storage", "retrieval", "secret"],
            crate::conformance::storage_lab::retrieval_rejects_raw_secret
        ),
        c!(
            "storage_lab.retrieval_no_kernel_vector_namespace_or_credentials",
            ["storage_lab", "storage", "retrieval"],
            crate::conformance::storage_lab::retrieval_no_kernel_vector_namespace_or_secret_config
        ),
        c!(
            "tdb_retrieval_lab.contract_shape",
            ["tdb", "retrieval", "storage", "official"],
            crate::conformance::tdb_retrieval_lab::contract_shape
        ),
        c!(
            "tdb_retrieval_lab.index_plan_no_execution",
            ["tdb", "retrieval", "storage"],
            crate::conformance::tdb_retrieval_lab::index_plan_no_execution
        ),
        c!(
            "tdb_retrieval_lab.query_plan_no_execution",
            ["tdb", "retrieval", "storage"],
            crate::conformance::tdb_retrieval_lab::query_plan_no_execution
        ),
        c!(
            "tdb_retrieval_lab.backend_fit_boundary",
            ["tdb", "retrieval", "storage"],
            crate::conformance::tdb_retrieval_lab::backend_fit_boundary
        ),
        c!(
            "tdb_retrieval_lab.invalid_input_rejected",
            ["tdb", "retrieval", "storage"],
            crate::conformance::tdb_retrieval_lab::invalid_input_rejected
        ),
        c!(
            "tdb_retrieval_lab.raw_secret_and_unsafe_id_rejected",
            ["tdb", "retrieval", "storage", "secret"],
            crate::conformance::tdb_retrieval_lab::raw_secret_and_unsafe_id_rejected
        ),
        c!(
            "tdb_retrieval_lab.real_tdb_opt_in_seam_crate_adapter_available",
            ["tdb", "retrieval", "storage"],
            crate::conformance::tdb_retrieval_lab::real_tdb_opt_in_seam_crate_adapter_available
        ),
    ]
}

pub(super) fn real_tdb_rust_adapter_subprocess_proof_cases() -> Vec<ConformanceCase> {
    vec![
        // --- real TDB Rust adapter subprocess proof ---
        c!(
            "tdb_rust_adapter.subprocess_adapter_shell_invokes_disabled_smoke",
            ["tdb", "retrieval", "subprocess", "slow"],
            crate::conformance::tdb_rust_adapter::subprocess_adapter_shell_invokes_disabled_smoke
        ),
        c!(
            "tdb_rust_adapter.subprocess_adapter_rejects_secret_and_raw_path",
            ["tdb", "retrieval", "subprocess", "secret", "slow"],
            crate::conformance::tdb_rust_adapter::subprocess_adapter_rejects_secret_and_raw_path
        ),
        c!(
            "tdb_rust_adapter.real_crate_smoke_opt_in",
            ["tdb", "tdb_real", "retrieval", "slow"],
            crate::conformance::tdb_rust_adapter::real_crate_smoke_opt_in
        ),
    ]
}
