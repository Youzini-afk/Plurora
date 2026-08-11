import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/cn";

const pillVariants = cva(
  "inline-flex items-center gap-1.5 font-mono text-[11px] font-medium uppercase tracking-[0.06em] rounded-full px-2.5 py-1",
  {
    variants: {
      tone: {
        running: "bg-aged-brass-surface text-aged-brass-deep",
        stopped: "bg-whisper-border-strong/40 text-steel-secondary",
        starting: "bg-whisper-border-strong/40 text-muted-tone",
        failed: "bg-deep-rust-surface text-deep-rust",
        interrupted: "bg-deep-rust-surface text-deep-rust",
        // Update is muted+shimmer per spec — brass is reserved for Running.
        update: "bg-whisper-border-strong/40 text-steel-secondary",
        neutral: "bg-whisper-border-strong/40 text-steel-secondary",
        accent: "bg-aged-brass-surface text-aged-brass-deep",
      },
    },
    defaultVariants: { tone: "neutral" },
  },
);

const dotVariants = cva("inline-block size-1.5 rounded-full", {
  variants: {
    tone: {
      running: "bg-aged-brass animate-[pulse-dot_2.4s_ease-in-out_infinite]",
      stopped: "bg-steel-secondary",
      starting: "bg-muted-tone",
      failed: "bg-deep-rust",
      interrupted: "bg-deep-rust",
      update: "bg-muted-tone shimmer",
      neutral: "bg-steel-secondary",
      accent: "bg-aged-brass",
    },
  },
  defaultVariants: { tone: "neutral" },
});

export interface StatusPillProps extends VariantProps<typeof pillVariants> {
  label: string;
  className?: string;
  showDot?: boolean;
}

export function StatusPill({ tone, label, className, showDot = true }: StatusPillProps) {
  return (
    <span className={cn(pillVariants({ tone }), className)}>
      {showDot ? <span className={dotVariants({ tone })} aria-hidden /> : null}
      {label}
    </span>
  );
}

export type StatusTone = NonNullable<VariantProps<typeof pillVariants>["tone"]>;

/**
 * Centralized dot-color mapping. Imported by activity-timeline /
 * continue-card / utility-strip / installation-card so the four files
 * cannot drift out of sync.
 */
export const STATUS_DOT_CLASS: Record<StatusTone, string> = {
  running: "bg-aged-brass",
  stopped: "bg-steel-secondary",
  starting: "bg-muted-tone",
  failed: "bg-deep-rust",
  interrupted: "bg-deep-rust",
  update: "bg-muted-tone",
  neutral: "bg-steel-secondary",
  accent: "bg-aged-brass",
};

const STATE_TO_TONE: Record<string, StatusTone> = {
  installed: "stopped",
  stopped: "stopped",
  starting: "starting",
  stopping: "starting",
  // 'ready' is steady inactive — no pulse.
  ready: "stopped",
  running: "running",
  failed: "failed",
  interrupted: "interrupted",
  degraded: "failed",
  loading: "starting",
  archived: "stopped",
  planned: "stopped",
  applying: "starting",
  active: "running",
  outcome_unknown: "interrupted",
  recovery_required: "interrupted",
};

export function installationStateTone(state: string): StatusTone {
  return STATE_TO_TONE[state] ?? "neutral";
}

export function runStateTone(state: string): StatusTone {
  return STATE_TO_TONE[state] ?? "neutral";
}

export function realizationStateTone(state: string): StatusTone {
  return STATE_TO_TONE[state] ?? "neutral";
}
