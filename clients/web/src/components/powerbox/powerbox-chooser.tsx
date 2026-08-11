import { useEffect, useMemo, useReducer, useRef, useState } from "react";
import { ArrowsClockwise } from "@/components/icons";
import { Button } from "@/components/ui/button";
import { useT } from "@/lib/locale";
import type { HostAccessIdentity } from "@/client-core/host-access";
import {
  BrowserPowerboxPreferenceStore,
  createBindingCandidatesRequest,
  createBindingSelectRequest,
  powerboxAuthorityForContext,
  powerboxContextKey,
  powerboxInvalidationForLifecycleEvent,
  powerboxReducer,
  type PowerboxConsumerContext,
  type PowerboxEmptyKind,
} from "@/client-core/powerbox";
import {
  ProtocolRpcError,
  HOST_POWERBOX_RELAY_SESSIONS,
  type BindingCandidate,
  type BindingMutationResult,
  type PluroraProtocolClient,
} from "@/protocol/client";

export const POWERBOX_FRAME_POLICY = { stopOrRevokeOnUnmount: false } as const;

export function PowerboxChooser({
  client,
  context,
  identity,
  onSelected,
  onClose,
}: {
  client: PluroraProtocolClient;
  context: PowerboxConsumerContext;
  identity: HostAccessIdentity | null;
  onSelected: (result: BindingMutationResult) => void | Promise<void>;
  onClose?: () => void;
}) {
  const t = useT();
  const [state, dispatch] = useReducer(powerboxReducer, { kind: "idle" });
  const [riskConfirmed, setRiskConfirmed] = useState(false);
  const [durationConfirmed, setDurationConfirmed] = useState(false);
  const [selecting, setSelecting] = useState(false);
  const [retryNonce, setRetryNonce] = useState(0);
  const preferenceStore = useMemo(() => new BrowserPowerboxPreferenceStore(), []);
  const authority = useMemo(() => powerboxAuthorityForContext(identity, context), [identity, context]);
  const requestKey = powerboxContextKey(context);
  const headingRef = useRef<HTMLHeadingElement>(null);
  const mutationKeys = useRef(new Map<string, string>());
  const readyCandidates = state.kind === "ready" ? state.candidates : undefined;
  const candidateExposureIds = useMemo(
    () => new Set(readyCandidates?.map((candidate) => candidate.exposure.record.exposure_id) ?? []),
    [readyCandidates],
  );
  const candidateProviderRunIds = useMemo(
    () => new Set(readyCandidates?.flatMap((candidate) => candidate.provider.run ? [candidate.provider.run.run_id] : []) ?? []),
    [readyCandidates],
  );
  const candidateProviderInstallationIds = useMemo(
    () => new Set(readyCandidates?.map((candidate) => candidate.provider.installation.installation_id) ?? []),
    [readyCandidates],
  );

  useEffect(() => {
    headingRef.current?.focus();
  }, [requestKey]);

  useEffect(() => {
    const abortController = new AbortController();
    let active = true;
    setRiskConfirmed(false);
    setDurationConfirmed(false);
    if (!authority.canObserve) {
      dispatch({
        type: "invalidate",
        reasonCode: authority.reasonCode ?? "authority_denied",
        nextStep: authority.nextStep ?? t("powerboxAuthorityDenied"),
      });
      return () => {
        active = false;
        abortController.abort();
      };
    }
    dispatch({ type: "load" });
    const preference = preferenceStore.get(context);
    void client.bindingCandidates(createBindingCandidatesRequest(context, preference)).then((result) => {
      if (active) dispatch({ type: "loaded", result, preferenceProviderInstallationId: preference });
    }).catch((cause) => {
      if (!active) return;
      const failure = structuredPowerboxFailure(cause);
      if (["binding_expired", "outcome_unknown", "plan_stale"].includes(failure.reasonCode)) {
        dispatch({ type: "stale", reasonCode: failure.reasonCode, nextStep: failure.nextStep });
      } else {
        dispatch({ type: "invalidate", reasonCode: failure.reasonCode, nextStep: failure.nextStep });
      }
    });
    return () => {
      // Candidate requests become irrelevant on unmount. Closing the chooser is
      // observational and never stops a Run or revokes a Binding/Exposure.
      active = false;
      abortController.abort();
    };
  }, [authority, client, context, preferenceStore, requestKey, retryNonce, t]);

  useEffect(() => {
    if (state.kind !== "ready") return;
    const abortController = new AbortController();
    const close = client.subscribeHostEvents(
      HOST_POWERBOX_RELAY_SESSIONS,
      (event) => {
        if (abortController.signal.aborted) return;
        const invalidation = powerboxInvalidationForLifecycleEvent(
          event,
          candidateExposureIds,
          candidateProviderRunIds,
          candidateProviderInstallationIds,
        );
        if (!invalidation) return;
        dispatch({ type: "invalidate", ...invalidation });
        // Recompute against the public Host projection. The invalidated state is
        // still observable for the current render, while the next effect loads a
        // fresh candidate set instead of silently rebinding.
        setRetryNonce((value) => value + 1);
      },
      { signal: abortController.signal },
    );
    return () => {
      abortController.abort();
      close();
    };
  }, [candidateExposureIds, candidateProviderInstallationIds, candidateProviderRunIds, client, state.kind]);

  const explicitCandidate = state.kind === "ready" && state.explicitChoiceDigest
    ? state.candidates.find((candidate) => candidate.candidate_digest === state.explicitChoiceDigest)
    : undefined;
  const canConfirm = Boolean(explicitCandidate && riskConfirmed && durationConfirmed && authority.canSelect(explicitCandidate.exposure.record.exposure_id));

  const select = async () => {
    if (!explicitCandidate || !canConfirm || selecting) return;
    const keyName = `${requestKey}:${explicitCandidate.candidate_digest}`;
    const idempotencyKey = mutationKeys.current.get(keyName) ?? crypto.randomUUID();
    mutationKeys.current.set(keyName, idempotencyKey);
    setSelecting(true);
    try {
      const result = await client.selectBinding(createBindingSelectRequest(context, explicitCandidate, idempotencyKey));
      mutationKeys.current.delete(keyName);
      preferenceStore.set(context, explicitCandidate.provider.installation.installation_id);
      dispatch({ type: "selected", result });
      await onSelected(result);
    } catch (cause) {
      const failure = structuredPowerboxFailure(cause);
      if (["binding_expired", "outcome_unknown", "plan_stale", "binding_unavailable"].includes(failure.reasonCode)) {
        dispatch({ type: "stale", reasonCode: failure.reasonCode, nextStep: failure.nextStep });
      } else {
        dispatch({ type: "invalidate", reasonCode: failure.reasonCode, nextStep: failure.nextStep });
      }
    } finally {
      setSelecting(false);
    }
  };

  return (
    <section
      className="rounded-[18px] border border-aged-brass/50 bg-pure-surface p-4 shadow-card sm:p-5"
      aria-labelledby="powerbox-title"
      aria-busy={state.kind === "loading"}
    >
      <div className="flex items-start justify-between gap-3">
        <div>
          <h2 id="powerbox-title" ref={headingRef} tabIndex={-1} className="font-display text-xl font-bold outline-none">{t("powerboxTitle")}</h2>
          <p className="mt-1 max-w-3xl text-xs leading-5 text-steel-secondary">{t("powerboxDescription")}</p>
        </div>
        {onClose ? <Button tone="tertiary" size="sm" onClick={onClose}>{t("powerboxClose")}</Button> : null}
      </div>

      <dl className="mt-4 grid gap-2 text-xs sm:grid-cols-2 lg:grid-cols-4">
        <Disclosure label={t("powerboxConsumer")} value={context.installationId} />
        <Disclosure label={t("powerboxImportPort")} value={context.importPort} />
        <Disclosure label={t("powerboxPhase")} value={context.phase} />
        <Disclosure label={t("powerboxConsumerRun")} value={context.run ? `${context.run.run_id} · revision ${context.run.run_revision} · context ${context.run.context_id}` : "—"} />
      </dl>

      <div className="mt-4" aria-live="polite">
        {state.kind === "idle" || state.kind === "loading" ? (
          <p className="rounded-[12px] bg-warm-bone px-4 py-5 text-sm text-steel-secondary">{t("powerboxLoading")}</p>
        ) : state.kind === "empty" ? (
          <PowerboxEmpty kind={state.emptyKind} gaps={state.gaps} onRetry={() => setRetryNonce((value) => value + 1)} />
        ) : state.kind === "stale" || state.kind === "invalidated" ? (
          <div className="rounded-[12px] border border-deep-rust/30 bg-deep-rust-surface p-4 text-sm text-deep-rust">
            <p className="font-mono text-xs">{state.reasonCode}</p>
            <p className="mt-1">{state.nextStep}</p>
            <p className="mt-2 break-all font-mono text-[11px]">installation_id={context.installationId} · run_id={context.run?.run_id ?? "—"} · port_id={context.importPort}</p>
            <Button className="mt-3" tone="secondary" size="sm" onClick={() => setRetryNonce((value) => value + 1)}>
              <ArrowsClockwise size={14} /> {t("powerboxRetry")}
            </Button>
          </div>
        ) : state.kind === "selected" ? (
          <p className="rounded-[12px] border border-aged-brass/40 bg-aged-brass-surface px-4 py-3 text-sm text-aged-brass-deep">{t("powerboxSelected")}</p>
        ) : (
          <div role="radiogroup" aria-label={t("powerboxTitle")} className="space-y-3">
            {state.candidates.map((candidate) => {
              const checked = state.explicitChoiceDigest === candidate.candidate_digest;
              const preference = state.preferenceHintDigest === candidate.candidate_digest;
              return (
                <CandidateCard
                  key={candidate.candidate_digest}
                  candidate={candidate}
                  checked={checked}
                  preference={preference}
                  onChoose={() => {
                    dispatch({ type: "choose", candidateDigest: candidate.candidate_digest });
                    setRiskConfirmed(false);
                    setDurationConfirmed(false);
                  }}
                />
              );
            })}
          </div>
        )}
      </div>

      {state.kind === "ready" && explicitCandidate ? (
        <div className="mt-4 rounded-[12px] border border-whisper-border bg-warm-bone p-4">
          {!authority.canSelect(explicitCandidate.exposure.record.exposure_id) ? (
            <p className="mb-3 text-xs text-deep-rust"><span className="font-mono">authority_denied</span> · {t("powerboxAuthorityDenied")}</p>
          ) : null}
          <label className="flex min-h-11 cursor-pointer items-start gap-3 py-1 text-xs leading-5">
            <input className="mt-1 size-4" type="checkbox" checked={riskConfirmed} onChange={(event) => setRiskConfirmed(event.target.checked)} />
            <span>{t("powerboxRiskConfirm")}</span>
          </label>
          <label className="flex min-h-11 cursor-pointer items-start gap-3 py-1 text-xs leading-5">
            <input className="mt-1 size-4" type="checkbox" checked={durationConfirmed} onChange={(event) => setDurationConfirmed(event.target.checked)} />
            <span>{t("powerboxDurationConfirm")}</span>
          </label>
          <Button className="mt-2 w-full sm:w-auto" tone="primary" onClick={() => void select()} disabled={!canConfirm || selecting}>
            {selecting ? t("powerboxSelecting") : t("powerboxSelect")}
          </Button>
        </div>
      ) : null}
    </section>
  );
}

function CandidateCard({
  candidate,
  checked,
  preference,
  onChoose,
}: {
  candidate: BindingCandidate;
  checked: boolean;
  preference: boolean;
  onChoose: () => void;
}) {
  const t = useT();
  const exposure = candidate.exposure;
  const expiresAt = candidate.effective_expires_at ?? exposure.record.expires_at ?? null;
  const notDeclared = t("powerboxNotDeclared");
  const audience = formatAudience(exposure.record.audience);
  const remaining = formatRemaining(expiresAt, t("powerboxUnbounded"), t("powerboxExpired"));
  return (
    <label className={`block cursor-pointer rounded-[14px] border p-4 transition-colors ${checked ? "border-aged-brass bg-aged-brass-surface/50" : "border-whisper-border bg-pure-surface hover:border-aged-brass/60"}`}>
      <span className="flex min-h-11 items-center gap-3">
        <input type="radio" name="powerbox-provider" checked={checked} onChange={onChoose} className="size-4" />
        <span className="min-w-0 flex-1">
          <span className="block break-all font-mono text-xs font-semibold">{candidate.provider.installation.installation_id}</span>
          <span className="mt-0.5 block text-[11px] text-steel-secondary">{t("powerboxChooseProvider")}</span>
        </span>
        {preference ? <span className="rounded-full bg-aged-brass px-2 py-1 text-[10px] font-semibold text-deep-bark">{t("powerboxPreferenceHint")}</span> : null}
      </span>
      <dl className="mt-3 grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
        <Disclosure label={t("powerboxProviderInstallation")} value={`${candidate.provider_installation.display_name} · ${candidate.provider_installation.installation_id} · revision ${candidate.provider_installation.installation_revision}`} />
        <Disclosure label={t("powerboxProviderWork")} value={`${candidate.provider_work.title} · ${candidate.provider_work.work_id} · ${candidate.provider.installation.work_revision.digest}`} />
        <Disclosure label={t("powerboxProviderRun")} value={candidate.provider.run?.run_id ?? notDeclared} />
        <Disclosure label={t("powerboxProviderPort")} value={`${candidate.provider_port.port_id} · ${candidate.provider_port.contract.version}`} />
        <Disclosure label={t("powerboxOrigin")} value={formatOrigin(candidate, notDeclared)} />
        <Disclosure label={t("powerboxDeclaration")} value={`${candidate.provider_component.component_id} · ${candidate.provider_component.version} · ${candidate.capability.capability_id}@${candidate.capability.capability_version}`} />
        <Disclosure label={t("powerboxClaim")} value={candidate.provider_component.claim_status} />
        <Disclosure label={t("powerboxBoundaries")} value={formatBoundaries(candidate.provider_component.enforced_boundaries)} />
        <Disclosure label={t("powerboxEvidence")} value={formatEvidence(candidate, notDeclared)} />
        <Disclosure label={t("powerboxTrust")} value={`${candidate.provider_component.trust_class} · ${candidate.provider_component.entry_kind}`} />
        <Disclosure label={t("powerboxProtocol")} value={`${candidate.provider_port.contract.protocol_id} · consumer ${candidate.consumer_port.contract.protocol_id}`} />
        <Disclosure label={t("powerboxInterface")} value={`${candidate.provider_port.contract.interface_id} · consumer ${candidate.consumer_port.contract.interface_id}`} />
        <Disclosure label={t("powerboxVersion")} value={`${candidate.provider_port.contract.version} · consumer ${candidate.consumer_port.contract.version}`} />
        <Disclosure label={t("powerboxProfile")} value={`provider ${formatOptionalList(candidate.provider_port.contract.profiles, notDeclared)} · consumer ${formatOptionalList(candidate.consumer_port.contract.profiles, notDeclared)}`} />
        <Disclosure label={t("powerboxInteraction")} value={`provider ${candidate.provider_port.interaction} · consumer ${candidate.consumer_port.interaction}`} />
        <Disclosure label={t("powerboxTransport")} value={formatTransport(candidate, notDeclared)} />
        <Disclosure label={t("powerboxAudience")} value={audience} />
        <Disclosure label={t("powerboxScope")} value={`${candidate.provider_installation.installation_id}:${candidate.provider_port.port_id}`} />
        <Disclosure label={t("powerboxExpiry")} value={expiresAt ? new Date(expiresAt).toLocaleString() : notDeclared} />
        <Disclosure label={t("powerboxDuration")} value={remaining} />
        <Disclosure label={t("powerboxDataRisk")} value={`consumer availability ${candidate.consumer_port.role.kind === "import" ? candidate.consumer_port.role.availability : notDeclared}; transport ${formatTransportRequirements(candidate.consumer_port.transport, notDeclared)}`} />
        <Disclosure label={t("powerboxEffectRisk")} value={`provider ${candidate.provider_port.role.kind === "export" ? candidate.provider_port.role.effect_class : notDeclared}; accepted by consumer ${candidate.consumer_port.role.kind === "import" ? formatOptionalList(candidate.consumer_port.role.accepted_effects, notDeclared) : notDeclared}`} />
      </dl>
    </label>
  );
}

function Disclosure({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0 rounded-[9px] bg-warm-bone px-3 py-2">
      <dt className="text-[10px] uppercase tracking-[0.09em] text-muted-tone">{label}</dt>
      <dd className="mt-1 break-all font-mono text-[11px] leading-4 text-charcoal-ink">{value}</dd>
    </div>
  );
}

function formatAudience(audience: Array<{ kind: string; id: string }>): string {
  return audience.length > 0 ? audience.map((selector) => `${selector.kind}:${selector.id}`).join(", ") : "none";
}

function formatOptionalList(values: readonly string[] | null | undefined, undeclaredLabel: string): string {
  return values && values.length > 0 ? values.join(", ") : undeclaredLabel;
}

function formatOrigin(candidate: BindingCandidate, undeclaredLabel: string): string {
  const source = candidate.provider_installation.source;
  const refs = [
    source.source_ref?.digest,
    ...(source.provenance_refs ?? []).map((reference) => reference.digest),
  ].filter((reference): reference is string => Boolean(reference));
  return [source.kind, candidate.provider_component.package_id, refs.length > 0 ? refs.join(", ") : undeclaredLabel].join(" · ");
}

function formatEvidence(candidate: BindingCandidate, undeclaredLabel: string): string {
  const protocolEvidence = (candidate.provider_component.protocol_implementations ?? [])
    .map((descriptor) => `${descriptor.implementation.protocol_id}@${descriptor.implementation.version}`)
    .join(", ");
  return [
    `behavior ${candidate.provider_component.behavior.digest}`,
    `artifact ${candidate.provider_component.component_artifact.digest}`,
    `contract ${candidate.provider.port.canonical_contract_digest}`,
    protocolEvidence ? `protocols ${protocolEvidence}` : undeclaredLabel,
  ].join(" · ");
}

function formatBoundaries(boundaries: BindingCandidate["provider_component"]["enforced_boundaries"]): string {
  return Object.entries(boundaries)
    .map(([name, enforced]) => `${name}=${enforced ? "yes" : "no"}`)
    .join(", ");
}

function formatTransport(candidate: BindingCandidate, undeclaredLabel: string): string {
  return [
    `selected ${candidate.transport.class_id}`,
    `provider ${formatTransportRequirements(candidate.provider_port.transport, undeclaredLabel)}`,
    `consumer ${formatTransportRequirements(candidate.consumer_port.transport, undeclaredLabel)}`,
  ].join(" · ");
}

function formatTransportRequirements(requirements: BindingCandidate["provider_port"]["transport"], undeclaredLabel: string): string {
  if (!requirements) return undeclaredLabel;
  const values: string[] = [];
  if (requirements.allowed_classes?.length) values.push(`allowed=${requirements.allowed_classes.join(",")}`);
  for (const key of ["same_process", "local_only", "ordered", "reliable", "large_payload", "shared_memory_allowed"] as const) {
    if (requirements[key] !== undefined) values.push(`${key}=${requirements[key] ? "yes" : "no"}`);
  }
  if (requirements.max_latency_class) values.push(`latency=${requirements.max_latency_class}`);
  return values.length > 0 ? values.join(", ") : undeclaredLabel;
}

function PowerboxEmpty({ kind, gaps, onRetry }: { kind: PowerboxEmptyKind; gaps: Array<{ reason_code: string; next_step: string; installation_id?: string | null; run_id?: string | null; port_id?: string | null }>; onRetry: () => void }) {
  const t = useT();
  const label = {
    absent: t("powerboxEmptyAbsent"),
    forbidden: t("powerboxEmptyForbidden"),
    unsupported: t("powerboxEmptyUnsupported"),
    unavailable: t("powerboxEmptyUnavailable"),
    stale: t("powerboxEmptyStale"),
  }[kind];
  return (
    <div className="rounded-[12px] border border-whisper-border bg-warm-bone p-4 text-sm">
      <p>{label}</p>
      {gaps.map((gap, index) => (
        <div key={`${gap.reason_code}:${index}`} className="mt-2 text-xs text-steel-secondary">
          <p><span className="font-mono">{gap.reason_code}</span> · {t("powerboxNextStep")}: {gap.next_step}</p>
          <p className="mt-1 break-all font-mono text-[10px]">installation_id={gap.installation_id ?? "—"} · run_id={gap.run_id ?? "—"} · port_id={gap.port_id ?? "—"}</p>
        </div>
      ))}
      <Button className="mt-3" tone="secondary" size="sm" onClick={onRetry}><ArrowsClockwise size={14} /> {t("powerboxRetry")}</Button>
    </div>
  );
}

function formatRemaining(expiresAt: string | null, unboundedLabel: string, expiredLabel: string): string {
  if (!expiresAt) return unboundedLabel;
  const durationMs = Date.parse(expiresAt) - Date.now();
  if (!Number.isFinite(durationMs) || durationMs <= 0) return expiredLabel;
  const minutes = Math.ceil(durationMs / 60_000);
  if (minutes < 60) return `${minutes} min`;
  const hours = Math.ceil(minutes / 60);
  return `${hours} h`;
}

function structuredPowerboxFailure(cause: unknown): { reasonCode: string; nextStep: string } {
  if (cause instanceof ProtocolRpcError) {
    return {
      reasonCode: cause.reasonCode,
      nextStep: cause.nextStep ?? "Refresh Host state and recompute candidates before retrying.",
    };
  }
  return {
    reasonCode: "outcome_unknown",
    nextStep: "Refresh Host state and recompute candidates before retrying.",
  };
}
