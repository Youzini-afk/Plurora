const DEFAULT_LOCAL_HOST = "http://127.0.0.1:8787";
export const HOST_CONNECTIONS_STORAGE_KEY = "plurora_host_connections_v1";
export const CURRENT_HOST_CONNECTION_ID = "current-origin";
const MAX_HOST_CONNECTIONS = 32;

export interface HostEndpointProvider {
  readonly baseUrl: string;
}

export interface HostConnectionProfile {
  id: string;
  name: string;
  baseUrl: string;
}

interface StoredHostConnections {
  version: 1;
  activeProfileId?: string;
  profiles: HostConnectionProfile[];
}

type HostConnectionStorage = Pick<Storage, "getItem" | "setItem" | "removeItem">;

export interface PluroraRuntimeBootstrap {
  hostBaseUrl?: string;
  platform?: "web" | "desktop" | "pwa";
  profile?: string;
}

declare global {
  interface Window {
    __PLURORA_RUNTIME__?: PluroraRuntimeBootstrap;
  }
}

export function normalizeHostBaseUrl(value: string): string {
  return value.replace(/\/+$/, "");
}

function currentStorage(): HostConnectionStorage | undefined {
  try {
    return typeof window === "undefined" ? undefined : window.localStorage;
  } catch {
    return undefined;
  }
}

function isLoopbackHostname(hostname: string): boolean {
  const normalized = hostname.toLowerCase();
  if (normalized === "localhost" || normalized === "[::1]") return true;
  const octets = normalized.split(".");
  return (
    octets.length === 4 &&
    octets[0] === "127" &&
    octets.every((octet) => /^\d{1,3}$/.test(octet) && Number(octet) <= 255)
  );
}

export function normalizeHostConnectionBaseUrl(value: string): string {
  let url: URL;
  try {
    url = new URL(value.trim());
  } catch {
    throw new Error("Host endpoint must be an absolute HTTP(S) URL");
  }
  if (url.protocol !== "http:" && url.protocol !== "https:") {
    throw new Error("Host endpoint must use HTTP or HTTPS");
  }
  if (url.username || url.password || url.search || url.hash) {
    throw new Error("Host endpoint cannot contain credentials, query, or fragment");
  }
  if (url.pathname !== "/" && url.pathname !== "") {
    throw new Error("Host endpoint must not contain a path");
  }
  if (url.port === "0") {
    throw new Error("Host endpoint port must be non-zero");
  }
  if (url.protocol === "http:" && !isLoopbackHostname(url.hostname)) {
    throw new Error("Remote Host endpoints must use HTTPS");
  }
  return `${url.protocol}//${url.host}`;
}

function normalizeProfile(input: unknown): HostConnectionProfile | undefined {
  if (!input || typeof input !== "object") return undefined;
  const candidate = input as Partial<HostConnectionProfile>;
  const name = typeof candidate.name === "string" ? candidate.name.trim() : "";
  if (!name || name.length > 64 || /[\u0000-\u001f\u007f]/.test(name)) return undefined;
  try {
    if (typeof candidate.baseUrl !== "string") return undefined;
    const baseUrl = normalizeHostConnectionBaseUrl(candidate.baseUrl);
    return { id: baseUrl, name, baseUrl };
  } catch {
    return undefined;
  }
}

export class BrowserHostConnectionStore {
  constructor(private readonly storage: HostConnectionStorage | undefined = currentStorage()) {}

  list(): HostConnectionProfile[] {
    return this.read().profiles;
  }

  active(): HostConnectionProfile | undefined {
    const state = this.read();
    return state.profiles.find((profile) => profile.id === state.activeProfileId);
  }

  save(nameInput: string, baseUrlInput: string): HostConnectionProfile {
    const profile = normalizeProfile({ id: "", name: nameInput, baseUrl: baseUrlInput });
    if (!profile) throw new Error("Host connection name or endpoint is invalid");
    const state = this.read();
    const existing = state.profiles.findIndex((candidate) => candidate.id === profile.id);
    if (existing >= 0) state.profiles[existing] = profile;
    else {
      if (state.profiles.length >= MAX_HOST_CONNECTIONS) {
        throw new Error(`At most ${MAX_HOST_CONNECTIONS} Host connections can be saved`);
      }
      state.profiles.push(profile);
    }
    this.write(state);
    return profile;
  }

  select(profileId?: string): void {
    const state = this.read();
    if (profileId && !state.profiles.some((profile) => profile.id === profileId)) {
      throw new Error("Host connection does not exist");
    }
    state.activeProfileId = profileId;
    this.write(state);
  }

  remove(profileId: string): void {
    const state = this.read();
    state.profiles = state.profiles.filter((profile) => profile.id !== profileId);
    if (state.activeProfileId === profileId) state.activeProfileId = undefined;
    this.write(state);
  }

  private read(): StoredHostConnections {
    try {
      const parsed = JSON.parse(this.storage?.getItem(HOST_CONNECTIONS_STORAGE_KEY) ?? "null") as
        | Partial<StoredHostConnections>
        | null;
      const profiles = Array.isArray(parsed?.profiles)
        ? parsed.profiles
            .map(normalizeProfile)
            .filter((profile): profile is HostConnectionProfile => !!profile)
            .slice(0, MAX_HOST_CONNECTIONS)
        : [];
      const activeProfileId =
        typeof parsed?.activeProfileId === "string" &&
        profiles.some((profile) => profile.id === parsed.activeProfileId)
          ? parsed.activeProfileId
          : undefined;
      return { version: 1, activeProfileId, profiles };
    } catch {
      return { version: 1, profiles: [] };
    }
  }

  private write(state: StoredHostConnections): void {
    try {
      if (!state.activeProfileId && state.profiles.length === 0) {
        this.storage?.removeItem(HOST_CONNECTIONS_STORAGE_KEY);
      } else {
        this.storage?.setItem(HOST_CONNECTIONS_STORAGE_KEY, JSON.stringify(state));
      }
    } catch {
      throw new Error("Browser storage is unavailable");
    }
  }
}

export function activeHostConnectionProfile(): HostConnectionProfile | undefined {
  return new BrowserHostConnectionStore().active();
}

export function activeHostCredentialScope(): string {
  return activeHostConnectionProfile()?.id ?? CURRENT_HOST_CONNECTION_ID;
}

/** Resolve the control-plane endpoint once, independently of React. */
export function resolveHostBaseUrl(explicit?: string): string {
  if (explicit) return normalizeHostBaseUrl(explicit);

  if (typeof window !== "undefined") {
    const injected = window.__PLURORA_RUNTIME__?.hostBaseUrl;
    if (injected) return normalizeHostBaseUrl(injected);

    const selected = activeHostConnectionProfile();
    if (selected) return selected.baseUrl;
  }

  if (typeof location !== "undefined" && location.origin && location.origin !== "null") {
    return normalizeHostBaseUrl(location.origin);
  }

  return DEFAULT_LOCAL_HOST;
}

export function createHostEndpointProvider(explicit?: string): HostEndpointProvider {
  return Object.freeze({ baseUrl: resolveHostBaseUrl(explicit) });
}
