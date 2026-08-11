import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { StatusPill, realizationStateTone } from "@/components/ui/status-pill";
import type { HostAccessIdentity } from "@/client-core/host-access";
import {
  createOciRealizationPlanRequest,
  createRealizationApplyRequest,
  isRealizationActive,
  isRealizationEffectInProgress,
  realizationAuthorityForInstallation,
  realizationTargetId,
  validateOciRealizationDraft,
  type OciRealizationDraft,
} from "@/client-core/realization";
import {
  HOST_REALIZATIONS_SESSION,
  ProtocolRpcError,
  type ArtifactDescriptor,
  type ExecutionTarget,
  type InstallationView,
  type PlatformEvent,
  type PluroraProtocolClient,
  type RealizationPlan,
  type RealizationRevision,
} from "@/protocol/client";

interface PlannedSnapshot {
  realization: RealizationRevision;
  plan: RealizationPlan;
  planRef: ArtifactDescriptor;
  targetId: string;
}

interface RealizationFailure {
  reasonCode: string;
  nextStep: string;
  targetId?: string;
  realizationId?: string;
}

export function RealizationWorkbench({
  client,
  identity,
  installation,
}: {
  client: PluroraProtocolClient;
  identity: HostAccessIdentity | null;
  installation: InstallationView;
}) {
  const installationId = installation.record.installation_id;
  const [realizations, setRealizations] = useState<RealizationRevision[]>([]);
  const [targets, setTargets] = useState<ExecutionTarget[]>([]);
  const [targetId, setTargetId] = useState("");
  const [workloadId, setWorkloadId] = useState("");
  const [executionClass, setExecutionClass] = useState("oci-container.v1");
  const [image, setImage] = useState("");
  const [containerPort, setContainerPort] = useState("8080");
  const [portName, setPortName] = useState("http");
  const [routeId, setRouteId] = useState(`${installationId}-managed`);
  const [routeAccess, setRouteAccess] = useState<"host_authenticated" | "public">("host_authenticated");
  const [healthPath, setHealthPath] = useState("");
  const [pullIfMissing, setPullIfMissing] = useState(false);
  const [planned, setPlanned] = useState<PlannedSnapshot | null>(null);
  const [acceptedRisks, setAcceptedRisks] = useState(new Set<string>());
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<RealizationFailure | null>(null);
  const mutationKeys = useRef(new Map<string, string>());
  const authority = useMemo(
    () => realizationAuthorityForInstallation(identity, installationId),
    [identity, installationId],
  );

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const [nextRealizations, nextTargets] = await Promise.all([
        client.listRealizations({ installation_id: installationId }),
        client.listTargets(),
      ]);
      setRealizations(nextRealizations);
      setTargets(nextTargets);
      setTargetId((current) => current || nextTargets.find((target) => target.status === "available")?.id || nextTargets[0]?.id || "");
      setError(null);
    } catch (cause) {
      setError(structuredRealizationFailure(cause));
    } finally {
      setLoading(false);
    }
  }, [client, installationId]);

  useEffect(() => { void load(); }, [load]);
  useEffect(() => {
    const abortController = new AbortController();
    const close = client.subscribeEvents(HOST_REALIZATIONS_SESSION, (event) => {
      if (!abortController.signal.aborted && realizationEventTouchesInstallation(event, installationId)) void load();
    }, { signal: abortController.signal });
    return () => {
      abortController.abort();
      close();
    };
  }, [client, installationId, load]);

  const draft: OciRealizationDraft = {
    installationId,
    installationRevision: installation.revision,
    targetId,
    workloadId,
    executionClass,
    image,
    containerPort: Number(containerPort),
    portName,
    routeId,
    routeAccess,
    ...(healthPath.trim() ? { healthPath } : {}),
    pullIfMissing,
  };
  const draftFailure = validateOciRealizationDraft(draft);
  const canPlan = !draftFailure && authority.canPlan(targetId);

  const plan = async () => {
    if (!canPlan) {
      setError(draftFailure ?? { reasonCode: "authority_denied", nextStep: "Request realization.plan authority for this exact Installation and Target.", targetId });
      return;
    }
    const fingerprint = JSON.stringify(draft);
    const keyName = `plan:${fingerprint}`;
    const idempotencyKey = mutationKeys.current.get(keyName) ?? crypto.randomUUID();
    mutationKeys.current.set(keyName, idempotencyKey);
    setBusy("plan");
    try {
      const result = await client.planRealization(createOciRealizationPlanRequest(draft, idempotencyKey));
      const gaps = result.gaps ?? [];
      if (gaps.length > 0) {
        const gap = gaps[0];
        setError({ reasonCode: gap.reason_code, nextStep: gap.next_step, targetId: gap.target_id ?? targetId });
        return;
      }
      if (!result.realization || !result.plan || !result.plan_ref) {
        setError({ reasonCode: "outcome_unknown", nextStep: "Refresh Realization history before retrying this plan request.", targetId });
        return;
      }
      mutationKeys.current.delete(keyName);
      setPlanned({ realization: result.realization, plan: result.plan, planRef: result.plan_ref, targetId });
      setAcceptedRisks(new Set());
      setError(null);
      await load();
    } catch (cause) {
      setError(structuredRealizationFailure(cause, { targetId }));
    } finally {
      setBusy(null);
    }
  };

  const apply = async () => {
    if (!planned) return;
    if (!authority.canApply(planned.targetId, planned.realization.realization_id)) {
      setError({ reasonCode: "authority_denied", nextStep: "Request realization.apply authority for this exact Installation, Target, and Realization.", targetId: planned.targetId, realizationId: planned.realization.realization_id });
      return;
    }
    const allRisksAccepted = (planned.plan.risk_summary ?? []).every((risk) => acceptedRisks.has(risk));
    if (!allRisksAccepted) {
      setError({ reasonCode: "approval_required", nextStep: "Review and accept every risk declared by this exact plan digest.", targetId: planned.targetId, realizationId: planned.realization.realization_id });
      return;
    }
    const keyName = `apply:${planned.realization.realization_id}:${planned.realization.revision}`;
    const idempotencyKey = mutationKeys.current.get(keyName) ?? crypto.randomUUID();
    mutationKeys.current.set(keyName, idempotencyKey);
    setBusy("apply");
    try {
      await client.applyRealization(createRealizationApplyRequest({
        realization: planned.realization,
        targetId: planned.targetId,
        plan: planned.plan,
        planRef: planned.planRef,
        acceptedRisks: [...acceptedRisks],
        idempotencyKey,
      }));
      mutationKeys.current.delete(keyName);
      setPlanned(null);
      setAcceptedRisks(new Set());
      setError(null);
      await load();
    } catch (cause) {
      setError(structuredRealizationFailure(cause, { targetId: planned.targetId, realizationId: planned.realization.realization_id }));
    } finally {
      setBusy(null);
    }
  };

  const stop = async (realization: RealizationRevision) => {
    const exactTarget = realizationTargetId(realization) ?? targetId;
    if (!exactTarget || !authority.canApply(exactTarget, realization.realization_id)) {
      setError({ reasonCode: "authority_denied", nextStep: "Choose the exact Target and request realization.apply authority before stopping.", targetId: exactTarget || undefined, realizationId: realization.realization_id });
      return;
    }
    const keyName = `stop:${realization.realization_id}:${realization.revision}`;
    const idempotencyKey = mutationKeys.current.get(keyName) ?? crypto.randomUUID();
    mutationKeys.current.set(keyName, idempotencyKey);
    setBusy(keyName);
    try {
      await client.stopRealization({ installation_id: installationId, target_id: exactTarget, realization_id: realization.realization_id, expected_revision: realization.revision, idempotency_key: idempotencyKey });
      mutationKeys.current.delete(keyName);
      setError(null);
      await load();
    } catch (cause) {
      setError(structuredRealizationFailure(cause, { targetId: exactTarget, realizationId: realization.realization_id }));
    } finally {
      setBusy(null);
    }
  };

  const reconcile = async (realization: RealizationRevision) => {
    const exactTarget = realizationTargetId(realization) ?? targetId;
    if (!exactTarget || !authority.canApply(exactTarget, realization.realization_id)) {
      setError({ reasonCode: "authority_denied", nextStep: "Choose the exact Target and request realization.apply authority before reconciling.", targetId: exactTarget || undefined, realizationId: realization.realization_id });
      return;
    }
    const keyName = `reconcile:${realization.realization_id}:${realization.revision}`;
    const idempotencyKey = mutationKeys.current.get(keyName) ?? crypto.randomUUID();
    mutationKeys.current.set(keyName, idempotencyKey);
    setBusy(keyName);
    try {
      await client.reconcileRealization({ installation_id: installationId, target_id: exactTarget, realization_id: realization.realization_id, expected_revision: realization.revision, idempotency_key: idempotencyKey });
      mutationKeys.current.delete(keyName);
      setError(null);
      await load();
    } catch (cause) {
      setError(structuredRealizationFailure(cause, { targetId: exactTarget, realizationId: realization.realization_id }));
    } finally {
      setBusy(null);
    }
  };

  return (
    <section className="rounded-[16px] border border-whisper-border bg-pure-surface p-5" aria-busy={loading || busy !== null}>
      <div className="flex flex-col justify-between gap-3 sm:flex-row sm:items-start">
        <div>
          <h2 className="font-display text-lg font-bold">Managed Realizations</h2>
          <p className="mt-1 max-w-3xl text-xs leading-5 text-steel-secondary">Compile the verified OperationalIntent onto an explicit Target. Planning is effect-free; apply requires approval for the exact content digest.</p>
        </div>
        <Button tone="tertiary" size="sm" disabled={loading} onClick={() => void load()}>Refresh</Button>
      </div>
      <p className="mt-3 break-all rounded-[10px] bg-warm-bone p-3 font-mono text-[11px] text-steel-secondary">operational_intent={installation.work_summary.operational_intent?.digest ?? "missing"}</p>
      {error ? <FailureNotice failure={error} installationId={installationId} /> : null}

      <div className="mt-5 grid gap-3 sm:grid-cols-2">
        <Field label="Target">
          <select value={targetId} onChange={(event) => setTargetId(event.target.value)} className={inputClass}>
            <option value="">Choose a Target</option>
            {targets.map((target) => <option key={target.id} value={target.id}>{target.name} · {target.id} · {target.status}</option>)}
          </select>
        </Field>
        <Field label="Workload ID"><input value={workloadId} onChange={(event) => setWorkloadId(event.target.value)} className={inputClass} placeholder="OperationalIntent workload ID" /></Field>
        <Field label="Execution class"><input value={executionClass} onChange={(event) => setExecutionClass(event.target.value)} className={inputClass} /></Field>
        <Field label="Immutable OCI image"><input value={image} onChange={(event) => setImage(event.target.value)} className={inputClass} placeholder="registry.example/app@sha256:…" /></Field>
        <Field label="Container port"><input type="number" min="1" max="65535" value={containerPort} onChange={(event) => setContainerPort(event.target.value)} className={inputClass} /></Field>
        <Field label="Port name"><input value={portName} onChange={(event) => setPortName(event.target.value)} className={inputClass} /></Field>
        <Field label="Route ID"><input value={routeId} onChange={(event) => setRouteId(event.target.value)} className={inputClass} /></Field>
        <Field label="Route access"><select value={routeAccess} onChange={(event) => setRouteAccess(event.target.value as "host_authenticated" | "public")} className={inputClass}><option value="host_authenticated">Host authenticated</option><option value="public">Public</option></select></Field>
        <Field label="Health path (optional)"><input value={healthPath} onChange={(event) => setHealthPath(event.target.value)} className={inputClass} placeholder="/healthz" /></Field>
        <label className="flex min-h-11 items-center gap-3 text-xs"><input type="checkbox" checked={pullIfMissing} onChange={(event) => setPullIfMissing(event.target.checked)} className="size-4" /><span>Allow the approved executor to pull this exact missing image digest.</span></label>
      </div>
      {draftFailure ? <p className="mt-3 text-xs text-deep-rust"><span className="font-mono">{draftFailure.reasonCode}</span> · {draftFailure.nextStep}</p> : null}
      {!authority.canPlan(targetId) && targetId ? <p className="mt-3 text-xs text-deep-rust"><span className="font-mono">authority_denied</span> · Request realization.plan for this exact Installation and Target.</p> : null}
      <Button className="mt-4" tone="primary" disabled={!canPlan || busy !== null} onClick={() => void plan()}>{busy === "plan" ? "Planning…" : "Compile effect-free plan"}</Button>

      {planned ? (
        <div className="mt-5 rounded-[14px] border border-aged-brass/50 bg-aged-brass-surface/40 p-4">
          <h3 className="font-display font-bold">Plan ready for explicit approval</h3>
          <dl className="mt-3 grid gap-2 text-xs sm:grid-cols-2">
            <Datum label="Realization" value={`${planned.realization.realization_id} · revision ${planned.realization.revision}`} />
            <Datum label="Plan digest" value={planned.planRef.digest} />
            <Datum label="Target" value={planned.targetId} />
            <Datum label="Actions" value={`${planned.plan.build_actions?.length ?? 0} build · ${planned.plan.launch_actions?.length ?? 0} launch · ${planned.plan.endpoint_actions?.length ?? 0} endpoint · ${planned.plan.state_actions?.length ?? 0} state`} />
            <Datum label="Required authority" value={planned.plan.required_authority.join(", ") || "none declared"} />
            <Datum label="Inventory snapshots" value={planned.plan.inventory_refs.map((item) => item.digest).join(", ")} />
          </dl>
          <div className="mt-4 space-y-2">
            {(planned.plan.risk_summary?.length ?? 0) === 0 ? <p className="text-xs text-steel-secondary">This plan declares no additional risk labels.</p> : planned.plan.risk_summary?.map((risk) => (
              <label key={risk} className="flex min-h-11 items-start gap-3 rounded-[10px] bg-pure-surface px-3 py-2 text-xs leading-5">
                <input type="checkbox" className="mt-1 size-4" checked={acceptedRisks.has(risk)} onChange={(event) => setAcceptedRisks((current) => { const next = new Set(current); if (event.target.checked) next.add(risk); else next.delete(risk); return next; })} />
                <span><span className="font-mono">{risk}</span> · approve this risk only for plan {planned.planRef.digest}</span>
              </label>
            ))}
          </div>
          <Button className="mt-3" tone="primary" disabled={busy !== null || !(planned.plan.risk_summary ?? []).every((risk) => acceptedRisks.has(risk)) || !authority.canApply(planned.targetId, planned.realization.realization_id)} onClick={() => void apply()}>{busy === "apply" ? "Applying…" : "Approve and apply exact plan"}</Button>
        </div>
      ) : null}

      <div className="mt-6 space-y-2">
        <h3 className="font-display font-bold">Realization history</h3>
        {realizations.length === 0 ? <p className="text-sm text-muted-tone">No managed Realization recorded.</p> : realizations.map((realization) => {
          const exactTarget = realizationTargetId(realization) ?? targetId;
          const operationKey = `${realization.realization_id}:${realization.revision}`;
          const canApply = Boolean(exactTarget && authority.canApply(exactTarget, realization.realization_id));
          return (
            <article key={realization.realization_id} className="rounded-[12px] border border-whisper-border p-3 text-xs">
              <div className="flex flex-col justify-between gap-3 sm:flex-row sm:items-start">
                <dl className="grid min-w-0 flex-1 gap-2 sm:grid-cols-2">
                  <Datum label="Realization" value={`${realization.realization_id} · revision ${realization.revision}`} />
                  <Datum label="Plan" value={realization.plan_ref.digest} />
                  <Datum label="Target" value={realizationTargetId(realization) ?? "Choose the exact Target above"} />
                  <Datum label="Health" value={`${realization.health.status}${realization.health.reason_code ? ` · ${realization.health.reason_code}` : ""}`} />
                  <Datum label="Resources" value={(realization.actual_resources ?? []).map((resource) => `${resource.resource_type}:${resource.resource_id}`).join(", ") || "none recorded"} />
                  <Datum label="Receipts" value={(realization.receipts ?? []).map((receipt) => receipt.digest).join(", ") || "none recorded"} />
                </dl>
                <StatusPill tone={realizationStateTone(realization.status)} label={realization.status} />
              </div>
              <div className="mt-3 flex flex-wrap gap-2">
                {isRealizationActive(realization.status) ? <Button tone="destructive" size="sm" disabled={!canApply || busy !== null} onClick={() => void stop(realization)}>{busy === `stop:${operationKey}` ? "Stopping…" : "Stop"}</Button> : null}
                {realization.status === "outcome_unknown" || realization.status === "recovery_required" || realization.status === "failed" || isRealizationEffectInProgress(realization.status) ? <Button tone="secondary" size="sm" disabled={!canApply || busy !== null} onClick={() => void reconcile(realization)}>{busy === `reconcile:${operationKey}` ? "Reconciling…" : "Reconcile observed state"}</Button> : null}
              </div>
              {!canApply && realization.status !== "stopped" ? <p className="mt-2 font-mono text-[11px] text-deep-rust">authority_denied · exact Installation, Target, and Realization authority is required for effects.</p> : null}
              {realization.status === "planned" && planned?.realization.realization_id !== realization.realization_id ? <p className="mt-2 text-[11px] text-steel-secondary">This browser does not retain a private copy of the plan body. Re-plan or use the CLI to inspect the persisted plan before approval.</p> : null}
            </article>
          );
        })}
      </div>
    </section>
  );
}

const inputClass = "h-11 w-full rounded-[10px] border border-whisper-border bg-warm-bone px-3 font-mono text-sm";

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return <label className="text-xs"><span className="mb-1 block font-semibold">{label}</span>{children}</label>;
}

function Datum({ label, value }: { label: string; value: string }) {
  return <div className="min-w-0"><dt className="text-[10px] uppercase tracking-[0.08em] text-muted-tone">{label}</dt><dd className="mt-1 break-all font-mono text-[11px]">{value}</dd></div>;
}

function FailureNotice({ failure, installationId }: { failure: RealizationFailure; installationId: string }) {
  return <div className="mt-4 rounded-[12px] border border-deep-rust/30 bg-deep-rust-surface p-3 text-xs text-deep-rust"><p><span className="font-mono">{failure.reasonCode}</span> · {failure.nextStep}</p><p className="mt-2 break-all font-mono text-[11px]">installation_id={installationId} · target_id={failure.targetId ?? "—"} · realization_id={failure.realizationId ?? "—"}</p></div>;
}

function structuredRealizationFailure(cause: unknown, fallback: Partial<RealizationFailure> = {}): RealizationFailure {
  if (cause instanceof ProtocolRpcError) {
    return {
      reasonCode: cause.reasonCode,
      nextStep: cause.nextStep ?? "Refresh Realization history before retrying; do not assume an external effect failed or succeeded.",
      targetId: cause.details.target_id ?? fallback.targetId,
      realizationId: cause.details.realization_id ?? fallback.realizationId,
    };
  }
  return { reasonCode: "outcome_unknown", nextStep: "Refresh Realization history before retrying; do not assume an external effect failed or succeeded.", ...fallback };
}

function realizationEventTouchesInstallation(event: PlatformEvent, installationId: string): boolean {
  if (!event.kind.startsWith("host/realization.")) return false;
  if (!event.payload || typeof event.payload !== "object" || Array.isArray(event.payload)) return false;
  const realization = (event.payload as Record<string, unknown>).realization;
  return Boolean(realization && typeof realization === "object" && !Array.isArray(realization) && (realization as Record<string, unknown>).installation_id === installationId);
}
