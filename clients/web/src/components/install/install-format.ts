import type { InstallDetectedKind, InstallPlan } from "@/protocol/client";
import type { LocaleDictionary } from "@/lib/labels";

type LabelArgs<K extends keyof LocaleDictionary> = LocaleDictionary[K] extends (...args: infer Args) => string
  ? Args
  : [];
type TFunction = <K extends keyof LocaleDictionary>(key: K, ...args: LabelArgs<K>) => string;

export function formatList(values: string[] | undefined, empty: string, more: (count: number) => string, limit = 3) {
  if (!values || values.length === 0) return empty;
  const shown = values.slice(0, limit).join(" · ");
  const remaining = values.length - limit;
  return remaining > 0 ? `${shown} · ${more(remaining)}` : shown;
}

export function shortHash(hash: string) {
  return hash.length > 12 ? `${hash.slice(0, 12)}…` : hash;
}

export function formatDetectedKind(kind: InstallDetectedKind | null, t: TFunction) {
  switch (kind?.kind) {
    case "native":
      return t("installKindNative");
    case "declared_external":
      return t("installKindDeclaredExternal");
    case "external":
      return t("installKindExternal");
    default:
      return t("installKindDetected");
  }
}

export function detectKindFromInstallPlan(plan: InstallPlan): InstallDetectedKind | null {
  const descriptor = readProjectDescriptor(plan.project_descriptor);
  const projectType = descriptor?.project?.type;
  if (projectType === "plurora_native") {
    return { kind: "native", descriptor };
  }
  if (projectType === "external_wrapped" || projectType === "external_workspace") {
    return { kind: "declared_external", descriptor };
  }
  return null;
}

function readProjectDescriptor(value: unknown): { project?: { type?: unknown } } | null {
  if (!value || typeof value !== "object") return null;
  const descriptor = value as { project?: unknown };
  if (!descriptor.project || typeof descriptor.project !== "object") return null;
  return descriptor as { project?: { type?: unknown } };
}

export function errorMessage(err: unknown) {
  return err instanceof Error ? err.message : String(err);
}

export function summarizeConformance(plan: InstallPlan, t: TFunction) {
  let checks = 0;
  let failures = 0;
  let warnings = 0;
  for (const pkg of plan.packages) {
    const report = pkg.conformance;
    if (!report) continue;
    if (Array.isArray(report.checks)) {
      checks += report.checks.length;
      failures += report.checks.filter((check) => check.passed === false || check.status === "failed").length;
    }
    if (Array.isArray(report.failures)) failures += report.failures.length;
    if (Array.isArray(report.warnings)) warnings += report.warnings.length;
    if (report.passed === false && failures === 0) failures += 1;
  }
  if (checks === 0 && failures === 0 && warnings === 0) {
    return { hasFailures: false, label: t("installNoConformanceDetails") };
  }
  return {
    hasFailures: failures > 0,
    label: t("installConformanceSummary", checks, failures, warnings),
  };
}
