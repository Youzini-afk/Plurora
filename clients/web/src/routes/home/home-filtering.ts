import type { InstallationSummary } from "./use-home-installations";

export interface InstallationCounts { all: number; ready: number; blocked: number; failed: number }

export function countsForInstallations(items: InstallationSummary[]): InstallationCounts {
  return {
    all: items.length,
    ready: items.filter((item) => item.status === "ready").length,
    blocked: items.filter((item) => item.status === "blocked").length,
    failed: items.filter((item) => item.status === "failed").length,
  };
}

export function filterInstallations(items: InstallationSummary[], search: string): InstallationSummary[] {
  const query = search.trim().toLowerCase();
  if (!query) return items;
  return items.filter((item) => `${item.displayName} ${item.installationId} ${item.sourceKind}`.toLowerCase().includes(query));
}
