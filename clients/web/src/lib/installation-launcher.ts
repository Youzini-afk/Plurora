import { isValidInstallationId, installationPath } from "@/lib/router";
import {
  createBrowserPlatformAdapter,
  type InstallationNavigationWindow,
} from "@/client-core/platform-adapter";

export type InstallationTabWindow = InstallationNavigationWindow;
export type InstallationOpenOutcome = "tab" | "same-window" | "invalid" | "failed";

export function installationTabTargetName(installationId: string): string {
  return `plurora-installation-${fnv1a(installationId).toString(36).padStart(7, "0").slice(0, 16)}`;
}

export function openInstallationInTab(installationId: string, hostWindow: InstallationTabWindow = window): InstallationOpenOutcome {
  if (!isValidInstallationId(installationId)) return "invalid";
  const url = installationPath(installationId);
  const target = installationTabTargetName(installationId);
  return createBrowserPlatformAdapter(hostWindow).openInstallation(url, target);
}

function fnv1a(value: string): number {
  let hash = 0x811c9dc5;
  for (let i = 0; i < value.length; i += 1) {
    hash ^= value.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
  }
  return hash >>> 0;
}
