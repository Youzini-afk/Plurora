import type { FailureDetail } from "@/components/install/failure-modal";
import { formatRelativeAge, type RelativeAgeLabels } from "@/lib/format";
import type { PluroraProtocolClient, PackageRecord, SubprocessLogLine } from "@/protocol/client";

export interface FailureDiagnosticLabels {
  noDiagnosticAvailable: string;
  unavailableCause: string;
  packageFailureTitle: (packageId: string, state: string) => string;
  packageDegradedSummary: string;
  relativeAge: RelativeAgeLabels;
}

export function noFailureDiagnostic(
  installationName: string,
  reason: string,
  labels?: Pick<FailureDiagnosticLabels, "noDiagnosticAvailable" | "unavailableCause">,
): FailureDetail {
  return {
    installationName,
    title: labels?.noDiagnosticAvailable ?? "No diagnostic available",
    summary: reason,
    cause: labels?.unavailableCause ?? "unavailable",
    log: [],
  };
}

export function failureDetailFromPackage(
  installationName: string,
  record: PackageRecord,
  _logs: SubprocessLogLine[],
  labels?: FailureDiagnosticLabels,
): FailureDetail {
  const failure = record.last_failure;
  const redactionSafe = failure?.redaction_state === "redacted" || failure?.redaction_state === "safe";
  const stderrLines = redactionSafe ? tail(failure?.stderr_tail_redacted ?? [], 8) : [];
  return {
    installationName,
    title: labels?.packageFailureTitle(record.id, record.state) ?? `Package ${record.id} ${record.state}`,
    summary: failure?.reason ?? labels?.packageDegradedSummary ?? "Package status is degraded, but no failure summary was reported.",
    cause: failure?.reason ?? record.state,
    exitCode: failure?.exit_code ?? "—",
    failedAt: failure?.failed_at ? formatRelativeAge(failure.failed_at, labels?.relativeAge) : undefined,
    redactionState: failure?.redaction_state,
    log: stderrLines,
    logRedacted: redactionSafe,
  };
}

export async function resolvePackageStatus(
  client: PluroraProtocolClient,
  packageRef: string,
  packageLookup: Map<string, PackageRecord>,
): Promise<PackageRecord | null> {
  const direct = await client.packageStatus(packageRef).catch<PackageRecord | null>(() => null);
  if (direct) return direct;

  const fileName = packageRef.split(/[\\/]/).pop();
  const packageDir = fileName?.match(/^manifest\.ya?ml$/i)
    ? packageRef.split(/[\\/]/).slice(-2, -1)[0]
    : undefined;
  const candidates = [packageRef, packageDir].filter(Boolean) as string[];
  return (
    candidates
      .flatMap((candidate) => [candidate, ...Array.from(packageLookup.values()).filter((record) => record.id.endsWith(`/${candidate}`)).map((record) => record.id)])
      .map((candidate) => packageLookup.get(candidate))
      .find(Boolean) ?? null
  );
}

export function tail<T>(items: T[], limit: number): T[] {
  return items.slice(Math.max(0, items.length - limit));
}
