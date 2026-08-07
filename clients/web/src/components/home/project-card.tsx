import { motion } from "motion/react";
import { DotsThree, Play, ArrowsClockwise } from "@/components/icons";
import { Button } from "@/components/ui/button";
import { CardTitle } from "@/components/ui/typography";
import { SPRING } from "@/lib/motion";
import {
  Dropdown,
  DropdownTrigger,
  DropdownMenu,
  DropdownItem,
  DropdownSeparator,
} from "@/components/ui/dropdown";
import { StatusPill, projectStateTone, type StatusTone } from "@/components/ui/status-pill";
import { Tooltip } from "@/components/ui/tooltip";
import { cn } from "@/lib/cn";
import { projectIcon } from "@/lib/project-icon";

export interface ProjectCardData {
  id: string;
  title: string;
  description?: string;
  state: string; // running/stopped/starting/failed/installed
  type: "plurora_native" | "external_wrapped" | "external_workspace";
  version?: string;
  source?: string; // github / local / etc
  sizeMB?: number;
  metricsLine?: string; // "12 sessions · 4 ext · last 2h"
  failureLine?: string; // shown for failed state
}

export interface ProjectCardActions {
  onLaunch: () => void;
  onStop?: () => void;
  onRestart?: () => void;
  onUninstall: () => void;
  onViewLogs?: () => void;
  onConfigure?: () => void;
}

export interface ProjectCardLabels {
  restart: string;
  resume: string;
  play: string;
  more: string;
  actionsAria: (title: string) => string;
  stop: string;
  open: string;
  configure: string;
  viewLogs: string;
  uninstall: string;
}

const DEFAULT_LABELS: ProjectCardLabels = {
  restart: "Restart",
  resume: "Resume",
  play: "Play",
  more: "More",
  actionsAria: (title) => `${title} actions`,
  stop: "Stop",
  open: "Open",
  configure: "Configure…",
  viewLogs: "View logs",
  uninstall: "Uninstall…",
};

const iconToneClass: Record<StatusTone, string> = {
  running: "text-aged-brass",
  stopped: "text-steel-secondary",
  starting: "text-muted-tone",
  failed: "text-deep-rust",
  update: "text-aged-brass",
  neutral: "text-steel-secondary",
  accent: "text-aged-brass",
};

export function ProjectCard({
  data,
  actions,
  index = 0,
  labels: labelOverrides,
}: {
  data: ProjectCardData;
  actions: ProjectCardActions;
  index?: number;
  labels?: Partial<ProjectCardLabels>;
}) {
  const labels = { ...DEFAULT_LABELS, ...labelOverrides };
  const tone = projectStateTone(data.state);
  const Icon = projectIcon(data);
  const isFailed = data.state === "failed";
  const isRunning = data.state === "running";

  const primaryLabel = isFailed ? labels.restart : isRunning ? labels.resume : labels.play;
  const onPrimary = isFailed ? actions.onRestart ?? actions.onLaunch : actions.onLaunch;
  const PrimaryIcon = isFailed ? ArrowsClockwise : Play;

  const metaParts = [
    data.version ? data.version : null,
    data.source ?? null,
    typeof data.sizeMB === "number" ? `${data.sizeMB.toFixed(1)} MB` : null,
  ].filter(Boolean) as string[];

  return (
    <motion.article
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ delay: Math.min(index, 11) * 0.06, ...SPRING.soft }}
      whileHover={{ y: -2 }}
      className={cn(
        "group flex flex-col rounded-[20px] border border-whisper-border bg-pure-surface p-5 shadow-card transition-shadow hover:shadow-card-hover",
      )}
    >
      <header className="mb-4 flex items-start justify-between">
        <span className={cn("size-8 [&>svg]:size-7", iconToneClass[tone])}>
          <Icon size={28} />
        </span>
        <StatusPill tone={tone} label={data.state.toUpperCase()} />
      </header>

      <CardTitle className="text-[18px]">{data.title}</CardTitle>
      {data.description ? (
        <p className="mt-1 line-clamp-2 text-[13px] leading-snug text-steel-secondary">
          {data.description}
        </p>
      ) : null}

      <div className="my-4 h-px bg-whisper-border" />

      <dl className="space-y-1 text-[11px]">
        <dd className="font-mono text-muted-tone">{metaParts.join(" · ") || "—"}</dd>
        {data.failureLine ? (
          <dd className="text-deep-rust">{data.failureLine}</dd>
        ) : data.metricsLine ? (
          <dd className="text-steel-secondary">{data.metricsLine}</dd>
        ) : null}
      </dl>

      <footer className="mt-auto pt-4 flex items-center justify-between gap-2">
        <Button tone="primary" size="sm" onClick={onPrimary}>
          <PrimaryIcon size={14} weight="fill" />
          {primaryLabel}
        </Button>
        <Dropdown>
          {/* Tooltip wraps DropdownTrigger (not the other way around) so Radix
              gets a real button child for asChild forwarding. */}
          <Tooltip label={labels.more}>
            <DropdownTrigger asChild>
              <Button tone="icon" size="icon-sm" aria-label={labels.actionsAria(data.title)}>
                <DotsThree size={16} />
              </Button>
            </DropdownTrigger>
          </Tooltip>
          <DropdownMenu>
            {isRunning ? (
              <DropdownItem onSelect={actions.onStop ?? (() => {})}>{labels.stop}</DropdownItem>
            ) : (
              <DropdownItem onSelect={actions.onLaunch}>{labels.open}</DropdownItem>
            )}
            {actions.onConfigure ? (
              <DropdownItem onSelect={actions.onConfigure}>{labels.configure}</DropdownItem>
            ) : null}
            {actions.onViewLogs ? (
              <DropdownItem onSelect={actions.onViewLogs}>{labels.viewLogs}</DropdownItem>
            ) : null}
            <DropdownSeparator />
            <DropdownItem destructive onSelect={actions.onUninstall}>
              {labels.uninstall}
            </DropdownItem>
          </DropdownMenu>
        </Dropdown>
      </footer>
    </motion.article>
  );
}
