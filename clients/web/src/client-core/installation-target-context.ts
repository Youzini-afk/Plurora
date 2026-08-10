import { activeHostCredentialScope } from "./host-endpoint";

export const INSTALLATION_TARGET_CONTEXT_STORAGE_KEY = "plurora_installation_target_context_v1";

type ContextStorage = Pick<Storage, "getItem" | "setItem" | "removeItem">;

function currentStorage(): ContextStorage | undefined {
  try {
    return typeof window === "undefined" ? undefined : window.localStorage;
  } catch {
    return undefined;
  }
}

function validContextId(value: string): boolean {
  return value.length > 0 && value.length <= 256 && !/[\u0000-\u001f\u007f]/.test(value);
}

export class BrowserInstallationTargetContextStore {
  private readonly key: string;

  constructor(
    private readonly storage: ContextStorage | undefined = currentStorage(),
    hostScope: string = activeHostCredentialScope(),
  ) {
    this.key = `${INSTALLATION_TARGET_CONTEXT_STORAGE_KEY}:${encodeURIComponent(hostScope)}`;
  }

  get(installationId: string): string | undefined {
    return this.read()[installationId];
  }

  set(installationId: string, targetId: string): void {
    if (!validContextId(installationId) || !validContextId(targetId)) {
      throw new Error("Installation and target context ids must be bounded non-empty strings");
    }
    const contexts = this.read();
    contexts[installationId] = targetId;
    this.write(contexts);
  }

  clear(installationId: string): void {
    const contexts = this.read();
    delete contexts[installationId];
    this.write(contexts);
  }

  private read(): Record<string, string> {
    try {
      const parsed = JSON.parse(this.storage?.getItem(this.key) ?? "null") as unknown;
      if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return {};
      return Object.fromEntries(
        Object.entries(parsed).filter(
          ([installationId, targetId]) =>
            validContextId(installationId) && typeof targetId === "string" && validContextId(targetId),
        ),
      );
    } catch {
      return {};
    }
  }

  private write(contexts: Record<string, string>): void {
    try {
      if (Object.keys(contexts).length === 0) this.storage?.removeItem(this.key);
      else this.storage?.setItem(this.key, JSON.stringify(contexts));
    } catch {
      // Context is a convenience preference; requests still carry explicit ids.
    }
  }
}
