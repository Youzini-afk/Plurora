import {
  ArrowsClockwise,
  CheckCircle,
  Cloud,
  Globe,
  Key,
  Package,
  Stack,
  Warning,
} from "@/components/icons";
import { Card } from "@/components/ui/card";
import { Eyebrow } from "@/components/ui/typography";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/cn";
import { STATUS_DOT_CLASS, type StatusTone } from "@/components/ui/status-pill";

const eventIconMap: Record<string, typeof Globe> = {
  default: Stack,
  outbound: Globe,
  secret: Key,
  package: Package,
  failure: Warning,
  checkpoint: CheckCircle,
  retry: ArrowsClockwise,
  install: Cloud,
};

export interface TimelineRow {
  id: string;
  installationName: string;
  toneDot: StatusTone;
  age: string;
  message: string;
  iconKind?: keyof typeof eventIconMap;
  action?: { label: string; onClick: () => void };
}

export function ActivityTimeline({
  rows,
  loading = false,
  onViewAll,
  title,
  emptyLabel,
  viewAllLabel,
}: {
  rows: TimelineRow[];
  loading?: boolean;
  onViewAll?: () => void;
  title: string;
  emptyLabel: string;
  viewAllLabel: string;
}) {
  return (
    <section className="flex min-h-0 flex-col gap-3 lg:flex-1">
      <Eyebrow>{title}</Eyebrow>
      <Card className="flex min-h-[160px] flex-1 flex-col">
        {loading ? (
          <ul className="divide-y divide-whisper-border">
            {Array.from({ length: 4 }).map((_, idx) => (
              <li key={idx} className="flex gap-3 px-5 py-3">
                <Skeleton className="size-1.5 mt-1 rounded-full" />
                <div className="flex-1 space-y-1.5">
                  <Skeleton className="h-3 w-24" />
                  <Skeleton className="h-2.5 w-64" />
                </div>
              </li>
            ))}
          </ul>
        ) : rows.length === 0 ? (
          <div className="flex flex-1 flex-col items-center justify-center gap-2 px-6 py-12 text-center">
            <Stack size={20} className="text-muted-tone" />
            <p className="text-[13px] text-muted-tone">{emptyLabel}</p>
          </div>
        ) : (
          <ul className="divide-y divide-whisper-border">
            {rows.map((row) => {
              const Icon = eventIconMap[row.iconKind ?? "default"] ?? eventIconMap.default;
              return (
                <li key={row.id} className="flex gap-3 px-5 py-3">
                  <div className="flex flex-col items-center pt-0.5">
                    <span
                      className={cn("size-1.5 rounded-full", STATUS_DOT_CLASS[row.toneDot])}
                      aria-hidden
                    />
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="flex items-baseline justify-between gap-2">
                      <p className="text-[12px] font-medium text-charcoal-ink">{row.installationName}</p>
                      <span className="font-mono text-[10px] text-muted-tone">{row.age}</span>
                    </div>
                    <div className="mt-0.5 flex items-center gap-2 text-[11px] text-charcoal-ink">
                      <Icon size={12} className="shrink-0 text-steel-secondary" />
                      <span className="min-w-0 flex-1 truncate">{row.message}</span>
                      {row.action ? (
                        <button
                          type="button"
                          onClick={row.action.onClick}
                          className="shrink-0 font-medium text-charcoal-ink underline underline-offset-4 decoration-1 hover:decoration-aged-brass"
                        >
                          {row.action.label}
                        </button>
                      ) : null}
                    </div>
                  </div>
                </li>
              );
            })}
          </ul>
        )}
        {rows.length > 0 ? (
          <div className="border-t border-whisper-border px-5 py-3 text-right">
            <button
              type="button"
              onClick={onViewAll}
              className="text-[12px] font-medium text-charcoal-ink underline underline-offset-4 decoration-1 hover:decoration-aged-brass"
            >
              {viewAllLabel}
            </button>
          </div>
        ) : null}
      </Card>
    </section>
  );
}
