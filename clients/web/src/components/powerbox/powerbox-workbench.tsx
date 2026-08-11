import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { useT } from "@/lib/locale";
import { activeHostCredentialScope } from "@/client-core/host-endpoint";
import type { HostAccessIdentity, HostAccessResourceKind } from "@/client-core/host-access";
import {
  createPowerboxContext,
  installationPortResourceId,
  powerboxWorkbenchShouldReloadForLifecycleEvent,
  type PowerboxConsumerContext,
} from "@/client-core/powerbox";
import { PowerboxChooser } from "./powerbox-chooser";
import {
  ProtocolRpcError,
  HOST_POWERBOX_RELAY_SESSIONS,
  type BindingView,
  type ExposureView,
  type InstallationView,
  type PluroraProtocolClient,
  type RunView,
} from "@/protocol/client";

interface WorkbenchFailure {
  reasonCode: string;
  nextStep: string;
  installationId?: string;
  runId?: string;
  portId?: string;
}

export function PowerboxWorkbench({
  client,
  identity,
  installation,
  runs,
  onChanged,
}: {
  client: PluroraProtocolClient;
  identity: HostAccessIdentity | null;
  installation: InstallationView;
  runs: RunView[];
  onChanged: () => void | Promise<void>;
}) {
  const t = useT();
  const [bindings, setBindings] = useState<BindingView[]>([]);
  const [exposures, setExposures] = useState<ExposureView[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<WorkbenchFailure | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [exportPort, setExportPort] = useState("");
  const [audienceKind, setAudienceKind] = useState("installation");
  const [audienceId, setAudienceId] = useState("");
  const [expiresAt, setExpiresAt] = useState("");
  const [exposureConfirmed, setExposureConfirmed] = useState(false);
  const [runtimeImportPort, setRuntimeImportPort] = useState("");
  const [runtimeChooser, setRuntimeChooser] = useState<PowerboxConsumerContext | null>(null);
  const mutationKeys = useRef(new Map<string, string>());
  const installationId = installation.record.installation_id;
  const activeRun = useMemo(
    () => runs.find((run) => ["starting", "running", "degraded"].includes(run.record.status)) ?? null,
    [runs],
  );

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const [nextBindings, nextExposures] = await Promise.all([
        client.listBindings({ consumer_installation_id: installationId }),
        client.listExposures({ installation_id: installationId }),
      ]);
      setBindings(nextBindings);
      setExposures(nextExposures);
      setError(null);
    } catch (cause) {
      setError(structuredWorkbenchFailure(cause, { installationId }));
    } finally {
      setLoading(false);
    }
  }, [client, installationId]);

  useEffect(() => { void load(); }, [load]);

  useEffect(() => {
    const abortController = new AbortController();
    const close = client.subscribeHostEvents(
      HOST_POWERBOX_RELAY_SESSIONS,
      (event) => {
        if (!abortController.signal.aborted && powerboxWorkbenchShouldReloadForLifecycleEvent(event, installationId)) void load();
      },
      { signal: abortController.signal },
    );
    return () => {
      abortController.abort();
      close();
    };
  }, [client, load]);

  const revokeBinding = async (binding: BindingView) => {
    const record = binding.record;
    const canRevoke = canManageBinding(identity, installationId, record.consumer.port.root_port, record.exposure_id, record.consumer.run?.run_id);
    if (!canRevoke) {
      setError({ reasonCode: "authority_denied", nextStep: t("powerboxAuthorityDenied"), installationId, runId: record.consumer.run?.run_id, portId: record.consumer.port.root_port });
      return;
    }
    const keyName = `binding:${record.binding_id}:${binding.revision}`;
    const idempotencyKey = mutationKeys.current.get(keyName) ?? crypto.randomUUID();
    mutationKeys.current.set(keyName, idempotencyKey);
    setBusy(keyName);
    try {
      await client.revokeBinding({
        binding_id: record.binding_id,
        consumer_installation_id: installationId,
        expected_binding_revision: binding.revision,
        expected_consumer_installation_revision: installation.revision,
        exposure_id: record.exposure_id,
        idempotency_key: idempotencyKey,
        import_port: record.consumer.port.root_port,
        ...(record.phase === "runtime" && record.consumer.run ? { consumer_run: record.consumer.run } : {}),
      });
      mutationKeys.current.delete(keyName);
      await load();
      await onChanged();
    } catch (cause) {
      setError(structuredWorkbenchFailure(cause, { installationId, runId: record.consumer.run?.run_id, portId: record.consumer.port.root_port }));
    } finally {
      setBusy(null);
    }
  };

  const createExposure = async () => {
    const port = exportPort.trim();
    const audienceResourceKind = audienceKind.trim();
    const audienceResourceId = audienceId.trim();
    if (!activeRun || !port || !audienceResourceKind || !audienceResourceId || !exposureConfirmed) return;
    if (!canManageExposure(identity, installationId, activeRun.record.run_id, port)) {
      setError({ reasonCode: "authority_denied", nextStep: t("powerboxAuthorityDenied"), installationId, runId: activeRun.record.run_id, portId: port });
      return;
    }
    const keyName = `exposure:create:${installationId}:${activeRun.record.run_id}:${port}:${audienceResourceKind}:${audienceResourceId}:${expiresAt}`;
    const idempotencyKey = mutationKeys.current.get(keyName) ?? crypto.randomUUID();
    mutationKeys.current.set(keyName, idempotencyKey);
    setBusy(keyName);
    try {
      await client.createExposure({
        audience: [{ kind: audienceResourceKind, id: audienceResourceId }],
        expected_installation_revision: installation.revision,
        expected_run_revision: activeRun.revision,
        ...(expiresAt ? { expires_at: new Date(expiresAt).toISOString() } : {}),
        export_port: port,
        idempotency_key: idempotencyKey,
        installation_id: installationId,
        run_id: activeRun.record.run_id,
      });
      mutationKeys.current.delete(keyName);
      setExportPort("");
      setAudienceId("");
      setExpiresAt("");
      setExposureConfirmed(false);
      await load();
    } catch (cause) {
      setError(structuredWorkbenchFailure(cause, { installationId, runId: activeRun.record.run_id, portId: port }));
    } finally {
      setBusy(null);
    }
  };

  const revokeExposure = async (exposure: ExposureView) => {
    const record = exposure.record;
    if (!record.run_id || !canManageExposure(identity, installationId, record.run_id, record.export_port)) {
      setError({ reasonCode: "authority_denied", nextStep: t("powerboxAuthorityDenied"), installationId, runId: record.run_id ?? undefined, portId: record.export_port });
      return;
    }
    const providerRun = runs.find((run) => run.record.run_id === record.run_id);
    if (!providerRun) {
      setError({ reasonCode: "plan_stale", nextStep: "Refresh the provider Run revision before revoking this Exposure.", installationId, runId: record.run_id, portId: record.export_port });
      return;
    }
    const keyName = `exposure:revoke:${record.exposure_id}:${exposure.revision}`;
    const idempotencyKey = mutationKeys.current.get(keyName) ?? crypto.randomUUID();
    mutationKeys.current.set(keyName, idempotencyKey);
    setBusy(keyName);
    try {
      await client.revokeExposure({
        expected_exposure_revision: exposure.revision,
        expected_installation_revision: installation.revision,
        expected_run_revision: providerRun.revision,
        export_port: record.export_port,
        exposure_id: record.exposure_id,
        idempotency_key: idempotencyKey,
        installation_id: installationId,
        run_id: record.run_id,
      });
      mutationKeys.current.delete(keyName);
      await load();
      await onChanged();
    } catch (cause) {
      setError(structuredWorkbenchFailure(cause, { installationId, runId: record.run_id, portId: record.export_port }));
    } finally {
      setBusy(null);
    }
  };

  const openRuntimeChooser = () => {
    const port = runtimeImportPort.trim();
    if (!activeRun?.record.context_id || !port) {
      setError({ reasonCode: "run_context_unavailable", nextStep: t("powerboxRuntimeContextNext"), installationId, runId: activeRun?.record.run_id, portId: port || undefined });
      return;
    }
    setRuntimeChooser(createPowerboxContext({
      hostScope: activeHostCredentialScope(),
      installationId,
      installationRevision: installation.revision,
      importPort: port,
      phase: "runtime",
      run: {
        run_id: activeRun.record.run_id,
        run_revision: activeRun.revision,
        context_id: activeRun.record.context_id,
      },
    }));
  };

  const activeBindings = bindings.filter((binding) => binding.record.status === "selected" && binding.effective_status.kind === "active");
  const canCreateExposure = Boolean(activeRun && exportPort.trim() && canManageExposure(identity, installationId, activeRun.record.run_id, exportPort.trim()));

  return (
    <div className="space-y-4">
      {error ? (
        <div className="rounded-[12px] border border-deep-rust/30 bg-deep-rust-surface px-4 py-3 text-xs text-deep-rust">
          <p><span className="font-mono">{error.reasonCode}</span> · {error.nextStep}</p>
          <p className="mt-2 break-all font-mono text-[11px]">installation_id={error.installationId ?? installationId} · run_id={error.runId ?? "—"} · port_id={error.portId ?? "—"}</p>
        </div>
      ) : null}
      <section className="rounded-[16px] border border-whisper-border bg-pure-surface p-5">
        <div className="flex items-center justify-between gap-3">
          <div><h2 className="font-display text-lg font-bold">{t("powerboxBindingsTitle")}</h2><p className="mt-1 text-xs text-steel-secondary">{t("powerboxDescription")}</p></div>
          <Button tone="tertiary" size="sm" disabled={loading} onClick={() => void load()}>{t("powerboxRetry")}</Button>
        </div>
        <div className="mt-4 space-y-2">
          {activeBindings.length === 0 ? <p className="text-sm text-muted-tone">{t("powerboxBindingsEmpty")}</p> : activeBindings.map((binding) => {
            const record = binding.record;
            const keyName = `binding:${record.binding_id}:${binding.revision}`;
            const canRevoke = canManageBinding(identity, installationId, record.consumer.port.root_port, record.exposure_id, record.consumer.run?.run_id);
            return (
              <div key={record.binding_id} className="rounded-[12px] border border-whisper-border p-3 text-xs">
                <div className="flex flex-col justify-between gap-3 sm:flex-row sm:items-start">
                  <dl className="grid min-w-0 flex-1 gap-2 sm:grid-cols-2">
                    <BindingDatum label="Binding" value={record.binding_id} />
                    <BindingDatum label={t("powerboxPhase")} value={record.phase} />
                    <BindingDatum label={t("powerboxImportPort")} value={record.consumer.port.root_port} />
                    <BindingDatum label={t("powerboxProviderPort")} value={`${record.provider.installation.installation_id}:${record.provider.port.root_port}`} />
                    <BindingDatum label={t("powerboxTransport")} value={record.transport.class_id} />
                    <BindingDatum label={t("powerboxExpiry")} value={record.effective_expires_at ? new Date(record.effective_expires_at).toLocaleString() : "—"} />
                  </dl>
                  <Button tone="destructive" size="sm" disabled={!canRevoke || busy !== null} onClick={() => void revokeBinding(binding)}>
                    {busy === keyName ? t("powerboxRevoking") : t("powerboxRevoke")}
                  </Button>
                </div>
                {!canRevoke ? <p className="mt-2 font-mono text-[11px] text-deep-rust">authority_denied</p> : null}
              </div>
            );
          })}
        </div>
        {activeRun ? (
          <div className="mt-4 border-t border-whisper-border pt-4">
            <label className="text-xs font-semibold" htmlFor="runtime-import-port">{t("powerboxImportPort")} · runtime</label>
            <div className="mt-2 flex flex-col gap-2 sm:flex-row">
              <input id="runtime-import-port" value={runtimeImportPort} onChange={(event) => setRuntimeImportPort(event.target.value)} className="h-11 min-w-0 flex-1 rounded-[10px] border border-whisper-border bg-warm-bone px-3 font-mono text-sm" />
              <Button tone="secondary" onClick={openRuntimeChooser} disabled={!runtimeImportPort.trim() || !activeRun.record.context_id}>{t("powerboxTitle")}</Button>
            </div>
          </div>
        ) : null}
      </section>

      {runtimeChooser ? <PowerboxChooser client={client} identity={identity} context={runtimeChooser} onClose={() => setRuntimeChooser(null)} onSelected={async () => { setRuntimeChooser(null); await load(); await onChanged(); }} /> : null}

      <section className="rounded-[16px] border border-whisper-border bg-pure-surface p-5">
        <h2 className="font-display text-lg font-bold">{t("powerboxExposuresTitle")}</h2>
        <p className="mt-1 text-xs text-steel-secondary">{t("powerboxExposuresDescription")}</p>
        <p className="mt-3 rounded-[10px] bg-warm-bone p-3 text-xs leading-5 text-steel-secondary">{t("powerboxTypedPortNotice")}</p>
        {activeRun ? (
          <div className="mt-4 grid gap-3 sm:grid-cols-2">
            <label className="text-xs"><span className="mb-1 block font-semibold">{t("powerboxExportPort")}</span><input value={exportPort} onChange={(event) => setExportPort(event.target.value)} className="h-11 w-full rounded-[10px] border border-whisper-border bg-warm-bone px-3 font-mono text-sm" /></label>
            <label className="text-xs"><span className="mb-1 block font-semibold">{t("powerboxAudienceKind")}</span><input value={audienceKind} onChange={(event) => setAudienceKind(event.target.value)} className="h-11 w-full rounded-[10px] border border-whisper-border bg-warm-bone px-3 font-mono text-sm" /></label>
            <label className="text-xs"><span className="mb-1 block font-semibold">{t("powerboxAudienceId")}</span><input value={audienceId} onChange={(event) => setAudienceId(event.target.value)} className="h-11 w-full rounded-[10px] border border-whisper-border bg-warm-bone px-3 font-mono text-sm" /></label>
            <label className="text-xs"><span className="mb-1 block font-semibold">{t("powerboxExpiresAt")}</span><input type="datetime-local" value={expiresAt} onChange={(event) => setExpiresAt(event.target.value)} className="h-11 w-full rounded-[10px] border border-whisper-border bg-warm-bone px-3 text-sm" /></label>
            <label className="flex min-h-11 items-start gap-3 text-xs leading-5 sm:col-span-2"><input className="mt-1 size-4" type="checkbox" checked={exposureConfirmed} onChange={(event) => setExposureConfirmed(event.target.checked)} /><span>{t("powerboxExposureConfirm")}</span></label>
            {exportPort.trim() && !canCreateExposure ? <p className="font-mono text-[11px] text-deep-rust sm:col-span-2">authority_denied · {t("powerboxAuthorityDenied")}</p> : null}
            <Button className="sm:col-span-2 sm:w-fit" tone="primary" disabled={!exportPort.trim() || !audienceKind.trim() || !audienceId.trim() || !exposureConfirmed || busy !== null || !canCreateExposure} onClick={() => void createExposure()}>{busy?.startsWith("exposure:create") ? t("powerboxCreatingExposure") : t("powerboxCreateExposure")}</Button>
          </div>
        ) : <p className="mt-3 text-sm text-muted-tone">{t("powerboxStartRunFirst")}</p>}
        <div className="mt-5 space-y-2">
          {exposures.length === 0 ? <p className="text-sm text-muted-tone">{t("powerboxExposuresEmpty")}</p> : exposures.map((exposure) => {
            const record = exposure.record;
            const keyName = `exposure:revoke:${record.exposure_id}:${exposure.revision}`;
            const canRevoke = Boolean(record.run_id && canManageExposure(identity, installationId, record.run_id, record.export_port));
            return (
              <div key={record.exposure_id} className="flex flex-col justify-between gap-3 rounded-[12px] border border-whisper-border p-3 text-xs sm:flex-row sm:items-start">
                <dl className="grid min-w-0 flex-1 gap-2 sm:grid-cols-2">
                  <BindingDatum label="Exposure" value={record.exposure_id} />
                  <BindingDatum label={t("powerboxExportPort")} value={record.export_port} />
                  <BindingDatum label={t("powerboxAudience")} value={record.audience.map((item) => `${item.kind}:${item.id}`).join(", ")} />
                  <BindingDatum label={t("powerboxExpiry")} value={record.expires_at ? new Date(record.expires_at).toLocaleString() : "—"} />
                </dl>
                {record.status === "active" ? <Button tone="destructive" size="sm" disabled={!canRevoke || busy !== null} onClick={() => void revokeExposure(exposure)}>{busy === keyName ? t("powerboxRevoking") : t("powerboxRevoke")}</Button> : <span className="font-mono">{record.status}</span>}
              </div>
            );
          })}
        </div>
      </section>
    </div>
  );
}

function hasResource(identity: HostAccessIdentity, kind: HostAccessResourceKind, id: string): boolean {
  return Boolean(identity.resources?.some((resource) => resource.kind === kind && (resource.id === null || resource.id === id)));
}

function canManageBinding(identity: HostAccessIdentity | null, installationId: string, portId: string, exposureId: string, runId?: string): boolean {
  if (identity?.kind === "root") return true;
  if (!identity || !identity.scopes.includes("binding.manage")) return false;
  return hasResource(identity, "installation", installationId)
    && hasResource(identity, "port", installationPortResourceId(installationId, portId))
    && hasResource(identity, "exposure", exposureId)
    && (!runId || hasResource(identity, "run", runId));
}

function canManageExposure(identity: HostAccessIdentity | null, installationId: string, runId: string, portId: string): boolean {
  if (identity?.kind === "root") return true;
  if (!identity || !identity.scopes.includes("exposure.manage")) return false;
  return hasResource(identity, "installation", installationId)
    && hasResource(identity, "run", runId)
    && hasResource(identity, "port", installationPortResourceId(installationId, portId));
}

function BindingDatum({ label, value }: { label: string; value: string }) {
  return <div className="min-w-0"><dt className="text-[10px] uppercase tracking-[0.08em] text-muted-tone">{label}</dt><dd className="mt-1 break-all font-mono text-[11px]">{value}</dd></div>;
}

function structuredWorkbenchFailure(
  cause: unknown,
  fallback: Pick<WorkbenchFailure, "installationId" | "runId" | "portId">,
): WorkbenchFailure {
  if (cause instanceof ProtocolRpcError) {
    return {
      reasonCode: cause.reasonCode,
      nextStep: cause.nextStep ?? "Refresh Host state before retrying.",
      installationId: cause.details.installation_id ?? fallback.installationId,
      runId: cause.details.run_id ?? fallback.runId,
      portId: cause.details.port_id ?? fallback.portId,
    };
  }
  return { reasonCode: "outcome_unknown", nextStep: "Refresh Host state before retrying; do not assume the mutation failed or succeeded.", ...fallback };
}
