import { useCallback, useEffect } from "react";
import type { FailureDetail } from "@/components/install/failure-modal";
import type { ToastInput } from "@/components/ui/toast";
import type { RelativeAgeLabels } from "@/lib/format";
import type { PluroraProtocolClient, PackageRecord, ProjectRecord } from "@/protocol/client";
import {
  failureDetailFromPackage,
  noFailureDiagnostic,
  resolvePackageStatus,
} from "./failure-diagnostics";
import { recordOpen } from "./use-recently-opened";

interface UseProjectActionsArgs {
  client: PluroraProtocolClient;
  onLaunch: (projectId: string) => void;
  pushToast: (toast: ToastInput) => string;
  refreshProjects: () => void;
  setShowInstall: (show: boolean) => void;
  setFailureProjectId: (projectId: string | null) => void;
  setFailureDetail: (detail: FailureDetail | undefined) => void;
  labels: {
    stoppedToast: (title: string) => string;
    stopFailedTitle: string;
    stopFailedBody: string;
    uninstallTitle: (title: string) => string;
    uninstallingBody: string;
    uninstalledTitle: (title: string) => string;
    uninstalledBody: string;
    uninstallFailedTitle: string;
    uninstallFailedBody: string;
    loadingDiagnostics: string;
    loadingDiagnosticsSummary: string;
    descriptorNoPackages: string;
    noPackageStatus: string;
    diagnosticsUnavailable: string;
    noDiagnosticAvailable: string;
    diagnosticsUnavailableCause: string;
    packageFailureTitle: (packageId: string, state: string) => string;
    packageDegradedSummary: string;
    relativeAge: RelativeAgeLabels;
  };
}

export function useProjectActions({
  client,
  onLaunch,
  pushToast,
  refreshProjects,
  setShowInstall,
  setFailureProjectId,
  setFailureDetail,
  labels,
}: UseProjectActionsArgs) {
  // Cmd/Ctrl + N opens the install modal.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "n") {
        event.preventDefault();
        setShowInstall(true);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [setShowInstall]);

  const onStop = useCallback(
    async (projectId: string, title: string) => {
      try {
        await client.stopProject(projectId);
        pushToast({ variant: "success", title: labels.stoppedToast(title) });
        refreshProjects();
      } catch (err) {
        pushToast({
          variant: "error",
          title: labels.stopFailedTitle,
          body: labels.stopFailedBody,
        });
      }
    },
    [client, labels, pushToast, refreshProjects],
  );

  const onUninstall = useCallback(
    async (project: ProjectRecord) => {
      pushToast({ variant: "progress", title: labels.uninstallTitle(project.title), body: labels.uninstallingBody });
      try {
        if (project.state === "running" || project.state === "starting") {
          await client.stopProject(project.id).catch(() => undefined);
        }
        await client.uninstallProject(project.id);
        pushToast({ variant: "success", title: labels.uninstalledTitle(project.title), body: labels.uninstalledBody });
        refreshProjects();
      } catch {
        pushToast({
          variant: "error",
          title: labels.uninstallFailedTitle,
          body: labels.uninstallFailedBody,
        });
      }
    },
    [client, labels, pushToast, refreshProjects],
  );

  const onInstallClick = useCallback(() => setShowInstall(true), [setShowInstall]);

  const onShowFailure = useCallback(
    async (project: ProjectRecord) => {
      setFailureProjectId(project.id);
      setFailureDetail({
        projectName: project.title,
        title: labels.loadingDiagnostics,
        summary: labels.loadingDiagnosticsSummary,
      });
      try {
        const descriptor = await client.getProject(project.id);
        const packageIds = descriptor.packages ?? [];
        const knownPackages = await client.packages().catch<PackageRecord[]>(() => []);
        const packageLookup = new Map(knownPackages.map((record) => [record.id, record]));
        if (packageIds.length === 0) {
          setFailureDetail(noFailureDiagnostic(project.title, labels.descriptorNoPackages, {
            noDiagnosticAvailable: labels.noDiagnosticAvailable,
            unavailableCause: labels.diagnosticsUnavailableCause,
          }));
          return;
        }
        const records = (
          await Promise.all(packageIds.map((packageId) => resolvePackageStatus(client, packageId, packageLookup)))
        ).filter((record): record is PackageRecord => Boolean(record));
        const failed = records.find((record) => record.last_failure) ?? records.find((record) => record.state === "degraded") ?? records[0];
        if (!failed) {
          setFailureDetail(noFailureDiagnostic(project.title, labels.noPackageStatus, {
            noDiagnosticAvailable: labels.noDiagnosticAvailable,
            unavailableCause: labels.diagnosticsUnavailableCause,
          }));
          return;
        }
        setFailureDetail(failureDetailFromPackage(project.title, failed, [], {
          noDiagnosticAvailable: labels.noDiagnosticAvailable,
          unavailableCause: labels.diagnosticsUnavailableCause,
          packageFailureTitle: labels.packageFailureTitle,
          packageDegradedSummary: labels.packageDegradedSummary,
          relativeAge: labels.relativeAge,
        }));
      } catch (err) {
        setFailureDetail(noFailureDiagnostic(project.title, labels.diagnosticsUnavailable, {
          noDiagnosticAvailable: labels.noDiagnosticAvailable,
          unavailableCause: labels.diagnosticsUnavailableCause,
        }));
      }
    },
    [client, labels, setFailureDetail, setFailureProjectId],
  );

  const onCardLaunch = useCallback(
    (project: ProjectRecord) => {
      recordOpen(project.id);
      if (project.state === "failed") {
        void onShowFailure(project);
        return;
      }
      onLaunch(project.id);
    },
    [onLaunch, onShowFailure, recordOpen],
  );

  return { onStop, onUninstall, onInstallClick, onShowFailure, onCardLaunch };
}
