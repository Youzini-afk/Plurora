import { useEffect, useState } from "react";
import { ArrowLeft, ArrowsClockwise } from "@/components/icons";
import { Button } from "@/components/ui/button";
import { StatusPill, installationStateTone } from "@/components/ui/status-pill";
import { usePlurora } from "@/lib/plurora-client";
import { useRoute } from "@/lib/router";
import type { InstallationView } from "@/protocol/client";

export const RUN_UNAVAILABLE_REASON = {
  code: "run_unavailable_phase4",
  operation: "run.start",
  phase: 4,
} as const;

export function InstallationFrame({ installationId, chrome = "shell" }: { installationId: string; chrome?: "none" | "shell" }) {
  const client = usePlurora();
  const [, navigate] = useRoute();
  const [view, setView] = useState<InstallationView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const load = async () => {
    setLoading(true);
    try {
      setView(await client.getInstallation(installationId));
      setError(null);
    } catch {
      setError("Installation details are unavailable.");
    } finally {
      setLoading(false);
    }
  };
  useEffect(() => { void load(); }, [installationId]);

  const content = (
    <div className="mx-auto flex w-full max-w-[1000px] flex-1 flex-col gap-6 px-5 py-8 sm:px-8 lg:py-12">
      <div className="flex items-center justify-between gap-3">
        {chrome === "shell" ? <Button tone="tertiary" size="sm" onClick={() => navigate({ kind: "home" })}><ArrowLeft size={14} /> Home</Button> : <span />}
        <Button tone="tertiary" size="sm" onClick={() => void load()} disabled={loading}><ArrowsClockwise size={14} /> Refresh</Button>
      </div>
      {loading ? <p className="text-sm text-steel-secondary">Loading installation…</p> : error ? <p className="rounded-[12px] border border-deep-rust/30 bg-deep-rust-surface px-4 py-3 text-sm text-deep-rust">{error}</p> : view ? (
        <>
          <header className="flex flex-col gap-3 rounded-[20px] border border-whisper-border bg-pure-surface p-6 sm:flex-row sm:items-start sm:justify-between">
            <div><p className="font-mono text-[11px] text-muted-tone">{view.record.installation_id}</p><h1 className="mt-2 font-display text-3xl font-bold">{view.record.display_name}</h1></div>
            <StatusPill tone={installationStateTone(view.record.status)} label={view.record.status.replaceAll("_", " ").toUpperCase()} />
          </header>
          <section className="grid gap-4 sm:grid-cols-2">
            <Info label="Status" value={view.record.status} />
            <Info label="Revision" value={String(view.revision)} />
            <Info label="Source" value={view.record.source.kind} />
            <Info label="Updated" value={new Date(view.record.updated_at).toLocaleString()} />
          </section>
          <section className="rounded-[16px] border border-aged-brass-border bg-aged-brass-surface-soft p-5">
            <h2 className="font-display text-lg font-bold">Run unavailable</h2>
            <p className="mt-2 text-sm text-steel-secondary">Run lifecycle is not exposed in Phase 3. The installation remains available for inspection.</p>
            <pre className="mt-3 overflow-auto rounded-[10px] bg-pure-surface p-3 font-mono text-[11px] text-charcoal-ink">{JSON.stringify(RUN_UNAVAILABLE_REASON, null, 2)}</pre>
          </section>
        </>
      ) : null}
    </div>
  );
  return chrome === "none" ? <div className="flex min-h-[100dvh] flex-col bg-warm-bone text-charcoal-ink">{content}</div> : content;
}

function Info({ label, value }: { label: string; value: string }) {
  return <div className="rounded-[14px] border border-whisper-border bg-pure-surface p-4"><p className="text-[11px] uppercase tracking-[0.12em] text-muted-tone">{label}</p><p className="mt-2 font-mono text-sm text-charcoal-ink">{value}</p></div>;
}
