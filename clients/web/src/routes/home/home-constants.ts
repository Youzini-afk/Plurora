import type { FilterChip } from "@/components/home/utility-strip";

export const FILTER_OPTIONS: FilterChip[] = [
  { id: "all", label: "All", count: 0 },
  { id: "running", label: "Running", count: 0, toneDot: "running" },
  { id: "stopped", label: "Stopped", count: 0, toneDot: "stopped" },
  { id: "failed", label: "Failed", count: 0, toneDot: "failed" },
];

export const TONE_TO_DISK_CLASS: Record<string, string> = {
  running: "bg-aged-brass",
  stopped: "bg-steel-secondary",
  failed: "bg-deep-rust",
};

export const TIMELINE_SESSION = "host_installation_lifecycle";
