export type ClientPlatform = "web" | "desktop" | "pwa";

export interface ProjectNavigationWindow {
  open(url: string, target: string, features?: string): Window | null;
  location: Pick<Location, "assign"> & Partial<Pick<Location, "reload">>;
  history?: Pick<History, "length" | "state" | "pushState">;
  matchMedia?: (query: string) => Pick<MediaQueryList, "matches">;
  innerWidth?: number;
}

export interface PlatformAdapter {
  readonly kind: ClientPlatform;
  openProject(url: string, target: string): "tab" | "same-window" | "failed";
}

const PROJECT_WINDOW_FEATURES = "noopener,noreferrer";
export const PROJECT_SHELL_HISTORY_STATE = "__plurora_project_from_shell__";

export function detectClientPlatform(hostWindow: Window = window): ClientPlatform {
  if (hostWindow.__PLURORA_RUNTIME__?.platform) return hostWindow.__PLURORA_RUNTIME__.platform;
  const bootstrapPlatform = new URLSearchParams(hostWindow.location.search).get("plurora_platform");
  if (bootstrapPlatform === "desktop" || bootstrapPlatform === "pwa" || bootstrapPlatform === "web") {
    return bootstrapPlatform;
  }
  if (hostWindow.matchMedia?.("(display-mode: standalone)").matches) return "pwa";
  return "web";
}

export function createBrowserPlatformAdapter(
  hostWindow: ProjectNavigationWindow = window,
  kind: ClientPlatform = typeof window !== "undefined" ? detectClientPlatform(window) : "web",
): PlatformAdapter {
  return {
    kind,
    openProject(url, target) {
      if (shouldUseCurrentWindow(hostWindow, kind)) {
        return navigateSameWindow(hostWindow, url);
      }

      let opened: Window | null;
      try {
        opened = hostWindow.open(url, target, PROJECT_WINDOW_FEATURES);
      } catch {
        opened = null;
      }
      if (!opened) {
        return navigateSameWindow(hostWindow, url);
      }

      try {
        opened.opener = null;
      } catch {
        // `noopener` can make the property read-only, which is already safe.
      }
      return "tab";
    },
  };
}

export function shouldReturnToShellHistory(history: Pick<History, "length" | "state">): boolean {
  if (history.length <= 1 || !history.state || typeof history.state !== "object") return false;
  return (history.state as Record<string, unknown>)[PROJECT_SHELL_HISTORY_STATE] === true;
}

function navigateSameWindow(
  hostWindow: ProjectNavigationWindow,
  url: string,
): "same-window" | "failed" {
  try {
    if (hostWindow.history && hostWindow.location.reload) {
      const priorState = hostWindow.history.state;
      const state = priorState && typeof priorState === "object"
        ? { ...priorState, [PROJECT_SHELL_HISTORY_STATE]: true }
        : { [PROJECT_SHELL_HISTORY_STATE]: true };
      hostWindow.history.pushState(state, "", url);
      hostWindow.location.reload();
    } else {
      hostWindow.location.assign(url);
    }
    return "same-window";
  } catch {
    try {
      hostWindow.location.assign(url);
      return "same-window";
    } catch {
      return "failed";
    }
  }
}

function shouldUseCurrentWindow(hostWindow: ProjectNavigationWindow, kind: ClientPlatform): boolean {
  if (kind === "pwa") return true;
  if (hostWindow.matchMedia?.("(max-width: 767px)").matches) return true;
  return typeof hostWindow.innerWidth === "number" && hostWindow.innerWidth < 768;
}
