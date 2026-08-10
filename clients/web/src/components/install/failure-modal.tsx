import { ArrowsClockwise, Copy, GithubLogo, Warning } from "@/components/icons";
import { Button } from "@/components/ui/button";
import { Modal, ModalHeader, ModalFooter } from "@/components/ui/modal";
import { useToast } from "@/components/ui/toast";
import { useT } from "@/lib/locale";

export interface FailureDetail {
  installationName: string;
  installationId?: string;
  runId?: string;
  reasonCode?: string;
  nextStep?: string;
  installation_id?: string;
  run_id?: string;
  reason_code?: string;
  next_step?: string;
  icon?: React.ReactNode;
  title?: string;
  summary?: string;
  log?: string[];
  /** Real failure metadata from platform events, when available. */
  exitCode?: string;
  cause?: string;
  uptime?: string;
  lastCheckpoint?: string;
  failedAt?: string;
  redactionState?: string;
  logRedacted?: boolean;
}

export interface FailureModalProps {
  open: boolean;
  onClose: () => void;
  onRestart?: () => void;
  onUninstall?: () => void;
  detail?: FailureDetail;
}

export function FailureModal({ open, onClose, onRestart, onUninstall, detail }: FailureModalProps) {
  const toast = useToast();
  const t = useT();

  const installationName = detail?.installationName ?? t("failureInstallationFallback");
  const log = detail?.log ?? [];
  const canCopyLog = detail?.logRedacted === true && log.length > 0;

  const copy = () => {
    if (!canCopyLog) return;
    navigator.clipboard?.writeText(log.join("\n"));
    toast.push({ variant: "success", title: t("failureLogCopied"), duration: 2400 });
  };

  return (
    <Modal open={open} onOpenChange={onClose} accent="rust" size="lg" contentLabel={t("failureContentLabel", installationName)}>
      <ModalHeader
        eyebrow={t("failureEyebrow", installationName)}
        title={detail?.title ?? t("failureTitle")}
        description={
          detail?.summary ?? t("failureDescription")
        }
      />

      {/* Identity row */}
      <div className="flex items-center gap-3 rounded-[12px] border border-whisper-border px-4 py-3">
        {detail?.icon ?? <Warning size={20} className="text-deep-rust" />}
        <span className="font-display text-[14px] font-bold text-charcoal-ink">{installationName}</span>
        <GithubLogo size={14} className="ml-2 text-steel-secondary" />
        {detail?.failedAt ? (
          <span className="ml-auto font-mono text-[11px] text-muted-tone">{detail.failedAt}</span>
        ) : null}
      </div>
      {detail?.installationId || detail?.installation_id || detail?.runId || detail?.run_id || detail?.reasonCode || detail?.reason_code || detail?.nextStep || detail?.next_step ? (
        <div className="mt-3 rounded-[12px] border border-whisper-border bg-warm-bone px-4 py-3 text-[11px]">
          {detail.installationId || detail.installation_id ? <p><span className="text-steel-secondary">Installation</span> <span className="font-mono">{detail.installationId ?? detail.installation_id}</span></p> : null}
          {detail.runId || detail.run_id ? <p className="mt-1"><span className="text-steel-secondary">Run</span> <span className="font-mono">{detail.runId ?? detail.run_id}</span></p> : null}
          {detail.reasonCode || detail.reason_code ? <p className="mt-1"><span className="text-steel-secondary">Reason</span> <span className="font-mono text-deep-rust">{detail.reasonCode ?? detail.reason_code}</span></p> : null}
          {detail.nextStep || detail.next_step ? <p className="mt-1 text-steel-secondary">Next step: <span className="text-charcoal-ink">{detail.nextStep ?? detail.next_step}</span></p> : null}
        </div>
      ) : null}

      {/* Diagnosis & impact */}
      <div className="mt-6 grid grid-cols-2 gap-6">
        <DiagnosisColumn
          eyebrow={t("failureDiagnosis")}
          rows={[
            [t("failureExitCode"), detail?.exitCode ?? "—"],
            [t("failureCause"), detail?.cause ?? "—"],
            [t("failureUptime"), detail?.uptime ?? "—"],
          ]}
        />
        <DiagnosisColumn
          eyebrow={t("failureImpact")}
          rows={[
            [t("failureLastCheckpoint"), detail?.lastCheckpoint ?? "—"],
            [t("failureSessions"), t("failureSessionsPreserved")],
          ]}
        />
      </div>

      <div className="my-6 h-px bg-whisper-border" />

      {/* Log */}
      <section>
        <div className="flex items-center justify-between">
          <p className="font-mono text-[10px] uppercase tracking-[0.12em] text-muted-tone">
            {t("failureRedactedStderr", log.length)}
          </p>
          <div className="flex items-center gap-3">
            <button
              type="button"
              onClick={copy}
              disabled={!canCopyLog}
              className="inline-flex items-center gap-1 text-[11px] font-medium text-charcoal-ink underline underline-offset-4 decoration-1 hover:decoration-aged-brass"
            >
              <Copy size={12} />
              {canCopyLog ? t("failureCopyLog") : t("failureNoRedactedLog")}
            </button>
          </div>
        </div>
        <div
          className="mt-2 space-y-0.5 rounded-[10px] p-3 font-mono text-[11px] leading-relaxed text-charcoal-ink"
          style={{ background: "var(--color-inset-surface)" }}
        >
          {log.length > 0 ? (
            log.map((line, idx) => (
              <p key={idx} className={idx === log.length - 1 ? "text-deep-rust" : ""}>
                {line}
              </p>
            ))
          ) : (
            <p className="text-muted-tone">{t("failureNoDiagnosticLog")}</p>
          )}
        </div>
      </section>

      <ModalFooter className="justify-end">
        {/* No telemetry promise on About page; do not offer report opt-in
            until a real, audited reporting capability lands. */}
        <div className="flex items-center gap-3">
          <Button tone="tertiary" onClick={onClose}>
            {t("close")}
          </Button>
          <Button
            tone="destructive"
            onClick={() => {
              onUninstall?.();
              onClose();
            }}
          >
            {t("failureStopAndUninstall")}
          </Button>
          <Button
            tone="primary"
            onClick={() => {
              onRestart?.();
              onClose();
            }}
          >
            <ArrowsClockwise size={14} />
            {t("failureRestartInstallation")}
          </Button>
        </div>
      </ModalFooter>
    </Modal>
  );
}

function DiagnosisColumn({
  eyebrow,
  rows,
}: {
  eyebrow: string;
  rows: Array<[string, string]>;
}) {
  return (
    <div>
      <p className="font-mono text-[10px] uppercase tracking-[0.12em] text-muted-tone">{eyebrow}</p>
      <dl className="mt-3 space-y-2 text-[12px]">
        {rows.map(([label, value]) => (
          <div key={label} className="flex justify-between gap-4">
            <dt className="font-medium text-steel-secondary">{label}</dt>
            <dd className="font-mono text-charcoal-ink">{value}</dd>
          </div>
        ))}
      </dl>
    </div>
  );
}
