/**
 * Deterministic icon mapping for installation cards.
 *
 * Installations don't ship icons themselves — the platform picks a calm Phosphor
 * glyph based on the installation's title or kind. Mapping is stable so the same
 * installation always gets the same icon.
 */

import type { ComponentType } from "react";
import {
  Aperture,
  BookOpenText,
  Camera,
  GitBranch,
  Globe,
  Newspaper,
  Package,
  Plug,
  Stack,
  Terminal,
} from "@/components/icons";

const KEYWORD_MAP: Array<[RegExp, ComponentType<{ size?: number; className?: string }>]> = [
  [/tavern|chat|roleplay|persona|character/i, BookOpenText],
  [/code|repl|coding|workshop|terminal|cli/i, Terminal],
  [/image|studio|render|aperture|photo/i, Aperture],
  [/camera|video|capture/i, Camera],
  [/news|story|journal|writer/i, Newspaper],
  [/agent|director|crew/i, GitBranch],
  [/world|map|atlas|globe/i, Globe],
  [/lab|toolkit|kit/i, Plug],
];

export function installationIcon(
  installation: { title?: string; type?: string },
): ComponentType<{ size?: number; className?: string }> {
  const haystack = (installation.title ?? "") + " " + (installation.type ?? "");
  for (const [pattern, Icon] of KEYWORD_MAP) {
    if (pattern.test(haystack)) return Icon;
  }
  return Stack;
}

export const Package_ = Package;
