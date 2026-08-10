/**
 * Tiny in-app router — hash-based, three routes plus query/state params.
 *
 * We intentionally avoid React Router for a desktop shell with a small fixed
 * route set. Hash routing avoids server config, works in Tauri WebView, and
 * survives reloads.
 *
 * Route grammar:
 *   #/                                Home
 *   #/settings/api-connections        Settings page
 *   #/settings/host-access            Paired devices and scoped access
 *   #/settings/installed-packages     ...
 *   #/settings/profiles
 *   #/settings/storage
 *   #/settings/about
 *   /installation/<installationId>              Chrome-free installation tab host
 */

import { useEffect, useState } from "react";

export type SettingsTab =
  | "api-connections"
  | "host-access"
  | "installed-packages"
  | "profiles"
  | "storage"
  | "about";

const MAX_INSTALLATION_ID_LENGTH = 128;
const INSTALLATION_ID_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._-]*$/;

export type Route =
  | { kind: "home" }
  | { kind: "settings"; tab: SettingsTab }
  | { kind: "installation"; installationId: string };

export const SETTINGS_TABS: Array<{ id: SettingsTab; label: string }> = [
  { id: "api-connections", label: "API Connections" },
  { id: "host-access", label: "Host Access" },
  { id: "installed-packages", label: "Installed Packages" },
  { id: "profiles", label: "Profiles" },
  { id: "storage", label: "Storage" },
  { id: "about", label: "About" },
];

export function parseHash(hash: string): Route {
  const path = hash.replace(/^#/, "").replace(/^\//, "");
  if (!path) return { kind: "home" };
  const [head, ...rest] = path.split("/");
  if (head === "settings") {
    const tab = (rest[0] ?? "api-connections") as SettingsTab;
    if (SETTINGS_TABS.some((t) => t.id === tab)) {
      return { kind: "settings", tab };
    }
    return { kind: "settings", tab: "api-connections" };
  }
  if (head === "installation" && rest[0]) {
    // Malformed escapes (e.g., "%") would throw — fall back to Home.
    try {
      const installationId = decodeURIComponent(rest[0]);
      return isValidInstallationId(installationId) ? { kind: "installation", installationId } : { kind: "home" };
    } catch {
      return { kind: "home" };
    }
  }
  return { kind: "home" };
}

function encodeRouteInstallationId(installationId: string): string {
  // Keep installation ids with `/` out of hash routes too. Canonical installation tabs use
  // `/installation/<id>` and installation ids are a single URL segment there.
  return encodeURIComponent(installationId);
}

export function serializeRoute(route: Route): string {
  switch (route.kind) {
    case "home":
      return "#/";
    case "settings":
      return `#/settings/${route.tab}`;
    case "installation":
      return `#/installation/${encodeRouteInstallationId(route.installationId)}`;
  }
}

export function isValidInstallationId(value: string): boolean {
  return value.length > 0
    && value.length <= MAX_INSTALLATION_ID_LENGTH
    && !value.includes("/")
    && value !== "."
    && value !== ".."
    && !value.startsWith(".")
    && !value.includes("..")
    && !/[\u0000-\u001F\u007F]/.test(value)
    && INSTALLATION_ID_PATTERN.test(value);
}

export function installationPath(installationId: string): string {
  if (!isValidInstallationId(installationId)) throw new Error("invalid installation id");
  return `/installation/${encodeURIComponent(installationId)}`;
}

export function parseInstallationPath(pathname: string): { kind: "installation"; installationId: string } | null {
  if (!pathname.startsWith("/installation/")) return null;
  const suffix = pathname.slice("/installation/".length);
  if (!suffix || suffix.includes("/")) return null;
  try {
    const installationId = decodeURIComponent(suffix);
    return isValidInstallationId(installationId) ? { kind: "installation", installationId } : null;
  } catch {
    return null;
  }
}

export function usePathInstallationRoute(): { kind: "installation"; installationId: string } | null {
  const [route, setRoute] = useState<{ kind: "installation"; installationId: string } | null>(() =>
    typeof window === "undefined" ? null : parseInstallationPath(window.location.pathname),
  );

  useEffect(() => {
    const onPopState = () => setRoute(parseInstallationPath(window.location.pathname));
    window.addEventListener("popstate", onPopState);
    return () => window.removeEventListener("popstate", onPopState);
  }, []);

  return route;
}

export function useRoute(): [Route, (next: Route) => void] {
  const [route, setRoute] = useState<Route>(() =>
    typeof window === "undefined" ? { kind: "home" } : parseHash(window.location.hash),
  );

  useEffect(() => {
    const onHashChange = () => setRoute(parseHash(window.location.hash));
    window.addEventListener("hashchange", onHashChange);
    return () => window.removeEventListener("hashchange", onHashChange);
  }, []);

  const navigate = (next: Route) => {
    const hash = serializeRoute(next);
    if (window.location.hash !== hash) {
      window.location.hash = hash;
    } else {
      setRoute(next);
    }
  };

  return [route, navigate];
}

export function routeLabel(route: Route): string {
  switch (route.kind) {
    case "home":
      return "Home";
    case "settings": {
      const tab = SETTINGS_TABS.find((t) => t.id === route.tab);
      return tab ? `Settings / ${tab.label}` : "Settings";
    }
    case "installation":
      return `Installations / ${route.installationId}`;
  }
}
