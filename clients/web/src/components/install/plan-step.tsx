import { type ReactNode } from "react";
import {
  CheckCircle,
  Download,
  Folder as PIconFolder,
  GitBranch as PIconGit,
  GithubLogo,
  Globe as PIconGlobe,
  Key as PIconKey,
  Warning,
} from "@/components/icons";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/input";
import { ModalFooter, ModalHeader } from "@/components/ui/modal";
import { StatusPill } from "@/components/ui/status-pill";
import { EyebrowSm } from "@/components/ui/typography";
import { cn } from "@/lib/cn";
import { useT } from "@/lib/locale";
import type { InstallDetectedKind, InstallPlan } from "@/protocol/client";
import { formatDetectedKind, formatList, shortHash, summarizeConformance } from "./install-format";

export function PlanStep({
  url,
  plan,
  detectedKind,
  approvedPermissions,
  onApprovalChange,
  onBack,
  onCancel,
  onConfirm,
  installing,
}: {
  url: string;
  plan: InstallPlan;
  detectedKind: InstallDetectedKind | null;
  approvedPermissions: boolean;
  onApprovalChange: (v: boolean) => void;
  onBack: () => void;
  onCancel: () => void;
  onConfirm: () => void;
  installing: boolean;
}) {
  const t = useT();
  const rootPackage = plan.packages.find((pkg) => pkg.id === plan.root_id) ?? plan.packages[0];
  const permissionGroups = [
    {
      id: "capabilities",
      label: t("installPermissionCapabilities"),
      detail: formatList(plan.permissions_summary.new_capabilities, t("installNoNewCapabilityInvokes"), (count) => t("installListMore", count)),
      count: plan.permissions_summary.new_capabilities.length,
    },
    {
      id: "network",
      label: t("installPermissionNetwork"),
      detail: formatList(plan.permissions_summary.new_network_hosts, t("installNoNewNetworkHosts"), (count) => t("installListMore", count)),
      count: plan.permissions_summary.new_network_hosts.length,
    },
    {
      id: "secrets",
      label: t("installPermissionSecrets"),
      detail: formatList(plan.permissions_summary.new_secret_refs, t("installNoNewSecretRefs"), (count) => t("installListMore", count)),
      count: plan.permissions_summary.new_secret_refs.length,
    },
  ];
  const conformance = summarizeConformance(plan, t);
  const isExternal = detectedKind?.kind === "external";

  return (
    <>
      <ModalHeader
        eyebrow={t("installPlanEyebrow")}
        title={t("installPlanTitle")}
        description={t("installPlanDescription")}
      />

      {/* Installation identity */}
      <div className="flex items-center gap-3 rounded-[12px] border border-whisper-border bg-aged-brass-surface-soft px-4 py-3">
        <GithubLogo size={20} className="text-charcoal-ink shrink-0" />
        <span className="flex-1 truncate font-mono text-[13px] text-charcoal-ink">{url}</span>
        <StatusPill tone="accent" label={t("installResolved")} />
        <span className="font-mono text-[11px] text-muted-tone">{t("installRootPrefix")} {plan.root_id}</span>
      </div>

      {isExternal ? (
        <div className="mt-4 rounded-[12px] border border-aged-brass-border bg-aged-brass-surface-soft px-4 py-3 text-[12px] text-charcoal-ink">
          <div className="flex items-start gap-2">
            <Warning size={15} className="mt-0.5 shrink-0 text-aged-brass-deep" />
            <div>
              <p className="font-medium">{t("installExternalCliOnlyTitle")}</p>
              <p className="mt-1 text-steel-secondary">
                {t("installExternalCliOnlyBody")}
              </p>
            </div>
          </div>
        </div>
      ) : null}

      {/* Installation metadata */}
      <section className="mt-6">
        <EyebrowSm>{t("installInstallationSection")}</EyebrowSm>
        <dl className="mt-3 grid grid-cols-2 gap-x-8 gap-y-2 text-[12px]">
          {[
            [t("installKindLabel"), formatDetectedKind(detectedKind, t), "accent"],
            [t("installRootPackageLabel"), plan.root_id, "mono"],
            [t("installVersionLabel"), rootPackage?.version ?? "—", "mono"],
            [t("installSourceMetaLabel"), rootPackage?.source ?? "—"],
            [t("installCommitLabel"), rootPackage?.commit_sha ? shortHash(rootPackage.commit_sha) : "—", "mono"],
            [t("installSignedLabel"), plan.signature_summary.all_signed ? t("installAllSigned") : t("installUnsignedPackages"), plan.signature_summary.all_signed ? "accent" : "neutral"],
          ].map(([label, value, hint]) => (
            <div key={label as string} className="flex justify-between">
              <dt className="font-medium text-steel-secondary">{label}</dt>
              <dd className={cn("text-charcoal-ink", hint === "mono" && "font-mono")}>
                {hint === "accent" ? (
                  <span className="inline-flex items-center gap-1.5">
                    <span className="size-1.5 rounded-full bg-aged-brass" aria-hidden />
                    {value}
                  </span>
                ) : hint === "neutral" ? (
                  <span className="inline-flex items-center gap-1.5">
                    <span className="size-1.5 rounded-full bg-steel-secondary" aria-hidden />
                    {value}
                  </span>
                ) : (
                  value
                )}
              </dd>
            </div>
          ))}
        </dl>
      </section>

      <div className="my-6 h-px bg-whisper-border" />

      {/* Dependencies */}
      <section>
        <div className="flex items-center justify-between">
          <EyebrowSm>{t("installPackagesSection")}</EyebrowSm>
          <span className="text-[11px] text-steel-secondary">
            {t("installPackagesWillInstall", plan.packages.length)}
          </span>
        </div>
        <ul className="mt-3 divide-y divide-whisper-border">
          {plan.packages.map((pkg) => (
            <li key={`${pkg.id}:${pkg.tree_hash}`} className="flex items-center gap-3 py-2.5">
              <PackageMarker signed={pkg.signed} />
              <span className="flex-1 truncate font-mono text-[12px] text-charcoal-ink">{pkg.id}</span>
              <span className="font-mono text-[11px] text-muted-tone">{pkg.version}</span>
              <StatusPill tone={pkg.id === plan.root_id ? "accent" : "neutral"} label={pkg.source.toUpperCase()} showDot={false} />
            </li>
          ))}
        </ul>
      </section>

      <div className="my-6 h-px bg-whisper-border" />

      {/* Permissions */}
      <section>
        <div className="flex items-center justify-between">
          <EyebrowSm>{t("installPermissionsRequested")}</EyebrowSm>
          <span className="text-[11px] text-steel-secondary">
            {t("installTotalEntries", permissionGroups.reduce((sum, group) => sum + group.count, 0))}
          </span>
        </div>
        <ul className="mt-3 divide-y divide-whisper-border">
          {permissionGroups.map((p) => (
            <li key={p.id} className="flex gap-3 py-3">
              <span className="rounded-full bg-aged-brass-surface-soft p-2 text-aged-brass shrink-0">
                <PermissionIcon id={p.id} />
              </span>
              <div className="min-w-0 flex-1">
                <p className="text-[12px] font-medium text-charcoal-ink">{p.label}</p>
                <p className="mt-0.5 truncate text-[11px] text-steel-secondary">{p.detail}</p>
              </div>
            </li>
          ))}
        </ul>
      </section>

      <div className="my-6 h-px bg-whisper-border" />

      <section className="grid gap-3 text-[12px] md:grid-cols-3">
        <SummaryCard
          icon={<CheckCircle size={16} className="text-aged-brass" />}
          title={t("installSignaturesTitle")}
          body={
            plan.signature_summary.all_signed
              ? t("installAllSigned")
              : `${t("installUnsignedPrefix")} ${formatList(plan.signature_summary.unsigned_packages, t("installNone"), (count) => t("installListMore", count))}`
          }
        />
        <SummaryCard
          icon={plan.integrity_summary.manifest_hashes_match_lockfile ? <CheckCircle size={16} className="text-aged-brass" /> : <Warning size={16} className="text-aged-brass-deep" />}
          title={t("installIntegrityTitle")}
          body={
            plan.integrity_summary.manifest_hashes_match_lockfile
              ? t("installNoLockfileDrift")
              : t("installDriftItems", plan.integrity_summary.drift_detected.length)
          }
        />
        <SummaryCard
          icon={conformance.hasFailures ? <Warning size={16} className="text-aged-brass-deep" /> : <CheckCircle size={16} className="text-aged-brass" />}
          title={t("installConformanceTitle")}
          body={conformance.label}
        />
      </section>

      <ModalFooter className="justify-between">
        <Checkbox
          checked={approvedPermissions}
          onCheckedChange={onApprovalChange}
          label={t("installApprovePermissions")}
        />
        <div className="flex items-center gap-3">
          <Button tone="tertiary" onClick={onBack} disabled={installing}>
            {t("back")}
          </Button>
          <Button tone="secondary" onClick={onCancel} disabled={installing}>
            {t("cancel")}
          </Button>
          <Button tone="primary" onClick={onConfirm} disabled={!approvedPermissions || installing || isExternal}>
            <Download size={14} />
            {installing ? t("installInstalling") : t("installInstallButton")}
          </Button>
        </div>
      </ModalFooter>
    </>
  );
}

function PermissionIcon({ id }: { id: string }) {
  switch (id) {
    case "network":
      return <PIconGlobe size={16} />;
    case "secrets":
      return <PIconKey size={16} />;
    case "filesystem":
      return <PIconFolder size={16} />;
    default:
      return <PIconGit size={16} />;
  }
}

function PackageMarker({ signed }: { signed: boolean }) {
  return signed ? (
    <CheckCircle size={14} className="shrink-0 text-aged-brass" weight="fill" />
  ) : (
    <span className="flex size-3.5 shrink-0 items-center justify-center rounded-full border border-whisper-border-strong/70">
      <span className="size-1.5 rounded-full bg-muted-tone" aria-hidden />
    </span>
  );
}

function SummaryCard({ icon, title, body }: { icon: ReactNode; title: string; body: string }) {
  return (
    <div className="rounded-[12px] border border-whisper-border bg-pure-surface p-3">
      <div className="flex items-center gap-2">
        {icon}
        <p className="font-medium text-charcoal-ink">{title}</p>
      </div>
      <p className="mt-2 line-clamp-2 text-[11px] leading-snug text-steel-secondary">{body}</p>
    </div>
  );
}
