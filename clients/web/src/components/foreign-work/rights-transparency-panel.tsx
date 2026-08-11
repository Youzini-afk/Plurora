import type { ReactNode } from "react";
import type { InstallationWorkSummary } from "@/protocol/generated-types";

const RIGHT_LABELS = [
  ["Install", "install"],
  ["Execute", "execute"],
  ["Backup", "backup"],
  ["Export state", "export_state"],
  ["Copy across Hosts", "copy_across_hosts"],
  ["Redistribute artifacts", "redistribute_artifacts"],
  ["Modify", "modify"],
  ["Derive", "derive"],
  ["Modding", "modding"],
  ["Dedicated server", "dedicated_server"],
] as const;

export function RightsTransparencyPanel({
  summary,
  sourceKind,
}: {
  summary: InstallationWorkSummary;
  sourceKind: string;
}) {
  const rights = summary.rights_declaration;
  const transparency = summary.transparency_declaration;
  const evidenceCount = (rights?.evidence_refs?.length ?? 0)
    + (transparency?.evidence_refs?.length ?? 0)
    + (transparency?.source_refs?.length ?? 0)
    + (transparency?.sbom_refs?.length ?? 0)
    + (transparency?.provenance_refs?.length ?? 0)
    + (transparency?.signature_refs?.length ?? 0);

  return (
    <section className="rounded-[16px] border border-whisper-border bg-pure-surface p-5">
      <h2 className="font-display text-lg font-bold">Rights, transparency, and enforcement</h2>
      <p className="mt-1 text-xs text-steel-secondary">
        Publisher/source declarations, referenced evidence, and Host-enforced gates are shown separately.
      </p>
      <div className="mt-4 grid gap-4 lg:grid-cols-3">
        <DisclosureColumn title="Declaration">
          <p>Source visibility: <Value>{transparency?.source_visibility ?? "unknown"}</Value></p>
          <p>State portability: <Value>{transparency?.state_portability ?? "unknown"}</Value></p>
          <p>License: <Value>{rights?.license_expression ?? "not declared"}</Value></p>
          <div className="mt-3 grid grid-cols-2 gap-x-3 gap-y-1">
            {RIGHT_LABELS.map(([label, key]) => (
              <p key={key} className="contents"><span>{label}</span><Value>{rights?.[key] ?? "unspecified"}</Value></p>
            ))}
          </div>
        </DisclosureColumn>
        <DisclosureColumn title="Evidence">
          <p>Reproducible build claim: <Value>{transparency?.reproducible_build_claim ?? "unknown"}</Value></p>
          <p>Referenced evidence objects: <Value>{String(evidenceCount)}</Value></p>
          <p>Telemetry disclosures: <Value>{String(transparency?.telemetry_disclosures?.length ?? 0)}</Value></p>
          {transparency?.telemetry_disclosures?.length ? (
            <ul className="mt-2 list-disc space-y-1 pl-4">
              {transparency.telemetry_disclosures.map((item) => <li key={item}>{item}</li>)}
            </ul>
          ) : null}
        </DisclosureColumn>
        <DisclosureColumn title="Host enforcement">
          <p>Installation source: <Value>{sourceKind}</Value></p>
          <p>Execute, backup, cross-Host copy, and dedicated-server effects are rechecked against the exact declaration before mutation.</p>
          <p className="mt-2">Rights are not capability authority. Component trust and enforced boundaries remain separate runtime evidence.</p>
        </DisclosureColumn>
      </div>
    </section>
  );
}

function DisclosureColumn({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="rounded-[12px] border border-whisper-border bg-warm-bone/40 p-4 text-xs text-steel-secondary">
      <h3 className="mb-3 font-mono text-[11px] uppercase tracking-[0.12em] text-charcoal-ink">{title}</h3>
      <div className="space-y-1.5">{children}</div>
    </div>
  );
}

function Value({ children }: { children: ReactNode }) {
  return <span className="font-mono text-charcoal-ink">{children}</span>;
}
