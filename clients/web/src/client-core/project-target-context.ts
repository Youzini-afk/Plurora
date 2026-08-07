import { activeHostCredentialScope } from "./host-endpoint";

export const PROJECT_TARGET_CONTEXT_STORAGE_KEY = "plurora_project_target_context_v1";

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

export class BrowserProjectTargetContextStore {
  private readonly key: string;

  constructor(
    private readonly storage: ContextStorage | undefined = currentStorage(),
    hostScope: string = activeHostCredentialScope(),
  ) {
    this.key = `${PROJECT_TARGET_CONTEXT_STORAGE_KEY}:${encodeURIComponent(hostScope)}`;
  }

  get(projectId: string): string | undefined {
    return this.read()[projectId];
  }

  set(projectId: string, targetId: string): void {
    if (!validContextId(projectId) || !validContextId(targetId)) {
      throw new Error("Project and target context ids must be bounded non-empty strings");
    }
    const contexts = this.read();
    contexts[projectId] = targetId;
    this.write(contexts);
  }

  clear(projectId: string): void {
    const contexts = this.read();
    delete contexts[projectId];
    this.write(contexts);
  }

  private read(): Record<string, string> {
    try {
      const parsed = JSON.parse(this.storage?.getItem(this.key) ?? "null") as unknown;
      if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return {};
      return Object.fromEntries(
        Object.entries(parsed).filter(
          ([projectId, targetId]) =>
            validContextId(projectId) && typeof targetId === "string" && validContextId(targetId),
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
