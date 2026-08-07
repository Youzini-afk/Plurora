import { activeHostCredentialScope, CURRENT_HOST_CONNECTION_ID } from "./host-endpoint";

export const BROWSER_ACCESS_TOKEN_STORAGE_KEY = "plurora_http_access_token";

export function browserAccessTokenStorageKey(
  scope: string = activeHostCredentialScope(),
): string {
  return scope === CURRENT_HOST_CONNECTION_ID
    ? BROWSER_ACCESS_TOKEN_STORAGE_KEY
    : `${BROWSER_ACCESS_TOKEN_STORAGE_KEY}:${encodeURIComponent(scope)}`;
}

export interface CredentialProvider {
  read(): string | undefined;
  write(token: string): void;
  clear(): void;
  consumeBootstrap(): string | undefined;
}

/**
 * Retains a scrubbed bootstrap credential in memory until the Host has made a
 * definitive authentication decision. This keeps transient connection errors
 * and React StrictMode probe restarts from discarding a one-time deep link.
 */
export class PendingCredentialLease {
  private consumedBootstrap = false;
  private pending: string | undefined;

  constructor(private readonly provider: CredentialProvider = new BrowserCredentialProvider()) {}

  resolve(): string | undefined {
    if (!this.consumedBootstrap) {
      this.pending = this.provider.consumeBootstrap();
      this.consumedBootstrap = true;
    }
    return this.pending ?? this.provider.read();
  }

  retain(token: string): void {
    this.pending = token;
  }

  clear(): void {
    this.pending = undefined;
  }
}

interface CredentialWindow {
  localStorage: Pick<Storage, "getItem" | "setItem" | "removeItem">;
  location: Pick<Location, "pathname" | "search" | "hash">;
  history: Pick<History, "state" | "replaceState">;
}

function currentWindow(): CredentialWindow | undefined {
  return typeof window === "undefined" ? undefined : window;
}

export class BrowserCredentialProvider implements CredentialProvider {
  constructor(
    private readonly browserWindow: CredentialWindow | undefined = currentWindow(),
    private readonly storageKey: string = browserAccessTokenStorageKey(),
  ) {}

  read(): string | undefined {
    try {
      return this.browserWindow?.localStorage.getItem(this.storageKey) ?? undefined;
    } catch {
      return undefined;
    }
  }

  write(token: string): void {
    try {
      this.browserWindow?.localStorage.setItem(this.storageKey, token);
    } catch {
      // Storage can be unavailable; AuthProvider still retains the token in memory.
    }
  }

  clear(): void {
    try {
      this.browserWindow?.localStorage.removeItem(this.storageKey);
    } catch {
      // Best effort only.
    }
  }

  consumeBootstrap(): string | undefined {
    const hostWindow = this.browserWindow;
    if (!hostWindow) return undefined;

    try {
      const params = new URLSearchParams(hostWindow.location.search);
      const primary = params.get("plurora_token")?.trim();
      const secondary = params.get("access_token")?.trim();
      const token = primary || secondary;
      const hadBootstrapParameter = params.has("plurora_token") || params.has("access_token");
      if (!hadBootstrapParameter) return this.read();

      params.delete("plurora_token");
      params.delete("access_token");
      const search = params.toString();
      const cleanUrl = `${hostWindow.location.pathname}${search ? `?${search}` : ""}${hostWindow.location.hash}`;
      hostWindow.history.replaceState(hostWindow.history.state, "", cleanUrl);
      return token || this.read();
    } catch {
      return undefined;
    }
  }
}

export function readBrowserAccessToken(): string | undefined {
  return new BrowserCredentialProvider().read();
}

export function storeBrowserAccessToken(token: string): void {
  new BrowserCredentialProvider().write(token);
}

export function clearBrowserAccessToken(): void {
  new BrowserCredentialProvider().clear();
}

export function resolveBrowserAccessToken(): string | undefined {
  return new BrowserCredentialProvider().consumeBootstrap();
}
