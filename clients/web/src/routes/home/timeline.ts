import type { TimelineRow } from "@/components/home/activity-timeline";
import type { PlatformEvent } from "@/protocol/client";

/**
 * Map an event payload's structural hints to a timeline icon. Heuristic only —
 * we deliberately do not parse package-internal payloads.
 */
export function iconKindFor(event: PlatformEvent): TimelineRow["iconKind"] {
  const kind = event.kind.toLowerCase();
  if (kind.includes("fail")) return "failure";
  if (kind.includes("install")) return "package";
  if (kind.includes("checkpoint")) return "checkpoint";
  if (kind.includes("retry")) return "retry";
  if (kind.includes("outbound")) return "outbound";
  if (kind.includes("secret")) return "secret";
  return "default";
}
