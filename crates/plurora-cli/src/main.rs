use clap::Parser;
use plurora_cli::cli::Cli;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();
    // Keep deep schema/installation flows off Windows' small process-entry stack.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(8 * 1024 * 1024)
        .build()?;
    runtime.block_on(async move { tokio::spawn(plurora_cli::run_cli(cli)).await? })
}
