import { openInstallationInTab, installationTabTargetName, type InstallationTabWindow } from "./installation-launcher";

function assertEqual<T>(actual: T, expected: T) {
  if (actual !== expected) throw new Error(`expected ${String(expected)}, got ${String(actual)}`);
}

function assertOk(value: unknown, message: string) {
  if (!value) throw new Error(message);
}

const target = installationTabTargetName("installation/with/slash");
assertOk(/^plurora-installation-[a-z0-9]+$/.test(target), "target name must be sanitized");
assertOk(!target.includes("installation/with/slash"), "target must not include raw installation id");

let openArgs: [string, string, string | undefined] | null = null;
let assigned = "";
const opened = { opener: {} } as Window;
const hostWindow: InstallationTabWindow = {
  open(url, targetName, features) {
    openArgs = [url, targetName, features];
    return opened;
  },
  location: {
    assign(url: string) {
      assigned = url;
    },
  },
};

assertEqual(openInstallationInTab("demo-installation", hostWindow), "tab");
assertEqual(openArgs?.[0], "/installation/demo-installation");
assertEqual(openArgs?.[2], "noopener,noreferrer");
assertEqual(opened.opener, null);
assertEqual(assigned, "");

const popupBlocked: InstallationTabWindow = {
  open() {
    return null;
  },
  location: {
    assign(url: string) {
      assigned = url;
    },
  },
};

assertEqual(openInstallationInTab("fallback-installation", popupBlocked), "same-window");
assertEqual(assigned, "/installation/fallback-installation");
assertEqual(openInstallationInTab("bad/id", popupBlocked), "invalid");

let mobileOpened = false;
assigned = "";
let reloaded = false;
let pushedUrl = "";
let pushedState: Record<string, unknown> | null = null;
const mobileWindow: InstallationTabWindow = {
  open() {
    mobileOpened = true;
    return opened;
  },
  location: {
    assign(url: string) {
      assigned = url;
    },
    reload() {
      reloaded = true;
    },
  },
  history: {
    length: 2,
    state: { existing: true },
    pushState(state, _unused, url) {
      pushedState = state as Record<string, unknown>;
      pushedUrl = String(url);
    },
  },
  matchMedia(query: string) {
    return { matches: query === "(max-width: 767px)" };
  },
};
assertEqual(openInstallationInTab("mobile-installation", mobileWindow), "same-window");
assertEqual(mobileOpened, false);
assertEqual(assigned, "");
assertEqual(pushedUrl, "/installation/mobile-installation");
assertEqual((pushedState as Record<string, unknown> | null)?.existing, true);
assertEqual((pushedState as Record<string, unknown> | null)?.__plurora_installation_from_shell__, true);
assertEqual(reloaded, true);
