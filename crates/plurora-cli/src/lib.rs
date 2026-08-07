pub mod cli;
pub mod commands;
pub mod conformance;
pub mod install;
pub mod schema_export;
pub mod templates;

use cli::{
    CapabilityCommand, Cli, Command, CompositionCommand, ConformanceCommand, ContractCommand,
    HostAccessCommand, HostChangeCommand, HostCommand, HostConnectionCommand, ManifestCommand,
    PackageCommand, PerfCommand, TargetAgentCommand, WorldBundleCommand,
};
use commands::audit;
use commands::{
    capability, composition, conformance_package, contract, demo, host, install as install_command,
    list_installed, lockfile, manifest, package, perf, project, uninstall, update, world_bundle,
};

pub async fn run_cli(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Command::Demo => demo::demo().await,
        Command::SqliteDemo { path } => demo::sqlite_demo(path).await,
        Command::Serve { bind } => demo::serve(bind).await,
        Command::Host { command } => match command {
            HostCommand::Serve {
                http,
                profile,
                static_dir,
                data_dir,
                access_token,
                app_base_domain,
            } => {
                host::host_serve(
                    http,
                    profile,
                    static_dir,
                    data_dir,
                    access_token,
                    app_base_domain,
                )
                .await
            }
            HostCommand::Access {
                endpoint,
                access_token,
                command,
            } => {
                let context = commands::host_connection::resolve(endpoint.as_deref())?;
                match command {
                    HostAccessCommand::Me => {
                        commands::host_access::me(&context.endpoint, &access_token).await
                    }
                    HostAccessCommand::List => {
                        commands::host_access::list(&context.endpoint, &access_token).await
                    }
                    HostAccessCommand::Pair {
                        device_name,
                        scopes,
                        projects,
                        targets,
                        grant_days,
                    } => {
                        commands::host_access::pair(
                            &context.endpoint,
                            &access_token,
                            device_name,
                            scopes,
                            projects,
                            targets,
                            grant_days,
                        )
                        .await
                    }
                    HostAccessCommand::Revoke { grant_id } => {
                        commands::host_access::revoke(&context.endpoint, &access_token, &grant_id)
                            .await
                    }
                    HostAccessCommand::BulkRevoke { grant_ids } => {
                        commands::host_access::bulk_revoke(
                            &context.endpoint,
                            &access_token,
                            grant_ids,
                        )
                        .await
                    }
                    HostAccessCommand::Projects => {
                        commands::host_access::projects(&context.endpoint, &access_token).await
                    }
                    HostAccessCommand::Targets => {
                        commands::host_access::targets(&context.endpoint, &access_token).await
                    }
                    HostAccessCommand::ProjectStatus { project } => {
                        let project_id = project.or(context.project_id).ok_or_else(|| {
                        anyhow::anyhow!(
                            "project id is required; pass --project or select `host connection context`"
                        )
                    })?;
                        commands::host_access::project_status(
                            &context.endpoint,
                            &access_token,
                            &project_id,
                        )
                        .await
                    }
                    HostAccessCommand::TargetStatus { target } => {
                        let target_id = target.or(context.target_id).ok_or_else(|| {
                        anyhow::anyhow!(
                            "target id is required; pass --target or select `host connection context`"
                        )
                    })?;
                        commands::host_access::target_status(
                            &context.endpoint,
                            &access_token,
                            &target_id,
                        )
                        .await
                    }
                    HostAccessCommand::Changes { project, command } => {
                        let project_id = project.or(context.project_id).ok_or_else(|| {
                            anyhow::anyhow!(
                                "project id is required; pass --project or select `host connection context`"
                            )
                        })?;
                        match command {
                            HostChangeCommand::List => {
                                commands::host_changes::list(
                                    &context.endpoint,
                                    &access_token,
                                    &project_id,
                                )
                                .await
                            }
                            HostChangeCommand::Get { change_set_id } => {
                                commands::host_changes::get(
                                    &context.endpoint,
                                    &access_token,
                                    &project_id,
                                    &change_set_id,
                                )
                                .await
                            }
                            HostChangeCommand::Draft { request } => {
                                commands::host_changes::draft(
                                    &context.endpoint,
                                    &access_token,
                                    &project_id,
                                    &request,
                                )
                                .await
                            }
                            HostChangeCommand::Approve {
                                change_set_id,
                                reason,
                            } => {
                                commands::host_changes::decide(
                                    &context.endpoint,
                                    &access_token,
                                    &project_id,
                                    &change_set_id,
                                    true,
                                    reason,
                                )
                                .await
                            }
                            HostChangeCommand::Reject {
                                change_set_id,
                                reason,
                            } => {
                                commands::host_changes::decide(
                                    &context.endpoint,
                                    &access_token,
                                    &project_id,
                                    &change_set_id,
                                    false,
                                    reason,
                                )
                                .await
                            }
                            HostChangeCommand::Execute { change_set_id } => {
                                commands::host_changes::execute(
                                    &context.endpoint,
                                    &access_token,
                                    &project_id,
                                    &change_set_id,
                                )
                                .await
                            }
                            HostChangeCommand::Recover { change_set_id } => {
                                commands::host_changes::recover(
                                    &context.endpoint,
                                    &access_token,
                                    &project_id,
                                    &change_set_id,
                                )
                                .await
                            }
                            HostChangeCommand::Bundle { change_set_id } => {
                                commands::host_changes::bundle(
                                    &context.endpoint,
                                    &access_token,
                                    &project_id,
                                    &change_set_id,
                                )
                                .await
                            }
                        }
                    }
                }
            }
            HostCommand::Connection { command } => match command {
                HostConnectionCommand::List => commands::host_connection::list(),
                HostConnectionCommand::Save { name, endpoint } => {
                    commands::host_connection::save(&name, &endpoint)
                }
                HostConnectionCommand::Use { name } => commands::host_connection::select(&name),
                HostConnectionCommand::Local => commands::host_connection::local(),
                HostConnectionCommand::Remove { name } => commands::host_connection::remove(&name),
                HostConnectionCommand::Context { project, target } => {
                    commands::host_connection::set_context(&project, &target)
                }
                HostConnectionCommand::ClearContext => commands::host_connection::clear_context(),
            },
            HostCommand::Backup {
                data_dir,
                profile,
                output,
            } => commands::host_data::backup(data_dir, profile, output).await,
            HostCommand::Restore { backup, data_dir } => {
                commands::host_data::restore(backup, data_dir).await
            }
        },
        Command::TargetAgent { command } => match command {
            TargetAgentCommand::Enroll {
                endpoint,
                enrollment_token,
                data_dir,
                capabilities,
            } => {
                commands::target_agent::enroll(&endpoint, &enrollment_token, data_dir, capabilities)
                    .await
            }
            TargetAgentCommand::Run {
                data_dir,
                credential,
            } => commands::target_agent::run(data_dir, credential).await,
        },
        Command::HostStdio => host::host_stdio().await,
        Command::Manifest { command } => match command {
            ManifestCommand::Validate { path } => manifest::validate_manifest(path).await,
        },
        Command::Package { command } => match command {
            PackageCommand::Load { path } => package::package_load(path).await,
            PackageCommand::Check { path } => package::package_check(path).await,
            PackageCommand::RunFixture { path } => package::package_run_fixture(path).await,
            PackageCommand::InvokeLocal {
                path,
                capability_id,
                input,
            } => package::package_invoke_local(path, capability_id, input).await,
            PackageCommand::Conformance { path } => package::package_conformance(path).await,
            PackageCommand::Reload { path } => package::package_reload(path).await,
        },
        Command::Capability { command } => match command {
            CapabilityCommand::Invoke {
                manifest,
                capability_id,
                input,
            } => capability::capability_invoke(manifest, capability_id, input).await,
        },
        Command::Audit(args) => audit::run(args).await,
        Command::Install(args) => install_command::run(args).await,
        Command::Uninstall(args) => uninstall::run(args).await,
        Command::Project(args) => project::run(args).await,
        Command::ListInstalled(args) => list_installed::run(args).await,
        Command::Update(args) => update::run(args).await,
        Command::Lockfile(args) => lockfile::run(args).await,
        Command::InitPackage {
            path,
            id,
            entry,
            language,
            template,
        } => package::init_package(path, id, entry, language, template).await,
        Command::InitComposition { path, id } => composition::init_composition(path, id).await,
        Command::Composition { command } => match command {
            CompositionCommand::Check { path } => composition::composition_check(path).await,
        },
        Command::Conformance(args) => match args.command {
            Some(ConformanceCommand::Package(package_args)) => {
                conformance_package::run(package_args).await
            }
            Some(ConformanceCommand::Protocol(protocol_args)) => {
                conformance::run_protocol_report(
                    &protocol_args.protocol,
                    protocol_args.implementation.as_deref(),
                    protocol_args.json,
                )
                .await
            }
            None => {
                conformance::run(conformance::ConformanceOptions {
                    list: args.list,
                    case: args.case,
                    tag: args.tag,
                    fail_fast: args.fail_fast,
                    slowest: args.slowest,
                })
                .await
            }
        },
        Command::PlayCreateDemo => demo::play_create_demo().await,
        Command::PlayableBoardDemo => demo::playable_board_demo().await,
        Command::Perf { command } => match command {
            PerfCommand::Baseline {
                iterations,
                warmup,
                format,
                baseline_out,
                compare,
                threshold_pct,
            } => {
                perf::perf_baseline(perf::BaselineOptions {
                    iterations,
                    warmup,
                    format,
                    baseline_out,
                    compare,
                    threshold_pct,
                })
                .await
            }
        },
        Command::WorldBundle { command } => match command {
            WorldBundleCommand::Verify { path, json } => world_bundle::verify(path, json).await,
            WorldBundleCommand::Audit { path, json } => world_bundle::audit(path, json).await,
            WorldBundleCommand::Replay { path, json } => world_bundle::replay(path, json).await,
            WorldBundleCommand::Import {
                path,
                data_dir,
                json,
            } => world_bundle::import(path, data_dir, json).await,
        },
        Command::Contract { command } => match command {
            ContractCommand::Migrate {
                path,
                write,
                json,
                all_aliases,
            } => contract::migrate(path, write, json, all_aliases).await,
        },
    }
}
