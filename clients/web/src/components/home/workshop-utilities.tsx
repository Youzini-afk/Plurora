import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Eyebrow, EyebrowSm } from "@/components/ui/typography";
import { cn } from "@/lib/cn";
import { formatBytes } from "@/lib/format";
import { QuickActionList } from "@/surfaces/shell-contribution-renderers";
import type { QuickActionContribution, WorkshopCardContribution } from "@/surfaces/shell-contributions";
import { WorkshopCardList } from "@/surfaces/shell-contribution-renderers";

export interface UpdateEntry {
  id: string;
  packageId: string;
  fromVersion: string;
  toVersion: string;
  onUpdate: () => void;
}

export interface DiskSegment {
  id: string;
  label: string;
  bytes: number | null;
  measurementState?: string;
  toneClass: string;
}

export interface WorkshopUtilitiesProps {
  updates: UpdateEntry[];
  totalDisk: number; // bytes
  diskSegments: DiskSegment[];
  diskCapacity: number; // bytes
  quickActions: QuickActionContribution[];
  workshopCards?: WorkshopCardContribution[];
  onQuickActionClick?: (action: QuickActionContribution) => void;
  onWorkshopCardClick?: (card: WorkshopCardContribution) => void;
  onUpdateAll?: () => void;
  onManageStorage?: () => void;
  labels?: Partial<WorkshopUtilitiesLabels>;
}

export interface WorkshopUtilitiesLabels {
  workshop: string;
  updates: string;
  updatesAvailable: (count: number) => string;
  everythingUpToDate: string;
  update: string;
  updateAll: string;
  diskUsage: string;
  diskUsed: (value: string) => string;
  unknown: string;
  measuring: string;
  noStorageMeasured: string;
  manageStorage: string;
  workshopCards: string;
  categoryTool: string;
  categoryTemplate: string;
  categoryExample: string;
  quickActions: string;
}

const DEFAULT_LABELS: WorkshopUtilitiesLabels = {
  workshop: "Workshop",
  updates: "Updates",
  updatesAvailable: (count) => `${count} available`,
  everythingUpToDate: "Everything is up to date.",
  update: "Update",
  updateAll: "Update all →",
  diskUsage: "Disk usage",
  diskUsed: (value) => `${value} used`,
  unknown: "Unknown",
  measuring: "Measuring",
  noStorageMeasured: "No installation storage measured.",
  manageStorage: "Manage storage →",
  workshopCards: "",
  categoryTool: "",
  categoryTemplate: "",
  categoryExample: "",
  quickActions: "Quick actions",
};

function isMeasured(segment: DiskSegment): segment is DiskSegment & { bytes: number } {
  return typeof segment.bytes === "number" && Number.isFinite(segment.bytes);
}

function formatSegmentBytes(segment: DiskSegment, labels: WorkshopUtilitiesLabels): string {
  if (isMeasured(segment)) return formatBytes(segment.bytes);
  return segment.measurementState === "measuring" ? labels.measuring : labels.unknown;
}

export function WorkshopUtilities({
  updates,
  totalDisk,
  diskSegments,
  diskCapacity,
  quickActions,
  workshopCards = [],
  onQuickActionClick,
  onWorkshopCardClick,
  onUpdateAll,
  onManageStorage,
  labels: labelOverrides,
}: WorkshopUtilitiesProps) {
  const labels = { ...DEFAULT_LABELS, ...labelOverrides };
  const measuredSegments = diskSegments.filter(isMeasured);
  const hasMeasuredStorage = measuredSegments.length > 0;
  const hasPositiveStorage = totalDisk > 0 && measuredSegments.some((segment) => segment.bytes > 0);

  return (
    <section className="flex min-h-0 flex-col gap-3 lg:flex-[1.15]">
      <Eyebrow>{labels.workshop}</Eyebrow>
      <Card className="flex flex-1 flex-col divide-y divide-whisper-border">
        {/* Updates */}
        <div className="flex flex-col gap-3 p-5">
          <div className="flex items-center justify-between">
            <EyebrowSm>{labels.updates}</EyebrowSm>
            <span className="font-mono text-[10px] text-aged-brass">
              {labels.updatesAvailable(updates.length)}
            </span>
          </div>
          {updates.length === 0 ? (
            <p className="text-[12px] text-muted-tone">{labels.everythingUpToDate}</p>
          ) : (
            <ul className="space-y-2">
              {updates.map((update) => (
                <li key={update.id} className="flex items-center justify-between gap-2">
                  <div className="min-w-0 flex-1">
                    <p className="truncate font-mono text-[12px] text-charcoal-ink">{update.packageId}</p>
                    <p className="font-mono text-[10px] text-steel-secondary">
                      {update.fromVersion} → {update.toVersion}
                    </p>
                  </div>
                  <Button tone="secondary" size="sm" onClick={update.onUpdate} className="h-7 px-2 text-[11px]">
                    {labels.update}
                  </Button>
                </li>
              ))}
            </ul>
          )}
          {updates.length > 0 ? (
            <button
              type="button"
              onClick={onUpdateAll}
              className="self-start text-[12px] font-medium text-charcoal-ink underline underline-offset-4 decoration-1 hover:decoration-aged-brass"
            >
              {labels.updateAll}
            </button>
          ) : null}
        </div>

        {/* Disk */}
        <div className="flex flex-1 flex-col gap-2 p-5">
          <div className="flex items-center justify-between">
            <EyebrowSm>{labels.diskUsage}</EyebrowSm>
            <span className="font-mono text-[10px] text-charcoal-ink">
              {hasMeasuredStorage ? labels.diskUsed(formatBytes(totalDisk)) : labels.unknown}
            </span>
          </div>
          <div className="flex h-1.5 w-full overflow-hidden rounded-full bg-whisper-border-strong/40">
            {hasPositiveStorage
              ? measuredSegments
                  .filter((segment) => segment.bytes > 0)
                  .map((segment) => (
                    <span
                      key={segment.id}
                      className={cn("h-full", segment.toneClass)}
                      style={{ width: `${Math.min(100, (segment.bytes / Math.max(diskCapacity, 1)) * 100)}%` }}
                      title={`${segment.label} · ${formatBytes(segment.bytes)}`}
                    />
                  ))
              : null}
          </div>
          {diskSegments.length === 0 ? (
            <p className="text-[12px] text-muted-tone">{labels.noStorageMeasured}</p>
          ) : hasMeasuredStorage ? (
            <ul className="flex flex-wrap gap-x-3 gap-y-1 text-[10px] text-muted-tone">
              {diskSegments.map((segment) => (
                <li key={segment.id} className="flex items-center gap-1">
                  <span className={cn("size-1.5 rounded-full", segment.toneClass)} aria-hidden />
                  {segment.label} · {formatSegmentBytes(segment, labels)}
                </li>
              ))}
            </ul>
          ) : (
            <p className="text-[12px] text-muted-tone">{labels.noStorageMeasured}</p>
          )}
          <button
            type="button"
            onClick={onManageStorage}
            className="self-start text-[12px] font-medium text-charcoal-ink underline underline-offset-4 decoration-1 hover:decoration-aged-brass"
          >
            {labels.manageStorage}
          </button>
        </div>

        {workshopCards.length > 0 ? (
          <div className="flex flex-col gap-3 p-5">
            <EyebrowSm>{labels.workshopCards}</EyebrowSm>
            <WorkshopCardList
              items={workshopCards}
              onCardClick={onWorkshopCardClick}
              ariaLabel={labels.workshopCards}
              categoryLabels={{
                tool: labels.categoryTool,
                template: labels.categoryTemplate,
                example: labels.categoryExample,
              }}
              className="[&>div]:grid-cols-1 [&>div]:gap-2"
            />
          </div>
        ) : null}

        {/* Quick actions */}
        <div className="flex flex-col gap-2 p-5">
          <EyebrowSm>{labels.quickActions}</EyebrowSm>
          <QuickActionList items={quickActions} onActionClick={onQuickActionClick} ariaLabel={labels.quickActions} />
        </div>
      </Card>
    </section>
  );
}
