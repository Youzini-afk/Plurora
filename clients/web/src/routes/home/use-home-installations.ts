import { useCallback, useEffect, useState } from "react";
import { usePlurora } from "@/lib/plurora-client";
import type { InstallationView, RunView } from "@/protocol/client";

export interface InstallationSummary {
  installationId: string;
  displayName: string;
  status: string;
  revision: number;
  sourceKind: string;
  updatedAt: string;
  workTitle: string;
  workDescription: string;
  workId: string;
  entrypoints: InstallationView["work_summary"]["entrypoints"];
  activeRun: RunView | null;
  runs: RunView[];
  view: InstallationView;
}

function summarize(view: InstallationView, runViews: RunView[]): InstallationSummary {
  const activeRun = runViews.find((run) => ["starting", "running", "degraded", "stopping"].includes(run.record.status)) ?? null;
  return {
    installationId: view.record.installation_id,
    displayName: view.record.display_name,
    status: view.record.status,
    revision: view.revision,
    sourceKind: view.record.source.kind,
    updatedAt: view.record.updated_at,
    workTitle: view.work_summary.title,
    workDescription: view.work_summary.description,
    workId: view.work_summary.work_id,
    entrypoints: view.work_summary.entrypoints,
    activeRun,
    runs: runViews,
    view,
  };
}

export function useHomeInstallations() {
  const client = usePlurora();
  const [installations, setInstallations] = useState<InstallationSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const views = await client.listInstallations();
      const runGroups = await Promise.all(views.map(async (view) => ({
        installationId: view.record.installation_id,
        runs: await client.listRuns({ installation_id: view.record.installation_id }),
      })));
      const runsByInstallation = new Map<string, RunView[]>();
      for (const group of runGroups) {
        runsByInstallation.set(group.installationId, group.runs);
      }
      setInstallations(views.map((view) => summarize(view, runsByInstallation.get(view.record.installation_id) ?? [])));
      setError(null);
    } catch {
      setError("Installation inventory is unavailable.");
    } finally {
      setLoading(false);
    }
  }, [client]);
  useEffect(() => { void refresh(); }, [refresh]);
  return { installations, loading, error, refresh };
}
