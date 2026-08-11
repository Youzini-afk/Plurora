import { useEffect, useMemo, useRef, useState } from "react";
import { ArrowLeft, ArrowsClockwise } from "@/components/icons";
import { Button } from "@/components/ui/button";
import { StatusPill, installationStateTone, runStateTone } from "@/components/ui/status-pill";
import { usePlurora } from "@/lib/plurora-client";
import { useRoute } from "@/lib/router";
import { ProtocolRpcError, type InstallationView, type RunView, type WorkEntrypoint } from "@/protocol/client";
import { libraryAuthorityForInstallation } from "@/client-core/library-affordance";
import { useAuth } from "@/lib/auth-gate";
import { PowerboxChooser } from "@/components/powerbox/powerbox-chooser";
import { PowerboxWorkbench } from "@/components/powerbox/powerbox-workbench";
import { activeHostCredentialScope } from "@/client-core/host-endpoint";
import { createPowerboxContext, isBindingGapForPowerbox, type PowerboxConsumerContext } from "@/client-core/powerbox";
import { RealizationWorkbench } from "@/components/realization/realization-workbench";

/** Navigation and tab close are observational. They never stop a Run. */
export const INSTALLATION_FRAME_POLICY = { stopRunOnUnmount: false } as const;

interface FailureDetail {
  reasonCode: string;
  nextStep: string;
  installationId?: string;
  runId?: string;
  portId?: string;
}

interface LaunchChooserState {
  entrypoint: WorkEntrypoint;
  context: PowerboxConsumerContext;
}

export function InstallationFrame({ installationId, chrome = "shell" }: { installationId: string; chrome?: "none" | "shell" }) {
  const client = usePlurora();
  const { identity } = useAuth();
  const [, navigate] = useRoute();
  const [view, setView] = useState<InstallationView | null>(null);
  const [runs, setRuns] = useState<RunView[]>([]);
  const [error, setError] = useState<FailureDetail | null>(null);
  const [launchChooser, setLaunchChooser] = useState<LaunchChooserState | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<"play" | "stop" | "restart" | null>(null);
  const mutationKeys = useRef(new Map<string, string>());

  const load = async () => {
    setLoading(true);
    try {
      const [nextView, nextRuns] = await Promise.all([
        client.getInstallation(installationId),
        client.listRuns({ installation_id: installationId }),
      ]);
      setView(nextView);
      setRuns(nextRuns);
      setError(null);
    } catch (cause) {
      setError(structuredError(cause, installationId));
    } finally {
      setLoading(false);
    }
  };
  useEffect(() => { void load(); }, [installationId]);

  const activeRun = useMemo(
    () => runs.find((run) => ["starting", "running", "degraded", "stopping"].includes(run.record.status)) ?? null,
    [runs],
  );
  const restartableRun = useMemo(
    () => runs.find((run) => run.record.status === "interrupted" || run.record.status === "failed") ?? null,
    [runs],
  );
  const runAuthority = useMemo(
    () => libraryAuthorityForInstallation(identity, installationId),
    [identity, installationId],
  );

  const play = async (entrypoint: WorkEntrypoint, restart = false) => {
    if (!view) return;
    if (!runAuthority.can_run) {
      setError({
        reasonCode: runAuthority.reason_code ?? "authority_denied",
        nextStep: runAuthority.next_step ?? "Request Run authority for this Installation.",
        installationId,
      });
      return;
    }
    setBusy(restart ? "restart" : "play");
    try {
      const status = await client.statusRun({ installation_id: installationId, entrypoint_id: entrypoint.id });
      const statusGaps = status.preflight?.gaps ?? [];
      const gap = statusGaps.find(isBindingGapForPowerbox) ?? statusGaps[0];
      if (gap) {
        if (isBindingGapForPowerbox(gap) && gap.port_id) {
          setLaunchChooser({
            entrypoint,
            context: createPowerboxContext({
              hostScope: activeHostCredentialScope(),
              installationId,
              installationRevision: status.installation_revision,
              importPort: gap.port_id,
              phase: "launch",
            }),
          });
          setError(null);
        } else {
          setError({ reasonCode: gap.reason_code, nextStep: gap.next_step, installationId, portId: gap.port_id ?? undefined });
        }
        return;
      }
      if (status.active_run && ["starting", "running", "degraded", "stopping"].includes(status.active_run.record.status)) {
        mutationKeys.current.delete(`start:${installationId}:${entrypoint.id}`);
        await load();
        return;
      }
      const keyName = `start:${installationId}:${entrypoint.id}`;
      const idempotencyKey = mutationKeys.current.get(keyName) ?? crypto.randomUUID();
      mutationKeys.current.set(keyName, idempotencyKey);
      const result = await client.startRun({
        installation_id: installationId,
        entrypoint_id: entrypoint.id,
        expected_installation_revision: status.installation_revision,
        idempotency_key: idempotencyKey,
      });
      const startGaps = result.gaps ?? [];
      const startGap = startGaps.find(isBindingGapForPowerbox) ?? startGaps[0];
      mutationKeys.current.delete(keyName);
      if (startGap) {
        if (isBindingGapForPowerbox(startGap) && startGap.port_id) {
          setLaunchChooser({
            entrypoint,
            context: createPowerboxContext({
              hostScope: activeHostCredentialScope(),
              installationId,
              installationRevision: status.installation_revision,
              importPort: startGap.port_id,
              phase: "launch",
            }),
          });
          setError(null);
        } else {
          setError({ reasonCode: startGap.reason_code, nextStep: startGap.next_step, installationId, portId: startGap.port_id ?? undefined });
        }
        return;
      }
      await load();
    } catch (cause) {
      setError(structuredError(cause, installationId));
    } finally {
      setBusy(null);
    }
  };

  const stop = async () => {
    if (!activeRun) return;
    if (!runAuthority.can_stop) {
      setError({
        reasonCode: runAuthority.reason_code ?? "authority_denied",
        nextStep: runAuthority.next_step ?? "Request Run authority for this Installation.",
        installationId,
        runId: activeRun.record.run_id,
      });
      return;
    }
    setBusy("stop");
    try {
      const keyName = `stop:${installationId}:${activeRun.record.run_id}:${activeRun.revision}`;
      const idempotencyKey = mutationKeys.current.get(keyName) ?? crypto.randomUUID();
      mutationKeys.current.set(keyName, idempotencyKey);
      await client.stopRun({
        installation_id: installationId,
        run_id: activeRun.record.run_id,
        expected_revision: activeRun.revision,
        idempotency_key: idempotencyKey,
      });
      mutationKeys.current.delete(keyName);
      await load();
    } catch (cause) {
      setError(structuredError(cause, installationId));
    } finally {
      setBusy(null);
    }
  };

  const content = (
    <div className="mx-auto flex w-full max-w-[1000px] flex-1 flex-col gap-6 px-5 py-8 sm:px-8 lg:py-12">
      <div className="flex items-center justify-between gap-3">
        {chrome === "shell" ? <Button tone="tertiary" size="sm" onClick={() => navigate({ kind: "home" })}><ArrowLeft size={14} /> Home</Button> : <span />}
        <Button tone="tertiary" size="sm" onClick={() => void load()} disabled={loading}><ArrowsClockwise size={14} /> Refresh</Button>
      </div>
      {loading ? <p className="text-sm text-steel-secondary">Loading installation…</p> : error && !view ? <ErrorNotice error={error} /> : view ? (
        <>
          <header className="flex flex-col gap-3 rounded-[20px] border border-whisper-border bg-pure-surface p-6 sm:flex-row sm:items-start sm:justify-between">
            <div>
              <p className="font-mono text-[11px] text-muted-tone">{view.record.installation_id}</p>
              <h1 className="mt-2 font-display text-3xl font-bold">{view.work_summary.title}</h1>
              <p className="mt-1 text-sm text-steel-secondary">{view.work_summary.description || view.record.display_name}</p>
            </div>
            <StatusPill tone={installationStateTone(view.record.status)} label={view.record.status.replaceAll("_", " ").toUpperCase()} />
          </header>
          {error ? <ErrorNotice error={error} /> : null}
          {launchChooser ? (
            <PowerboxChooser
              client={client}
              identity={identity}
              context={launchChooser.context}
              onClose={() => setLaunchChooser(null)}
              onSelected={async () => {
                const entrypoint = launchChooser.entrypoint;
                setLaunchChooser(null);
                await play(entrypoint);
              }}
            />
          ) : null}
          {view.work_summary.operational_intent ? <RealizationWorkbench client={client} identity={identity} installation={view} /> : null}
          <section className="rounded-[16px] border border-whisper-border bg-pure-surface p-5">
            <div className="flex items-center justify-between gap-3">
              <div><h2 className="font-display text-lg font-bold">Entrypoints</h2><p className="mt-1 text-xs text-steel-secondary">Preflight runs only when you choose an entrypoint.</p></div>
              {activeRun ? <Button tone="tertiary" size="sm" onClick={() => void stop()} disabled={busy !== null || !runAuthority.can_stop}>{busy === "stop" ? "Stopping…" : "Stop"}</Button> : null}
            </div>
            <div className="mt-4 flex flex-wrap gap-2">
              {view.work_summary.entrypoints.map((entrypoint) => (
                <Button key={entrypoint.id} tone="primary" size="sm" onClick={() => void play(entrypoint)} disabled={busy !== null || !runAuthority.can_run}>
                  {busy === "play" ? "Checking…" : entrypoint.id}
                </Button>
              ))}
            </div>
            {!runAuthority.can_run ? <p className="mt-3 text-xs text-deep-rust"><span className="font-mono">authority_denied</span> · {runAuthority.next_step}</p> : null}
            {view.work_summary.entrypoints.length === 0 ? <p className="mt-3 text-sm text-muted-tone">This Work has no executable entrypoint.</p> : null}
          </section>
          <PowerboxWorkbench client={client} identity={identity} installation={view} runs={runs} onChanged={load} />
          <section className="grid gap-4 sm:grid-cols-2">
            <Info label="Installation" value={view.record.installation_id} />
            <Info label="Work" value={view.work_summary.work_id} />
            <Info label="Installation revision" value={String(view.revision)} />
            <Info label="Source" value={view.record.source.kind} />
          </section>
          <section className="rounded-[16px] border border-whisper-border bg-pure-surface p-5">
            <h2 className="font-display text-lg font-bold">Run history</h2>
            <div className="mt-3 space-y-2">
              {runs.length === 0 ? <p className="text-sm text-muted-tone">No Runs recorded.</p> : runs.map((run) => (
                <div key={run.record.run_id} className="flex items-center justify-between gap-3 rounded-[10px] border border-whisper-border px-3 py-2 text-sm">
                  <div><p className="font-mono text-xs">{run.record.run_id}</p><p className="text-xs text-steel-secondary">{run.entrypoint_id}</p></div>
                  <div className="flex items-center gap-2"><StatusPill tone={runStateTone(run.record.status)} label={run.record.status} />{(run.record.status === "failed" || run.record.status === "interrupted") && view.work_summary.entrypoints.length > 0 ? <Button tone="tertiary" size="sm" onClick={() => { const entrypoint = view.work_summary.entrypoints.find((candidate) => candidate.id === run.entrypoint_id) ?? view.work_summary.entrypoints[0]; if (entrypoint) void play(entrypoint, true); }} disabled={busy !== null}>{busy === "restart" ? "Restarting…" : "Restart"}</Button> : null}</div>
                </div>
              ))}
            </div>
            {restartableRun && !view.work_summary.entrypoints.some((entrypoint) => entrypoint.id === restartableRun.entrypoint_id) ? <p className="mt-3 text-xs text-steel-secondary">The failed Run entrypoint is no longer exposed by this Work revision.</p> : null}
          </section>
        </>
      ) : null}
    </div>
  );
  return chrome === "none" ? <div className="flex min-h-[100dvh] flex-col bg-warm-bone text-charcoal-ink">{content}</div> : content;
}

function ErrorNotice({ error }: { error: FailureDetail }) {
  return (
    <div className="rounded-[12px] border border-deep-rust/30 bg-deep-rust-surface px-4 py-3 text-sm text-deep-rust">
      <p><span className="font-mono">{error.reasonCode}</span><span className="mx-2">·</span>{error.nextStep}</p>
      <p className="mt-2 break-all font-mono text-[11px]">
        installation_id={error.installationId ?? "—"} · run_id={error.runId ?? "—"} · port_id={error.portId ?? "—"}
      </p>
    </div>
  );
}

function structuredError(cause: unknown, fallbackInstallationId?: string): FailureDetail {
  if (cause instanceof ProtocolRpcError) {
    return {
      reasonCode: cause.reasonCode,
      nextStep: cause.nextStep ?? "Inspect the structured Host diagnostic before retrying.",
      installationId: cause.details.installation_id ?? fallbackInstallationId,
      runId: cause.details.run_id,
      portId: cause.details.port_id,
    };
  }
  return { reasonCode: "outcome_unknown", nextStep: "Refresh the Installation and inspect Run history before retrying.", installationId: fallbackInstallationId };
}

function Info({ label, value }: { label: string; value: string }) {
  return <div className="rounded-[14px] border border-whisper-border bg-pure-surface p-4"><p className="text-[11px] uppercase tracking-[0.12em] text-muted-tone">{label}</p><p className="mt-2 break-all font-mono text-sm text-charcoal-ink">{value}</p></div>;
}
