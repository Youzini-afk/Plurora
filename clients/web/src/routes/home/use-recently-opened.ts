const STORAGE_KEY = "plurora-recently-opened";
const CAP = 8;

export interface Entry {
  installationId: string;
  openedAt: number;
}

interface Store {
  entries: Entry[];
}

export interface StorageLike {
  getItem: (key: string) => string | null;
  setItem: (key: string, value: string) => void;
}

function defaultStorage(): StorageLike | null {
  try {
    if (typeof localStorage !== "undefined") return localStorage;
  } catch { /* no-op */ }
  return null;
}

export function loadStore(storage: StorageLike | null = defaultStorage()): Entry[] {
  try {
    const raw = storage?.getItem(STORAGE_KEY) ?? null;
    if (!raw) return [];
    const parsed = JSON.parse(raw) as unknown;
    if (!parsed || typeof parsed !== "object" || !("entries" in parsed)) return [];
    const entries = (parsed as Store).entries;
    if (!Array.isArray(entries)) return [];
    return entries.filter(
      (e): e is Entry =>
        e &&
        typeof e === "object" &&
        typeof e.installationId === "string" &&
        typeof e.openedAt === "number",
    );
  } catch {
    return [];
  }
}

export function saveStore(entries: Entry[], storage: StorageLike | null = defaultStorage()) {
  try {
    storage?.setItem(STORAGE_KEY, JSON.stringify({ entries }));
  } catch {
    // storage quota or private-browsing: best-effort.
  }
}

export function recordOpen(installationId: string, storage: StorageLike | null = defaultStorage()) {
  const current = loadStore(storage);
  const deduped = current.filter((e) => e.installationId !== installationId);
  const next: Entry[] = [{ installationId, openedAt: Date.now() }, ...deduped].slice(0, CAP);
  saveStore(next, storage);
}

export function clearStore(storage: StorageLike | null = defaultStorage()) {
  saveStore([], storage);
}

export function useRecentlyOpened() {
  // Intentionally not reactive across renders: Home remounts when returning.
  const list = loadStore();

  return {
    list,
    recordOpen: (installationId: string) => recordOpen(installationId),
    clear: () => clearStore(),
  };
}
