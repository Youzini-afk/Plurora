import { motion } from "motion/react";
import { ArrowRight, DotsThree } from "@/components/icons";
import { Button } from "@/components/ui/button";
import { CardTitle } from "@/components/ui/typography";
import { StatusPill, installationStateTone } from "@/components/ui/status-pill";
import { Dropdown, DropdownItem, DropdownMenu, DropdownSeparator, DropdownTrigger } from "@/components/ui/dropdown";
import { Tooltip } from "@/components/ui/tooltip";
import { installationIcon } from "@/lib/installation-icon";

export interface InstallationCardData {
  installationId: string;
  displayName: string;
  status: string;
  revision: number;
  sourceKind?: string;
  updatedAt?: string;
}

export function InstallationCard({
  data,
  onOpen,
  onRemove,
  index = 0,
}: {
  data: InstallationCardData;
  onOpen: () => void;
  onRemove?: () => void;
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
      <CardTitle className="mt-4 text-[18px]">{data.displayName}</CardTitle>
      <p className="mt-2 font-mono text-[11px] text-muted-tone">{data.installationId}</p>
      <div className="my-4 h-px bg-whisper-border" />
      <dl className="space-y-1 text-[11px] text-steel-secondary">
        <div className="flex justify-between"><dt>Revision</dt><dd className="font-mono">{data.revision}</dd></div>
        <div className="flex justify-between"><dt>Source</dt><dd>{data.sourceKind ?? "—"}</dd></div>
        {data.updatedAt ? <div className="flex justify-between"><dt>Updated</dt><dd>{new Date(data.updatedAt).toLocaleString()}</dd></div> : null}
      </dl>
      <footer className="mt-auto flex items-center justify-between gap-2 pt-5">
        <Button tone="primary" size="sm" onClick={onOpen}><ArrowRight size={14} /> Open</Button>
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
