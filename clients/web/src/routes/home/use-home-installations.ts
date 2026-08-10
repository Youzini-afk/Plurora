import { useCallback, useEffect, useState } from "react";
import { usePlurora } from "@/lib/plurora-client";
import type { InstallationView } from "@/protocol/client";

export interface InstallationSummary {
  installationId: string;
  displayName: string;
  status: string;
  revision: number;
  sourceKind: string;
  updatedAt: string;
}

function summarize(view: InstallationView): InstallationSummary {
  return {
    installationId: view.record.installation_id,
    displayName: view.record.display_name,
    status: view.record.status,
    revision: view.revision,
    sourceKind: view.record.source.kind,
    updatedAt: view.record.updated_at,
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
      setInstallations(views.map(summarize));
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
