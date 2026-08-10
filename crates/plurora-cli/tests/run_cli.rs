use clap::Parser;
use plurora_cli::cli::Cli;

const INSTALLATION_ID: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const RUN_ID: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";

#[test]
fn run_commands_parse_and_project_start_stop_remain_rejected() {
    assert!(Cli::try_parse_from(["plurora", "run", "list", "--json"]).is_ok());
    assert!(Cli::try_parse_from([
        "plurora",
        "run",
        "status",
        INSTALLATION_ID,
        "--entrypoint-id",
        "play",
    ])
    .is_ok());
    assert!(Cli::try_parse_from([
        "plurora",
        "run",
        "start",
        INSTALLATION_ID,
        "--revision",
        "7",
        "--entrypoint-id",
        "play",
        "--idempotency-key",
        "start-1",
    ])
    .is_ok());
    assert!(Cli::try_parse_from([
        "plurora",
        "run",
        "stop",
        INSTALLATION_ID,
        RUN_ID,
        "--revision",
        "9",
        "--idempotency-key",
        "stop-1",
    ])
    .is_ok());

    assert!(Cli::try_parse_from(["plurora", "project", "start", INSTALLATION_ID]).is_err());
    assert!(Cli::try_parse_from(["plurora", "project", "stop", INSTALLATION_ID, RUN_ID]).is_err());
}
