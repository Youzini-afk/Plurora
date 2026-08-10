import { useMemo, useRef, useState } from "react";
import { Plus, ArrowsClockwise } from "@/components/icons";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { Skeleton } from "@/components/ui/skeleton";
import { useT } from "@/lib/locale";
import { openInstallationInTab } from "@/lib/installation-launcher";
import { InstallationCard } from "@/components/home/installation-card";
import { InstallModal } from "@/components/install/install-modal";
import { filterInstallations } from "./home/home-filtering";
import { useHomeInstallations } from "./home/use-home-installations";
import { usePlurora } from "@/lib/plurora-client";
import { ProtocolRpcError, type WorkEntrypoint } from "@/protocol/client";
import { affordanceForAction } from "@/client-core/library-affordance";
import { libraryAuthorityForInstallation } from "@/client-core/library-affordance";
import { useAuth } from "@/lib/auth-gate";

export function HomePage() {
  const t = useT();
  const client = usePlurora();
  const { identity } = useAuth();
  const [installOpen, setInstallOpen] = useState(false);
  const [search, setSearch] = useState("");
  const { installations, loading, error, refresh } = useHomeInstallations();
  const [busy, setBusy] = useState<{ installationId: string; action: "play" | "stop" } | null>(null);
  const [actionErrors, setActionErrors] = useState<Record<string, { reasonCode: string; nextStep: string }>>({});
  const mutationKeys = useRef(new Map<string, string>());
  const visible = useMemo(() => filterInstallations(installations, search), [installations, search]);

  const playEntrypoint = async (installationId: string, entrypoint: WorkEntrypoint) => {
    setBusy({ installationId, action: "play" });
    setActionErrors((current) => {
      const next = { ...current };
      delete next[installationId];
      return next;
    });
    try {
      const status = await client.statusRun({ installation_id: installationId, entrypoint_id: entrypoint.id });
      const item = installations.find((candidate) => candidate.installationId === installationId);
      const authority = libraryAuthorityForInstallation(identity, installationId);
      const affordance = item ? affordanceForAction({
        installation: item.view,
        work_summary: item.view.work_summary,
        entrypoint,
        authority,
        preflight_gaps: status.preflight?.gaps ?? [],
      }, "play") : null;
      if (affordance && !affordance.available) {
        setActionErrors((current) => ({ ...current, [installationId]: { reasonCode: affordance.reason_code ?? "preflight_failed", nextStep: affordance.next_step ?? "Inspect the Run preflight details." } }));
        return;
      }
      if (status.active_run && ["starting", "running", "degraded", "stopping"].includes(status.active_run.record.status)) {
        mutationKeys.current.delete(`start:${installationId}:${entrypoint.id}`);
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
      const startGap = result.gaps?.[0];
      mutationKeys.current.delete(keyName);
      if (startGap) setActionErrors((current) => ({ ...current, [installationId]: { reasonCode: startGap.reason_code, nextStep: startGap.next_step } }));
      await refresh();
    } catch (error) {
      setActionErrors((current) => ({ ...current, [installationId]: structuredActionError(error) }));
    } finally {
      setBusy(null);
    }
  };

  const stopRun = async (installationId: string) => {
    const installation = installations.find((item) => item.installationId === installationId);
    const activeRun = installation?.activeRun;
    if (!activeRun) return;
    const authority = libraryAuthorityForInstallation(identity, installationId);
    if (!authority.can_stop) {
      setActionErrors((current) => ({
        ...current,
        [installationId]: {
          reasonCode: authority.reason_code ?? "authority_denied",
          nextStep: authority.next_step ?? "Request Run authority for this Installation.",
        },
      }));
      return;
    }
    setBusy({ installationId, action: "stop" });
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
      await refresh();
    } catch (error) {
      setActionErrors((current) => ({ ...current, [installationId]: structuredActionError(error) }));
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="mx-auto flex w-full max-w-[1440px] flex-1 flex-col gap-8 px-4 py-8 sm:px-6 lg:px-10 lg:py-12">
      <header className="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
        <div>
          <p className="font-mono text-[11px] uppercase tracking-[0.18em] text-muted-tone">{t("homeInstalledEyebrow", installations.length)}</p>
          <h1 className="mt-2 font-display text-4xl font-bold text-charcoal-ink">{t("homeGreeting")}</h1>
          <p className="mt-2 max-w-xl text-sm text-steel-secondary">Library entries combine Work metadata with exact Installation and Run state.</p>
        </div>
        <Button tone="primary" onClick={() => setInstallOpen(true)}><Plus size={16} /> {t("homeInstallLabel")}</Button>
      </header>

      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <input
          value={search}
          onChange={(event) => setSearch(event.target.value)}
          placeholder={t("homeSearchPlaceholder")}
          className="h-10 w-full max-w-md rounded-[10px] border border-whisper-border bg-pure-surface px-3 text-sm outline-none focus-visible:border-aged-brass"
        />
        <Button tone="tertiary" size="sm" onClick={() => void refresh()} disabled={loading}><ArrowsClockwise size={14} /> {t("retry")}</Button>
      </div>

      {error ? <p className="rounded-[12px] border border-deep-rust/30 bg-deep-rust-surface px-4 py-3 text-sm text-deep-rust">{error}</p> : null}
      {loading ? (
        <div className="grid gap-5 md:grid-cols-2 xl:grid-cols-3"><Skeleton className="h-64 rounded-[20px]" /><Skeleton className="h-64 rounded-[20px]" /></div>
      ) : visible.length === 0 ? (
        <EmptyState title={t("homeEmptyTitle")} body={t("homeEmptyBody")} action={{ label: t("homeInstallLabel"), onClick: () => setInstallOpen(true) }} />
      ) : (
        <div className="grid gap-5 md:grid-cols-2 xl:grid-cols-3">
          {visible.map((installation, index) => {
            const authority = libraryAuthorityForInstallation(identity, installation.installationId);
            const authorityError = !authority.can_run && installation.entrypoints.length > 0 ? {
              reasonCode: authority.reason_code ?? "authority_denied",
              nextStep: authority.next_step ?? "Request Run authority for this Installation.",
            } : null;
            return (
              <InstallationCard
                key={installation.installationId}
                index={index}
                data={installation}
                onOpen={() => { openInstallationInTab(installation.installationId); }}
                onPlay={authority.can_run ? (entrypoint) => void playEntrypoint(installation.installationId, entrypoint) : undefined}
                onStop={authority.can_stop ? () => void stopRun(installation.installationId) : undefined}
                busyAction={busy?.installationId === installation.installationId ? busy.action : null}
                actionError={actionErrors[installation.installationId] ?? authorityError}
              />
            );
          })}
        </div>
      )}

      <InstallModal open={installOpen} onClose={() => setInstallOpen(false)} onInstalled={() => { setInstallOpen(false); void refresh(); }} />
    </div>
  );
}

function structuredActionError(error: unknown): { reasonCode: string; nextStep: string } {
  if (error instanceof ProtocolRpcError) {
    const details = error.details;
    if (details && typeof details === "object" && !Array.isArray(details)) {
      const record = details as Record<string, unknown>;
      const reasonCode = typeof record.reason_code === "string" ? record.reason_code : error.code;
      const nextStep = typeof record.next_step === "string" ? record.next_step : "Inspect the structured Host diagnostic before retrying.";
      return { reasonCode, nextStep };
    }
    return { reasonCode: error.code, nextStep: "Inspect the structured Host diagnostic before retrying." };
  }
  return { reasonCode: "outcome_unknown", nextStep: "Refresh the Library and inspect the Run status before retrying." };
}
