import { openProjectInTab, projectTabTargetName, type ProjectTabWindow } from "./project-launcher";

function assertEqual<T>(actual: T, expected: T) {
  if (actual !== expected) throw new Error(`expected ${String(expected)}, got ${String(actual)}`);
}

function assertOk(value: unknown, message: string) {
  if (!value) throw new Error(message);
}

const target = projectTabTargetName("project/with/slash");
assertOk(/^plurora-project-[a-z0-9]+$/.test(target), "target name must be sanitized");
assertOk(!target.includes("project/with/slash"), "target must not include raw project id");

let openArgs: [string, string, string | undefined] | null = null;
let assigned = "";
const opened = { opener: {} } as Window;
const hostWindow: ProjectTabWindow = {
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

assertEqual(openProjectInTab("demo-project", hostWindow), "tab");
assertEqual(openArgs?.[0], "/project/demo-project");
assertEqual(openArgs?.[2], "noopener,noreferrer");
assertEqual(opened.opener, null);
assertEqual(assigned, "");

const popupBlocked: ProjectTabWindow = {
  open() {
    return null;
  },
  location: {
    assign(url: string) {
      assigned = url;
    },
  },
};

assertEqual(openProjectInTab("fallback-project", popupBlocked), "same-window");
assertEqual(assigned, "/project/fallback-project");
assertEqual(openProjectInTab("bad/id", popupBlocked), "invalid");

let mobileOpened = false;
assigned = "";
let reloaded = false;
let pushedUrl = "";
let pushedState: Record<string, unknown> | null = null;
const mobileWindow: ProjectTabWindow = {
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
assertEqual(openProjectInTab("mobile-project", mobileWindow), "same-window");
assertEqual(mobileOpened, false);
assertEqual(assigned, "");
assertEqual(pushedUrl, "/project/mobile-project");
assertEqual((pushedState as Record<string, unknown> | null)?.existing, true);
assertEqual((pushedState as Record<string, unknown> | null)?.__plurora_project_from_shell__, true);
assertEqual(reloaded, true);
