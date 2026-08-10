import { useCallback, useEffect, useMemo, useRef, useState, type Dispatch, type SetStateAction } from "react";
import { Button } from "@/components/ui/button";
import { Checkbox, Field, Input, Textarea } from "@/components/ui/input";
import { StatusPill, type StatusTone } from "@/components/ui/status-pill";
import { usePlurora } from "@/lib/plurora-client";
import { useT } from "@/lib/locale";
import { useToast } from "@/components/ui/toast";
import type { LabelKey } from "@/lib/labels";
import type {
  DevelopmentChangeRecord,
  DevelopmentChangeStatus,
  DevelopmentDeploymentStatus,
  DevelopmentFileOperationRequest,
  DevelopmentVerificationPlan,
  DeploymentRevision,
} from "@/protocol/client";

type DraftOperation = "file_write" | "file_delete";

const ACTIVE_STATUSES = new Set<DevelopmentChangeStatus>(["staging", "verifying", "promoting"]);
const ACTIVE_DEPLOYMENT_STATUSES = new Set<DevelopmentDeploymentStatus>(["preparing", "building", "previewing", "activating"]);
const DEVELOPMENT_STATUS_KEYS: Record<DevelopmentChangeStatus, LabelKey> = {
  drafted: "installationFrameDevelopmentStatusDrafted",
  approved: "installationFrameDevelopmentStatusApproved",
  rejected: "installationFrameDevelopmentStatusRejected",
  staging: "installationFrameDevelopmentStatusStaging",
  verifying: "installationFrameDevelopmentStatusVerifying",
  promoting: "installationFrameDevelopmentStatusPromoting",
  verified: "installationFrameDevelopmentStatusVerified",
  committed: "installationFrameDevelopmentStatusCommitted",
  recovery_required: "installationFrameDevelopmentStatusRecoveryRequired",
  failed: "installationFrameDevelopmentStatusFailed",
};

const DEPLOYMENT_STATUS_KEYS: Record<DevelopmentDeploymentStatus, LabelKey> = {
  preparing: "installationFrameDevelopmentDeploymentStatusPreparing",
  building: "installationFrameDevelopmentDeploymentStatusBuilding",
  previewing: "installationFrameDevelopmentDeploymentStatusPreviewing",
  preview_ready: "installationFrameDeploymentReady",
  approved: "installationFrameDevelopmentStatusApproved",
  rejected: "installationFrameDevelopmentStatusRejected",
  activating: "installationFrameDevelopmentDeploymentStatusActivating",
  active: "installationFrameActiveSession",
  recovery_required: "installationFrameDeploymentRecoveryRequired",
  failed: "installationFrameDevelopmentStatusFailed",
};

interface DevelopmentDeploymentDefaults {
  container_port: number;
  port_name: string;
  route_id: string;
  route_access: "host_authenticated" | "public";
  health_path?: string;
}

interface DevelopmentWorkflowCardProps {
  installationId: string;
  targetId: string;
  deploymentDefaults?: DevelopmentDeploymentDefaults;
  activeRevision?: DeploymentRevision | null;
  onDeploymentChanged?: () => void;
}

export function DevelopmentWorkflowCard({
  installationId,
  targetId,
  deploymentDefaults,
  activeRevision,
  onDeploymentChanged,
}: DevelopmentWorkflowCardProps) {
  const client = usePlurora();
  const t = useT();
  const toast = useToast();
  const [changes, setChanges] = useState<DevelopmentChangeRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [goal, setGoal] = useState("");
  const [path, setPath] = useState("");
  const [content, setContent] = useState("");
  const [operation, setOperation] = useState<DraftOperation>("file_write");
  const [executable, setExecutable] = useState(false);
  const [dockerBuild, setDockerBuild] = useState(false);
  const [dockerfile, setDockerfile] = useState("Dockerfile");
  const [allowNetwork, setAllowNetwork] = useState(false);
  const [deploymentPort, setDeploymentPort] = useState(() => String(deploymentDefaults?.container_port ?? 8080));
  const [deploymentPortName, setDeploymentPortName] = useState(deploymentDefaults?.port_name ?? "http");
  const [deploymentRouteId, setDeploymentRouteId] = useState(deploymentDefaults?.route_id ?? defaultDeploymentRouteId(installationId));
  const [deploymentRouteAccess, setDeploymentRouteAccess] = useState<"host_authenticated" | "public">(deploymentDefaults?.route_access ?? "host_authenticated");
  const [deploymentHealthPath, setDeploymentHealthPath] = useState(deploymentDefaults?.health_path ?? "");
  const loadGeneration = useRef(0);
  const loadInFlight = useRef(false);
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      loadGeneration.current += 1;
    };
  }, []);

  useEffect(() => {
    setDeploymentPort(String(deploymentDefaults?.container_port ?? 8080));
    setDeploymentPortName(deploymentDefaults?.port_name ?? "http");
    setDeploymentRouteId(deploymentDefaults?.route_id ?? defaultDeploymentRouteId(installationId));
    setDeploymentRouteAccess(deploymentDefaults?.route_access ?? "host_authenticated");
    setDeploymentHealthPath(deploymentDefaults?.health_path ?? "");
  }, [
    deploymentDefaults?.container_port,
    deploymentDefaults?.health_path,
    deploymentDefaults?.port_name,
    deploymentDefaults?.route_access,
    deploymentDefaults?.route_id,
    installationId,
  ]);

  const load = useCallback(async () => {
    if (loadInFlight.current) return;
    loadInFlight.current = true;
    const generation = ++loadGeneration.current;
    try {
      const result = await client.listInstallationChanges(installationId);
      if (mounted.current && generation === loadGeneration.current) {
        setChanges((current) => mergeChangeList(current, result.changes));
      }
    } catch (error) {
      if (mounted.current && generation === loadGeneration.current) {
        toast.push({
          variant: "error",
          title: t("installationFrameDevelopmentLoadFailed"),
          body: errorMessage(error),
        });
      }
    } finally {
      loadInFlight.current = false;
      if (mounted.current && generation === loadGeneration.current) {
        setLoading(false);
      }
    }
  }, [client, installationId, t, toast]);

  useEffect(() => {
    void load();
  }, [load]);

  const hasActiveChange = useMemo(
    () => changes.some((change) => (
      ACTIVE_STATUSES.has(change.status)
      || (change.deployment ? ACTIVE_DEPLOYMENT_STATUSES.has(change.deployment.status) : false)
    )),
    [changes],
  );

  useEffect(() => {
    if (!hasActiveChange) return;
    const timer = window.setInterval(() => void load(), 1800);
    return () => window.clearInterval(timer);
  }, [hasActiveChange, load]);

  const onDraft = useCallback(async () => {
    if (!goal.trim() || !path.trim() || (operation === "file_write" && !content)) return;
    setBusy("draft");
    try {
      const fileOperation: DevelopmentFileOperationRequest = operation === "file_write"
        ? { op: "file_write", path: path.trim(), content, executable }
        : { op: "file_delete", path: path.trim() };
      const verification: DevelopmentVerificationPlan = dockerBuild
        ? {
            kind: "docker_build",
            dockerfile: dockerfile.trim() || "Dockerfile",
            network_mode: allowNetwork ? "bridge" : "none",
          }
        : { kind: "static_validation" };
      const change = await client.draftInstallationChange(installationId, {
        goal: goal.trim(),
        operations: [fileOperation],
        verification,
        idempotency_key: newDevelopmentIdempotencyKey(),
      });
      if (!mounted.current) return;
      loadGeneration.current += 1;
      setLoading(false);
      setChanges((current) => [change, ...current.filter((item) => item.change_set.id !== change.change_set.id)]);
      setGoal("");
      setPath("");
      setContent("");
      toast.push({ variant: "success", title: t("installationFrameDevelopmentDrafted") });
    } catch (error) {
      if (mounted.current) {
        toast.push({ variant: "error", title: t("installationFrameDevelopmentDraftFailed"), body: errorMessage(error) });
      }
    } finally {
      if (mounted.current) setBusy(null);
    }
  }, [allowNetwork, client, content, dockerBuild, dockerfile, executable, goal, operation, path, installationId, t, toast]);

  const decide = useCallback(async (change: DevelopmentChangeRecord, approved: boolean) => {
    if (approved && typeof window !== "undefined" && !window.confirm(t("installationFrameDevelopmentApproveConfirm"))) return;
    setBusy(change.change_set.id);
    try {
      const updated = await client.approveInstallationChange(
        installationId,
        change.change_set.id,
        approved,
        approved ? "approved from the installation development console" : "rejected from the installation development console",
      );
      if (!mounted.current) return;
      loadGeneration.current += 1;
      setLoading(false);
      replaceChange(setChanges, updated);
    } catch (error) {
      if (mounted.current) {
        toast.push({ variant: "error", title: t("installationFrameDevelopmentDecisionFailed"), body: errorMessage(error) });
      }
    } finally {
      if (mounted.current) setBusy(null);
    }
  }, [client, installationId, t, toast]);

  const execute = useCallback(async (change: DevelopmentChangeRecord) => {
    if (typeof window !== "undefined" && !window.confirm(t("installationFrameDevelopmentExecuteConfirm"))) return;
    setBusy(change.change_set.id);
    try {
      const result = await client.executeInstallationChange(installationId, change.change_set.id);
      if (!mounted.current) return;
      loadGeneration.current += 1;
      setLoading(false);
      replaceChange(setChanges, result.change);
      toast.push({
        variant: result.accepted ? "success" : "info",
        title: result.accepted
          ? t("installationFrameDevelopmentExecutionStarted")
          : t("installationFrameDevelopmentExecutionAlreadyActive"),
      });
      if (!result.accepted) void load();
    } catch (error) {
      if (mounted.current) {
        toast.push({ variant: "error", title: t("installationFrameDevelopmentExecutionFailed"), body: errorMessage(error) });
      }
    } finally {
      if (mounted.current) setBusy(null);
    }
  }, [client, load, installationId, t, toast]);

  const recover = useCallback(async (change: DevelopmentChangeRecord) => {
    setBusy(change.change_set.id);
    try {
      const updated = await client.recoverInstallationChange(installationId, change.change_set.id);
      if (!mounted.current) return;
      loadGeneration.current += 1;
      setLoading(false);
      replaceChange(setChanges, updated);
      toast.push({ variant: "success", title: t("installationFrameDevelopmentRecovered") });
    } catch (error) {
      if (mounted.current) {
        toast.push({ variant: "error", title: t("installationFrameDevelopmentRecoveryFailed"), body: errorMessage(error) });
      }
    } finally {
      if (mounted.current) setBusy(null);
    }
  }, [client, installationId, t, toast]);

  const previewDeployment = useCallback(async (change: DevelopmentChangeRecord) => {
    const containerPort = Number(deploymentPort);
    if (!Number.isInteger(containerPort) || containerPort < 1 || containerPort > 65535 || !deploymentPortName.trim() || !deploymentRouteId.trim()) {
      toast.push({ variant: "error", title: t("installationFrameDevelopmentDeploymentPreviewFailed"), body: t("installationFrameDevelopmentDeploymentInvalidConfig") });
      return;
    }
    if (typeof window !== "undefined" && !window.confirm(t("installationFrameDevelopmentDeploymentPreviewConfirm", targetId, deploymentRouteId.trim()))) return;
    setBusy(change.change_set.id);
    try {
      const updated = await client.createInstallationDeploymentPreview(installationId, change.change_set.id, {
        target_id: targetId,
        container_port: containerPort,
        port_name: deploymentPortName.trim(),
        route_id: deploymentRouteId.trim(),
        route_access: deploymentRouteAccess,
        ...(deploymentHealthPath.trim() ? { health_path: deploymentHealthPath.trim() } : {}),
        idempotency_key: newDevelopmentIdempotencyKey(),
      });
      if (!mounted.current) return;
      loadGeneration.current += 1;
      setLoading(false);
      replaceChange(setChanges, updated);
      onDeploymentChanged?.();
      toast.push({ variant: "success", title: t("installationFrameDevelopmentDeploymentPreviewStarted") });
    } catch (error) {
      if (mounted.current) {
        toast.push({ variant: "error", title: t("installationFrameDevelopmentDeploymentPreviewFailed"), body: errorMessage(error) });
      }
    } finally {
      if (mounted.current) setBusy(null);
    }
  }, [client, deploymentHealthPath, deploymentPort, deploymentPortName, deploymentRouteAccess, deploymentRouteId, onDeploymentChanged, installationId, t, targetId, toast]);

  const decideDeployment = useCallback(async (change: DevelopmentChangeRecord, approved: boolean) => {
    const deployment = change.deployment;
    if (approved && deployment && typeof window !== "undefined" && !window.confirm(t(
      "installationFrameDevelopmentDeploymentApproveConfirm",
      deployment.target_id,
      deployment.route_id,
      deployment.route_access === "public" ? t("installationFrameRoutePublic") : t("installationFrameRoutePrivate"),
    ))) return;
    setBusy(change.change_set.id);
    try {
      const updated = await client.approveInstallationDeployment(installationId, change.change_set.id, {
        approved,
        reason: approved
          ? "approved after inspecting the private deployment preview"
          : "rejected after inspecting the private deployment preview",
      });
      if (!mounted.current) return;
      loadGeneration.current += 1;
      setLoading(false);
      replaceChange(setChanges, updated);
      onDeploymentChanged?.();
      toast.push({
        variant: approved ? "success" : "info",
        title: approved
          ? t("installationFrameDevelopmentDeploymentApproved")
          : t("installationFrameDevelopmentDeploymentRejected"),
      });
    } catch (error) {
      if (mounted.current) {
        toast.push({ variant: "error", title: t("installationFrameDevelopmentDecisionFailed"), body: errorMessage(error) });
      }
    } finally {
      if (mounted.current) setBusy(null);
    }
  }, [client, onDeploymentChanged, installationId, t, toast]);

  const activateDeployment = useCallback(async (change: DevelopmentChangeRecord) => {
    const deployment = change.deployment;
    const routeId = deployment?.route_id ?? deploymentRouteId;
    if (typeof window !== "undefined" && !window.confirm(t(
      "installationFrameDevelopmentDeploymentActivateConfirm",
      deployment?.target_id ?? targetId,
      routeId,
      deployment?.route_access === "public" ? t("installationFrameRoutePublic") : t("installationFrameRoutePrivate"),
    ))) return;
    setBusy(change.change_set.id);
    try {
      const updated = await client.activateInstallationDeployment(installationId, change.change_set.id);
      if (!mounted.current) return;
      loadGeneration.current += 1;
      setLoading(false);
      replaceChange(setChanges, updated);
      onDeploymentChanged?.();
      toast.push({ variant: "success", title: t("installationFrameDevelopmentDeploymentActivated") });
    } catch (error) {
      if (mounted.current) {
        toast.push({ variant: "error", title: t("installationFrameDevelopmentDeploymentActivationFailed"), body: errorMessage(error) });
      }
    } finally {
      if (mounted.current) setBusy(null);
    }
  }, [client, deploymentRouteId, onDeploymentChanged, installationId, t, targetId, toast]);

  const reconcileDeployment = useCallback(async (change: DevelopmentChangeRecord) => {
    if (typeof window !== "undefined" && !window.confirm(t("installationFrameDevelopmentDeploymentReconcileConfirm"))) return;
    setBusy(change.change_set.id);
    try {
      const updated = await client.reconcileInstallationDeployment(installationId, change.change_set.id);
      if (!mounted.current) return;
      loadGeneration.current += 1;
      setLoading(false);
      replaceChange(setChanges, updated);
      toast.push({ variant: "success", title: t("installationFrameDevelopmentDeploymentReconciled") });
    } catch (error) {
      if (mounted.current) {
        toast.push({ variant: "error", title: t("installationFrameDevelopmentDeploymentReconciliationFailed"), body: errorMessage(error) });
      }
    } finally {
      if (mounted.current) {
        onDeploymentChanged?.();
        setBusy(null);
      }
    }
  }, [client, onDeploymentChanged, installationId, t, toast]);

  const exportBundle = useCallback(async (change: DevelopmentChangeRecord) => {
    setBusy(change.change_set.id);
    try {
      const bundle = await client.getInstallationChangeBundle(installationId, change.change_set.id);
      if (!mounted.current) return;
      const blob = new Blob([JSON.stringify(bundle, null, 2)], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = `${change.change_set.id}.plurora-change.json`;
      anchor.click();
      URL.revokeObjectURL(url);
    } catch (error) {
      if (mounted.current) {
        toast.push({ variant: "error", title: t("installationFrameDevelopmentExportFailed"), body: errorMessage(error) });
      }
    } finally {
      if (mounted.current) setBusy(null);
    }
  }, [client, installationId, t, toast]);

  return (
    <div className="space-y-4">
      <div className="rounded-[16px] border border-aged-brass/40 bg-warm-bone p-4">
        <div className="grid gap-3 lg:grid-cols-2">
          <Field label={t("installationFrameDevelopmentGoal")} required>
            <Input value={goal} onChange={(event) => setGoal(event.target.value)} placeholder={t("installationFrameDevelopmentGoalPlaceholder")} />
          </Field>
          <Field label={t("installationFrameDevelopmentTarget")} required>
            <Input value={path} onChange={(event) => setPath(event.target.value)} placeholder="src/example.ts" className="font-mono" />
          </Field>
          <Field label={t("installationFrameDevelopmentOperation")}>
            <select
              value={operation}
              onChange={(event) => setOperation(event.target.value as DraftOperation)}
              className="h-10 rounded-[10px] border border-whisper-border bg-transparent px-3 text-[13px] text-charcoal-ink outline-none focus-visible:border-aged-brass"
            >
              <option value="file_write">{t("installationFrameDevelopmentWrite")}</option>
              <option value="file_delete">{t("installationFrameDevelopmentDelete")}</option>
            </select>
          </Field>
          <div className="flex flex-wrap items-end gap-4 pb-2">
            {operation === "file_write" ? <Checkbox checked={executable} onCheckedChange={setExecutable} label={t("installationFrameDevelopmentExecutable")} /> : null}
            <Checkbox checked={dockerBuild} onCheckedChange={setDockerBuild} label={t("installationFrameDevelopmentDockerBuild")} />
            {dockerBuild ? <Checkbox checked={allowNetwork} onCheckedChange={setAllowNetwork} label={t("installationFrameDevelopmentAllowNetwork")} /> : null}
          </div>
          {dockerBuild ? (
            <Field label={t("installationFrameDevelopmentDockerfile")}>
              <Input value={dockerfile} onChange={(event) => setDockerfile(event.target.value)} className="font-mono" />
            </Field>
          ) : null}
          {operation === "file_write" ? (
            <Field label={t("installationFrameDevelopmentContent")} required className={dockerBuild ? "lg:col-span-2" : "lg:col-span-2"}>
              <Textarea value={content} onChange={(event) => setContent(event.target.value)} className="min-h-[180px] font-mono text-[12px]" spellCheck={false} />
            </Field>
          ) : null}
        </div>
        <div className="mt-4 flex flex-wrap items-center justify-between gap-3">
          <p className="max-w-3xl text-[11px] leading-relaxed text-steel-secondary">{t("installationFrameDevelopmentSafetyHint")}</p>
          <Button
            tone="primary"
            size="sm"
            disabled={busy !== null || !goal.trim() || !path.trim() || (operation === "file_write" && !content)}
            onClick={() => void onDraft()}
          >
            {busy === "draft" ? t("installationFrameDevelopmentDrafting") : t("installationFrameDevelopmentDraft")}
          </Button>
        </div>
      </div>

      <div className="rounded-[16px] border border-whisper-border bg-pure-surface p-4">
        <div className="flex items-center justify-between gap-3">
          <div>
            <p className="font-display text-[15px] font-bold text-charcoal-ink">{t("installationFrameDevelopmentHistory")}</p>
            <p className="mt-1 text-[12px] text-steel-secondary">{t("installationFrameDevelopmentHistoryDescription")}</p>
          </div>
          <Button tone="tertiary" size="sm" onClick={() => void load()} disabled={loading}>{t("installationFrameDevelopmentRefresh")}</Button>
        </div>
        {loading ? <p className="mt-4 text-[12px] text-steel-secondary">{t("installationFrameDiagnosticsLoading")}</p> : changes.length === 0 ? (
          <p className="mt-4 text-[12px] text-steel-secondary">{t("installationFrameDevelopmentEmpty")}</p>
        ) : (
          <div className="mt-4 space-y-2">
            {changes.slice(0, 8).map((change) => {
              const isBusy = busy === change.change_set.id;
              const deployment = change.deployment;
              const deploymentEligible = change.status === "committed"
                && change.workspace_ownership === "managed_external"
                && change.verification_plan.kind === "docker_build";
              const activeDeploymentRevision = deployment?.activation_revision_id
                && activeRevision?.revision_id === deployment.activation_revision_id
                ? activeRevision
                : null;
              const previewUrl = safeHttpUrl(deployment?.preview?.public_url);
              const productionUrl = safeHttpUrl(activeDeploymentRevision?.receipt.public_url);
              const verification = change.verification_plan.kind === "docker_build"
                ? `${t("installationFrameDevelopmentVerificationDocker")} · ${change.verification_plan.dockerfile ?? "Dockerfile"} · ${change.verification_plan.network_mode ?? "none"}`
                : t("installationFrameDevelopmentVerificationStatic");
              return (
                <div key={change.change_set.id} className="rounded-[12px] border border-whisper-border bg-warm-bone p-3">
                  <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                    <div className="min-w-0">
                      <div className="flex flex-wrap items-center gap-2">
                        <span className="font-mono text-[11px] text-charcoal-ink">{change.change_set.id}</span>
                        <StatusPill tone={developmentStatusTone(change.status)} label={t(DEVELOPMENT_STATUS_KEYS[change.status])} />
                        <span className="rounded-full border border-whisper-border bg-pure-surface px-2 py-0.5 font-mono text-[10px] text-steel-secondary">{change.workspace_ownership}</span>
                      </div>
                      <p className="mt-2 text-[12px] text-charcoal-ink">{intentSummary(change)}</p>
                      <p className="mt-1 truncate font-mono text-[10px] text-muted-tone" title={change.proposed_tree_digest ?? change.base_tree_digest}>
                        {change.proposed_tree_digest ?? change.base_tree_digest}
                      </p>
                      {change.error ? <p className="mt-2 text-[11px] text-deep-rust">{change.error}</p> : null}
                      {change.workspace_ownership === "linked_local" ? <p className="mt-2 text-[11px] text-steel-secondary">{t("installationFrameDevelopmentLinkedHint")}</p> : null}
                      <div className="mt-3 grid gap-2 rounded-[10px] border border-whisper-border bg-pure-surface p-3 text-[11px] sm:grid-cols-2">
                        <div>
                          <p className="font-semibold text-charcoal-ink">{t("installationFrameDevelopmentReviewOperations")}</p>
                          <ul className="mt-1 space-y-1 text-steel-secondary">
                            {change.change_set.operations.map((item, index) => (
                              <li key={`${item.op}-${item.target ?? index}`} className="font-mono">
                                {item.op} · {item.target ?? "—"}
                              </li>
                            ))}
                          </ul>
                        </div>
                        <div>
                          <p className="font-semibold text-charcoal-ink">{t("installationFrameDevelopmentReviewVerification")}</p>
                          <p className="mt-1 font-mono text-steel-secondary">{verification}</p>
                        </div>
                        <div>
                          <p className="font-semibold text-charcoal-ink">{t("installationFrameDevelopmentReviewAuthority")}</p>
                          <div className="mt-1 flex flex-wrap gap-1">
                            {change.change_set.required_authority.map((authority) => (
                              <span key={authority} className="rounded-full bg-warm-bone px-2 py-0.5 font-mono text-[10px] text-steel-secondary">{authority}</span>
                            ))}
                          </div>
                        </div>
                        <div>
                          <p className="font-semibold text-charcoal-ink">{t("installationFrameDevelopmentReviewEffects")}</p>
                          <p className="mt-1 break-all font-mono text-[10px] text-steel-secondary">{compactJson(change.change_set.expected_effects)}</p>
                        </div>
                      </div>
                      {change.approval_decision ? (
                        <div className="mt-2 rounded-[10px] border border-whisper-border bg-pure-surface p-3 text-[11px]">
                          <p className="font-semibold text-charcoal-ink">{t("installationFrameDevelopmentApprovalRecord")}</p>
                          <p className="mt-1 font-mono text-steel-secondary">
                            {change.approval_decision.outcome} · {change.approval_decision.principal.kind} · {new Date(change.approval_decision.decided_at).toLocaleString()}
                          </p>
                          {change.approval_decision.reason ? <p className="mt-1 text-steel-secondary">{change.approval_decision.reason}</p> : null}
                          <div className="mt-2 flex flex-wrap gap-1">
                            {change.approval_decision.evaluated_authority.map((authority) => (
                              <span key={authority} className="rounded-full bg-warm-bone px-2 py-0.5 font-mono text-[10px] text-steel-secondary">{authority}</span>
                            ))}
                          </div>
                          {change.approval_ref ? <p className="mt-2 break-all font-mono text-[10px] text-muted-tone">{change.approval_ref.digest}</p> : null}
                        </div>
                      ) : null}
                      {change.recovery_kind ? (
                        <div className="mt-2 rounded-[10px] border border-deep-rust/30 bg-pure-surface p-3 text-[11px]">
                          <p className="font-semibold text-deep-rust">{t("installationFrameDevelopmentRecoveryTarget")}</p>
                          <p className="mt-1 font-mono text-steel-secondary">{change.recovery_kind}</p>
                          {change.managed_promotion ? (
                            <p className="mt-1 break-all font-mono text-[10px] text-muted-tone">
                              {change.managed_promotion.previous_tree_digest} → {change.managed_promotion.proposed_tree_digest} · {change.managed_promotion.destination_preexisting ? "destination_preexisting" : "destination_created_by_change"}
                            </p>
                          ) : null}
                        </div>
                      ) : null}
                      {deploymentEligible || deployment ? (
                        <div className="mt-3 rounded-[12px] border border-aged-brass/40 bg-pure-surface p-3">
                          <div className="flex flex-wrap items-start justify-between gap-2">
                            <div>
                              <p className="text-[12px] font-semibold text-charcoal-ink">{t("installationFrameDevelopmentDeploymentTitle")}</p>
                              <p className="mt-1 max-w-3xl text-[11px] leading-relaxed text-steel-secondary">
                                {t("installationFrameDevelopmentDeploymentDescription")}
                              </p>
                            </div>
                            <div className="flex flex-wrap items-center gap-2">
                              <span className="rounded-full border border-whisper-border bg-warm-bone px-2 py-0.5 font-mono text-[10px] text-steel-secondary">
                                {deployment?.target_id ?? targetId}
                              </span>
                              {deployment ? (
                                <StatusPill
                                  tone={deploymentStatusTone(deployment.status)}
                                  label={t(DEPLOYMENT_STATUS_KEYS[deployment.status])}
                                />
                              ) : null}
                            </div>
                          </div>

                          {!deployment ? (
                            <>
                              <div className="mt-3 grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
                                <Field label={t("installationFrameDeployContainerPort")} required>
                                  <Input
                                    type="number"
                                    min={1}
                                    max={65535}
                                    value={deploymentPort}
                                    onChange={(event) => setDeploymentPort(event.target.value)}
                                    className="font-mono"
                                  />
                                </Field>
                                <Field label={t("installationFrameDeployPortName")} required>
                                  <Input
                                    value={deploymentPortName}
                                    onChange={(event) => setDeploymentPortName(event.target.value)}
                                    className="font-mono"
                                  />
                                </Field>
                                <Field label={t("installationFrameDeployRouteId")} required>
                                  <Input
                                    value={deploymentRouteId}
                                    onChange={(event) => setDeploymentRouteId(event.target.value)}
                                    className="font-mono"
                                  />
                                </Field>
                                <Field label={t("installationFrameRouteExposure")}>
                                  <select
                                    value={deploymentRouteAccess}
                                    onChange={(event) => setDeploymentRouteAccess(event.target.value as "host_authenticated" | "public")}
                                    className="h-10 w-full rounded-[10px] border border-whisper-border bg-transparent px-3 text-[13px] text-charcoal-ink outline-none focus-visible:border-aged-brass"
                                  >
                                    <option value="host_authenticated">{t("installationFrameRoutePrivate")}</option>
                                    <option value="public">{t("installationFrameRoutePublic")}</option>
                                  </select>
                                </Field>
                                <Field label={t("installationFrameDeployHealthPath")}>
                                  <Input
                                    value={deploymentHealthPath}
                                    onChange={(event) => setDeploymentHealthPath(event.target.value)}
                                    placeholder="/healthz"
                                    className="font-mono"
                                  />
                                </Field>
                              </div>
                              {deploymentRouteAccess === "public" ? (
                                <p className="mt-2 text-[11px] leading-relaxed text-deep-rust">{t("installationFrameRoutePublicWarning")}</p>
                              ) : null}
                              <div className="mt-3 flex justify-end">
                                <Button
                                  tone="primary"
                                  size="sm"
                                  disabled={busy !== null}
                                  onClick={() => void previewDeployment(change)}
                                >
                                  {isBusy ? t("installationFrameDevelopmentDeploymentPreviewing") : t("installationFrameDevelopmentDeploymentPreview")}
                                </Button>
                              </div>
                            </>
                          ) : (
                            <>
                              <dl className="mt-3 grid gap-2 text-[11px] sm:grid-cols-2">
                                <div>
                                  <dt className="font-semibold text-charcoal-ink">{t("installationFrameDeployRouteId")}</dt>
                                  <dd className="mt-1 break-all font-mono text-steel-secondary">{deployment.route_id}</dd>
                                </div>
                                <div>
                                  <dt className="font-semibold text-charcoal-ink">{t("installationFrameDevelopmentDeploymentSourceTree")}</dt>
                                  <dd className="mt-1 break-all font-mono text-steel-secondary">{deployment.source_tree_digest}</dd>
                                </div>
                                <div>
                                  <dt className="font-semibold text-charcoal-ink">{t("installationFrameDevelopmentReviewVerification")}</dt>
                                  <dd className="mt-1 break-all font-mono text-steel-secondary">{deployment.verification_ref.digest}</dd>
                                </div>
                                <div>
                                  <dt className="font-semibold text-charcoal-ink">{t("installationFrameDevelopmentDeploymentBuildContext")}</dt>
                                  <dd className="mt-1 break-all font-mono text-steel-secondary">{deployment.build_context_ref.digest}</dd>
                                </div>
                                <div>
                                  <dt className="font-semibold text-charcoal-ink">{t("installationFrameDevelopmentReviewAuthority")}</dt>
                                  <dd className="mt-1 break-all font-mono text-steel-secondary">{deployment.authority_ref.digest}</dd>
                                </div>
                                <div>
                                  <dt className="font-semibold text-charcoal-ink">{t("installationFrameDeployImage")}</dt>
                                  <dd className="mt-1 break-all font-mono text-steel-secondary">
                                    {deployment.preview?.image_id ?? deployment.preview?.image ?? deployment.build_id}
                                  </dd>
                                </div>
                                <div>
                                  <dt className="font-semibold text-charcoal-ink">{t("installationFrameDeployContainerPort")}</dt>
                                  <dd className="mt-1 font-mono text-steel-secondary">{deployment.container_port}</dd>
                                </div>
                                <div>
                                  <dt className="font-semibold text-charcoal-ink">{t("installationFrameDeployPortName")}</dt>
                                  <dd className="mt-1 break-all font-mono text-steel-secondary">{deployment.port_name}</dd>
                                </div>
                                <div>
                                  <dt className="font-semibold text-charcoal-ink">{t("installationFrameRouteExposure")}</dt>
                                  <dd className={`mt-1 font-mono ${deployment.route_access === "public" ? "text-deep-rust" : "text-steel-secondary"}`}>
                                    {deployment.route_access === "public" ? t("installationFrameRoutePublic") : t("installationFrameRoutePrivate")}
                                  </dd>
                                </div>
                                <div>
                                  <dt className="font-semibold text-charcoal-ink">{t("installationFrameDevelopmentDockerfile")}</dt>
                                  <dd className="mt-1 break-all font-mono text-steel-secondary">{deployment.dockerfile} · {deployment.network_mode}</dd>
                                </div>
                                <div>
                                  <dt className="font-semibold text-charcoal-ink">{t("installationFrameDeployHealthPath")}</dt>
                                  <dd className="mt-1 break-all font-mono text-steel-secondary">{deployment.health_path || "—"}</dd>
                                </div>
                              </dl>

                              {deployment.route_access === "public" ? (
                                <p className="mt-3 text-[11px] leading-relaxed text-deep-rust">{t("installationFrameRoutePublicWarning")}</p>
                              ) : null}

                              {deployment.preview ? (
                                <div className="mt-3 rounded-[10px] border border-whisper-border bg-warm-bone p-3 text-[11px]">
                                  <p className="font-semibold text-charcoal-ink">{t("installationFrameDevelopmentDeploymentCandidate")}</p>
                                  <p className="mt-1 break-all font-mono text-steel-secondary">{deployment.preview.deployment.deployment_id}</p>
                                  {deployment.preview_ref ? (
                                    <p className="mt-1 break-all font-mono text-[10px] text-muted-tone">{deployment.preview_ref.digest}</p>
                                  ) : null}
                                  {previewUrl && ["preview_ready", "approved", "activating", "active"].includes(deployment.status) ? (
                                    <a
                                      href={previewUrl}
                                      target="_blank"
                                      rel="noreferrer"
                                      className="mt-2 inline-flex text-[11px] font-semibold text-aged-brass-deep underline underline-offset-2"
                                    >
                                      {t("installationFrameDevelopmentDeploymentOpenPreview")}
                                    </a>
                                  ) : null}
                                </div>
                              ) : null}

                              {deployment.approval_decision ? (
                                <div className="mt-3 rounded-[10px] border border-whisper-border bg-warm-bone p-3 text-[11px]">
                                  <p className="font-semibold text-charcoal-ink">{t("installationFrameDevelopmentApprovalRecord")}</p>
                                  <p className="mt-1 font-mono text-steel-secondary">
                                    {deployment.approval_decision.outcome} · {new Date(deployment.approval_decision.decided_at).toLocaleString()}
                                  </p>
                                  {deployment.approval_decision.reason ? (
                                    <p className="mt-1 text-steel-secondary">{deployment.approval_decision.reason}</p>
                                  ) : null}
                                  {deployment.approval_ref ? (
                                    <p className="mt-2 break-all font-mono text-[10px] text-muted-tone">{deployment.approval_ref.digest}</p>
                                  ) : null}
                                </div>
                              ) : null}

                              {deployment.status === "recovery_required" ? (
                                <p className="mt-3 text-[11px] leading-relaxed text-deep-rust">
                                  {t("installationFrameDevelopmentDeploymentRecoveryHint")}
                                </p>
                              ) : null}

                              {productionUrl ? (
                                <a
                                  href={productionUrl}
                                  target="_blank"
                                  rel="noreferrer"
                                  className="mt-3 inline-flex text-[11px] font-semibold text-aged-brass-deep underline underline-offset-2"
                                >
                                  {t("installationFrameDevelopmentDeploymentOpenProduction")}
                                </a>
                              ) : null}

                              {deployment.error ? <p className="mt-3 text-[11px] text-deep-rust">{deployment.error}</p> : null}

                              <div className="mt-3 flex flex-wrap justify-end gap-2">
                                {deployment.status === "preview_ready" ? (
                                  <>
                                    <Button
                                      tone="secondary"
                                      size="sm"
                                      disabled={busy !== null}
                                      onClick={() => void decideDeployment(change, false)}
                                    >
                                      {t("installationFrameDevelopmentReject")}
                                    </Button>
                                    <Button
                                      tone="primary"
                                      size="sm"
                                      disabled={busy !== null}
                                      onClick={() => void decideDeployment(change, true)}
                                    >
                                      {t("installationFrameDevelopmentApprove")}
                                    </Button>
                                  </>
                                ) : null}
                                {deployment.status === "approved" ? (
                                  <Button
                                    tone="primary"
                                    size="sm"
                                    disabled={busy !== null}
                                    onClick={() => void activateDeployment(change)}
                                  >
                                    {isBusy ? t("installationFrameDevelopmentDeploymentActivating") : t("installationFrameDevelopmentDeploymentActivate")}
                                  </Button>
                                ) : null}
                                {deployment.status === "recovery_required" ? (
                                  <Button
                                    tone="primary"
                                    size="sm"
                                    disabled={busy !== null}
                                    onClick={() => void reconcileDeployment(change)}
                                  >
                                    {isBusy ? t("installationFrameDevelopmentDeploymentReconciling") : t("installationFrameDevelopmentDeploymentReconcile")}
                                  </Button>
                                ) : null}
                              </div>
                            </>
                          )}
                        </div>
                      ) : null}
                    </div>
                    <div className="flex shrink-0 flex-wrap gap-2">
                      <Button tone="tertiary" size="sm" disabled={isBusy} onClick={() => void exportBundle(change)}>{t("installationFrameDevelopmentExport")}</Button>
                      {change.status === "drafted" ? (
                        <>
                          <Button tone="secondary" size="sm" disabled={isBusy} onClick={() => void decide(change, false)}>{t("installationFrameDevelopmentReject")}</Button>
                          <Button tone="primary" size="sm" disabled={isBusy} onClick={() => void decide(change, true)}>{t("installationFrameDevelopmentApprove")}</Button>
                        </>
                      ) : null}
                      {change.status === "approved" ? <Button tone="primary" size="sm" disabled={isBusy} onClick={() => void execute(change)}>{t("installationFrameDevelopmentExecute")}</Button> : null}
                      {change.status === "recovery_required" ? <Button tone="primary" size="sm" disabled={isBusy} onClick={() => void recover(change)}>{t("installationFrameDevelopmentRecover")}</Button> : null}
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}

function replaceChange(
  setChanges: Dispatch<SetStateAction<DevelopmentChangeRecord[]>>,
  updated: DevelopmentChangeRecord,
) {
  setChanges((current) => {
    const existing = current.find((item) => item.change_set.id === updated.change_set.id);
    if (existing && existing.revision > updated.revision) return current;
    return [updated, ...current.filter((item) => item.change_set.id !== updated.change_set.id)];
  });
}

function mergeChangeList(
  current: DevelopmentChangeRecord[],
  incoming: DevelopmentChangeRecord[],
): DevelopmentChangeRecord[] {
  const currentById = new Map(current.map((item) => [item.change_set.id, item]));
  return incoming.map((item) => {
    const existing = currentById.get(item.change_set.id);
    return existing && existing.revision > item.revision ? existing : item;
  });
}

function intentSummary(change: DevelopmentChangeRecord): string {
  const goal = change.intent.goal;
  if (goal && typeof goal === "object" && "summary" in goal && typeof (goal as { summary?: unknown }).summary === "string") {
    return (goal as { summary: string }).summary;
  }
  return change.change_set.operations.map((operation) => operation.target ?? operation.op).join(", ");
}

function developmentStatusTone(status: DevelopmentChangeStatus): StatusTone {
  if (status === "committed" || status === "verified") return "stopped";
  if (status === "failed" || status === "rejected" || status === "recovery_required") return "failed";
  if (ACTIVE_STATUSES.has(status)) return "starting";
  if (status === "approved") return "accent";
  return "neutral";
}

function deploymentStatusTone(status: DevelopmentDeploymentStatus): StatusTone {
  if (status === "active") return "running";
  if (status === "preview_ready") return "stopped";
  if (status === "failed" || status === "rejected" || status === "recovery_required") return "failed";
  if (ACTIVE_DEPLOYMENT_STATUSES.has(status)) return "starting";
  if (status === "approved") return "accent";
  return "neutral";
}

function defaultDeploymentRouteId(installationId: string): string {
  const normalized = installationId
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 64);
  return normalized || "installation-app";
}

function safeHttpUrl(value?: string): string | undefined {
  if (!value) return undefined;
  try {
    const url = new URL(value, typeof window === "undefined" ? "http://localhost" : window.location.origin);
    return url.protocol === "http:" || url.protocol === "https:" ? url.toString() : undefined;
  } catch {
    return undefined;
  }
}

function compactJson(value: unknown): string {
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}

function newDevelopmentIdempotencyKey(): string {
  const entropy = globalThis.crypto?.randomUUID?.().replaceAll("-", "")
    ?? Math.random().toString(36).slice(2);
  return `dev-web-${Date.now().toString(36)}-${entropy.slice(0, 20)}`;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
