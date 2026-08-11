use clap::Parser;
use plurora_cli::cli::Cli;

const INSTALLATION: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const REALIZATION: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
const HISTORIC: &str = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";

#[test]
fn realization_commands_require_explicit_target_revision_plan_and_approval_inputs() {
    assert!(Cli::try_parse_from(["plurora", "realization", "list", "--json"]).is_ok());
    assert!(Cli::try_parse_from([
        "plurora",
        "realization",
        "plan",
        INSTALLATION,
        "--installation-revision",
        "3",
        "--target-id",
        "local",
        "--workload-id",
        "api",
        "--execution-class",
        "oci-container.v1",
        "--backend",
        "oci-image",
        "--image",
        "registry.example/app@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "--container-port",
        "8080",
        "--port-name",
        "http",
        "--route-id",
        "api",
        "--idempotency-key",
        "plan-1",
    ])
    .is_ok());
    assert!(Cli::try_parse_from([
        "plurora",
        "realization",
        "apply",
        INSTALLATION,
        REALIZATION,
        "--target-id",
        "local",
        "--revision",
        "1",
        "--plan-ref",
        "plan-ref.json",
        "--approve",
        "--accept-risk",
        "managed_target_effects",
        "--idempotency-key",
        "apply-1",
    ])
    .is_ok());
    assert!(Cli::try_parse_from([
        "plurora",
        "realization",
        "rollback",
        INSTALLATION,
        REALIZATION,
        "--target-id",
        "local",
        "--revision",
        "4",
        "--rollback-to-realization-id",
        HISTORIC,
        "--plan-digest",
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "--approve",
        "--accept-risk",
        "managed_target_effects",
        "--idempotency-key",
        "rollback-1",
    ])
    .is_ok());
    assert!(Cli::try_parse_from([
        "plurora",
        "realization",
        "stop",
        INSTALLATION,
        REALIZATION,
        "--target-id",
        "local",
        "--revision",
        "4",
        "--idempotency-key",
        "stop-1",
    ])
    .is_ok());
}
