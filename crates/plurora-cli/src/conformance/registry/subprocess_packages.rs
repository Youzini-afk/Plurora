use super::{case, ConformanceCase};

macro_rules! c {
    ($id:expr, [$($tag:expr),*], $func:path) => {
        case($id, &[$($tag),*], || Box::pin($func()))
    };
}

pub(super) fn subprocess_cases() -> Vec<ConformanceCase> {
    vec![
        // --- subprocess ---
        c!(
            "subprocess.load_ready",
            ["subprocess", "slow"],
            crate::conformance::subprocess::subprocess_load_ready
        ),
        c!(
            "subprocess.invoke_echo",
            ["subprocess", "slow"],
            crate::conformance::subprocess::subprocess_invoke_echo
        ),
        c!(
            "subprocess.bindings_injected_invokable",
            ["subprocess", "binding", "slow"],
            crate::conformance::subprocess::subprocess_bindings_injected_and_invokable
        ),
        c!(
            "package.lifecycle_timeline",
            ["subprocess", "package", "slow"],
            crate::conformance::subprocess::package_lifecycle_timeline
        ),
        c!(
            "package.path_b_self_contained",
            ["subprocess", "package", "path_b", "slow"],
            crate::conformance::subprocess::path_b_self_contained_contract_none
        ),
        c!(
            "package.logs_capture",
            ["subprocess", "package", "slow"],
            crate::conformance::subprocess::package_logs_capture
        ),
        c!(
            "package.restart_subprocess",
            ["subprocess", "package", "slow"],
            crate::conformance::subprocess::package_restart_subprocess
        ),
    ]
}

pub(super) fn subprocess_error_cases_cases() -> Vec<ConformanceCase> {
    vec![
        // --- subprocess error cases ---
        c!(
            "subprocess.bad_handshake",
            ["subprocess", "slow"],
            crate::conformance::subprocess::subprocess_bad_handshake
        ),
        c!(
            "subprocess.invoke_timeout",
            ["subprocess", "slow"],
            crate::conformance::subprocess::subprocess_timeout
        ),
        c!(
            "subprocess.invalid_output_schema",
            ["subprocess", "slow"],
            crate::conformance::subprocess::subprocess_invalid_output_schema
        ),
        c!(
            "subprocess.unload_removes_capability",
            ["subprocess", "slow"],
            crate::conformance::subprocess::subprocess_unload_removes_capability
        ),
    ]
}

pub(super) fn package_check_reload_cases() -> Vec<ConformanceCase> {
    vec![
        // --- package check / reload ---
        c!(
            "package.check_diagnostics",
            ["subprocess", "package", "slow"],
            crate::conformance::subprocess::package_check_diagnostics
        ),
        c!(
            "package.reload_smoke",
            ["subprocess", "package", "slow"],
            crate::conformance::subprocess::package_reload_smoke
        ),
    ]
}

pub(super) fn generated_packages_cases() -> Vec<ConformanceCase> {
    vec![
        // --- generated packages ---
        c!(
            "package.generated_subprocess_conformance",
            ["generated", "slow"],
            crate::conformance::generated::generated_subprocess_package
        ),
        c!(
            "package.generated_typescript_subprocess_conformance",
            ["generated", "slow"],
            crate::conformance::generated::generated_typescript_subprocess_package
        ),
        c!(
            "package.generated_experience_template",
            ["generated", "slow"],
            crate::conformance::generated::generated_experience_template
        ),
        c!(
            "package.generated_basic_template",
            ["generated", "slow"],
            crate::conformance::generated::generated_basic_template
        ),
        c!(
            "package.generated_explicit_experience_template",
            ["generated", "slow"],
            crate::conformance::generated::generated_explicit_experience_template
        ),
        c!(
            "package.generated_assistant_action_template",
            ["generated", "slow"],
            crate::conformance::generated::generated_assistant_action_template
        ),
        c!(
            "package.generated_asset_editor_template",
            ["generated", "slow"],
            crate::conformance::generated::generated_asset_editor_template
        ),
        c!(
            "package.generated_full_surface_template",
            ["generated", "slow"],
            crate::conformance::generated::generated_full_surface_template
        ),
        c!(
            "package.generated_networked_template",
            ["generated", "network", "slow"],
            crate::conformance::generated::generated_networked_template
        ),
        c!(
            "package.generated_streaming_template",
            ["generated", "stream", "slow"],
            crate::conformance::generated::generated_streaming_template
        ),
        c!(
            "package.generated_agent_runtime_template",
            ["generated", "agentic", "slow"],
            crate::conformance::generated::generated_agent_runtime_template
        ),
        c!(
            "package.generated_experience_runtime_template",
            ["generated", "experience", "slow"],
            crate::conformance::generated::generated_experience_runtime_template
        ),
        c!(
            "package.faux_model_readiness",
            ["generated", "slow"],
            crate::conformance::generated::faux_model_readiness_package
        ),
        c!(
            "package.faux_agent_readiness",
            ["generated", "agentic", "slow"],
            crate::conformance::generated::faux_agent_readiness_package
        ),
    ]
}

pub(super) fn inproc_cases() -> Vec<ConformanceCase> {
    vec![
        // --- inproc ---
        c!(
            "inproc.non_official_preview_rejected",
            ["runtime"],
            crate::conformance::inproc::non_official_preview_rejected
        ),
        c!(
            "inproc.unknown_capability_errors",
            ["runtime"],
            crate::conformance::inproc::unknown_inproc_capability_errors
        ),
        c!(
            "inproc.bindings_init",
            ["runtime", "binding"],
            crate::conformance::inproc::inproc_bindings_init_receives_manifest_bindings
        ),
    ]
}
