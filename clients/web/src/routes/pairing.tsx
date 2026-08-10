import { useEffect, useMemo, useState } from "react";
import { CheckCircle, ShieldCheck, Warning } from "@/components/icons";
import { LocaleSwitcher } from "@/components/layout/locale-switcher";
import { Button } from "@/components/ui/button";
import { Card, CardSection } from "@/components/ui/card";
import {
  claimHostPairing,
  inspectHostPairing,
  type HostPairing,
  type HostAccessResourceSelector,
  type HostAccessScope,
} from "@/client-core/host-access";
import { PendingPairingCredentialLease } from "@/client-core/pairing-credential";
import { useT } from "@/lib/locale";

const pairingCredential = new PendingPairingCredentialLease();

export function PairingPage() {
  const t = useT();
  const token = useMemo(() => pairingCredential.resolve(), []);
  const [pairing, setPairing] = useState<HostPairing | null>(null);
  const [status, setStatus] = useState<"loading" | "ready" | "claiming" | "complete" | "error">(
    token ? "loading" : "error",
  );
  const [error, setError] = useState<"missing" | "invalid" | null>(token ? null : "missing");
  const secureContext = typeof window === "undefined" || window.location.protocol === "https:";

  useEffect(() => {
    if (!token) return;
    let active = true;
    void inspectHostPairing(token)
      .then((record) => {
        if (!active) return;
        setPairing(record);
        setStatus("ready");
      })
      .catch(() => {
        if (!active) return;
        setError("invalid");
        setStatus("error");
      });
    return () => {
      active = false;
    };
  }, [token]);

  const claim = async () => {
    if (!token || !secureContext) return;
    setStatus("claiming");
    setError(null);
    try {
      await claimHostPairing(token);
      pairingCredential.clear();
      setStatus("complete");
    } catch {
      setError("invalid");
      setStatus("error");
    }
  };

  return (
    <main className="plurora-safe-page flex min-h-[100dvh] flex-col bg-warm-bone text-charcoal-ink">
      <header className="flex items-center justify-between px-4 py-4 sm:px-6">
        <span className="font-display text-[18px] font-bold tracking-[-0.015em]">Plurora</span>
        <LocaleSwitcher />
      </header>
      <div className="mx-auto flex w-full max-w-[620px] flex-1 items-center px-4 py-8 sm:px-6">
        <Card className="w-full overflow-hidden">
          <CardSection className="px-5 py-6 sm:px-8 sm:py-8">
            <div className="mb-5 flex size-12 items-center justify-center rounded-[16px] bg-aged-brass-surface text-aged-brass-deep">
              <ShieldCheck size={26} weight="duotone" />
            </div>
            <p className="eyebrow mb-2">{t("pairEyebrow")}</p>
            <h1 className="font-display text-[clamp(1.75rem,6vw,2.5rem)] font-bold leading-[1.05] tracking-[-0.025em]">
              {status === "complete" ? t("pairCompleteTitle") : t("pairTitle")}
            </h1>
            <p className="mt-3 max-w-[52ch] text-[14px] leading-6 text-steel-secondary">
              {status === "complete" ? t("pairCompleteBody") : t("pairBody")}
            </p>
          </CardSection>

          {status === "loading" ? (
            <CardSection divided className="px-5 py-6 sm:px-8">
              <p className="text-[13px] text-steel-secondary">{t("pairLoading")}</p>
            </CardSection>
          ) : null}

          {pairing && status !== "complete" ? (
            <CardSection divided className="space-y-5 px-5 py-6 sm:px-8">
              <PairingDetail label={t("pairDevice")} value={pairing.device_name} />
              <div>
                <p className="mb-2 text-[11px] font-semibold uppercase tracking-[0.12em] text-muted-tone">
                  {t("pairPermissions")}
                </p>
                <div className="flex flex-wrap gap-2">
                  {pairing.scopes.map((scope) => (
                    <span
                      key={scope}
                      className="rounded-full border border-whisper-border bg-aged-brass-surface px-2.5 py-1 text-[11px] font-medium text-charcoal-ink"
                    >
                      {scopeLabel(scope, t)}
                    </span>
                  ))}
                </div>
              </div>
              <div>
                <p className="mb-2 text-[11px] font-semibold uppercase tracking-[0.12em] text-muted-tone">
                  {t("pairResources")}
                </p>
                <div className="flex flex-wrap gap-2">
                  {(pairing.resources ?? []).map((resource, index) => (
                    <span
                      key={`${resource.kind}:${resource.id ?? "*"}:${index}`}
                      className="rounded-full border border-whisper-border bg-aged-brass-surface px-2.5 py-1 text-[11px] font-medium text-charcoal-ink"
                    >
                      {pairingResourceLabel(resource, t)}
                    </span>
                  ))}
                </div>
              </div>
              <PairingDetail
                label={t("pairGrantExpires")}
                value={new Date(pairing.grant_expires_at_ms).toLocaleString()}
              />
            </CardSection>
          ) : null}

          {!secureContext && status !== "complete" ? (
            <CardSection divided className="flex gap-3 bg-deep-rust-surface px-5 py-5 sm:px-8">
              <Warning className="mt-0.5 shrink-0 text-deep-rust" size={20} />
              <div>
                <p className="text-[13px] font-semibold">{t("pairSecureRequiredTitle")}</p>
                <p className="mt-1 text-[12px] leading-5 text-steel-secondary">
                  {t("pairSecureRequiredBody")}
                </p>
              </div>
            </CardSection>
          ) : null}

          {error ? (
            <CardSection divided className="flex gap-3 px-5 py-5 sm:px-8">
              <Warning className="mt-0.5 shrink-0 text-deep-rust" size={20} />
              <div>
                <p className="text-[13px] font-semibold">{t("pairInvalidTitle")}</p>
                <p className="mt-1 text-[12px] leading-5 text-steel-secondary">
                  {error === "missing" ? t("pairMissingBody") : t("pairInvalidBody")}
                </p>
              </div>
            </CardSection>
          ) : null}

          <CardSection divided className="flex flex-col-reverse gap-3 px-5 py-5 sm:flex-row sm:justify-end sm:px-8">
            {status === "complete" ? (
              <Button tone="primary" size="lg" onClick={() => window.location.replace("/")}>
                <CheckCircle size={18} weight="fill" />
                {t("pairOpenHost")}
              </Button>
            ) : (
              <Button
                tone="primary"
                size="lg"
                disabled={status !== "ready" || !secureContext}
                onClick={() => void claim()}
              >
                {status === "claiming" ? t("pairClaiming") : t("pairConfirm")}
              </Button>
            )}
          </CardSection>
        </Card>
      </div>
    </main>
  );
}

function PairingDetail({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid gap-1 sm:grid-cols-[140px_1fr] sm:gap-4">
      <span className="text-[11px] font-semibold uppercase tracking-[0.12em] text-muted-tone">
        {label}
      </span>
      <span className="min-w-0 break-words text-[13px] font-medium">{value}</span>
    </div>
  );
}

function scopeLabel(scope: HostAccessScope, t: ReturnType<typeof useT>): string {
  const keys: Record<HostAccessScope, Parameters<typeof t>[0]> = {
    observe: "accessScopeObserve",
    "installation.manage": "accessScopeInstallationOperate",
    run: "accessScopeInstallationOperate",
    "binding.manage": "accessScopeManage",
    "exposure.manage": "accessScopeManage",
    "realization.plan": "accessScopeDeploy",
    "realization.apply": "accessScopeDeploy",
    deploy: "accessScopeDeploy",
    "develop.propose": "accessScopeDevelopPropose",
    "develop.approve": "accessScopeDevelopApprove",
    "develop.execute": "accessScopeDevelopExecute",
    access_manage: "accessScopeManage",
  };
  return t(keys[scope]);
}

function pairingResourceLabel(
  resource: HostAccessResourceSelector,
  t: ReturnType<typeof useT>,
): string {
  if (resource.kind === "installation") {
    return resource.id ? t("accessInstallationResource", resource.id) : t("accessAllInstallations");
  }
  if (resource.kind === "target") return resource.id ? t("accessTargetResource", resource.id) : t("accessAllTargets");
  return resource.id ? `${resource.kind} · ${resource.id}` : `${resource.kind} · all`;
}
