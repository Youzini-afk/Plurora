import { isValidProjectId, projectPath } from "@/lib/router";
import {
  createBrowserPlatformAdapter,
  type ProjectNavigationWindow,
} from "@/client-core/platform-adapter";

export type ProjectTabWindow = ProjectNavigationWindow;
export type ProjectOpenOutcome = "tab" | "same-window" | "invalid" | "failed";

export function projectTabTargetName(projectId: string): string {
  return `plurora-project-${fnv1a(projectId).toString(36).padStart(7, "0").slice(0, 16)}`;
}

export function openProjectInTab(projectId: string, hostWindow: ProjectTabWindow = window): ProjectOpenOutcome {
  if (!isValidProjectId(projectId)) return "invalid";
  const url = projectPath(projectId);
  const target = projectTabTargetName(projectId);
  return createBrowserPlatformAdapter(hostWindow).openProject(url, target);
}

function fnv1a(value: string): number {
  let hash = 0x811c9dc5;
  for (let i = 0; i < value.length; i += 1) {
    hash ^= value.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
  }
  return hash >>> 0;
}
