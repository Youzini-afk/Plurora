import { affordanceForAction, resolveLibraryAffordances, type LibraryAffordanceInput } from "./library-affordance";
import { libraryAuthorityForInstallation } from "./library-affordance";

const installation = {
  record: {
    installation_id: "installation-1",
    display_name: "Demo",
    status: "ready",
  },
  revision: 3,
  work_summary: {
    title: "Demo",
    description: "",
    work_id: "demo/work",
    annotations: {},
    content_roots: [],
    entrypoints: [{ id: "play", intent_uri: "plurora.shell.default/play", target: { kind: "surface", surface_id: "demo" } }],
  },
} as unknown as LibraryAffordanceInput["installation"];

const workSummary = {
  title: "Demo",
  description: "",
  work_id: "demo/work",
  annotations: {},
  content_roots: [],
  entrypoints: [{ id: "play", intent_uri: "plurora.shell.default/play", target: { kind: "surface", surface_id: "demo" } }],
} as NonNullable<LibraryAffordanceInput["work_summary"]>;
const entrypoint = workSummary.entrypoints[0];

const ready: LibraryAffordanceInput = {
  installation,
  work_summary: workSummary,
  entrypoint,
  authority: { can_observe: true, can_run: true, can_stop: true },
  preflight_gaps: [],
};
if (!affordanceForAction(ready, "play").available) throw new Error("ready entrypoint should be playable");

const missing: LibraryAffordanceInput = {
  ...ready,
  preflight_gaps: [{ reason_code: "binding_unavailable", next_step: "Select a provider" }],
};
const blocked = affordanceForAction(missing, "play");
if (blocked.available || blocked.reason_code !== "binding_unavailable" || blocked.next_step !== "Select a provider") {
  throw new Error("preflight gaps must remain structured and block Play");
}

const unknownBindingGap = affordanceForAction({
  ...ready,
  preflight_gaps: [],
  binding_gaps: [{ reason_code: "future_binding_gap", next_step: "Use a future adapter" }],
}, "play");
if (unknownBindingGap.available || unknownBindingGap.reason_code !== "future_binding_gap") {
  throw new Error("unknown Binding gaps must fail closed without being collapsed");
}

const active: LibraryAffordanceInput = {
  ...ready,
  active_run: {
    entrypoint_id: "play",
    installation_revision: 3,
    revision: 8,
    record: {
      installation_id: "installation-1",
      run_id: "run-1",
      status: "interrupted",
      started_at: "2026-01-01T00:00:00Z",
      health: { status: "unknown" },
    },
  },
};
if (!affordanceForAction(active, "restart").available) throw new Error("interrupted Run should be restartable");
if (affordanceForAction(active, "stop").available) throw new Error("interrupted Run must not expose Stop");

if (resolveLibraryAffordances(ready).length < 5) throw new Error("all core Library actions should be returned");

const exactAuthority = libraryAuthorityForInstallation({
  kind: "device",
  device_name: "pwa",
  scopes: ["observe", "run"],
  resources: [{ kind: "installation", id: "installation-1" }],
}, "installation-1");
if (!exactAuthority.can_observe || !exactAuthority.can_run || !exactAuthority.can_stop) {
  throw new Error("an exact Installation grant should expose Run affordances");
}
const wrongInstallation = libraryAuthorityForInstallation({
  kind: "device",
  device_name: "pwa",
  scopes: ["observe", "run"],
  resources: [{ kind: "installation", id: "installation-2" }],
}, "installation-1");
if (wrongInstallation.can_run || wrongInstallation.reason_code !== "authority_denied") {
  throw new Error("a different Installation grant must not expose Run affordances");
}

const rightsDeclaration = {
  install: "allowed",
  execute: "allowed",
  backup: "allowed",
  export_state: "denied",
  copy_across_hosts: "denied",
  redistribute_artifacts: "denied",
  modify: "denied",
  derive: "denied",
  modding: "denied",
  dedicated_server: "denied",
} as const;
const deniedRights = affordanceForAction({
  ...ready,
  work_summary: {
    ...workSummary,
    rights_declaration: { ...rightsDeclaration, execute: "denied" },
  },
}, "play");
if (deniedRights.available || deniedRights.reason_code !== "rights_denied") {
  throw new Error("Denied execute Rights must block Play before a mutation");
}

const backup = affordanceForAction({
  ...ready,
  work_summary: {
    ...workSummary,
    rights_declaration: rightsDeclaration,
  },
  authority: { ...ready.authority, can_manage_installation: true },
}, "backup");
if (!backup.available) {
  throw new Error("explicit Allowed backup Rights and exact authority should expose Backup");
}
