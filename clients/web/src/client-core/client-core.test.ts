import {
  BROWSER_ACCESS_TOKEN_STORAGE_KEY,
  BrowserCredentialProvider,
  PendingCredentialLease,
  browserAccessTokenStorageKey,
} from "./credentials";
import { INSTALLATION_SHELL_HISTORY_STATE, shouldReturnToShellHistory } from "./platform-adapter";
import { BrowserInstallationTargetContextStore } from "./installation-target-context";
import {
  BrowserHostConnectionStore,
  HOST_CONNECTIONS_STORAGE_KEY,
  normalizeHostBaseUrl,
  normalizeHostConnectionBaseUrl,
  resolveHostBaseUrl,
} from "./host-endpoint";

function assertEqual<T>(actual: T, expected: T) {
  if (actual !== expected) throw new Error(`expected ${String(expected)}, got ${String(actual)}`);
}

function assertThrows(fn: () => unknown, message: string) {
  try {
    fn();
  } catch (error) {
    if (error instanceof Error && error.message.includes(message)) return;
    throw error;
  }
  throw new Error(`expected error containing ${message}`);
}

class MemoryStorage {
  values = new Map<string, string>();
  getItem(key: string) { return this.values.get(key) ?? null; }
  setItem(key: string, value: string) { this.values.set(key, value); }
  removeItem(key: string) { this.values.delete(key); }
}

const storage = new MemoryStorage();
let replaced = "";
const credential = new BrowserCredentialProvider({
  localStorage: storage,
  location: {
    pathname: "/installation/demo",
    search: "?keep=1&plurora_token=&access_token=fallback-token",
    hash: "#view",
  },
  history: {
    state: { preserved: true },
    replaceState: (_state, _title, url) => { replaced = String(url); },
  },
});

assertEqual(credential.consumeBootstrap(), "fallback-token");
assertEqual(replaced, "/installation/demo?keep=1#view");
credential.write("stored-token");
assertEqual(storage.getItem(BROWSER_ACCESS_TOKEN_STORAGE_KEY), "stored-token");
assertEqual(credential.read(), "stored-token");
credential.clear();
assertEqual(credential.read(), undefined);

let bootstrapReads = 0;
const pending = new PendingCredentialLease({
  read: () => undefined,
  write: () => {},
  clear: () => {},
  consumeBootstrap: () => {
    bootstrapReads += 1;
    return bootstrapReads === 1 ? "one-time-token" : undefined;
  },
});
assertEqual(pending.resolve(), "one-time-token");
assertEqual(pending.resolve(), "one-time-token");
assertEqual(bootstrapReads, 1);
pending.clear();
assertEqual(pending.resolve(), undefined);

assertEqual(shouldReturnToShellHistory({ length: 5, state: null }), false);
assertEqual(shouldReturnToShellHistory({ length: 5, state: { [INSTALLATION_SHELL_HISTORY_STATE]: true } }), true);

assertEqual(normalizeHostBaseUrl("https://host.example///"), "https://host.example");
assertEqual(resolveHostBaseUrl("https://host.example/root/"), "https://host.example/root");

assertEqual(normalizeHostConnectionBaseUrl("https://host.example:9443/"), "https://host.example:9443");
assertEqual(normalizeHostConnectionBaseUrl("http://127.0.0.2:8787"), "http://127.0.0.2:8787");
assertThrows(() => normalizeHostConnectionBaseUrl("http://host.example"), "must use HTTPS");
assertThrows(() => normalizeHostConnectionBaseUrl("https://user@host.example"), "cannot contain credentials");
assertThrows(() => normalizeHostConnectionBaseUrl("https://host.example/api"), "must not contain a path");

const connectionStorage = new MemoryStorage();
const connections = new BrowserHostConnectionStore(connectionStorage);
const remote = connections.save("Remote Host", "https://host.example/");
assertEqual(remote.id, "https://host.example");
assertEqual(connections.list().length, 1);
connections.select(remote.id);
assertEqual(connections.active()?.name, "Remote Host");
connections.save("Renamed Host", "https://host.example");
assertEqual(connections.list().length, 1);
assertEqual(connections.active()?.name, "Renamed Host");

const remoteCredential = new BrowserCredentialProvider(
  {
    localStorage: connectionStorage,
    location: { pathname: "/", search: "", hash: "" },
    history: { state: null, replaceState: () => {} },
  },
  browserAccessTokenStorageKey(remote.id),
);
remoteCredential.write("remote-token");
assertEqual(remoteCredential.read(), "remote-token");
assertEqual(connectionStorage.getItem(BROWSER_ACCESS_TOKEN_STORAGE_KEY), null);
connections.remove(remote.id);
assertEqual(connections.active(), undefined);
assertEqual(connections.list().length, 0);
assertEqual(connectionStorage.getItem(HOST_CONNECTIONS_STORAGE_KEY), null);

const installationContexts = new BrowserInstallationTargetContextStore(connectionStorage, remote.id);
installationContexts.set("installation-one", "target-a");
assertEqual(installationContexts.get("installation-one"), "target-a");
const otherHostContexts = new BrowserInstallationTargetContextStore(
  connectionStorage,
  "https://other.example",
);
assertEqual(otherHostContexts.get("installation-one"), undefined);
installationContexts.clear("installation-one");
assertEqual(installationContexts.get("installation-one"), undefined);
