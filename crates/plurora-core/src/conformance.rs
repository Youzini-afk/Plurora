use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use jsonschema::JSONSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    package_envelope_for_manifest, ComponentBoundaryClaims, ComponentClaimStatus,
    ComponentTrustClass, PackageEnvelopeDescriptor, PackageManifest,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageConformanceReport {
    pub package_id: String,
    pub manifest_path: PathBuf,
    pub contract_version: String,
    pub checks: Vec<CheckResult>,
    pub summary: ConformanceSummary,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_envelope: Option<PackageEnvelopeDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<PackageComponentConformance>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transitive_reports: Vec<PackageConformanceReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolConformanceReport {
    pub protocol_id: String,
    pub protocol_version: String,
    pub profile: String,
    pub vector_results: Vec<CheckResult>,
    pub summary: ConformanceSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageComponentConformance {
    pub component_id: String,
    pub component_digest: String,
    pub behavior_digest: String,
    pub trust_class: ComponentTrustClass,
    pub claim_status: ComponentClaimStatus,
    pub enforced_boundaries: ComponentBoundaryClaims,
    pub composability_guaranteed: bool,
    pub portability_guaranteed: bool,
    pub guarantee: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplementationConformanceReport {
    pub implementation_id: String,
    pub provider: String,
    pub protocol_id: String,
    pub protocol_version: String,
    pub profiles: Vec<String>,
    pub vector_results: Vec<CheckResult>,
    pub summary: ConformanceSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub id: String,
    pub status: CheckStatus,
    pub details: Option<String>,
    pub subreports: Vec<SubReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubReport {
    pub id: String,
    pub status: CheckStatus,
    pub details: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum CheckStatus {
    Pass,
    Fail,
    Skip,
    Warning,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConformanceSummary {
    pub total: u32,
    pub passed: u32,
    pub failed: u32,
    pub skipped: u32,
    pub warnings: u32,
    pub compliance_pct: f32,
}

impl PackageConformanceReport {
    pub fn failed_checks(&self) -> Vec<String> {
        self.checks
            .iter()
            .filter(|check| check.status == CheckStatus::Fail)
            .map(|check| {
                let details = check.details.as_deref().unwrap_or("no details");
                format!("{}: FAIL ({details})", check.id)
            })
            .collect()
    }
}

impl ConformanceSummary {
    pub fn passed_all_blocking(&self) -> bool {
        self.failed == 0
    }
}
pub async fn run_checks(
    package_path: &Path,
    contract: &str,
    static_only: bool,
) -> Result<PackageConformanceReport> {
    let manifest_path = resolve_manifest_path(package_path)?;
    let raw_value = read_manifest_value(&manifest_path)?;
    let package_id = raw_value
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();

    let mut checks = Vec::new();
    let schema_check = check_manifest_schema_valid(&manifest_path, &raw_value);
    let schema_ok = schema_check.status == CheckStatus::Pass;
    checks.push(schema_check);

    let manifest = if schema_ok {
        Some(read_manifest(&manifest_path)?)
    } else {
        None
    };

    if let Some(manifest) = manifest.as_ref() {
        checks.push(check_manifest_declarations_consistent(manifest));
        checks.push(check_component_identity_and_trust(manifest));
    } else {
        checks.push(skip(
            "manifest.declarations_consistent",
            "manifest schema invalid",
        ));
        checks.push(skip(
            "component.identity_and_trust",
            "manifest schema invalid",
        ));
    }

    if !schema_ok {
        for id in PROCESS_CHECK_IDS {
            checks.push(skip(id, "manifest schema invalid"));
        }
        checks.push(skip(
            "events_and_errors_consistent_with_registry",
            "manifest schema invalid",
        ));
    } else if static_only {
        for id in PROCESS_CHECK_IDS {
            checks.push(skip(id, "static-only mode"));
        }
        checks.push(skip(
            "events_and_errors_consistent_with_registry",
            "static-only mode",
        ));
    } else {
        for id in PROCESS_CHECK_IDS {
            checks.push(skip(id, "runtime not supplied"));
        }
        checks.push(skip(
            "events_and_errors_consistent_with_registry",
            "runtime not supplied",
        ));
    }

    let package_envelope = manifest
        .as_ref()
        .and_then(|manifest| package_envelope_for_manifest(manifest).ok());
    let components = package_envelope
        .as_ref()
        .map(component_conformance_records)
        .unwrap_or_default();
    let summary = summarize(&checks);
    Ok(PackageConformanceReport {
        package_id,
        manifest_path,
        contract_version: contract.to_string(),
        checks,
        summary,
        package_envelope,
        components,
        transitive_reports: Vec::new(),
    })
}

pub const PROCESS_CHECK_IDS: &[&str] = &[
    "handshake.feature_negotiation",
    "capability.smoke_invocations",
    "streaming.cancel_and_timeout",
    "permission.denial_paths",
    "handle.lifecycle",
];

pub fn resolve_manifest_path(path: &Path) -> Result<PathBuf> {
    if path.is_file() {
        return Ok(path.to_path_buf());
    }
    for name in ["manifest.yaml", "manifest.yml", "manifest.json"] {
        let candidate = path.join(name);
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    anyhow::bail!(
        "no manifest.yaml, manifest.yml, or manifest.json found in {}",
        path.display()
    )
}

fn read_manifest(path: &Path) -> Result<PackageManifest> {
    let raw = fs::read_to_string(path)?;
    let manifest = match path.extension().and_then(|ext| ext.to_str()) {
        Some("yaml") | Some("yml") => serde_yaml::from_str(&raw)?,
        _ => serde_json::from_str(&raw)?,
    };
    Ok(manifest)
}

fn read_manifest_value(path: &Path) -> Result<Value> {
    let raw = fs::read_to_string(path)?;
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("yaml") | Some("yml") => Ok(serde_yaml::from_str(&raw)?),
        _ => Ok(serde_json::from_str(&raw)?),
    }
}

pub fn check_manifest_schema_valid(manifest_path: &Path, manifest_value: &Value) -> CheckResult {
    let schema_path_buf = schema_path();
    let schema_path = schema_path_buf.as_path();
    let schema_text = match fs::read_to_string(schema_path) {
        Ok(text) => text,
        Err(error) => return fail("manifest.schema_valid", format!("read schema: {error}")),
    };
    let mut schema_value: Value = match serde_json::from_str(&schema_text) {
        Ok(value) => value,
        Err(error) => return fail("manifest.schema_valid", format!("parse schema: {error}")),
    };
    // schemars emits `$defs`, while the current exported artifact still uses
    // legacy `#/definitions/*` refs. Mirror `$defs` under `definitions` for
    // jsonschema validation until the exporter is normalized.
    if schema_value.get("definitions").is_none() {
        if let Some(defs) = schema_value.get("$defs").cloned() {
            if let Some(object) = schema_value.as_object_mut() {
                object.insert("definitions".to_string(), defs);
            }
        }
    }
    let compiled = match JSONSchema::compile(&schema_value) {
        Ok(compiled) => compiled,
        Err(error) => return fail("manifest.schema_valid", format!("compile schema: {error}")),
    };
    let result = match compiled.validate(manifest_value) {
        Ok(()) => pass(
            "manifest.schema_valid",
            Some(format!("{} is schema-valid", manifest_path.display())),
        ),
        Err(errors) => fail(
            "manifest.schema_valid",
            errors.map(|e| e.to_string()).collect::<Vec<_>>().join("; "),
        ),
    };
    result
}

fn schema_path() -> PathBuf {
    let cwd_relative = PathBuf::from("docs/spec/v1/schemas/manifest.schema.json");
    if cwd_relative.is_file() {
        cwd_relative
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("docs/spec/v1/schemas/manifest.schema.json")
    }
}

pub fn check_manifest_declarations_consistent(manifest: &PackageManifest) -> CheckResult {
    let mut failures = Vec::new();
    let mut warnings = Vec::new();
    if let Err(error) = manifest.validate_basic() {
        failures.push(error.to_string());
    }
    let mut owners = vec![manifest.id.as_str()];
    if let Some(component) = &manifest.entry.component {
        owners.push(component.id.as_str());
    }

    for cap in &manifest.provides {
        if !owners.iter().any(|owner| {
            cap.id
                .strip_prefix(owner)
                .is_some_and(|rest| rest.starts_with('/'))
        }) {
            failures.push(format!(
                "provided capability '{}' is not under package or component namespace",
                cap.id
            ));
        }
    }

    let known_namespaces: BTreeSet<String> = manifest
        .provides
        .iter()
        .filter_map(|cap| namespace_of(&cap.id))
        .chain(std::iter::once(manifest.id.clone()))
        .chain(std::iter::once("kernel/v1".to_string()))
        .collect();
    for req in &manifest.consumes {
        if namespace_of(&req.id)
            .map(|ns| !known_namespaces.contains(&ns))
            .unwrap_or(true)
        {
            warnings.push(format!(
                "consumed capability '{}' references an unrecognized namespace",
                req.id
            ));
        }
    }

    for secret_ref in &manifest.permissions.secret_refs {
        if !valid_contract_secret_ref(secret_ref) {
            failures.push(format!("secret_ref '{}' is not env-formatted", secret_ref));
        }
    }

    for declaration in &manifest.permissions.network.declarations {
        for method in &declaration.methods {
            if !valid_http_method(method) {
                failures.push(format!(
                    "network method '{}' for host '{}' is invalid",
                    method, declaration.host
                ));
            }
        }
    }

    if !failures.is_empty() {
        fail("manifest.declarations_consistent", failures.join("; "))
    } else if !warnings.is_empty() {
        warning("manifest.declarations_consistent", warnings.join("; "))
    } else {
        pass("manifest.declarations_consistent", None)
    }
}

pub fn check_component_identity_and_trust(manifest: &PackageManifest) -> CheckResult {
    let envelope = match package_envelope_for_manifest(manifest) {
        Ok(envelope) => envelope,
        Err(error) => return fail("component.identity_and_trust", error.to_string()),
    };
    let Some(component) = envelope.components.first() else {
        return fail(
            "component.identity_and_trust",
            "package has no component descriptor",
        );
    };
    match component.claim_status {
        ComponentClaimStatus::Declared => pass(
            "component.identity_and_trust",
            Some(format!(
                "declared component '{}' is independently addressable as {} with trust class {:?}",
                component.component_id, component.artifact.digest, component.trust_class
            )),
        ),
        ComponentClaimStatus::LegacyAdapted => warning(
            "component.identity_and_trust",
            format!(
                "component '{}' uses a package-scoped legacy identity; package-contract composability is retained, but cross-package portability is not guaranteed",
                component.component_id
            ),
        ),
        ComponentClaimStatus::ForeignCapsule => warning(
            "component.identity_and_trust",
            format!(
                "component '{}' is a Foreign Capsule; it may be hosted, but composability and portability are not guaranteed",
                component.component_id
            ),
        ),
    }
}

fn component_conformance_records(
    envelope: &PackageEnvelopeDescriptor,
) -> Vec<PackageComponentConformance> {
    envelope
        .components
        .iter()
        .map(|component| {
            let (composability_guaranteed, portability_guaranteed, guarantee) =
                match component.claim_status {
                    ComponentClaimStatus::Declared => (
                        true,
                        false,
                        "explicit component identity and behavior claims are independently pinned; portability requires a separate passing implementation-conformance report"
                            .to_string(),
                    ),
                    ComponentClaimStatus::LegacyAdapted => (
                        true,
                        false,
                        "package-contract composability is retained; cross-package portability is not guaranteed"
                            .to_string(),
                    ),
                    ComponentClaimStatus::ForeignCapsule => (
                        false,
                        false,
                        "host startup is supported; composability and portability are not guaranteed"
                            .to_string(),
                    ),
                };
            PackageComponentConformance {
                component_id: component.component_id.clone(),
                component_digest: component.artifact.digest.clone(),
                behavior_digest: component.behavior.digest.clone(),
                trust_class: component.trust_class,
                claim_status: component.claim_status,
                enforced_boundaries: component.enforced_boundaries.clone(),
                composability_guaranteed,
                portability_guaranteed,
                guarantee,
            }
        })
        .collect()
}

pub fn summarize(checks: &[CheckResult]) -> ConformanceSummary {
    let total = checks.len() as u32;
    let passed = checks
        .iter()
        .filter(|c| c.status == CheckStatus::Pass)
        .count() as u32;
    let failed = checks
        .iter()
        .filter(|c| c.status == CheckStatus::Fail)
        .count() as u32;
    let skipped = checks
        .iter()
        .filter(|c| c.status == CheckStatus::Skip)
        .count() as u32;
    let warnings = checks
        .iter()
        .filter(|c| c.status == CheckStatus::Warning)
        .count() as u32;
    let applicable = total.saturating_sub(skipped);
    let compliance_pct = if applicable == 0 {
        100.0
    } else {
        ((passed + warnings) as f32 / applicable as f32) * 100.0
    };
    ConformanceSummary {
        total,
        passed,
        failed,
        skipped,
        warnings,
        compliance_pct,
    }
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

fn namespace_of(id: &str) -> Option<String> {
    let mut parts = id.split('/');
    Some(format!("{}/{}", parts.next()?, parts.next()?))
}

fn valid_contract_secret_ref(value: &str) -> bool {
    if let Some(name) = value.strip_prefix("secret_ref:env:") {
        !name.is_empty()
    } else if let Some(name) = value.strip_prefix("secret_ref:") {
        !name.is_empty() && !name.contains(':')
    } else {
        false
    }
}

fn valid_http_method(method: &str) -> bool {
    matches!(
        method.to_ascii_uppercase().as_str(),
        "GET" | "POST" | "PUT" | "DELETE" | "PATCH" | "HEAD" | "OPTIONS" | "WEBSOCKET"
    )
}
