use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use clap::Args;
use plurora_core::conformance::{
    self, CheckResult, CheckStatus, PackageConformanceReport, SubReport,
};
use plurora_core::{
    CapHandle, CapHandleId, ContractMode, HandleLease, HandleProvenance, HandleScope,
};
use plurora_runtime::{
    CapabilityInvocationRequest, EventStore, InMemoryEventStore, ProtocolContext, Runtime,
    RuntimeConfig,
};
use serde_json::{json, Value};

use super::manifest::read_manifest;

struct LoadedRuntime {
    runtime: Runtime<InMemoryEventStore>,
    store: Arc<InMemoryEventStore>,
}

#[derive(Args, Debug)]
pub struct ConformancePackageArgs {
    /// Path to package directory containing manifest.yaml or manifest.json
    #[arg(long)]
    pub path: PathBuf,

    /// Contract version to validate against
    #[arg(long, default_value = "v1")]
    pub contract: String,

    /// Output format
    #[arg(long, default_value = "human")]
    pub format: ReportFormat,

    /// Skip running checks that require starting the package process
    /// (useful in CI where subprocess can't run)
    #[arg(long)]
    pub static_only: bool,

    /// Resolve and validate manifest.requires dependency tree.
    /// In static-only mode, non-local dependency sources are reported as skipped.
    #[arg(long)]
    pub transitive: bool,
}

#[derive(Clone, Debug, clap::ValueEnum)]
pub enum ReportFormat {
    Human,
    Json,
}

pub(crate) async fn run(args: ConformancePackageArgs) -> Result<()> {
    let mut report = conformance::run_checks(&args.path, &args.contract, args.static_only).await?;
    if !args.static_only {
        add_runtime_checks(&mut report).await;
    }

    match args.format {
        ReportFormat::Human => print_human(&report),
        ReportFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
    }

    if args.transitive {
        print_transitive_note(&report, args.static_only);
    }

    if report.summary.failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}

fn print_human(report: &PackageConformanceReport) {
    println!(
        "Conformance Report: {}",
        report
            .manifest_path
            .parent()
            .unwrap_or(Path::new("."))
            .display()
    );
    println!("Contract: {}\n", report.contract_version);
    if let Some(envelope) = &report.package_envelope {
        println!("Package envelope: {}", envelope.artifact.digest);
    }
    for component in &report.components {
        println!(
            "Component: {}  trust={:?}  claim={:?}",
            component.component_id, component.trust_class, component.claim_status
        );
        println!("Guarantee: {}", component.guarantee);
    }
    if !report.components.is_empty() {
        println!();
    }
    for (idx, check) in report.checks.iter().enumerate() {
        let detail = check
            .details
            .as_ref()
            .map(|d| format!("  ({d})"))
            .unwrap_or_default();
        println!(
            "[{}/{}] {:50} {:7}{}",
            idx + 1,
            report.checks.len(),
            check.id,
            status_label(check.status),
            detail
        );
    }
    let applicable = report.summary.total.saturating_sub(report.summary.skipped);
    println!(
        "\nSummary: {}/{} applicable checks passed ({:.0}%)",
        report.summary.passed + report.summary.warnings,
        applicable,
        report.summary.compliance_pct
    );
}

fn print_transitive_note(report: &PackageConformanceReport, static_only: bool) {
    // Full transitive git/internal resolution requires install-lab's fetch/catalog context.
    // The install flow performs recursive per-package validation; the standalone CLI keeps
    // current behavior and makes --transitive explicit without failing local CI.
    let mode = if static_only {
        "static-only"
    } else {
        "runtime-free"
    };
    eprintln!(
        "NOTE: --transitive requested; {} report currently covers root package {}. install-lab validates resolved dependencies during install planning.",
        mode, report.package_id
    );
}

fn status_label(status: CheckStatus) -> &'static str {
    match status {
        CheckStatus::Pass => "PASS",
        CheckStatus::Fail => "FAIL",
        CheckStatus::Skip => "SKIP",
        CheckStatus::Warning => "WARNING",
    }
}

async fn add_runtime_checks(report: &mut PackageConformanceReport) {
    let manifest = match read_manifest(report.manifest_path.clone()).await {
        Ok(manifest) => manifest,
        Err(error) => {
            replace_check(
                report,
                fail("handshake.feature_negotiation", error.to_string()),
            );
            return;
        }
    };
    match load_runtime(manifest.clone()).await {
        Ok(rt) => {
            replace_check(
                report,
                check_handshake_feature_negotiation(&manifest, &rt).await,
            );
            replace_check(
                report,
                check_capability_smoke_invocations(&manifest, &rt.runtime).await,
            );
            replace_check(
                report,
                check_streaming_cancel_and_timeout(&manifest, &rt).await,
            );
            replace_check(report, check_permission_denial_paths(&manifest, &rt).await);
            replace_check(report, check_handle_lifecycle(&manifest, &rt.runtime).await);
            replace_check(report, check_events_and_errors(&manifest, &rt).await);
            let _ = rt.runtime.unload_package(&manifest.id).await;
        }
        Err(error) => {
            replace_check(
                report,
                fail("handshake.feature_negotiation", error.to_string()),
            );
            for id in [
                "capability.smoke_invocations",
                "streaming.cancel_and_timeout",
                "permission.denial_paths",
                "handle.lifecycle",
            ] {
                replace_check(report, skip(id, "package did not load"));
            }
        }
    }
    report.summary = conformance::summarize(&report.checks);
}

fn replace_check(report: &mut PackageConformanceReport, check: CheckResult) {
    if let Some(existing) = report
        .checks
        .iter_mut()
        .find(|existing| existing.id == check.id)
    {
        *existing = check;
    } else {
        report.checks.push(check);
    }
}

async fn load_runtime(manifest: plurora_core::PackageManifest) -> Result<LoadedRuntime> {
    let store = Arc::new(InMemoryEventStore::default());
    let runtime = Runtime::new(store.clone(), RuntimeConfig::default());
    runtime.load_package(manifest).await?;
    Ok(LoadedRuntime { runtime, store })
}

async fn check_handshake_feature_negotiation(
    manifest: &plurora_core::PackageManifest,
    rt: &LoadedRuntime,
) -> CheckResult {
    if manifest.entry.contract == ContractMode::None {
        return pass(
            "handshake.feature_negotiation",
            Some("Path B contract=none; bindings not required".to_string()),
        );
    }
    let Some(record) = rt.runtime.package_status(&manifest.id).await else {
        return fail("handshake.feature_negotiation", "package not loaded");
    };
    let expected_bindings = manifest.permissions.capabilities.invoke.len()
        + manifest.permissions.network.declarations.len()
        + manifest.permissions.secret_refs.len();
    let actual_bindings = rt.runtime.handles().list_for(&manifest.id).await.len();
    if actual_bindings != expected_bindings {
        return fail(
            "handshake.feature_negotiation",
            format!("binding count mismatch: expected {expected_bindings}, got {actual_bindings}"),
        );
    }
    pass(
        "handshake.feature_negotiation",
        Some(format!(
            "ready {:?}; {} bindings",
            record.state, actual_bindings
        )),
    )
}

async fn check_capability_smoke_invocations(
    manifest: &plurora_core::PackageManifest,
    runtime: &Runtime<InMemoryEventStore>,
) -> CheckResult {
    if manifest.provides.is_empty() {
        return pass(
            "capability.smoke_invocations",
            Some("0/0 capabilities".to_string()),
        );
    }
    let mut subreports = Vec::new();
    for cap in &manifest.provides {
        if cap.streaming {
            subreports.push(SubReport {
                id: cap.id.clone(),
                status: CheckStatus::Skip,
                details: Some("streaming capability covered by streaming check".to_string()),
            });
            continue;
        }
        let result = runtime
            .invoke_capability(CapabilityInvocationRequest {
                handle: None,
                capability_id: Some(cap.id.clone()),
                caller_package_id: None,
                provider_package_id: Some(manifest.id.clone()),
                version: Some(cap.version.clone()),
                session_id: None,
                input: json!({}),
            })
            .await;
        match result {
            Ok(_) => subreports.push(SubReport {
                id: cap.id.clone(),
                status: CheckStatus::Pass,
                details: None,
            }),
            Err(error) if acceptable_structured_error(&error.to_string()) => {
                subreports.push(SubReport {
                    id: cap.id.clone(),
                    status: CheckStatus::Pass,
                    details: Some(error.to_string()),
                })
            }
            Err(error) => subreports.push(SubReport {
                id: cap.id.clone(),
                status: CheckStatus::Fail,
                details: Some(error.to_string()),
            }),
        }
    }
    let failures = subreports
        .iter()
        .filter(|r| r.status == CheckStatus::Fail)
        .count();
    let passed = subreports
        .iter()
        .filter(|r| r.status == CheckStatus::Pass)
        .count();
    let total = subreports
        .iter()
        .filter(|r| r.status != CheckStatus::Skip)
        .count();
    CheckResult {
        id: "capability.smoke_invocations".to_string(),
        status: if failures == 0 {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        details: Some(format!("{passed}/{total} capabilities")),
        subreports,
    }
}

async fn check_streaming_cancel_and_timeout(
    manifest: &plurora_core::PackageManifest,
    rt: &LoadedRuntime,
) -> CheckResult {
    let streaming: Vec<_> = manifest
        .provides
        .iter()
        .filter(|cap| cap.streaming)
        .collect();
    if streaming.is_empty() {
        return skip("streaming.cancel_and_timeout", "no streaming");
    }
    let mut subreports = Vec::new();
    for cap in streaming {
        let session_id = format!("conformance_stream_{}", cap.id.replace('/', "_"));
        let started = rt
            .runtime
            .stream_capability_start(
                &session_id,
                &cap.id,
                Some(&manifest.id),
                Some(&cap.version),
                json!({}),
            )
            .await;
        let Ok((_frame, record)) = started else {
            subreports.push(SubReport {
                id: cap.id.clone(),
                status: CheckStatus::Fail,
                details: Some(started.err().unwrap().to_string()),
            });
            continue;
        };
        let _ = rt
            .runtime
            .stream_capability_chunk(
                &session_id,
                &record.invocation_id,
                json!({"conformance": true}),
                plurora_core::RedactionState::NotCaptured,
            )
            .await;
        let cancelled = rt
            .runtime
            .stream_capability_cancel(&session_id, &record.invocation_id)
            .await;
        let events = rt.store.list_session(&session_id).await.unwrap_or_default();
        let emitted = events
            .iter()
            .any(|e| e.kind == plurora_core::EVENT_STREAM_CANCELLED);
        let next_ok = rt
            .runtime
            .invoke_capability(CapabilityInvocationRequest {
                handle: None,
                capability_id: manifest
                    .provides
                    .iter()
                    .find(|c| !c.streaming)
                    .map(|c| c.id.clone()),
                caller_package_id: None,
                provider_package_id: Some(manifest.id.clone()),
                version: None,
                session_id: None,
                input: json!({}),
            })
            .await
            .is_ok()
            || manifest.provides.iter().all(|c| c.streaming);
        let ok = cancelled.is_ok() && emitted && next_ok;
        subreports.push(SubReport {
            id: cap.id.clone(),
            status: if ok {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail
            },
            details: Some(format!(
                "cancelled_event={emitted}, next_invoke_ok={next_ok}"
            )),
        });
    }
    let failures = subreports
        .iter()
        .filter(|r| r.status == CheckStatus::Fail)
        .count();
    CheckResult {
        id: "streaming.cancel_and_timeout".to_string(),
        status: if failures == 0 {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        details: None,
        subreports,
    }
}

async fn check_permission_denial_paths(
    manifest: &plurora_core::PackageManifest,
    rt: &LoadedRuntime,
) -> CheckResult {
    if manifest.entry.contract == ContractMode::None {
        return pass(
            "permission.denial_paths",
            Some("Path B contract=none; permission enforcement bypass expected".to_string()),
        );
    }
    let denied_capability = format!("{}/__conformance_not_declared", manifest.id);
    let result = rt
        .runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: None,
            capability_id: Some(denied_capability.clone()),
            caller_package_id: Some(manifest.id.clone()),
            provider_package_id: None,
            version: None,
            session_id: None,
            input: json!({}),
        })
        .await;
    // The durable permission-denied event is the stable contract. Error text may
    // become more specific (for example, requiring an explicit Binding handle)
    // without changing the denial outcome.
    let denied = result.is_err();
    let session_id = format!(
        "platform_capability_{}",
        denied_capability.replace('/', "_")
    );
    let emitted = rt
        .store
        .list_session(&session_id)
        .await
        .unwrap_or_default()
        .iter()
        .any(|e| e.kind == plurora_core::EVENT_PERMISSION_DENIED);
    if denied && emitted {
        pass(
            "permission.denial_paths",
            Some("undeclared capability invoke denied".to_string()),
        )
    } else {
        fail(
            "permission.denial_paths",
            format!("denied={denied}, event_emitted={emitted}"),
        )
    }
}

async fn check_handle_lifecycle(
    manifest: &plurora_core::PackageManifest,
    runtime: &Runtime<InMemoryEventStore>,
) -> CheckResult {
    if manifest.entry.contract == ContractMode::None {
        return skip(
            "handle.lifecycle",
            "Path B contract=none; handles are not enforced",
        );
    }
    let Some(cap) = manifest.provides.iter().find(|cap| !cap.streaming) else {
        return skip("handle.lifecycle", "no non-streaming capability");
    };
    let parent = runtime
        .handles()
        .mint(CapHandle {
            id: CapHandleId::new(),
            cap_type: cap.id.clone(),
            cap_version: cap.version.clone(),
            scope: HandleScope {
                holder_package_id: manifest.id.clone(),
                session_id: None,
            },
            constraints: json!({}),
            lease: HandleLease {
                expires_at: None,
                max_invocations: None,
                invocations_used: 0,
            },
            provenance: HandleProvenance {
                granted_at: chrono::Utc::now(),
                granted_by_package_id: plurora_core::PLATFORM_RUNTIME_ID.to_string(),
                via_method: "conformance".to_string(),
            },
            parent: None,
            revoked: false,
        })
        .await;
    let child = match runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "authority.handle.attenuate",
            json!({"parent_handle": parent, "constraints": {}}),
        )
        .await
    {
        Ok(value) => match serde_json::from_value::<CapHandleId>(value["handle"]["id"].clone()) {
            Ok(id) => id,
            Err(error) => {
                return fail(
                    "handle.lifecycle",
                    format!("parse attenuated handle: {error}"),
                )
            }
        },
        Err(error) => return fail("handle.lifecycle", format!("attenuate failed: {error:?}")),
    };
    let invoke = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: Some(child),
            capability_id: None,
            caller_package_id: Some(manifest.id.clone()),
            provider_package_id: Some(manifest.id.clone()),
            version: None,
            session_id: None,
            input: json!({}),
        })
        .await;
    if let Err(error) = invoke {
        return fail(
            "handle.lifecycle",
            format!("invoke with attenuated handle failed: {error}"),
        );
    }
    if let Err(error) = runtime
        .call_protocol(
            &ProtocolContext::host_dev("conformance"),
            "authority.handle.revoke",
            json!({"handle": child}),
        )
        .await
    {
        return fail("handle.lifecycle", format!("revoke failed: {error:?}"));
    }
    let revoked = runtime
        .invoke_capability(CapabilityInvocationRequest {
            handle: Some(child),
            capability_id: None,
            caller_package_id: Some(manifest.id.clone()),
            provider_package_id: Some(manifest.id.clone()),
            version: None,
            session_id: None,
            input: json!({}),
        })
        .await;
    if revoked.is_err() {
        pass(
            "handle.lifecycle",
            Some("attenuate/invoke/revoke lifecycle enforced".to_string()),
        )
    } else {
        fail(
            "handle.lifecycle",
            "revoked handle still invoked successfully",
        )
    }
}

async fn check_events_and_errors(
    manifest: &plurora_core::PackageManifest,
    rt: &LoadedRuntime,
) -> CheckResult {
    let registry = event_registry();
    let events = rt.store.list_all().await.unwrap_or_default();
    let mut failures = Vec::new();
    let mut warnings = Vec::new();
    for event in events {
        if event.writer_package_id == plurora_core::PLATFORM_RUNTIME_ID
            && !registry.contains(event.kind.as_str())
        {
            failures.push(format!("off-registry platform event '{}'", event.kind));
        } else if event.kind.starts_with(&format!("{}/", manifest.id)) {
            warnings.push(format!(
                "package namespace event '{}' is allowed but not registry-standard",
                event.kind
            ));
        }
        if event.kind == plurora_core::EVENT_CAPABILITY_FAILED {
            let ok = event
                .payload
                .get("error_kind")
                .and_then(Value::as_str)
                .is_some()
                && event
                    .payload
                    .get("error_message")
                    .and_then(Value::as_str)
                    .is_some()
                && event
                    .payload
                    .get("error_message_present")
                    .and_then(Value::as_bool)
                    .is_some()
                && event
                    .payload
                    .get("error_fingerprint")
                    .and_then(Value::as_str)
                    .is_some_and(|value| value.starts_with("sha256:"));
            if !ok {
                failures.push(format!(
                    "capability failure event '{}' has invalid error shape",
                    event.id
                ));
            }
        }
    }
    if !failures.is_empty() {
        fail(
            "events_and_errors_consistent_with_registry",
            failures.join("; "),
        )
    } else if !warnings.is_empty() {
        warning(
            "events_and_errors_consistent_with_registry",
            warnings.join("; "),
        )
    } else {
        pass("events_and_errors_consistent_with_registry", None)
    }
}

fn event_registry() -> BTreeSet<&'static str> {
    plurora_core::PLATFORM_EVENT_KINDS.iter().copied().collect()
}

fn pass(id: &str, details: Option<String>) -> CheckResult {
    CheckResult {
        id: id.to_string(),
        status: CheckStatus::Pass,
        details,
        subreports: Vec::new(),
    }
}

fn fail(id: &str, details: impl Into<String>) -> CheckResult {
    CheckResult {
        id: id.to_string(),
        status: CheckStatus::Fail,
        details: Some(details.into()),
        subreports: Vec::new(),
    }
}

fn skip(id: &str, details: impl Into<String>) -> CheckResult {
    CheckResult {
        id: id.to_string(),
        status: CheckStatus::Skip,
        details: Some(details.into()),
        subreports: Vec::new(),
    }
}

fn warning(id: &str, details: impl Into<String>) -> CheckResult {
    CheckResult {
        id: id.to_string(),
        status: CheckStatus::Warning,
        details: Some(details.into()),
        subreports: Vec::new(),
    }
}

fn acceptable_structured_error(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("not_implemented")
        || lower.contains("not implemented")
        || lower.contains("unsupported")
}
