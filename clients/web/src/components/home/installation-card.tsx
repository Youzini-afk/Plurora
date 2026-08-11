import { motion } from "motion/react";
import { ArrowRight, DotsThree } from "@/components/icons";
import { Button } from "@/components/ui/button";
import { CardTitle } from "@/components/ui/typography";
import { StatusPill, installationStateTone } from "@/components/ui/status-pill";
import { Dropdown, DropdownItem, DropdownMenu, DropdownSeparator, DropdownTrigger } from "@/components/ui/dropdown";
import { Tooltip } from "@/components/ui/tooltip";
import { installationIcon } from "@/lib/installation-icon";
import type { RunView, WorkEntrypoint } from "@/protocol/client";

export interface InstallationCardData {
  installationId: string;
  displayName: string;
  status: string;
  revision: number;
  sourceKind?: string;
  updatedAt?: string;
  workTitle?: string;
  workDescription?: string;
  workId?: string;
  entrypoints?: WorkEntrypoint[];
  activeRun?: RunView | null;
  sourceVisibility?: string;
  executeRight?: string;
  statePortability?: string;
}

export function InstallationCard({
  data,
  onOpen,
  onRemove,
  onPlay,
  onStop,
  busyAction,
  actionError,
  index = 0,
}: {
  data: InstallationCardData;
  onOpen: () => void;
  onRemove?: () => void;
  onPlay?: (entrypoint: WorkEntrypoint) => void;
  onStop?: () => void;
  busyAction?: "play" | "stop" | null;
  actionError?: { reasonCode: string; nextStep: string } | null;
  index?: number;
}) {
  const tone = installationStateTone(data.status);
  const Icon = installationIcon({ title: data.displayName, type: data.sourceKind });
  return (
    <motion.article
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ delay: Math.min(index, 11) * 0.04 }}
      className="flex flex-col rounded-[20px] border border-whisper-border bg-pure-surface p-5 shadow-card"
    >
      <header className="flex items-start justify-between">
        <Icon size={28} className="text-aged-brass" />
        <StatusPill tone={tone} label={data.status.replaceAll("_", " ").toUpperCase()} />
      </header>
      <CardTitle className="mt-4 text-[18px]">{data.workTitle ?? data.displayName}</CardTitle>
      {data.workDescription ? <p className="mt-1 line-clamp-2 text-[12px] text-steel-secondary">{data.workDescription}</p> : null}
      <p className="mt-2 font-mono text-[11px] text-muted-tone">{data.installationId}</p>
      <div className="my-4 h-px bg-whisper-border" />
      <dl className="space-y-1 text-[11px] text-steel-secondary">
        <div className="flex justify-between"><dt>Revision</dt><dd className="font-mono">{data.revision}</dd></div>
        <div className="flex justify-between"><dt>Source</dt><dd>{data.sourceKind ?? "—"}</dd></div>
        <div className="flex justify-between"><dt>Run</dt><dd>{data.activeRun?.record.status ?? "stopped"}</dd></div>
        <div className="flex justify-between"><dt>Source visibility</dt><dd>{data.sourceVisibility ?? "unknown"}</dd></div>
        <div className="flex justify-between"><dt>Execute right</dt><dd>{data.executeRight ?? "unspecified"}</dd></div>
        <div className="flex justify-between"><dt>State</dt><dd>{data.statePortability ?? "unknown"}</dd></div>
        {data.updatedAt ? <div className="flex justify-between"><dt>Updated</dt><dd>{new Date(data.updatedAt).toLocaleString()}</dd></div> : null}
      </dl>
      {data.entrypoints?.length ? (
        <div className="mt-4 flex flex-wrap gap-2">
          {data.entrypoints.map((entrypoint) => (
            <Button
              key={entrypoint.id}
              tone="secondary"
              size="sm"
              onClick={() => onPlay?.(entrypoint)}
              disabled={!onPlay || busyAction != null}
            >
              {busyAction === "play" ? "Checking…" : entrypoint.id}
            </Button>
          ))}
        </div>
      ) : null}
      {actionError ? (
        <div className="mt-3 rounded-[10px] border border-deep-rust/30 bg-deep-rust-surface px-3 py-2 text-[11px] text-deep-rust">
          <p className="font-mono">{actionError.reasonCode}</p>
          <p className="mt-1">{actionError.nextStep}</p>
        </div>
      ) : null}
      <footer className="mt-auto flex items-center justify-between gap-2 pt-5">
        <div className="flex items-center gap-2">
          <Button tone="primary" size="sm" onClick={onOpen}><ArrowRight size={14} /> Open</Button>
          {data.activeRun && onStop ? <Button tone="tertiary" size="sm" onClick={onStop} disabled={busyAction != null}>{busyAction === "stop" ? "Stopping…" : "Stop"}</Button> : null}
        </div>
        {onRemove ? (
          <Dropdown>
            <Tooltip label="More"><DropdownTrigger asChild><Button tone="icon" size="icon-sm" aria-label={`${data.displayName} actions`}><DotsThree size={16} /></Button></DropdownTrigger></Tooltip>
            <DropdownMenu><DropdownSeparator /><DropdownItem destructive onSelect={onRemove}>Remove</DropdownItem></DropdownMenu>
          </Dropdown>
        ) : null}
      </footer>
    </motion.article>
  );
}
