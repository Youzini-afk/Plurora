/**
 * Format helpers used across the settings panels and Home.
 */

export interface RelativeAgeLabels {
  now: string;
  minutesAgo: (count: number) => string;
  hoursAgo: (count: number) => string;
  daysAgo: (count: number) => string;
  weeksAgo: (count: number) => string;
  monthsAgo: (count: number) => string;
  yearsAgo: (count: number) => string;
}

const DEFAULT_RELATIVE_AGE_LABELS: RelativeAgeLabels = {
  now: "just now",
  minutesAgo: (count) => `${count} minute${count === 1 ? "" : "s"} ago`,
  hoursAgo: (count) => `${count} hour${count === 1 ? "" : "s"} ago`,
  daysAgo: (count) => `${count} day${count === 1 ? "" : "s"} ago`,
  weeksAgo: (count) => `${count} week${count === 1 ? "" : "s"} ago`,
  monthsAgo: (count) => `${count} month${count === 1 ? "" : "s"} ago`,
  yearsAgo: (count) => `${count} year${count === 1 ? "" : "s"} ago`,
};

export function formatRelativeAge(
  timestamp: string | number | Date | undefined,
  labels: RelativeAgeLabels = DEFAULT_RELATIVE_AGE_LABELS,
): string {
  if (!timestamp) return labels.now;
  const value = timestamp instanceof Date ? timestamp.getTime() : new Date(timestamp).getTime();
  if (Number.isNaN(value)) return labels.now;
  const diff = Date.now() - value;
  if (diff < 0) return labels.now;
  if (diff < 60_000) return labels.now;
  if (diff < 3_600_000) return labels.minutesAgo(Math.floor(diff / 60_000));
  if (diff < 86_400_000) return labels.hoursAgo(Math.floor(diff / 3_600_000));
  if (diff < 604_800_000) return labels.daysAgo(Math.floor(diff / 86_400_000));
  if (diff < 2_628_000_000) return labels.weeksAgo(Math.floor(diff / 604_800_000));
  if (diff < 31_536_000_000) return labels.monthsAgo(Math.floor(diff / 2_628_000_000));
  const years = Math.floor(diff / 31_536_000_000);
  return labels.yearsAgo(years);
}

export function formatBytes(bytes: number | undefined, fractionDigits = 1): string {
  if (bytes == null || Number.isNaN(bytes)) return "—";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1_048_576) return `${(bytes / 1024).toFixed(fractionDigits)} KB`;
  if (bytes < 1_073_741_824) return `${(bytes / 1_048_576).toFixed(fractionDigits)} MB`;
  return `${(bytes / 1_073_741_824).toFixed(fractionDigits)} GB`;
}

export function formatGreetingTime(locale: string = "en", now = new Date(), prefix = "WORKSHOP"): string {
  const day = now.toLocaleDateString([locale, "en"], { weekday: "short" }).toUpperCase();
  const time = now.toLocaleTimeString([locale, "en"], { hour: "2-digit", minute: "2-digit", hour12: false });
  return `${prefix} · ${day} ${time}`;
}

/** Categorize a package id for filtering. Package ids are not Installation ids. */
export function classifyPackageKind(packageId: string): "EXTERNAL" | "PLURORA" | "THIRD-PARTY" {
  if (packageId.startsWith("plurora/") || packageId.startsWith("plurora__")) return "PLURORA";
  if (
    packageId.includes("__") ||
    packageId.startsWith("github__") ||
    packageId.startsWith("local__") ||
    packageId.includes("/")
  ) {
    return "EXTERNAL";
  }
  return "THIRD-PARTY";
}
