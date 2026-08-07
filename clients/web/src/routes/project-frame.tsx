import { useCallback, useEffect, useId, useMemo, useRef, useState, type ReactNode } from "react";
import { ArrowLeft, ArrowsClockwise, BookOpenText, Copy, LinkSimple, StopCircle } from "@/components/icons";
import { Button } from "@/components/ui/button";
import { Modal, ModalFooter, ModalHeader } from "@/components/ui/modal";
import { StatusPill, projectStateTone, type StatusTone } from "@/components/ui/status-pill";
import { Tooltip } from "@/components/ui/tooltip";
import { useKernel } from "@/lib/kernel-client";
import { useRoute } from "@/lib/router";
import { useToast } from "@/components/ui/toast";
import { useT } from "@/lib/locale";
import { openProjectInTab } from "@/lib/project-launcher";
import { shouldReturnToShellHistory } from "@/client-core/platform-adapter";
import { BrowserProjectTargetContextStore } from "@/client-core/project-target-context";
import { mountSurface, type SurfaceHostHandle } from "@/surfaces/surface-host";
import { resolveSurfaceBundle, type ResolvedSurfaceBundle } from "@/surfaces/bundle-resolver";
import { formatBytes } from "@/lib/format";
import { DevelopmentWorkflowCard } from "@/components/development/change-workflow-card";
import { parseBuildDeployDescriptor, parseDockerDeploymentDescriptor, type BuildDeployDescriptor, type DockerDeploymentDescriptor } from "@/lib/project-deployment";
import type {
  BuildDeployJobEvent,
  BuildDeployJobStatusResponse,
  BuildDeployJobSubmitResponse,
  DeploymentRevision,
  ExecStatus,
  ExecutionTarget,
  KernelEvent,
  LocalExecLogLine,
  PackageRecord,
  PortLeaseRecord,
  ProjectRecord,
  ProxyRouteRecord,
  ProjectDeploymentsResponse,
  SurfaceContributionRecord,
  TargetOperationRecord,
  TargetOperationStatus,
  UpdateCheckResult,
} from "@/protocol/client";

const FRAME_CONTAINER_ID = "plurora-project-frame";
const UPDATE_AVAILABLE_STATUSES = new Set(["available", "update_available", "repair_required"]);

interface ProjectDiagnostics {
  projectId: string;
  bundle?: ResolvedSurfaceBundle;
  packages: PackageRecord[];
  events: KernelEvent[];
  targets: ExecutionTarget[];
  executions: ExecStatus[];
  portLeases: PortLeaseRecord[];
  proxyRoutes: ProxyRouteRecord[];
  operations?: TargetOperationRecord[];
  deployments?: ProjectDeploymentsResponse;
  updates?: UpdateCheckResult;
  errors: string[];
  refreshedAt: string;
}

type DeploymentUiOperation = "idle" | "deploying" | "stopping" | "recovering" | "rolling_back";

interface ConsoleSummary {
  packageTotal: number;
  packageHealthy: number;
  packageProblem: number;
  recentEvents: number;
  updateAvailable: number;
  updateChecked: boolean;
  targetTotal: number;
  execTotal: number;
  execRunning: number;
  portActive: number;
  proxyActive: number;
}

export function ProjectFrame({ projectId, chrome = "shell" }: { projectId: string; chrome?: "shell" | "none" }) {
  const client = useKernel();
  const toast = useToast();
  const t = useT();
  const [, navigate] = useRoute();
  const [project, setProject] = useState<(ProjectRecord & { running_session_id?: string; entry_surface_id?: string }) | null>(null);
  const [stopping, setStopping] = useState(false);
  const [refreshingDiagnostics, setRefreshingDiagnostics] = useState(false);
  const [updatingProject, setUpdatingProject] = useState(false);
  const [deploymentOperation, setDeploymentOperation] = useState<DeploymentUiOperation>("idle");
  const [buildDeployJob, setBuildDeployJob] = useState<BuildDeployJobStatusResponse | BuildDeployJobSubmitResponse | null>(null);
  const [buildDeployEvents, setBuildDeployEvents] = useState<BuildDeployJobEvent[]>([]);
  const [diagnostics, setDiagnostics] = useState<ProjectDiagnostics | null>(null);
  const diagnosticsRequestRef = useRef(0);
  const [frameState, setFrameState] = useState<"loading" | "mounted" | "start_failed" | "mount_failed" | "stopped">("loading");
  const handleRef = useRef<SurfaceHostHandle | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const projectRef = useRef(project);
  const [mountAttempt, setMountAttempt] = useState(0);
  const targetContextStore = useMemo(() => new BrowserProjectTargetContextStore(), []);
  const [selectedTargetId, setSelectedTargetId] = useState(
    () => targetContextStore.get(projectId) ?? "local",
  );

  useEffect(() => {
    setSelectedTargetId(targetContextStore.get(projectId) ?? "local");
  }, [projectId, targetContextStore]);

  useEffect(() => {
    projectRef.current = project;
  }, [project]);

  useEffect(() => {
    if (typeof document === "undefined") return;
    const previousTitle = document.title;
    document.title = chrome === "none"
      ? `${project?.title ?? projectId} — Plurora`
      : `${project?.title ?? projectId} console — Plurora`;
    return () => {
      document.title = previousTitle;
    };
  }, [chrome, project?.title, projectId]);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        setFrameState("loading");
        const detail = await client.getProject(projectId);
        if (cancelled) return;

        // Capture the session id from start_project — the project record from
        // get_project is a stale snapshot, and ignoring the new id would mount
        // the surface with sessionId=undefined.
        let sessionId = detail.running_session_id;
        let runtimeState = detail.state;
        if (!sessionId) {
          const started = await client.startProject(projectId);
          if (cancelled) return;
          sessionId = started.session_id;
          runtimeState = (started.new_state as typeof runtimeState) ?? "running";
        }

        // Reflect the live state in the project frame topbar.
        setProject({ ...detail, state: runtimeState, running_session_id: sessionId });

        if (!detail.entry_surface_id) {
          if (chrome !== "none") {
            setFrameState("mounted");
            return;
          }
          setFrameState("start_failed");
          return;
        }
        const bundle = await resolveSurfaceBundle(client, detail.entry_surface_id);
        if (chrome !== "none") {
          setDiagnostics((current) => {
            const matching = current?.projectId === projectId ? current : null;
            return {
              projectId,
              bundle,
              packages: matching?.packages ?? [],
              events: matching?.events ?? [],
              targets: matching?.targets ?? [],
              executions: matching?.executions ?? [],
              portLeases: matching?.portLeases ?? [],
              proxyRoutes: matching?.proxyRoutes ?? [],
              operations: matching?.operations ?? [],
              deployments: matching?.deployments,
              updates: matching?.updates,
              errors: matching?.errors ?? [],
              refreshedAt: matching?.refreshedAt ?? new Date().toISOString(),
            };
          });
          setFrameState("mounted");
          return;
        }

        // Installed native projects can resolve to a copied project bundle even
        // when the contributing surface package has not been autoloaded into the
        // current host process yet (for example immediately after a web install).
        // Treat contribution metadata as best-effort so an available project UI
        // does not fail before the iframe has a chance to mount.
        const contribution = await client
          .describeSurface(detail.entry_surface_id)
          .catch((): SurfaceContributionRecord | null => null);
        if (cancelled) return;
        const allowedCapabilityIds = allowedSurfaceCapabilityIdsForTest(contribution);
        let handle: SurfaceHostHandle;
        try {
          handle = await mountSurface({
            containerId: FRAME_CONTAINER_ID,
            surfaceId: bundle.surfaceId,
            bundleUrl: bundle.bundleUrl,
            exportName: bundle.exportName,
            stylesheets: bundle.stylesheets,
            wrapperClass: bundle.wrapperClass,
            initialProps: { projectId },
            hostBridge: {
              currentSessionId: sessionId,
              allowedCapabilityIds,
              callRpc: (method, params) => client.invokeWithSession(method, params, sessionId),
              subscribeEvents: (cb) =>
                client.subscribeEvents(sessionId, (event) => cb(event)),
            },
          });
        } catch (err) {
          setFrameState("mount_failed");
          toast.push({
            variant: "error",
            title: t("projectFrameMountFailedTitle"),
            body: t("projectFrameMountFailedBody"),
          });
          return;
        }
        if (cancelled) {
          await handle.unmount();
          return;
        }
        handleRef.current = handle;
        setFrameState("mounted");
      } catch (err) {
        setFrameState("start_failed");
        toast.push({
          variant: "error",
          title: t("projectFrameStartFailedTitle"),
          body: t("projectFrameStartFailedBody"),
        });
      }
    })();

    return () => {
      cancelled = true;
      handleRef.current?.unmount().catch(() => {});
      handleRef.current = null;
    };
  }, [chrome, client, mountAttempt, projectId, toast, t]);

  const onRetry = useCallback(() => {
    void handleRef.current?.unmount().catch(() => {});
    handleRef.current = null;
    setMountAttempt((attempt) => attempt + 1);
  }, []);

  const onStop = useCallback(async () => {
    if (stopping) return;
    if (typeof window !== "undefined" && !window.confirm(t("projectFrameStopConfirm"))) return;
    setStopping(true);
    try {
      await client.stopProject(projectId);
      toast.push({ variant: "success", title: t("projectFrameStopped", project?.title ?? projectId) });
      await handleRef.current?.unmount().catch(() => {});
      handleRef.current = null;
      setProject((current) => current ? { ...current, state: "stopped", running_session_id: undefined } : current);
      setFrameState("stopped");
    } catch (err) {
      toast.push({
        variant: "error",
        title: t("projectFrameStopFailedTitle"),
        body: t("projectFrameStopFailedBody"),
      });
    } finally {
      setStopping(false);
    }
  }, [client, project?.title, projectId, stopping, t, toast]);

  const onOpenProjectTab = useCallback(() => {
    const outcome = openProjectInTab(projectId);
    if (outcome === "failed" || outcome === "invalid") {
      toast.push({
        variant: "warning",
        title: t("projectFrameProjectTabBlockedTitle"),
        body: t("projectFrameProjectTabBlockedBody"),
      });
    }
  }, [projectId, t, toast]);

  useEffect(() => {
    if (chrome !== "none") return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (!event.isTrusted) return;
      if (!(event.metaKey || event.ctrlKey)) return;
      if (event.key !== "." && event.code !== "Period") return;
      const target = event.target as Element | null;
      if (target?.closest('input, textarea, select, [contenteditable="true"]')) return;
      event.preventDefault();
      void onStop();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [chrome, onStop]);

  const loadDiagnostics = useCallback(async () => {
    const requestId = ++diagnosticsRequestRef.current;
    const targetId = selectedTargetId;
    setRefreshingDiagnostics(true);
    const errors: string[] = [];
    let latestProject = projectRef.current?.id === projectId ? projectRef.current : null;
    try {
      const fetchedProject = await client.getProject(projectId);
      if (requestId !== diagnosticsRequestRef.current) return;
      latestProject = fetchedProject;
      setProject((current) => ({
        ...(fetchedProject as ProjectRecord & { running_session_id?: string; entry_surface_id?: string }),
        state: (current?.state === "running" && !fetchedProject.running_session_id ? current.state : fetchedProject.state),
        running_session_id: fetchedProject.running_session_id ?? current?.running_session_id,
      }));
    } catch (err) {
      errors.push(errorMessage(err));
    }

    const currentProject = projectRef.current?.id === projectId ? projectRef.current : null;
    const entrySurfaceId = latestProject?.entry_surface_id ?? currentProject?.entry_surface_id;
    const sessionId = latestProject?.running_session_id ?? currentProject?.running_session_id;
    const declaredPackageRefs = latestProject?.packages ?? currentProject?.packages ?? [];

    const [bundleResult, packageListResult, eventResult, updateResult, targetResult, execResult, portResult, proxyResult, operationResult, deploymentResult] = await Promise.allSettled([
      entrySurfaceId ? resolveSurfaceBundle(client, entrySurfaceId) : Promise.resolve(undefined),
      client.packages(),
      sessionId ? client.listEvents(sessionId) : Promise.resolve([]),
      client.checkProjectUpdates(projectId),
      client.listTargets(),
      client.listExecs(),
      client.listPortLeases(),
      client.listProxyRoutes(),
      client.listTargetOperations(targetId),
      client.getProjectDeployments(projectId),
    ]);

    if (requestId !== diagnosticsRequestRef.current) return;

    const bundle = unwrapSettled(bundleResult, errors);
    const packageList = unwrapSettled(packageListResult, errors) ?? [];
    const events = unwrapSettled(eventResult, errors) ?? [];
    const updates = unwrapSettled(updateResult, errors);
    const targets = unwrapSettled(targetResult, errors) ?? [];
    const executions = unwrapSettled(execResult, errors)?.executions ?? [];
    const portLeases = unwrapSettled(portResult, errors) ?? [];
    const proxyRoutes = unwrapSettled(proxyResult, errors) ?? [];
    const operations = targetOperationsForProject(
      unwrapSettled(operationResult, errors) ?? [],
      projectId,
      targetId,
    );
    const deployments = unwrapSettled(deploymentResult, errors);
    const projectPackages = filterProjectPackages(packageList, declaredPackageRefs, projectId, updates?.results ?? []);

    setDiagnostics({
      projectId,
      bundle,
      packages: projectPackages,
      events: [...events].slice(-8).reverse(),
      targets,
      executions,
      portLeases,
      proxyRoutes,
      operations,
      deployments,
      updates,
      errors,
      refreshedAt: new Date().toISOString(),
    });
    setRefreshingDiagnostics(false);
  }, [client, projectId, selectedTargetId]);

  useEffect(() => {
    if (chrome === "none") return;
    void loadDiagnostics();
  }, [chrome, loadDiagnostics]);

  useEffect(() => {
    const targets = diagnostics?.targets ?? [];
    if (targets.length === 0 || targets.some((target) => target.id === selectedTargetId)) return;
    const fallback = targets.find((target) => target.id === "local") ?? targets[0];
    setSelectedTargetId(fallback.id);
    targetContextStore.set(projectId, fallback.id);
  }, [diagnostics?.targets, projectId, selectedTargetId, targetContextStore]);

  const onRefreshDiagnostics = useCallback(() => {
    void loadDiagnostics();
  }, [loadDiagnostics]);

  const onUpdateProject = useCallback(async () => {
    if (updatingProject) return;
    setUpdatingProject(true);
    try {
      const result = await client.updateProject(projectId);
      const updated = result.updated_packages?.length ?? 0;
      toast.push({
        variant: result.updated ? "success" : "info",
        title: result.updated ? t("projectFrameUpdateCompleteTitle") : t("projectFrameUpdateCurrentTitle"),
        body: result.updated
          ? t("projectFrameUpdateCompleteBody", updated)
          : result.reason ?? result.status ?? t("projectFrameUpdateCurrentBody"),
      });
      await loadDiagnostics();
    } catch (err) {
      toast.push({
        variant: "error",
        title: t("projectFrameUpdateFailedTitle"),
        body: errorMessage(err),
      });
    } finally {
      setUpdatingProject(false);
    }
  }, [client, loadDiagnostics, projectId, t, toast, updatingProject]);

  const onDeployDocker = useCallback(async (descriptor: DockerDeploymentDescriptor) => {
    if (deploymentOperation !== "idle") return;
    if (typeof window !== "undefined" && !window.confirm(t("projectFrameDeployConfirm", descriptor.image))) return;
    setDeploymentOperation("deploying");
    try {
      const result = await client.deployProject({
        project_id: projectId,
        image: descriptor.image,
        container_port: descriptor.container_port,
        port_name: descriptor.port_name,
        route_id: descriptor.route_id,
        route_access: descriptor.route_access,
        ...(descriptor.health_path ? { health_path: descriptor.health_path } : {}),
        pull_if_missing: descriptor.pull_if_missing,
      });
      void result;
      toast.push({ variant: "success", title: t("projectFrameDeploySuccessTitle"), body: t("projectFrameDeploySuccessBody", descriptor.route_id) });
      await loadDiagnostics();
    } catch (err) {
      toast.push({
        variant: "error",
        title: t("projectFrameDeployFailedTitle"),
        body: `${t("projectFrameDeployFailedBody")} ${errorMessage(err)}`,
      });
      await loadDiagnostics().catch(() => {});
    } finally {
      setDeploymentOperation("idle");
    }
  }, [client, deploymentOperation, loadDiagnostics, t, toast]);

  const onStopDeploymentRoute = useCallback(async (routeId: string) => {
    if (deploymentOperation !== "idle") return;
    if (typeof window !== "undefined" && !window.confirm(t("projectFrameStopDeploymentConfirm", routeId))) return;
    setDeploymentOperation("stopping");
    try {
      const result = await client.stopProjectDeployment({ route_id: routeId });
      await loadDiagnostics();
      if (result.warnings.length > 0) {
        toast.push({ variant: "warning", title: t("projectFrameStopDeploymentPartialTitle"), body: result.warnings.slice(0, 2).join("; ") });
      } else {
        toast.push({ variant: "success", title: t("projectFrameStopDeploymentSuccessTitle") });
      }
    } catch (err) {
      toast.push({ variant: "error", title: t("projectFrameStopDeploymentPartialTitle"), body: errorMessage(err) });
      await loadDiagnostics().catch(() => {});
    } finally {
      setDeploymentOperation("idle");
    }
  }, [client, deploymentOperation, loadDiagnostics, t, toast]);

  const onStopDockerDeployment = useCallback((descriptor: DockerDeploymentDescriptor) => {
    void onStopDeploymentRoute(descriptor.route_id);
  }, [onStopDeploymentRoute]);

  const onRecoverDeployment = useCallback(async () => {
    if (deploymentOperation !== "idle") return;
    if (typeof window !== "undefined" && !window.confirm(t("projectFrameDeploymentRecoverConfirm"))) return;
    setDeploymentOperation("recovering");
    try {
      const result = await client.recoverProjectDeployment(projectId);
      toast.push({
        variant: result.warnings.length ? "warning" : "success",
        title: t("projectFrameDeploymentRecoveredTitle"),
        body: result.warnings.slice(0, 2).join("; ") || result.revision.revision_id,
      });
      await loadDiagnostics();
    } catch (err) {
      toast.push({ variant: "error", title: t("projectFrameDeploymentRecoveryFailedTitle"), body: errorMessage(err) });
      await loadDiagnostics().catch(() => {});
    } finally {
      setDeploymentOperation("idle");
    }
  }, [client, deploymentOperation, loadDiagnostics, projectId, t, toast]);

  const onRollbackDeployment = useCallback(async (revision: DeploymentRevision) => {
    if (deploymentOperation !== "idle") return;
    if (typeof window !== "undefined" && !window.confirm(t("projectFrameDeploymentRollbackConfirm", revision.revision_id))) return;
    setDeploymentOperation("rolling_back");
    try {
      const result = await client.rollbackProjectDeployment(projectId, revision.revision_id);
      toast.push({
        variant: result.warnings.length ? "warning" : "success",
        title: t("projectFrameDeploymentRolledBackTitle"),
        body: result.warnings.slice(0, 2).join("; ") || result.revision.revision_id,
      });
      await loadDiagnostics();
    } catch (err) {
      toast.push({ variant: "error", title: t("projectFrameDeploymentRollbackFailedTitle"), body: errorMessage(err) });
      await loadDiagnostics().catch(() => {});
    } finally {
      setDeploymentOperation("idle");
    }
  }, [client, deploymentOperation, loadDiagnostics, projectId, t, toast]);

  const onBuildDeploy = useCallback(async (descriptor: BuildDeployDescriptor) => {
    if (typeof window !== "undefined" && !window.confirm(t("projectFrameBuildDeployConfirm", descriptor.route_id))) return;
    try {
      const job = await client.buildDeployProject({
        project_id: projectId,
        source_url: descriptor.source_url,
        ref_name: descriptor.ref_name,
        strategy: descriptor.strategy,
        ...(descriptor.dockerfile ? { dockerfile: descriptor.dockerfile } : {}),
        container_port: descriptor.container_port,
        port_name: descriptor.port_name,
        route_id: descriptor.route_id,
        route_access: descriptor.route_access,
        ...(descriptor.health_path ? { health_path: descriptor.health_path } : {}),
        approved: true,
        runtime_env: descriptor.runtime_env,
        runtime_mounts: descriptor.runtime_mounts,
        idempotency_key: newDeploymentIdempotencyKey(),
      });
      setBuildDeployJob(job);
      setBuildDeployEvents([]);
      toast.push({ variant: "success", title: t("projectFrameBuildDeployStartedTitle"), body: t("projectFrameBuildDeployStartedBody", job.job_id) });
    } catch (err) {
      toast.push({ variant: "error", title: t("projectFrameBuildDeployFailedTitle"), body: errorMessage(err) });
    }
  }, [client, projectId, t, toast]);

  const onCancelBuildDeploy = useCallback(async (jobId: string) => {
    try {
      const result = await client.cancelBuildDeployJob(jobId);
      setBuildDeployJob((current) => current && current.job_id === jobId ? { ...current, state: result.state } : current);
      toast.push({ variant: "info", title: t("projectFrameBuildDeployCancelTitle"), body: jobId });
    } catch (err) {
      toast.push({ variant: "error", title: t("projectFrameBuildDeployFailedTitle"), body: errorMessage(err) });
    }
  }, [client, t, toast]);

  useEffect(() => {
    const jobId = buildDeployJob?.job_id;
    if (!jobId) return;
    let closed = false;
    const close = client.subscribeBuildDeployJob(jobId, (event) => {
      if (closed) return;
      setBuildDeployEvents((events) => [...events.filter((item) => item.sequence !== event.sequence), event].slice(-80));
      void client.getBuildDeployJob(jobId).then((status) => {
        if (!closed) {
          setBuildDeployJob(status);
          if (["ready", "failed", "cancelled"].includes(status.state)) void loadDiagnostics();
        }
      }).catch(() => {});
    });
    void client.getBuildDeployJob(jobId).then((status) => {
      if (!closed) setBuildDeployJob(status);
    }).catch(() => {});
    return () => { closed = true; close(); };
  }, [buildDeployJob?.job_id, client, loadDiagnostics]);

  const projectDiagnostics = diagnostics?.projectId === projectId ? diagnostics : null;
  const consoleSummary = useMemo(
    () => summarizeConsoleDiagnostics(projectDiagnostics, selectedTargetId),
    [projectDiagnostics, selectedTargetId],
  );

  const onSelectTarget = useCallback(
    (targetId: string) => {
      setSelectedTargetId(targetId);
      targetContextStore.set(projectId, targetId);
    },
    [projectId, targetContextStore],
  );
  const isStandalone = chrome === "none";
  const returnToShell = useCallback(() => {
    if (shouldReturnToShellHistory(window.history)) {
      window.history.back();
    } else {
      window.location.assign("/#/");
    }
  }, []);

  return (
    <div className={isStandalone ? "flex h-[100dvh] flex-col overflow-hidden bg-warm-bone" : "flex min-h-[calc(100dvh-60px)] flex-col"}>
      {isStandalone ? (
      <div className="plurora-mobile-project-bar flex shrink-0 items-center gap-2 border-b border-whisper-border bg-pure-surface md:hidden">
        <Button tone="icon" size="icon-sm" onClick={returnToShell} aria-label={t("projectFrameBackHome")}>
          <ArrowLeft size={16} />
        </Button>
        <span className="truncate font-display text-[13px] font-bold text-charcoal-ink">
          {project?.title ?? projectId}
        </span>
      </div>
      ) : (
      <div className="flex h-10 items-center justify-between border-b border-whisper-border bg-pure-surface px-3 sm:px-4">
        <div className="flex min-w-0 items-center gap-2 sm:gap-3">
          <Tooltip label={t("projectFrameBackHome")}>
            <Button tone="icon" size="icon-sm" onClick={() => navigate({ kind: "home" })} aria-label={t("projectFrameBackHome")}>
              <ArrowLeft size={16} />
            </Button>
          </Tooltip>
          <BookOpenText size={16} className="hidden text-aged-brass sm:inline" />
          <span className="truncate font-display text-[14px] font-bold leading-none text-charcoal-ink">
            {project?.title ?? projectId}
          </span>
          <StatusPill
            tone={projectStateTone(project?.state ?? "starting")}
            label={t("projectFrameState", project?.state ?? "starting")}
          />
        </div>
        <div className="flex items-center gap-1">
          <Tooltip label={t("projectFrameRefreshDiagnostics")}>
            <Button tone="tertiary" size="sm" onClick={onRefreshDiagnostics} disabled={refreshingDiagnostics}>
              <ArrowsClockwise size={14} />
              {refreshingDiagnostics ? t("projectFrameRefreshing") : t("projectFrameRefresh")}
            </Button>
          </Tooltip>
          <Button tone="tertiary" size="sm" onClick={onUpdateProject} disabled={updatingProject}>
            {updatingProject ? t("projectFrameUpdating") : t("projectFrameUpdateProject")}
          </Button>
          <span className="mx-2 hidden h-4 w-px bg-whisper-border sm:inline-block" aria-hidden />
          {project?.state === "running" ? (
            <Tooltip label={t("projectFrameStopProject")}>
              <Button tone="icon" size="icon-sm" onClick={onStop} disabled={stopping} aria-label={t("projectFrameStop")}>
                <StopCircle size={16} className="text-deep-rust" />
              </Button>
            </Tooltip>
          ) : null}
        </div>
      </div>
      )}

      <div className="relative min-h-0 flex-1 overflow-hidden">
        {isStandalone ? (
          <div
            ref={containerRef}
            id={FRAME_CONTAINER_ID}
            className="h-full w-full"
            style={{ background: "var(--color-warm-bone)" }}
          />
        ) : (
          <div className="h-full overflow-auto bg-warm-bone p-4 sm:p-6">
            <ProjectConsole
              projectId={projectId}
              project={project}
              diagnostics={projectDiagnostics}
              summary={consoleSummary}
              selectedTargetId={selectedTargetId}
              onSelectTarget={onSelectTarget}
              loading={refreshingDiagnostics && !projectDiagnostics}
              stopping={stopping}
              updating={updatingProject}
              onOpenProjectTab={onOpenProjectTab}
              onRefresh={onRefreshDiagnostics}
              onUpdate={onUpdateProject}
              onStop={onStop}
              deploymentOperation={deploymentOperation}
              onDeployDocker={onDeployDocker}
              onStopDockerDeployment={onStopDockerDeployment}
              buildDeployJob={buildDeployJob}
              buildDeployEvents={buildDeployEvents}
              onBuildDeploy={onBuildDeploy}
              onCancelBuildDeploy={onCancelBuildDeploy}
              onRecoverDeployment={onRecoverDeployment}
              onRollbackDeployment={onRollbackDeployment}
              onStopDeploymentRoute={onStopDeploymentRoute}
            />
          </div>
        )}
        {frameState === "loading" ? (
          <div className="pointer-events-none absolute inset-0 grid place-items-center bg-warm-bone">
            <div className="min-w-[280px] max-w-[420px] rounded-[24px] border border-whisper-border bg-pure-surface p-5 shadow-card">
              <p className="font-display text-[18px] font-bold text-charcoal-ink">{project?.title ?? projectId}</p>
              <p className="mt-2 text-[13px] text-steel-secondary">{t("projectFrameLoadingSurface")}</p>
            </div>
          </div>
        ) : null}
        {frameState === "start_failed" || frameState === "mount_failed" || frameState === "stopped" ? (
          <div className="absolute inset-0 grid place-items-center bg-warm-bone p-6">
            <div className="max-w-[460px] rounded-[24px] border border-whisper-border bg-pure-surface p-6 text-center shadow-card">
              <p className="font-display text-[20px] font-bold text-charcoal-ink">
                {frameState === "stopped"
                  ? t("projectFrameStoppedTitle")
                  : frameState === "start_failed"
                    ? t("projectFrameStartFailedTitle")
                    : t("projectFrameMountFailedTitle")}
              </p>
              <p className="mt-2 text-[13px] leading-relaxed text-steel-secondary">
                {frameState === "stopped"
                  ? t("projectFrameStoppedBody")
                  : frameState === "start_failed"
                    ? t("projectFrameStartFailedBody")
                    : t("projectFrameMountFailedBody")}
              </p>
              <div className="mt-5 flex flex-col justify-center gap-2 sm:flex-row">
                {frameState !== "stopped" ? (
                  <Button tone="primary" size="sm" onClick={onRetry}>
                    {t("retry")}
                  </Button>
                ) : null}
                {frameState === "mount_failed" && project?.state === "running" ? (
                  <Button tone="secondary" size="sm" onClick={onStop} disabled={stopping}>
                    {t("projectFrameStopProject")}
                  </Button>
                ) : null}
                <Button tone="tertiary" size="sm" onClick={() => navigate({ kind: "home" })}>
                  {t("projectFrameBackHome")}
                </Button>
              </div>
            </div>
          </div>
        ) : null}
      </div>
    </div>
  );
}

export function allowedSurfaceCapabilityIdsForTest(record: SurfaceContributionRecord | null): Set<string> {
  const ids = new Set<string>();
  const surface = record?.surface;
  if (surface?.capability_id) ids.add(surface.capability_id);
  if (surface?.activation.launch_capability_id) ids.add(surface.activation.launch_capability_id);
  for (const capabilityId of surface?.allowed_capability_ids ?? []) ids.add(capabilityId);
  return ids;
}

export function summarizeConsoleDiagnostics(
  diagnostics: ProjectDiagnostics | null,
  targetId?: string,
): ConsoleSummary {
  const packages = diagnostics?.packages ?? [];
  const updateResults = diagnostics?.updates?.results ?? [];
  const executions = (diagnostics?.executions ?? []).filter(
    (execution) => !targetId || execution.target_id === targetId,
  );
  const portLeases = (diagnostics?.portLeases ?? []).filter(
    (lease) => !targetId || lease.target_id === targetId,
  );
  const leaseIds = new Set(portLeases.map((lease) => lease.id));
  const proxyRoutes = (diagnostics?.proxyRoutes ?? []).filter(
    (route) => !targetId || leaseIds.has(route.upstream.port_lease_id),
  );
  return {
    packageTotal: packages.length,
    packageHealthy: packages.filter((pkg) => pkg.state === "ready" || pkg.state === "running").length,
    packageProblem: packages.filter((pkg) => pkg.state === "degraded" || pkg.state === "failed" || pkg.last_failure).length,
    recentEvents: diagnostics?.events.length ?? 0,
    updateAvailable: updateResults.filter((record) => record.available || UPDATE_AVAILABLE_STATUSES.has(record.status ?? "")).length,
    updateChecked: Boolean(diagnostics?.updates),
    targetTotal: (diagnostics?.targets ?? []).filter(
      (target) => !targetId || target.id === targetId,
    ).length,
    execTotal: executions.length,
    execRunning: executions.filter((execution) => execution.kind === "running").length,
    portActive: portLeases.filter((lease) => lease.status === "active").length,
    proxyActive: proxyRoutes.filter((route) => route.status === "active").length,
  };
}

export function targetOperationsForProject(
  operations: TargetOperationRecord[],
  projectId: string,
  targetId: string,
): TargetOperationRecord[] {
  return operations
    .filter((operation) => operation.project_id === projectId && operation.target_id === targetId)
    .sort((left, right) => right.updated_at_ms - left.updated_at_ms);
}

function diagnosticsForTarget(
  diagnostics: ProjectDiagnostics | null,
  targetId: string,
): ProjectDiagnostics | null {
  if (!diagnostics) return null;
  const portLeases = diagnostics.portLeases.filter((lease) => lease.target_id === targetId);
  const leaseIds = new Set(portLeases.map((lease) => lease.id));
  return {
    ...diagnostics,
    targets: diagnostics.targets.filter((target) => target.id === targetId),
    executions: diagnostics.executions.filter((execution) => execution.target_id === targetId),
    portLeases,
    proxyRoutes: diagnostics.proxyRoutes.filter((route) =>
      leaseIds.has(route.upstream.port_lease_id),
    ),
    operations: (diagnostics.operations ?? []).filter((operation) => operation.target_id === targetId),
  };
}

function ProjectConsole({
  projectId,
  project,
  diagnostics,
  summary,
  selectedTargetId,
  onSelectTarget,
  loading,
  stopping,
  updating,
  onOpenProjectTab,
  onRefresh,
  onUpdate,
  onStop,
  deploymentOperation,
  onDeployDocker,
  onStopDockerDeployment,
  buildDeployJob,
  buildDeployEvents,
  onBuildDeploy,
  onCancelBuildDeploy,
  onRecoverDeployment,
  onRollbackDeployment,
  onStopDeploymentRoute,
}: {
  projectId: string;
  project: (ProjectRecord & { running_session_id?: string; entry_surface_id?: string }) | null;
  diagnostics: ProjectDiagnostics | null;
  summary: ConsoleSummary;
  selectedTargetId: string;
  onSelectTarget: (targetId: string) => void;
  loading: boolean;
  stopping: boolean;
  updating: boolean;
  onOpenProjectTab: () => void;
  onRefresh: () => void;
  onUpdate: () => void;
  onStop: () => void;
  deploymentOperation: DeploymentUiOperation;
  onDeployDocker: (descriptor: DockerDeploymentDescriptor) => void;
  onStopDockerDeployment: (descriptor: DockerDeploymentDescriptor) => void;
  buildDeployJob: BuildDeployJobStatusResponse | BuildDeployJobSubmitResponse | null;
  buildDeployEvents: BuildDeployJobEvent[];
  onBuildDeploy: (descriptor: BuildDeployDescriptor) => void;
  onCancelBuildDeploy: (jobId: string) => void;
  onRecoverDeployment: () => void;
  onRollbackDeployment: (revision: DeploymentRevision) => void;
  onStopDeploymentRoute: (routeId: string) => void;
}) {
  const t = useT();
  const bundle = diagnostics?.bundle;
  const selectedDiagnostics = useMemo(
    () => diagnosticsForTarget(diagnostics, selectedTargetId),
    [diagnostics, selectedTargetId],
  );
  const deploymentParse = parseDockerDeploymentDescriptor(projectId, project?.metadata);
  const buildDeployParse = parseBuildDeployDescriptor(projectId, project?.metadata);
  const updateResults = diagnostics?.updates?.results ?? [];
  const updateStatus = loading
    ? t("projectFrameDiagnosticsLoading")
    : !summary.updateChecked
      ? t("projectFrameUpdateUnavailable")
      : summary.updateAvailable > 0
        ? t("projectFrameUpdatesAvailable", summary.updateAvailable)
        : t("projectFrameUpdatesCurrent");
  const projectStatusItems = [
    [t("projectFrameProjectId"), project?.id ?? projectId],
    [t("projectFrameProjectType"), formatProjectType(project?.type)],
    [t("projectFrameSession"), project?.running_session_id ? t("projectFrameActiveSession") : "—"],
    [t("projectFrameStorage"), project?.storage_summary ? formatBytes(project.storage_summary.total_bytes ?? undefined) : "—"],
  ];
  const deploymentSummary = t(
    "projectFrameDeploymentSummary",
    summary.targetTotal,
    summary.execTotal,
    summary.execRunning,
    summary.portActive,
    summary.proxyActive,
  );

  if (loading && !diagnostics) return <ProjectConsoleSkeleton />;

  return (
    <div className="mx-auto max-w-6xl space-y-5">
      <section className="rounded-[24px] border border-whisper-border bg-pure-surface p-5 shadow-card sm:p-6">
        <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
          <div>
            <p className="font-display text-[22px] font-bold text-charcoal-ink">{t("projectFrameConsoleTitle")}</p>
            <label className="mt-3 flex items-center gap-2 text-[12px] text-steel-secondary">
              <span>{t("accessTargetResources")}</span>
              <select
                value={selectedTargetId}
                onChange={(event) => onSelectTarget(event.target.value)}
                className="max-w-[260px] rounded-[9px] border border-whisper-border bg-pure-surface px-2 py-1.5 font-mono text-[12px] text-charcoal-ink"
              >
                {(diagnostics?.targets ?? []).map((target) => (
                  <option key={target.id} value={target.id}>
                    {target.name} · {target.id} · {target.status}
                  </option>
                ))}
              </select>
            </label>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <Button tone="primary" size="sm" onClick={onOpenProjectTab}>{t("projectFrameOpenProjectTab")}</Button>
            <Button tone="secondary" size="sm" onClick={onRefresh} disabled={loading}>
              <ArrowsClockwise size={14} />
              {loading ? t("projectFrameRefreshing") : t("projectFrameRefresh")}
            </Button>
            <Button tone="secondary" size="sm" onClick={onUpdate} disabled={updating}>{updating ? t("projectFrameUpdating") : t("projectFrameUpdateProject")}</Button>
            {project?.state === "running" ? <Button tone="destructive" size="sm" onClick={onStop} disabled={stopping}>{t("projectFrameStopProject")}</Button> : null}
          </div>
        </div>

        <div className="mt-5 grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
          <MetricCard label={t("projectFramePackages") } value={t("projectFramePackageHealth", summary.packageHealthy, summary.packageTotal)} warn={summary.packageProblem > 0} />
          <MetricCard label={t("projectFrameUpdates") } value={updateStatus} warn={summary.updateAvailable > 0} />
          <MetricCard label={t("projectFrameDeploymentMetric") } value={deploymentSummary} warn={summary.execRunning === 0 && summary.execTotal > 0} />
        </div>

        <dl className="mt-5 grid gap-3 text-[12px] sm:grid-cols-2 xl:grid-cols-4">
          {projectStatusItems.map(([label, value]) => <KeyValue key={label} label={label} value={value} />)}
        </dl>
      </section>

      <div className="grid gap-5 xl:grid-cols-[1.05fr_0.95fr]">
        <ConsoleSection title={t("projectFrameInterfaceSection")} description={t("projectFrameInterfaceDescription")}>
          <div className="space-y-3">
            <KeyValue label={t("projectFrameEntrySurface")} value={project?.entry_surface_id ?? "—"} />
            <KeyValue label={t("projectFrameBundleUrl")} value={bundle?.bundleUrl ?? t("projectFrameBundleUnavailable")} />
            <KeyValue label={t("projectFrameBundleFingerprint")} value={bundle?.bundleFingerprint ?? fingerprintFromUrl(bundle?.bundleUrl) ?? "—"} />
            <KeyValue label={t("projectFrameLastResolved")} value={diagnostics?.refreshedAt ? new Date(diagnostics.refreshedAt).toLocaleString() : "—"} />
          </div>
        </ConsoleSection>

        <ConsoleSection title={t("projectFrameUpdatesSection")} description={t("projectFrameUpdatesDescription")}>
          <div className="space-y-3">
            <StatusPill tone={summary.updateAvailable > 0 ? "update" : "neutral"} label={updateStatus} />
            {updateResults.length === 0 ? <p className="text-[12px] text-steel-secondary">{summary.updateChecked ? t("projectFrameNoUpdateRecords") : t("projectFrameUpdateUnavailable")}</p> : null}
            {updateResults.slice(0, 4).map((record) => (
              <div key={record.package_id ?? record.id ?? record.reason} className="rounded-[12px] border border-whisper-border bg-warm-bone p-3">
                <p className="truncate font-mono text-[12px] text-charcoal-ink">{record.package_id ?? record.id ?? "—"}</p>
                <p className="mt-1 text-[12px] text-steel-secondary">{record.reason ?? record.status ?? "—"}</p>
              </div>
            ))}
          </div>
        </ConsoleSection>
      </div>

      <ConsoleSection title={t("projectFrameDevelopmentSection")} description={t("projectFrameDevelopmentDescription")}>
        <DevelopmentWorkflowCard
          key={`${projectId}:${selectedTargetId}`}
          projectId={projectId}
          targetId={selectedTargetId}
          deploymentDefaults={deploymentParse.descriptor ?? undefined}
          activeRevision={diagnostics?.deployments?.active_revision}
          onDeploymentChanged={onRefresh}
        />
      </ConsoleSection>

      <ConsoleSection title={t("projectFrameDeploymentSection")} description={t("projectFrameDeploymentDescription")}>
        {selectedTargetId === "local" && (deploymentParse.descriptor || deploymentParse.error) ? (
          <DeploymentActionCard
            descriptor={deploymentParse.descriptor}
            error={deploymentParse.error}
            diagnostics={selectedDiagnostics}
            operation={deploymentOperation}
            onDeploy={onDeployDocker}
            onStop={onStopDockerDeployment}
          />
        ) : null}
        {selectedTargetId === "local" && (buildDeployParse.descriptor || buildDeployParse.error) ? (
          <BuildDeployActionCard
            descriptor={buildDeployParse.descriptor}
            error={buildDeployParse.error}
            job={buildDeployJob}
            events={buildDeployEvents}
            onBuildDeploy={onBuildDeploy}
            onCancel={onCancelBuildDeploy}
          />
        ) : null}
        <DeploymentRevisionHistory
          deployments={diagnostics?.deployments}
          targetId={selectedTargetId}
          operation={deploymentOperation}
          onRecover={onRecoverDeployment}
          onRollback={onRollbackDeployment}
          onStop={onStopDeploymentRoute}
        />
        {selectedTargetId !== "local" ? (
          <p className="rounded-[12px] border border-aged-brass/30 bg-aged-brass/10 p-3 text-[12px] text-steel-secondary">
            {t("projectFrameRemoteTargetOperationsOnly")}
          </p>
        ) : null}
        <DeploymentDiagnostics key={`${projectId}:${selectedTargetId}`} diagnostics={selectedDiagnostics} targetId={selectedTargetId} />
      </ConsoleSection>

      <ConsoleSection title={t("projectFramePackagesSection")} description={t("projectFramePackagesDescription")}>
        {diagnostics?.packages.length ? (
          <div className="grid gap-3 lg:grid-cols-2">
            {diagnostics.packages.map((pkg) => <PackageDiagnosticCard key={pkg.id} pkg={pkg} />)}
          </div>
        ) : <p className="text-[13px] text-steel-secondary">{loading ? t("projectFrameDiagnosticsLoading") : t("projectFrameNoPackages")}</p>}
      </ConsoleSection>

      <ConsoleSection title={t("projectFrameActivitySection")} description={t("projectFrameActivityDescription")}>
        {diagnostics?.events.length ? (
          <div className="space-y-2">
            {diagnostics.events.map((event) => (
              <div key={event.id} className="flex gap-3 rounded-[12px] border border-whisper-border bg-warm-bone p-3 text-[12px]">
                <span className="font-mono text-muted-tone">#{event.sequence}</span>
                <div className="min-w-0 flex-1">
                  <p className="truncate text-charcoal-ink">{humanEventKind(event.kind)}</p>
                  <p className="truncate text-steel-secondary">{event.writer_package_id} · {formatEventTime(event.created_at)}</p>
                </div>
              </div>
            ))}
          </div>
        ) : <p className="text-[13px] text-steel-secondary">{project?.running_session_id ? t("projectFrameNoEvents") : t("projectFrameNoSession")}</p>}
      </ConsoleSection>

      {diagnostics?.errors.length ? (
        <ConsoleSection title={t("projectFrameDiagnosticsWarnings")}>
          <ul className="space-y-2 text-[12px] text-deep-rust">
            {diagnostics.errors.slice(0, 4).map((error) => <li key={error}>{error}</li>)}
          </ul>
        </ConsoleSection>
      ) : null}
    </div>
  );
}

function MetricCard({ label, value, warn = false }: { label: string; value: string; warn?: boolean }) {
  return <div className="rounded-[16px] border border-whisper-border bg-warm-bone p-4"><p className="font-mono text-[10px] uppercase tracking-[0.12em] text-muted-tone">{label}</p><p className={warn ? "mt-2 text-[14px] font-semibold text-deep-rust" : "mt-2 text-[14px] font-semibold text-charcoal-ink"}>{value}</p></div>;
}

function ProjectConsoleSkeleton() {
  return (
    <div className="mx-auto max-w-6xl space-y-5">
      <section className="rounded-[24px] border border-whisper-border bg-pure-surface p-5 shadow-card sm:p-6">
        <div className="h-6 w-48 rounded-full bg-whisper-border" />
        <div className="mt-5 grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
          {Array.from({ length: 3 }).map((_, index) => <div key={index} className="h-20 rounded-[16px] bg-warm-bone" />)}
        </div>
      </section>
      <div className="grid gap-5 xl:grid-cols-2">
        <div className="h-56 rounded-[20px] border border-whisper-border bg-pure-surface shadow-card" />
        <div className="h-56 rounded-[20px] border border-whisper-border bg-pure-surface shadow-card" />
      </div>
      <div className="h-72 rounded-[20px] border border-whisper-border bg-pure-surface shadow-card" />
      <div className="h-64 rounded-[20px] border border-whisper-border bg-pure-surface shadow-card" />
    </div>
  );
}

function ConsoleSection({ title, description, children }: { title: string; description?: string; children: ReactNode }) {
  return <section className="rounded-[20px] border border-whisper-border bg-pure-surface p-5 shadow-card"><p className="font-display text-[18px] font-bold text-charcoal-ink">{title}</p>{description ? <p className="mt-1 text-[12px] leading-relaxed text-steel-secondary">{description}</p> : null}<div className="mt-4">{children}</div></section>;
}

function KeyValue({ label, value }: { label: string; value: string }) {
  return <div className="min-w-0 rounded-[12px] border border-whisper-border bg-warm-bone p-3"><dt className="font-mono text-[10px] uppercase tracking-[0.12em] text-muted-tone">{label}</dt><dd className="mt-1 truncate font-mono text-[12px] text-charcoal-ink" title={value}>{value}</dd></div>;
}

function DeploymentActionCard({
  descriptor,
  error,
  diagnostics,
  operation,
  onDeploy,
  onStop,
}: {
  descriptor: DockerDeploymentDescriptor | null;
  error?: string;
  diagnostics: ProjectDiagnostics | null;
  operation: DeploymentUiOperation;
  onDeploy: (descriptor: DockerDeploymentDescriptor) => void;
  onStop: (descriptor: DockerDeploymentDescriptor) => void;
}) {
  const t = useT();
  const [routeAccess, setRouteAccess] = useState<DockerDeploymentDescriptor["route_access"]>(descriptor?.route_access ?? "host_authenticated");

  useEffect(() => {
    setRouteAccess(descriptor?.route_access ?? "host_authenticated");
  }, [descriptor?.route_access, descriptor?.route_id]);

  if (!descriptor) {
    return (
      <div className="mb-4 rounded-[14px] border border-deep-rust bg-deep-rust-surface p-4 text-[12px] text-deep-rust">
        <p className="font-semibold">{t("projectFrameDeployInvalidTitle")}</p>
        <p className="mt-1">{error}</p>
      </div>
    );
  }

  const activeRoute = diagnostics?.proxyRoutes.find((route) => route.id === descriptor.route_id && route.status === "active");
  const activePort = diagnostics?.portLeases.find((lease) =>
    lease.status === "active" && (lease.id === activeRoute?.upstream.port_lease_id || lease.port_name === descriptor.port_name),
  );
  const hasActiveDeployment = Boolean(activeRoute || activePort);
  const deployDisabled = operation !== "idle" || hasActiveDeployment;
  const stopDisabled = operation !== "idle" || !hasActiveDeployment;

  return (
    <div className="mb-4 rounded-[16px] border border-aged-brass/40 bg-warm-bone p-4">
      <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
        <div>
          <p className="font-display text-[15px] font-bold text-charcoal-ink">{t("projectFrameDeployActionTitle")}</p>
          <p className="mt-1 text-[12px] text-steel-secondary">{t("projectFrameDeployActionDescription")}</p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button tone="primary" size="sm" onClick={() => onDeploy({ ...descriptor, route_access: routeAccess })} disabled={deployDisabled}>
            {operation === "deploying" ? t("projectFrameDeploying") : t("projectFrameDeploy")}
          </Button>
          {hasActiveDeployment ? (
            <Button tone="destructive" size="sm" onClick={() => onStop(descriptor)} disabled={stopDisabled}>
              {operation === "stopping" ? t("projectFrameStoppingDeployment") : t("projectFrameStopDeployment")}
            </Button>
          ) : null}
        </div>
      </div>
      <RouteAccessControl value={routeAccess} onChange={setRouteAccess} disabled={operation !== "idle" || hasActiveDeployment} />
      <dl className="mt-4 grid gap-2 text-[12px] sm:grid-cols-2 xl:grid-cols-4">
        <TinyValue label={t("projectFrameDeployImage")} value={descriptor.image} />
        <TinyValue label={t("projectFrameDeployContainerPort")} value={String(descriptor.container_port)} />
        <TinyValue label={t("projectFrameDeployRouteId")} value={descriptor.route_id} />
        <TinyValue label={t("projectFrameDeployPortName")} value={descriptor.port_name} />
        {descriptor.health_path ? <TinyValue label={t("projectFrameDeployHealthPath")} value={descriptor.health_path} /> : null}
      </dl>
      {descriptor.pull_if_missing ? <p className="mt-3 rounded-[10px] bg-pure-surface p-2 text-[12px] text-deep-rust">{t("projectFrameDeployPullWarning")}</p> : null}
      {hasActiveDeployment ? <p className="mt-3 text-[12px] text-steel-secondary">{t("projectFrameDeployActiveHint")}</p> : null}
    </div>
  );
}

function BuildDeployActionCard({
  descriptor,
  error,
  job,
  events,
  onBuildDeploy,
  onCancel,
}: {
  descriptor: BuildDeployDescriptor | null;
  error?: string;
  job: BuildDeployJobStatusResponse | BuildDeployJobSubmitResponse | null;
  events: BuildDeployJobEvent[];
  onBuildDeploy: (descriptor: BuildDeployDescriptor) => void;
  onCancel: (jobId: string) => void;
}) {
  const t = useT();
  const [strategy, setStrategy] = useState<"dockerfile" | "nixpacks">(descriptor?.strategy ?? "dockerfile");
  const [routeAccess, setRouteAccess] = useState<BuildDeployDescriptor["route_access"]>(descriptor?.route_access ?? "host_authenticated");
  const [mountApprovals, setMountApprovals] = useState<Record<number, boolean>>({});
  const [riskApprovals, setRiskApprovals] = useState<Record<number, boolean>>({});
  const [showConfig, setShowConfig] = useState(false);
  const [confirmCancel, setConfirmCancel] = useState(false);

  useEffect(() => {
    setStrategy(descriptor?.strategy ?? "dockerfile");
    setRouteAccess(descriptor?.route_access ?? "host_authenticated");
    setMountApprovals({});
    setRiskApprovals({});
  }, [descriptor?.route_access, descriptor?.route_id, descriptor?.strategy]);

  if (!descriptor) {
    return <div className="mb-4 rounded-[14px] border border-deep-rust bg-deep-rust-surface p-4 text-[12px] text-deep-rust"><p className="font-semibold">{t("projectFrameBuildDeployInvalidTitle")}</p><p className="mt-1">{error}</p></div>;
  }

  const mounts = descriptor.runtime_mounts.map((mount, index) => ({
    ...mount,
    approved: mount.approved || mountApprovals[index] === true,
    high_risk_approved: mount.mode !== "rw" ? mount.high_risk_approved : mount.high_risk_approved || riskApprovals[index] === true,
  }));
  const approvalsReady = mounts.every((mount) => mount.approved && (mount.mode !== "rw" || mount.high_risk_approved));
  const isRunning = job ? !["ready", "failed", "cancelled"].includes(job.state) : false;
  const isResolved = job ? ["ready", "failed", "cancelled"].includes(job.state) : false;
  const activeJobId = job?.job_id;
  const status = job && "result" in job ? job : null;
  const submitDescriptor = { ...descriptor, strategy, route_access: routeAccess, runtime_mounts: mounts };
  const mode = isRunning ? "running" : isResolved ? "resolved" : "idle";
  const showIdleConfig = mode === "idle" || (mode === "resolved" && showConfig);

  return (
    <div className="mb-4 rounded-[16px] border border-aged-brass/40 bg-warm-bone p-4">
      <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
        <div>
          <p className="font-display text-[15px] font-bold text-charcoal-ink">{t("projectFrameBuildDeployTitle")}</p>
          <p className="mt-1 text-[12px] text-steel-secondary">{t("projectFrameBuildDeployDescription")}</p>
        </div>
        <div className="flex flex-wrap gap-2">
          {mode === "resolved" ? <Button tone="secondary" size="sm" onClick={() => setShowConfig((value) => !value)}>{t("projectFrameBuildDeployEditConfig")}</Button> : null}
          {mode !== "running" ? <Button tone="primary" size="sm" disabled={!approvalsReady} onClick={() => onBuildDeploy(submitDescriptor)}>{mode === "resolved" ? t("projectFrameBuildDeployNewBuild") : t("projectFrameBuildDeployButton")}</Button> : null}
        </div>
      </div>

      {showIdleConfig ? (
        <>
          <RouteAccessControl value={routeAccess} onChange={setRouteAccess} />
          <BuildDeployConfigPanel descriptor={descriptor} strategy={strategy} setStrategy={setStrategy} mountApprovals={mountApprovals} riskApprovals={riskApprovals} setMountApprovals={setMountApprovals} setRiskApprovals={setRiskApprovals} />
        </>
      ) : null}

      {mode === "running" && activeJobId ? <div className="mt-4 space-y-4"><JobStatusTracker state={job.state} /><TerminalLogPanel events={events} live onCopy={() => void navigator.clipboard?.writeText(events.map((event) => `#${event.sequence} [${event.state}] ${event.message}`).join("\n"))} /><div className="flex justify-end"><Button tone="destructive" size="sm" onClick={() => setConfirmCancel(true)}>{t("projectFrameBuildDeployCancel")}</Button></div></div> : null}

      {mode === "resolved" && job ? <BuildDeployResultCard job={job} status={status} events={events} onRetry={() => onBuildDeploy(submitDescriptor)} onEdit={() => setShowConfig(true)} /> : null}

      <Modal open={confirmCancel} onOpenChange={setConfirmCancel} accent="rust" size="sm" contentLabel={t("projectFrameBuildDeployCancel")}>
        <ModalHeader title={t("projectFrameBuildDeployCancelConfirmTitle")} description={t("projectFrameBuildDeployCancelConfirmBody")} />
        <ModalFooter className="justify-end">
          <Button tone="secondary" size="sm" onClick={() => setConfirmCancel(false)}>{t("back")}</Button>
          <Button tone="destructive" size="sm" onClick={() => { if (activeJobId) onCancel(activeJobId); setConfirmCancel(false); }}>{t("projectFrameBuildDeployCancel")}</Button>
        </ModalFooter>
      </Modal>
    </div>
  );
}

function DeploymentRevisionHistory({
  deployments,
  targetId,
  operation,
  onRecover,
  onRollback,
  onStop,
}: {
  deployments?: ProjectDeploymentsResponse;
  targetId: string;
  operation: DeploymentUiOperation;
  onRecover: () => void;
  onRollback: (revision: DeploymentRevision) => void;
  onStop: (routeId: string) => void;
}) {
  const t = useT();
  const active = deployments?.active_revision?.target_id === targetId ? deployments.active_revision : null;
  const revisions = (deployments?.revisions ?? []).filter((revision) => revision.target_id === targetId);
  const jobs = targetId === "local" ? deployments?.jobs ?? [] : [];
  const busy = operation !== "idle";

  return (
    <div className="mb-4 rounded-[16px] border border-whisper-border bg-pure-surface p-4">
      <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
        <div>
          <p className="font-display text-[15px] font-bold text-charcoal-ink">{t("projectFrameDeploymentHistoryTitle")}</p>
          <p className="mt-1 text-[12px] text-steel-secondary">{t("projectFrameDeploymentHistoryDescription")}</p>
        </div>
        {active ? (
          <div className="flex flex-wrap gap-2">
            {deployments?.recovery_required ? (
              <Button tone="primary" size="sm" disabled={busy || !active.recoverable} onClick={onRecover}>
                {operation === "recovering" ? t("projectFrameDeploymentRecovering") : t("projectFrameDeploymentRecover")}
              </Button>
            ) : null}
            {targetId === "local" && active.source_kind === "git_clone" ? (
              <Button tone="destructive" size="sm" disabled={busy} onClick={() => onStop(active.route_id)}>
                {operation === "stopping" ? t("projectFrameStoppingDeployment") : t("projectFrameStopDeployment")}
              </Button>
            ) : null}
          </div>
        ) : null}
      </div>

      {active ? (
        <div className="mt-4 rounded-[14px] border border-aged-brass/30 bg-warm-bone p-4">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div>
              <p className="font-mono text-[10px] uppercase tracking-[0.12em] text-muted-tone">{t("projectFrameDeploymentActiveRevision")}</p>
              <p className="mt-1 font-mono text-[12px] text-charcoal-ink">{active.revision_id}</p>
              <div className="mt-2 flex flex-wrap gap-1">
                <span className="rounded-full border border-whisper-border bg-pure-surface px-2 py-0.5 font-mono text-[10px] text-steel-secondary">{active.target_id}</span>
                <span className="rounded-full border border-whisper-border bg-pure-surface px-2 py-0.5 font-mono text-[10px] text-steel-secondary">{active.source_kind.replace("_", " ")}</span>
              </div>
            </div>
            <StatusPill
              tone={deployments?.runtime_ready ? "running" : "failed"}
              label={deployments?.runtime_ready ? t("projectFrameStatusReady") : t("projectFrameDeploymentRecoveryRequired")}
            />
          </div>
          <dl className="mt-3 grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
            <TinyValue label={t("projectFrameDeployRouteId")} value={active.route_id} />
            <TinyValue label={t("projectFrameDeployImage")} value={active.image} />
            <TinyValue label={t("projectFrameBuildDeployStrategy")} value={active.strategy} />
            <TinyValue label={t("projectFrameRouteExposure")} value={active.route_access === "public" ? t("projectFrameRoutePublic") : t("projectFrameRoutePrivate")} />
            <TinyValue label={t("projectFrameDeploymentCreatedAt")} value={new Date(active.created_at_ms).toLocaleString()} />
            <TinyValue label={t("projectFrameDeploymentSourceCommit")} value={active.source_commit || "—"} />
            <TinyValue label={t("projectFrameDeploymentBuildDigest")} value={active.build_descriptor_hash || "—"} />
            <TinyValue label={t("projectFrameDeploymentParentRevision")} value={active.parent_revision_id ?? "—"} />
          </dl>
          {!active.recoverable ? <p className="mt-3 rounded-[10px] bg-deep-rust-surface p-2 text-[12px] text-deep-rust">{t("projectFrameDeploymentNotRecoverable")}: {active.recovery_blockers.join("; ")}</p> : null}
        </div>
      ) : <p className="mt-4 text-[12px] text-steel-secondary">{t("projectFrameDeploymentNoActiveOnTarget", targetId)}</p>}

      {revisions.length ? (
        <div className="mt-4">
          <p className="text-[12px] font-semibold text-charcoal-ink">{t("projectFrameDeploymentRevisionHistory")}</p>
          <div className="mt-2 space-y-2">
            {revisions.slice(0, 8).map((revision) => {
              const isActive = revision.revision_id === active?.revision_id;
              return (
                <div key={revision.revision_id} className="flex flex-col gap-3 rounded-[12px] border border-whisper-border bg-warm-bone p-3 sm:flex-row sm:items-center sm:justify-between">
                  <div className="min-w-0">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="font-mono text-[12px] text-charcoal-ink">{revision.revision_id}</span>
                      <span className="rounded-full border border-whisper-border bg-pure-surface px-2 py-0.5 font-mono text-[10px] uppercase text-steel-secondary">{revision.operation.replace("_", " ")}</span>
                      <span className="rounded-full border border-whisper-border bg-pure-surface px-2 py-0.5 font-mono text-[10px] text-steel-secondary">{revision.target_id}</span>
                      <span className="rounded-full border border-whisper-border bg-pure-surface px-2 py-0.5 font-mono text-[10px] text-steel-secondary">{revision.source_kind.replace("_", " ")}</span>
                      {isActive ? <StatusPill tone="accent" label={t("projectFrameDeploymentActiveRevision")} showDot={false} /> : null}
                    </div>
                    <p className="mt-1 truncate text-[11px] text-steel-secondary" title={revision.image}>{revision.image} · {revision.route_access === "public" ? t("projectFrameRoutePublic") : t("projectFrameRoutePrivate")} · {new Date(revision.created_at_ms).toLocaleString()}</p>
                    <p className="mt-1 truncate font-mono text-[10px] text-muted-tone" title={`${revision.source_commit} · ${revision.build_descriptor_hash}`}>
                      {revision.source_commit || "—"} · {revision.build_descriptor_hash || "—"}
                    </p>
                  </div>
                  {!isActive ? (
                    <Button tone="secondary" size="sm" disabled={busy || !revision.recoverable} onClick={() => onRollback(revision)}>
                      {operation === "rolling_back" ? t("projectFrameDeploymentRollingBack") : t("projectFrameDeploymentRollback")}
                    </Button>
                  ) : null}
                </div>
              );
            })}
          </div>
        </div>
      ) : null}

      {jobs.length ? (
        <div className="mt-4">
          <p className="text-[12px] font-semibold text-charcoal-ink">{t("projectFrameDeploymentJobHistory")}</p>
          <div className="mt-2 grid gap-2 lg:grid-cols-2">
            {jobs.slice(0, 6).map((job) => (
              <div key={job.job_id} className="flex items-start justify-between gap-3 rounded-[12px] border border-whisper-border bg-warm-bone p-3">
                <div className="min-w-0"><p className="truncate font-mono text-[11px] text-charcoal-ink">{job.job_id}</p><p className="mt-1 text-[11px] text-steel-secondary">{humanEventKind(job.operation)} · {new Date(job.updated_at_ms).toLocaleString()}</p>{job.error ? <p className="mt-1 text-[11px] text-deep-rust">{job.error}</p> : null}</div>
                <StatusPill tone={job.state === "ready" ? "stopped" : job.state === "failed" || job.state === "cancelled" ? "failed" : "starting"} label={job.state} />
              </div>
            ))}
          </div>
        </div>
      ) : null}
    </div>
  );
}

function RouteAccessControl({
  value,
  onChange,
  disabled = false,
}: {
  value: DockerDeploymentDescriptor["route_access"];
  onChange: (value: DockerDeploymentDescriptor["route_access"]) => void;
  disabled?: boolean;
}) {
  const t = useT();
  const controlId = useId();
  const options = [
    {
      value: "host_authenticated" as const,
      title: t("projectFrameRoutePrivate"),
      body: t("projectFrameRoutePrivateBody"),
    },
    {
      value: "public" as const,
      title: t("projectFrameRoutePublic"),
      body: t("projectFrameRoutePublicBody"),
    },
  ];

  return (
    <fieldset className="mt-4 rounded-[14px] border border-whisper-border bg-pure-surface p-3" disabled={disabled}>
      <legend className="px-1 font-mono text-[10px] uppercase tracking-[0.12em] text-muted-tone">{t("projectFrameRouteExposure")}</legend>
      <div className="grid gap-2 sm:grid-cols-2">
        {options.map((option) => (
          <label
            key={option.value}
            className={option.value === value
              ? "flex cursor-pointer gap-3 rounded-[12px] border border-aged-brass bg-aged-brass/10 p-3"
              : "flex cursor-pointer gap-3 rounded-[12px] border border-whisper-border bg-warm-bone p-3"}
          >
            <input
              type="radio"
              name={`${controlId}-route-access`}
              checked={option.value === value}
              onChange={() => onChange(option.value)}
              disabled={disabled}
            />
            <span>
              <span className="block text-[12px] font-semibold text-charcoal-ink">{option.title}</span>
              <span className="mt-1 block text-[11px] leading-relaxed text-steel-secondary">{option.body}</span>
            </span>
          </label>
        ))}
      </div>
      {value === "public" ? <p className="mt-2 rounded-[10px] bg-deep-rust-surface p-2 text-[11px] text-deep-rust">{t("projectFrameRoutePublicWarning")}</p> : null}
    </fieldset>
  );
}

function BuildDeployConfigPanel({ descriptor, strategy, setStrategy, mountApprovals, riskApprovals, setMountApprovals, setRiskApprovals }: { descriptor: BuildDeployDescriptor; strategy: "dockerfile" | "nixpacks"; setStrategy: (strategy: "dockerfile" | "nixpacks") => void; mountApprovals: Record<number, boolean>; riskApprovals: Record<number, boolean>; setMountApprovals: React.Dispatch<React.SetStateAction<Record<number, boolean>>>; setRiskApprovals: React.Dispatch<React.SetStateAction<Record<number, boolean>>> }) {
  const t = useT();
  return <div className="mt-4 space-y-4"><div className="grid gap-3 lg:grid-cols-2"><div className="rounded-[12px] border border-whisper-border bg-pure-surface p-3 text-[12px]"><p className="font-mono uppercase tracking-[0.12em] text-muted-tone">{t("projectFrameBuildDeployStrategy")}</p><div className="mt-2 flex gap-2">{(["dockerfile", "nixpacks"] as const).map((item) => <button key={item} type="button" onClick={() => setStrategy(item)} className={item === strategy ? "rounded-full bg-charcoal-ink px-3 py-1.5 text-[12px] font-semibold text-pure-surface" : "rounded-full border border-whisper-border bg-warm-bone px-3 py-1.5 text-[12px] font-semibold text-steel-secondary hover:text-charcoal-ink"}>{item === "dockerfile" ? "Dockerfile" : "Nixpacks"}</button>)}</div></div><TinyValue label={t("projectFrameBuildDeploySource")} value={descriptor.source_url.replace(/^https:\/\//, "")} /><TinyValue label={t("projectFrameBuildDeployRef")} value={descriptor.ref_name} /><TinyValue label={t("projectFrameDeployRouteId")} value={descriptor.route_id} /></div>{descriptor.runtime_env.length ? <div><p className="text-[12px] font-semibold text-charcoal-ink">{t("projectFrameBuildDeployEnv")}</p><div className="mt-2 flex flex-wrap gap-2">{descriptor.runtime_env.map((env) => <span key={env.name} className="rounded-full border border-whisper-border bg-pure-surface px-3 py-1 text-[12px] text-steel-secondary">{env.name} · {env.secret_ref ? t("projectFrameBuildDeploySecretRef") : t("projectFrameBuildDeployPlainEnv")}</span>)}</div></div> : null}{descriptor.runtime_mounts.length ? <div className="space-y-2"><p className="text-[12px] font-semibold text-charcoal-ink">{t("projectFrameBuildDeployMounts")}</p>{descriptor.runtime_mounts.map((mount, index) => <VolumeApprovalRow key={`${mount.container_path}-${index}`} mount={mount} index={index} mountApprovals={mountApprovals} riskApprovals={riskApprovals} setMountApprovals={setMountApprovals} setRiskApprovals={setRiskApprovals} />)}</div> : null}</div>;
}

function VolumeApprovalRow({ mount, index, mountApprovals, riskApprovals, setMountApprovals, setRiskApprovals }: { mount: BuildDeployDescriptor["runtime_mounts"][number]; index: number; mountApprovals: Record<number, boolean>; riskApprovals: Record<number, boolean>; setMountApprovals: React.Dispatch<React.SetStateAction<Record<number, boolean>>>; setRiskApprovals: React.Dispatch<React.SetStateAction<Record<number, boolean>>> }) {
  const t = useT();
  return <div className="rounded-[12px] border border-whisper-border bg-pure-surface p-3 text-[12px]"><div className="flex flex-wrap items-center justify-between gap-2"><span className="font-mono text-charcoal-ink">{basename(mount.source_host_path)} → {mount.container_path}</span><span className={mount.mode === "rw" ? "rounded-full bg-deep-rust-surface px-2 py-0.5 font-semibold text-deep-rust" : "rounded-full bg-warm-bone px-2 py-0.5 text-steel-secondary"}>{mount.mode === "rw" ? t("projectFrameBuildDeployHighRiskBadge") : "RO"}</span></div><p className="mt-1 text-steel-secondary">{mount.reason}</p><label className="mt-2 flex items-center gap-2 text-steel-secondary"><input type="checkbox" checked={mount.approved || mountApprovals[index] === true} onChange={(event) => setMountApprovals((value) => ({ ...value, [index]: event.target.checked }))} />{t("projectFrameBuildDeployApproveMount")}</label>{mount.mode === "rw" ? <label className="mt-1 flex items-center gap-2 text-deep-rust"><input type="checkbox" checked={mount.high_risk_approved || riskApprovals[index] === true} onChange={(event) => setRiskApprovals((value) => ({ ...value, [index]: event.target.checked }))} />{t("projectFrameBuildDeployApproveRisk")}</label> : null}</div>;
}

const BUILD_JOB_PHASES = ["queued", "cloning", "building", "starting", "registering_proxy", "probing", "ready"];

function JobStatusTracker({ state }: { state: string }) {
  const t = useT();
  const current = state === "failed" || state === "cancelled" ? BUILD_JOB_PHASES.length - 1 : Math.max(0, BUILD_JOB_PHASES.indexOf(state));
  return <div className="rounded-[14px] border border-aged-brass/30 bg-pure-surface p-4"><p className="font-display text-[14px] font-bold text-charcoal-ink">{t("projectFrameBuildDeployTracker")}</p><div className="mt-3 grid gap-2 sm:grid-cols-4 lg:grid-cols-7">{BUILD_JOB_PHASES.map((phase, index) => <div key={phase} className={index <= current ? "rounded-[10px] border border-aged-brass bg-aged-brass/10 p-2" : "rounded-[10px] border border-whisper-border bg-warm-bone p-2 opacity-70"}><span className={index <= current ? "block size-2 rounded-full bg-aged-brass" : "block size-2 rounded-full bg-muted-tone"} /><p className="mt-2 text-[11px] font-semibold text-charcoal-ink">{phase.replace("_", " ")}</p></div>)}</div></div>;
}

function TerminalLogPanel({ events, live, onCopy }: { events: BuildDeployJobEvent[]; live: boolean; onCopy: () => void }) {
  const t = useT();
  return <div className="overflow-hidden rounded-[14px] border border-charcoal-ink/20 bg-[#201a15]"><div className="flex items-center justify-between border-b border-white/10 px-3 py-2"><span className="font-mono text-[11px] uppercase tracking-[0.16em] text-warm-bone">{live ? "LIVE" : "CLOSED"}</span><Button tone="tertiary" size="sm" onClick={onCopy}>{t("projectFrameBuildDeployCopyLogs")}</Button></div><ol className="max-h-[min(42dvh,360px)] space-y-1 overflow-auto p-3 font-mono text-[11px] text-warm-bone/90">{events.length ? events.map((event) => <li key={event.sequence}>#{event.sequence} [{event.state}] {event.message}</li>) : <li>{t("projectFrameBuildDeployNoLogs")}</li>}</ol></div>;
}

function BuildDeployResultCard({ job, status, events, onRetry, onEdit }: { job: BuildDeployJobStatusResponse | BuildDeployJobSubmitResponse; status: BuildDeployJobStatusResponse | null; events: BuildDeployJobEvent[]; onRetry: () => void; onEdit: () => void }) {
  const t = useT();
  const ready = job.state === "ready";
  const failed = job.state === "failed";
  const url = status?.result?.public_url;
  return <div className="mt-4 space-y-3 rounded-[14px] border border-whisper-border bg-pure-surface p-4"><JobStatusTracker state={job.state} /><div className="rounded-[12px] bg-warm-bone p-3 text-[12px]"><p className="font-display text-[15px] font-bold text-charcoal-ink">{ready ? t("projectFrameBuildDeployResultReady") : failed ? t("projectFrameBuildDeployResultFailed") : t("projectFrameBuildDeployResultCancelled")}</p>{url ? <p className="mt-1 break-all text-steel-secondary">{url}</p> : null}{status?.error ? <p className="mt-1 text-deep-rust">{status.error}</p> : null}<div className="mt-3 flex flex-wrap gap-2">{ready && url ? <Button tone="secondary" size="sm" onClick={() => window.open(url, "_blank", "noopener,noreferrer")}>{t("projectFrameOpenUrl")}</Button> : null}<Button tone="primary" size="sm" onClick={onRetry}>{ready ? t("projectFrameBuildDeployNewBuild") : failed ? t("retry") : t("projectFrameBuildDeployRestart")}</Button>{!ready ? <Button tone="secondary" size="sm" onClick={onEdit}>{t("projectFrameBuildDeployEditConfig")}</Button> : null}</div></div><TerminalLogPanel events={events} live={false} onCopy={() => void navigator.clipboard?.writeText(events.map((event) => `#${event.sequence} [${event.state}] ${event.message}`).join("\n"))} /></div>;
}

function DeploymentDiagnostics({ diagnostics, targetId }: { diagnostics: ProjectDiagnostics | null; targetId: string }) {
  const t = useT();
  const [tab, setTab] = useState<"operations" | "exec" | "ports" | "proxy">("operations");
  const tabs = [
    { id: "operations" as const, label: t("projectFrameTargetOperations"), count: diagnostics?.operations?.length ?? 0 },
    { id: "exec" as const, label: t("projectFrameDeploymentExecutions"), count: diagnostics?.executions.length ?? 0 },
    { id: "ports" as const, label: t("projectFrameDeploymentPortLeases"), count: diagnostics?.portLeases.length ?? 0 },
    { id: "proxy" as const, label: t("projectFrameDeploymentProxyRoutes"), count: diagnostics?.proxyRoutes.length ?? 0 },
  ];

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap gap-2">
        {tabs.map((item) => (
          <button
            key={item.id}
            type="button"
            onClick={() => setTab(item.id)}
            aria-pressed={item.id === tab}
            className={item.id === tab
              ? "rounded-full bg-charcoal-ink px-3 py-1.5 text-[12px] font-semibold text-pure-surface"
              : "rounded-full border border-whisper-border bg-warm-bone px-3 py-1.5 text-[12px] font-semibold text-steel-secondary hover:text-charcoal-ink"}
          >
            {item.label} · {item.count}
          </button>
        ))}
      </div>
      {tab === "operations" ? <TargetOperationDiagnostics key={targetId} targetId={targetId} operations={diagnostics?.operations ?? []} /> : null}
      {tab === "exec" ? <ExecutionDiagnostics executions={diagnostics?.executions ?? []} /> : null}
      {tab === "ports" ? <PortLeaseDiagnostics leases={diagnostics?.portLeases ?? []} /> : null}
      {tab === "proxy" ? <ProxyRouteDiagnostics routes={diagnostics?.proxyRoutes ?? []} /> : null}
    </div>
  );
}

function TargetOperationDiagnostics({
  targetId,
  operations,
}: {
  targetId: string;
  operations: TargetOperationRecord[];
}) {
  const t = useT();
  const client = useKernel();
  const [expanded, setExpanded] = useState<string | null>(null);
  const [details, setDetails] = useState<Record<string, {
    loading: boolean;
    record: TargetOperationRecord;
    error?: string;
  }>>({});
  const visible = operations.slice(0, 20);

  const toggleDetails = useCallback((operation: TargetOperationRecord) => {
    const opening = expanded !== operation.operation_id;
    setExpanded(opening ? operation.operation_id : null);
    if (!opening) return;
    setDetails((current) => ({
      ...current,
      [operation.operation_id]: { loading: true, record: operation },
    }));
    client.getTargetOperation(targetId, operation.operation_id)
      .then((record) => {
        setDetails((current) => ({
          ...current,
          [operation.operation_id]: { loading: false, record },
        }));
      })
      .catch((err: unknown) => {
        setDetails((current) => ({
          ...current,
          [operation.operation_id]: { loading: false, record: operation, error: errorMessage(err) },
        }));
      });
  }, [client, expanded, targetId]);

  if (visible.length === 0) return <QuietEmpty>{t("projectFrameTargetOperationEmpty")}</QuietEmpty>;

  return (
    <div className="space-y-3">
      {visible.map((operation) => {
        const cachedDetail = details[operation.operation_id];
        const detailState = cachedDetail
          && cachedDetail.record.revision >= operation.revision
          && cachedDetail.record.updated_at_ms >= operation.updated_at_ms
          ? cachedDetail
          : undefined;
        const record = detailState?.record ?? operation;
        const isExpanded = expanded === operation.operation_id;
        const receiptOutput = JSON.stringify(record.receipt?.output ?? null, null, 2);
        return (
          <div key={operation.operation_id} className="rounded-[14px] border border-whisper-border bg-warm-bone p-4">
            <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
              <div className="min-w-0">
                <p className="truncate font-mono text-[12px] text-charcoal-ink" title={operation.operation_id}>{operation.operation_id}</p>
                <p className="mt-1 text-[11px] text-steel-secondary">
                  {humanEventKind(operation.spec.kind)} · {new Date(operation.updated_at_ms).toLocaleString()}
                </p>
              </div>
              <div className="flex shrink-0 flex-wrap items-center gap-2">
                <StatusPill tone={targetOperationTone(operation.status)} label={humanEventKind(operation.status)} />
                <Button tone="tertiary" size="sm" aria-expanded={isExpanded} onClick={() => toggleDetails(operation)}>
                  {isExpanded ? t("projectFrameTargetOperationHideDetails") : t("projectFrameTargetOperationShowDetails")}
                </Button>
              </div>
            </div>
            {isExpanded ? (
              <div className="mt-4 space-y-3 border-t border-whisper-border pt-4">
                {detailState?.loading ? <p className="text-[12px] text-steel-secondary">{t("projectFrameDiagnosticsLoading")}</p> : null}
                {detailState?.error ? <p className="rounded-[10px] bg-deep-rust-surface p-2 text-[12px] text-deep-rust">{detailState.error}</p> : null}
                <dl className="grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
                  <TinyValue label={t("projectFrameTargetOperationRevision")} value={String(record.revision)} />
                  <TinyValue label={t("projectFrameTargetOperationExecution")} value={record.execution_id ?? "—"} />
                  <TinyValue label={t("projectFrameDeploymentCreatedAt")} value={new Date(record.created_at_ms).toLocaleString()} />
                  <TinyValue label={t("projectFrameTargetOperationUpdatedAt")} value={new Date(record.updated_at_ms).toLocaleString()} />
                  <TinyValue label={t("projectFrameTargetOperationRequestDigest")} value={record.authority.request_digest} />
                  <TinyValue label={t("projectFrameTargetOperationAuthorityDigest")} value={record.authority.authority_digest} />
                </dl>
                {record.receipt ? (
                  <div className="grid gap-3 lg:grid-cols-2">
                    <div className="min-w-0 rounded-[12px] bg-pure-surface p-3">
                      <p className="font-mono text-[10px] uppercase tracking-[0.12em] text-muted-tone">{t("projectFrameTargetOperationOutput")}</p>
                      <pre className="mt-2 max-h-52 overflow-auto whitespace-pre-wrap break-words font-mono text-[11px] leading-relaxed text-charcoal-ink">{receiptOutput}</pre>
                    </div>
                    <div className="min-w-0 rounded-[12px] bg-charcoal-ink p-3 text-pure-surface">
                      <p className="font-mono text-[10px] uppercase tracking-[0.12em] text-warm-bone/70">{t("projectFrameTargetOperationDiagnostics")}</p>
                      <pre className="mt-2 max-h-52 overflow-auto whitespace-pre-wrap break-words font-mono text-[11px] leading-relaxed">
                        {record.receipt.diagnostics.length
                          ? record.receipt.diagnostics.join("\n")
                          : t("projectFrameTargetOperationNoDiagnostics")}
                      </pre>
                    </div>
                  </div>
                ) : (
                  <p className="rounded-[10px] bg-pure-surface p-2 text-[12px] text-steel-secondary">
                    {t("projectFrameTargetOperationReceiptPending")}
                  </p>
                )}
              </div>
            ) : null}
          </div>
        );
      })}
      {operations.length > visible.length ? (
        <p className="text-[11px] text-muted-tone">{t("projectFrameTargetOperationShowingRecent", visible.length, operations.length)}</p>
      ) : null}
    </div>
  );
}

function targetOperationTone(status: TargetOperationStatus): StatusTone {
  if (status === "succeeded") return "stopped";
  if (status === "failed" || status === "outcome_unknown" || status === "expired") return "failed";
  if (status === "running") return "running";
  if (status === "requested" || status === "accepted") return "starting";
  return "neutral";
}

function ExecutionDiagnostics({ executions }: { executions: ExecStatus[] }) {
  const t = useT();
  const client = useKernel();
  const [expanded, setExpanded] = useState<string | null>(null);
  const [logs, setLogs] = useState<Record<string, { loading: boolean; lines: LocalExecLogLine[]; error?: string }>>({});

  const toggleLogs = useCallback((execId: string) => {
    setExpanded((current) => current === execId ? null : execId);
    if (logs[execId]) return;
    setLogs((current) => ({ ...current, [execId]: { loading: true, lines: [] } }));
    client.execLogs(execId, 80)
      .then((result) => {
        setLogs((current) => ({ ...current, [execId]: { loading: false, lines: result.lines ?? [], error: result.error ?? undefined } }));
      })
      .catch((err: unknown) => {
        setLogs((current) => ({ ...current, [execId]: { loading: false, lines: [], error: errorMessage(err) } }));
      });
  }, [client, logs]);

  if (executions.length === 0) return <QuietEmpty>{t("projectFrameDeploymentEmpty")}</QuietEmpty>;

  return (
    <div className="grid gap-3 lg:grid-cols-2">
      {executions.map((execution, index) => {
        const execId = execution.exec_id ?? `${execution.target_id ?? "exec"}-${index}`;
        const logState = logs[execId];
        return (
          <div key={execId} className="min-w-0 rounded-[14px] border border-whisper-border bg-warm-bone p-4">
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <p className="truncate font-mono text-[12px] text-charcoal-ink" title={execId}>{execId}</p>
                <p className="mt-1 truncate font-mono text-[11px] text-muted-tone" title={execution.target_id ?? undefined}>{execution.target_id ?? "—"}</p>
              </div>
              <span className="flex shrink-0 items-center gap-1.5 rounded-full border border-whisper-border bg-pure-surface px-2 py-1 text-[11px] text-steel-secondary">
                <span className={execution.ready ? "size-2 rounded-full bg-aged-brass" : "size-2 rounded-full bg-muted-tone"} />
                {execution.kind}
              </span>
            </div>
            <div className="mt-3 grid gap-2 text-[12px] sm:grid-cols-2">
              <TinyValue label={t("projectFrameDeploymentReady")} value={execution.ready ? t("projectFrameStatusReady") : t("projectFrameStatusNotReady")} />
              <TinyValue label={t("projectFrameDeploymentExitCode")} value={execution.exit_code === null || execution.exit_code === undefined ? "—" : String(execution.exit_code)} />
            </div>
            {execution.message ? <p className="mt-2 rounded-[10px] bg-pure-surface p-2 text-[12px] text-steel-secondary">{execution.message}</p> : null}
            {execution.exec_id ? (
              <Button tone="tertiary" size="sm" onClick={() => toggleLogs(execution.exec_id ?? execId)} className="mt-3">
                {expanded === execId ? t("projectFrameHideLogs") : t("projectFrameShowLogs")}
              </Button>
            ) : null}
            {expanded === execId ? (
              <pre className="mt-3 max-h-52 overflow-auto rounded-[10px] bg-charcoal-ink p-3 font-mono text-[11px] leading-relaxed text-pure-surface">
                {logState?.loading
                  ? t("projectFrameLogsLoading")
                  : logState?.error
                    ? logState.error
                    : logState?.lines.length
                      ? logState.lines.map((line) => `${line.seq} ${line.stream}: ${line.message_redacted}`).join("\n")
                      : t("projectFrameNoLogs")}
              </pre>
            ) : null}
          </div>
        );
      })}
    </div>
  );
}

function PortLeaseDiagnostics({ leases }: { leases: PortLeaseRecord[] }) {
  const t = useT();
  if (leases.length === 0) return <QuietEmpty>{t("projectFrameDeploymentEmpty")}</QuietEmpty>;
  return (
    <div className="grid gap-3 lg:grid-cols-2">
      {leases.map((lease) => {
        const address = `${lease.host}:${lease.port}`;
        return (
          <div key={lease.id} className="min-w-0 rounded-[14px] border border-whisper-border bg-warm-bone p-4">
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <p className="truncate font-mono text-[12px] text-charcoal-ink" title={lease.id}>{lease.id}</p>
                <p className="mt-1 truncate font-mono text-[11px] text-muted-tone">{lease.port_name} · {lease.protocol}</p>
              </div>
              <StatusPill tone={lease.status === "active" ? "running" : "neutral"} label={lease.status} />
            </div>
            <div className="mt-3 flex items-center justify-between gap-2 rounded-[10px] bg-pure-surface p-2">
              <span className="truncate font-mono text-[12px] text-charcoal-ink" title={address}>{address}</span>
              <CopyButton value={address} label={t("projectFrameCopyAddress")} />
            </div>
          </div>
        );
      })}
    </div>
  );
}

function ProxyRouteDiagnostics({ routes }: { routes: ProxyRouteRecord[] }) {
  const t = useT();
  if (routes.length === 0) return <QuietEmpty>{t("projectFrameDeploymentEmpty")}</QuietEmpty>;
  return (
    <div className="grid gap-3 lg:grid-cols-2">
      {routes.map((route) => {
        const safePublic = safeHttpUrl(route.public_url);
        const safeIframe = safeHttpUrl(route.iframe_url);
        return (
          <div key={route.id} className="min-w-0 rounded-[14px] border border-whisper-border bg-warm-bone p-4">
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <p className="truncate font-mono text-[12px] text-charcoal-ink" title={route.id}>{route.id}</p>
                <p className="mt-1 truncate font-mono text-[11px] text-muted-tone">{route.protocol} · {route.upstream.port_name}</p>
              </div>
              <StatusPill tone={route.status === "active" ? "running" : "neutral"} label={route.status} />
            </div>
            <ProxyUrlRow label={t("projectFramePublicUrl")} value={route.public_url} safeUrl={safePublic} />
            <ProxyUrlRow label={t("projectFrameIframeUrl")} value={route.iframe_url} safeUrl={safeIframe} />
          </div>
        );
      })}
    </div>
  );
}

function ProxyUrlRow({ label, value, safeUrl }: { label: string; value: string; safeUrl?: string }) {
  const t = useT();
  return (
    <div className="mt-3 rounded-[10px] bg-pure-surface p-2">
      <p className="font-mono text-[10px] uppercase tracking-[0.12em] text-muted-tone">{label}</p>
      <div className="mt-1 flex items-center gap-2">
        <span className="min-w-0 flex-1 truncate font-mono text-[12px] text-charcoal-ink" title={value}>{value || "—"}</span>
        {value ? <CopyButton value={value} label={t("projectFrameCopyUrl")} /> : null}
        {safeUrl ? (
          <a className="rounded-full p-1.5 text-steel-secondary hover:bg-warm-bone hover:text-charcoal-ink" href={safeUrl} target="_blank" rel="noreferrer" aria-label={t("projectFrameOpenUrl")}>
            <LinkSimple size={14} />
          </a>
        ) : null}
      </div>
    </div>
  );
}

function CopyButton({ value, label }: { value: string; label: string }) {
  return (
    <button
      type="button"
      className="rounded-full p-1.5 text-steel-secondary hover:bg-warm-bone hover:text-charcoal-ink"
      aria-label={label}
      onClick={() => void navigator.clipboard?.writeText(value)}
    >
      <Copy size={14} />
    </button>
  );
}

function TinyValue({ label, value }: { label: string; value: string }) {
  return <div className="min-w-0 rounded-[10px] bg-pure-surface p-2"><p className="font-mono text-[10px] uppercase tracking-[0.12em] text-muted-tone">{label}</p><p className="mt-1 truncate font-mono text-[12px] text-charcoal-ink" title={value}>{value}</p></div>;
}

function basename(path: string): string {
  return path.split("/").filter(Boolean).pop() ?? path;
}

function QuietEmpty({ children }: { children: ReactNode }) {
  return <p className="text-[13px] text-steel-secondary">{children}</p>;
}

function PackageDiagnosticCard({ pkg }: { pkg: PackageRecord }) {
  const t = useT();
  const failure = pkg.last_failure;
  return <div className="rounded-[14px] border border-whisper-border bg-warm-bone p-4"><div className="flex items-start justify-between gap-3"><div className="min-w-0"><p className="truncate font-mono text-[12px] text-charcoal-ink">{pkg.id}</p><p className="mt-1 font-mono text-[11px] text-muted-tone">{pkg.version} · {pkg.entry_kind}</p></div><StatusPill tone={projectStateTone(pkg.state)} label={pkg.state} /></div><p className="mt-3 text-[12px] text-steel-secondary">{t("projectFramePackageCounts", pkg.capability_count, pkg.hook_count)}</p>{failure ? <p className="mt-2 rounded-[10px] bg-deep-rust-surface p-2 text-[12px] text-deep-rust">{failure.reason}</p> : null}{failure?.log_tail_redacted?.length ? <pre className="mt-2 max-h-24 overflow-auto rounded-[10px] bg-pure-surface p-2 font-mono text-[11px] text-steel-secondary">{failure.log_tail_redacted.slice(-3).map((line) => line.line).join("\n")}</pre> : null}</div>;
}

function filterProjectPackages(packages: PackageRecord[], refs: string[], projectId: string, updates: Array<{ package_id?: string; id?: string }>): PackageRecord[] {
  const refTokens = refs.flatMap((ref) => [ref, ref.split("/").at(-2), ref.split("/").at(-1)?.replace(/\.ya?ml$/, "")]).filter(Boolean) as string[];
  const updatePackageIds = new Set(
    updates
      .flatMap((record) => [record.package_id, record.id])
      .filter((value): value is string => Boolean(value)),
  );
  const filtered = packages.filter(
    (pkg) =>
      pkg.id === projectId ||
      updatePackageIds.has(pkg.id) ||
      refTokens.some((token) => pkg.id === token || pkg.id.endsWith(`/${token}`) || pkg.id.endsWith(`__${token}`)),
  );
  return filtered.length > 0 ? filtered : packages;
}

function unwrapSettled<T>(result: PromiseSettledResult<T>, errors: string[]): T | undefined {
  if (result.status === "fulfilled") return result.value;
  errors.push(errorMessage(result.reason));
  return undefined;
}

function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

function newDeploymentIdempotencyKey(): string {
  const entropy = globalThis.crypto?.randomUUID?.().replaceAll("-", "")
    ?? Math.random().toString(36).slice(2);
  return `web-${Date.now().toString(36)}-${entropy.slice(0, 20)}`;
}

function fingerprintFromUrl(bundleUrl?: string): string | undefined {
  if (!bundleUrl) return undefined;
  try {
    const url = new URL(bundleUrl, typeof window === "undefined" ? "http://localhost" : window.location.origin);
    return url.searchParams.get("v") ?? undefined;
  } catch {
    return undefined;
  }
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

function formatProjectType(type?: string): string {
  if (!type) return "—";
  return type
    .split("_")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function humanEventKind(kind: string): string {
  return kind
    .split(/[./_-]+/)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function formatEventTime(value?: string): string {
  if (!value) return "—";
  const timestamp = Date.parse(value);
  if (!Number.isFinite(timestamp)) return value;
  return new Date(timestamp).toLocaleString();
}
