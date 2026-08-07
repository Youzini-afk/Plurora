use std::time::Instant;

use plurora_core::{
    conformance::summarize, CheckResult, CheckStatus, ImplementationConformanceReport,
    ProtocolConformanceReport,
};
use plurora_runtime::protocol_descriptor;

use super::registry::build_cases;

/// Options parsed from CLI for the conformance command.
pub(crate) struct ConformanceOptions {
    pub(crate) list: bool,
    pub(crate) case: Vec<String>,
    pub(crate) tag: Vec<String>,
    pub(crate) fail_fast: bool,
    pub(crate) slowest: usize,
}

pub(crate) type CaseFuture =
    std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send>>;
pub(crate) type CaseFn = fn() -> CaseFuture;

/// A single conformance case with metadata.
pub(crate) struct ConformanceCase {
    pub(crate) id: &'static str,
    pub(crate) tags: &'static [&'static str],
    pub(crate) run: CaseFn,
}

/// Format a duration for display.
fn fmt_duration(d: std::time::Duration) -> String {
    if d < std::time::Duration::from_millis(1) {
        format!("{:.0}µs", d.as_micros())
    } else if d < std::time::Duration::from_secs(1) {
        format!("{:.1}ms", d.as_secs_f64() * 1000.0)
    } else {
        format!("{:.2}s", d.as_secs_f64())
    }
}

pub(crate) async fn run(opts: ConformanceOptions) -> anyhow::Result<()> {
    let all_cases = build_cases();

    // --- list mode: print ids + tags and exit ---
    if opts.list {
        for case in &all_cases {
            let tags = case.tags.join(", ");
            println!("{:<55} [{}]", case.id, tags);
        }
        println!("total: {} cases", all_cases.len());
        return Ok(());
    }

    // --- filter cases ---
    let selected: Vec<&ConformanceCase> = all_cases
        .iter()
        .filter(|case| {
            // --case <substring>: case id must contain at least one matching substring
            let case_match =
                opts.case.is_empty() || opts.case.iter().any(|p| case.id.contains(p.as_str()));
            // --tag <tag>: case must have at least one matching tag
            let tag_match =
                opts.tag.is_empty() || opts.tag.iter().any(|t| case.tags.contains(&t.as_str()));
            case_match && tag_match
        })
        .collect();

    if selected.is_empty() {
        eprintln!("no conformance cases matched the given filters");
        anyhow::bail!("no cases selected");
    }

    // --- execute cases ---
    let mut results: Vec<(&ConformanceCase, anyhow::Result<()>, std::time::Duration)> = Vec::new();
    let mut failed = false;

    for case in &selected {
        let start = Instant::now();
        let result = (case.run)().await;
        let elapsed = start.elapsed();
        let ok = result.is_ok();
        if !ok {
            failed = true;
        }
        match &result {
            Ok(()) => println!("{:<55} PASS  {}", case.id, fmt_duration(elapsed)),
            Err(err) => println!("{:<55} FAIL  {}  {}", case.id, fmt_duration(elapsed), err),
        }
        results.push((case, result, elapsed));
        if !ok && opts.fail_fast {
            break;
        }
    }

    // --- slowest report ---
    if opts.slowest > 0 && results.len() > 1 {
        let mut timed: Vec<(&ConformanceCase, std::time::Duration)> =
            results.iter().map(|(case, _, dur)| (*case, *dur)).collect();
        timed.sort_by(|a, b| b.1.cmp(&a.1));
        let n = opts.slowest.min(timed.len());
        println!();
        println!("slowest {} cases:", n);
        for (case, elapsed) in timed.iter().take(n) {
            let was_ok = results.iter().any(|(c, r, _)| c.id == case.id && r.is_ok());
            let status = if was_ok { "PASS" } else { "FAIL" };
            println!("  {:<55} {}  {}", case.id, status, fmt_duration(*elapsed));
        }
    }

    // --- summary ---
    let pass_count = results.iter().filter(|r| r.1.is_ok()).count();
    let fail_count = results.len() - pass_count;
    if failed {
        println!();
        anyhow::bail!(
            "conformance: {}/{} passed, {} failed",
            pass_count,
            results.len(),
            fail_count
        );
    }
    println!(
        "\nconformance: ok ({} cases, {})",
        results.len(),
        fmt_duration(results.iter().map(|r| r.2).sum::<std::time::Duration>())
    );
    Ok(())
}

pub(crate) async fn run_protocol_report(
    protocol_id: &str,
    implementation_id: Option<&str>,
    json: bool,
) -> anyhow::Result<()> {
    let descriptor = protocol_descriptor(protocol_id)
        .ok_or_else(|| anyhow::anyhow!("unknown Protocol Commons id '{protocol_id}'"))?;
    let profile = descriptor
        .compatibility_profiles
        .first()
        .ok_or_else(|| anyhow::anyhow!("protocol '{protocol_id}' has no compatibility profile"))?;
    let implementation = implementation_id
        .map(|implementation_id| {
            descriptor
                .conforming_implementations
                .iter()
                .find(|implementation| implementation.implementation_id == implementation_id)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "implementation '{implementation_id}' is not registered for '{protocol_id}'"
                    )
                })
        })
        .transpose()?;
    if implementation.is_some_and(|implementation| implementation.test_only) {
        anyhow::bail!(
            "test-only implementation claims are registry fixtures, not executable production reports"
        );
    }

    let cases = build_cases();
    let mut results = Vec::new();
    for vector in descriptor
        .conformance_vectors
        .iter()
        .filter(|vector| vector.required)
    {
        let result = if let Some(case) = cases.iter().find(|case| case.id == vector.id) {
            (case.run)().await
        } else {
            Err(anyhow::anyhow!(
                "required protocol vector '{}' has no executable conformance case",
                vector.id
            ))
        };
        results.push(CheckResult {
            id: vector.id.clone(),
            status: if result.is_ok() {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail
            },
            details: result.err().map(|error| error.to_string()),
            subreports: Vec::new(),
        });
    }
    let summary = summarize(&results);

    if let Some(implementation) = implementation {
        let report = ImplementationConformanceReport {
            implementation_id: implementation.implementation_id.clone(),
            provider: implementation.provider.clone(),
            protocol_id: descriptor.protocol_id.clone(),
            protocol_version: descriptor.version.clone(),
            profiles: implementation.profiles.clone(),
            vector_results: results,
            summary,
        };
        emit_protocol_report(&report, json)?;
        anyhow::ensure!(
            report.summary.passed_all_blocking(),
            "implementation conformance failed: {}/{} vectors passed",
            report.summary.passed,
            report.summary.total
        );
    } else {
        let report = ProtocolConformanceReport {
            protocol_id: descriptor.protocol_id.clone(),
            protocol_version: descriptor.version.clone(),
            profile: profile.id.clone(),
            vector_results: results,
            summary,
        };
        emit_protocol_report(&report, json)?;
        anyhow::ensure!(
            report.summary.passed_all_blocking(),
            "protocol conformance failed: {}/{} vectors passed",
            report.summary.passed,
            report.summary.total
        );
    }
    Ok(())
}

fn emit_protocol_report<T: serde::Serialize>(report: &T, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
    } else {
        let value = serde_json::to_value(report)?;
        if let Some(id) = value
            .get("implementation_id")
            .or_else(|| value.get("protocol_id"))
            .and_then(serde_json::Value::as_str)
        {
            println!("conformance subject: {id}");
        }
        if let Some(results) = value
            .get("vector_results")
            .and_then(|value| value.as_array())
        {
            for result in results {
                let id = result["id"].as_str().unwrap_or("unknown");
                let status = result["status"].as_str().unwrap_or("unknown");
                println!("{:<55} {}", id, status.to_ascii_uppercase());
            }
        }
        if let Some(summary) = value.get("summary") {
            println!(
                "summary: {}/{} passed",
                summary["passed"].as_u64().unwrap_or(0),
                summary["total"].as_u64().unwrap_or(0)
            );
        }
    }
    Ok(())
}
