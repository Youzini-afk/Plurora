export type InstallStep = "url" | "plan" | "progress" | "external";

export type InstallPhase = "resolving" | "detecting" | "reviewed" | "executing" | "completed" | "failed";

export interface ShortcutEntry {
  url: string;
  tag: string;
}

export const SHORTCUTS: ShortcutEntry[] = [
  { url: "https://github.com/Youzini-afk/Plurora-Tavern", tag: "native" },
];
