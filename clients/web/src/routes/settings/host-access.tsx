import { useCallback, useEffect, useMemo, useState } from "react";
import { ArrowsClockwise, Copy, ShieldCheck, Warning } from "@/components/icons";
import { Button } from "@/components/ui/button";
import { Card, CardRow, CardSection } from "@/components/ui/card";
import { Checkbox, Field, Input } from "@/components/ui/input";
import {
  cancelHostPairing,
  createHostPairing,
  getHostAccessOverview,
  revokeHostAccessGrant,
  type CreateHostPairingResponse,
  type HostAccessGrant,
  type HostAccessResourceKind,
  type HostAccessResourceSelector,
  type HostAccessScope,
  type HostPairing,
} from "@/client-core/host-access";
import { useAuth } from "@/lib/auth-gate";
import { useT } from "@/lib/locale";

const SCOPE_ORDER: HostAccessScope[] = [
  "observe",
  "project_operate",
  "deploy",
  "develop_propose",
  "develop_approve",
  "develop_execute",
  "access_manage",
];

export function HostAccessPanel() {
  const t = useT();
  const { token, identity } = useAuth();
  const [grants, setGrants] = useState<HostAccessGrant[]>([]);
  const [pairings, setPairings] = useState<HostPairing[]>([]);
  const [loading, setLoading] = useState(true);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [deviceName, setDeviceName] = useState("");
  const [grantDays, setGrantDays] = useState("90");
  const [allProjects, setAllProjects] = useState(true);
  const [projectIds, setProjectIds] = useState("");
  const [allTargets, setAllTargets] = useState(true);
  const [targetIds, setTargetIds] = useState("");
  const [linkBase, setLinkBase] = useState(() =>
    typeof window === "undefined" ? "" : window.location.origin,
  );
  const availableScopes = useMemo(
    () => new Set(identity?.scopes ?? []),
    [identity?.scopes],
  );
  const canManage = identity?.kind === "root" || availableScopes.has("access_manage");
  const defaultScopes = useMemo(
    () =>
      new Set(
        SCOPE_ORDER.filter(
          (scope) =>
            scope === "observe" &&
            (identity?.kind === "root" || availableScopes.has(scope)),
        ),
      ),
    [availableScopes, identity?.kind],
  );
  const [selectedScopes, setSelectedScopes] = useState<Set<HostAccessScope>>(defaultScopes);
  const [created, setCreated] = useState<CreateHostPairingResponse | null>(null);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    setSelectedScopes(defaultScopes);
  }, [defaultScopes]);

  useEffect(() => {
    if (!identity || identity.kind === "root" || !identity.resources) {
      setAllProjects(true);
      setProjectIds("");
      setAllTargets(true);
      setTargetIds("");
      return;
    }
    const projectResources = identity.resources.filter((resource) => resource.kind === "project");
    const targetResources = identity.resources.filter((resource) => resource.kind === "target");
    setAllProjects(projectResources.some((resource) => !resource.id));
    setProjectIds(projectResources.flatMap((resource) => resource.id ? [resource.id] : []).join(", "));
    setAllTargets(targetResources.some((resource) => !resource.id));
    setTargetIds(targetResources.flatMap((resource) => resource.id ? [resource.id] : []).join(", "));
  }, [identity]);

  const refresh = useCallback(async () => {
    if (!canManage) {
      setLoading(false);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const overview = await getHostAccessOverview(token);
      setGrants(overview.grants);
      setPairings(overview.pairings);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [canManage, token]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const pairingBase = normalizePairingBase(linkBase);
  const pairingLink = created && pairingBase
    ? `${pairingBase}/pair?pairing_token=${encodeURIComponent(created.pairing_token)}`
    : null;

  const toggleScope = (scope: HostAccessScope, checked: boolean) => {
    setSelectedScopes((current) => {
      const next = new Set(current);
      if (checked) next.add(scope);
      else if (scope !== "observe") next.delete(scope);
      return next;
    });
  };

  const createPairing = async () => {
    const days = Number.parseInt(grantDays, 10);
    if (!pairingBase || !deviceName.trim() || !Number.isInteger(days) || days < 1 || days > 365) {
      setError(t("accessCreateValidation"));
      return;
    }
    setBusyId("create");
    setError(null);
    setCreated(null);
    try {
      const resources: HostAccessResourceSelector[] = [
        ...(allProjects
          ? [{ kind: "project" as const }]
          : parseResourceIds(projectIds).map((id) => ({ kind: "project" as const, id }))),
        ...(allTargets
          ? [{ kind: "target" as const }]
          : parseResourceIds(targetIds).map((id) => ({ kind: "target" as const, id }))),
      ];
      const result = await createHostPairing(
        {
          device_name: deviceName.trim(),
          scopes: SCOPE_ORDER.filter((scope) => selectedScopes.has(scope)),
          resources,
          pairing_ttl_secs: 600,
          grant_ttl_secs: days * 24 * 60 * 60,
        },
        token,
      );
      setCreated(result);
      setCopied(false);
      await refresh();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusyId(null);
    }
  };

  const copyLink = async () => {
    if (!pairingLink) return;
    await navigator.clipboard.writeText(pairingLink);
    setCopied(true);
  };

  const shareLink = async () => {
    if (!pairingLink || !navigator.share) return;
    await navigator.share({ title: "Plurora", text: t("accessPairingShareText"), url: pairingLink });
  };

  const revoke = async (grant: HostAccessGrant) => {
    if (!window.confirm(t("accessRevokeConfirm", grant.device_name))) return;
    setBusyId(grant.id);
    setError(null);
    try {
      await revokeHostAccessGrant(grant.id, token);
      await refresh();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusyId(null);
    }
  };

  const cancel = async (pairing: HostPairing) => {
    setBusyId(pairing.id);
    setError(null);
    try {
      await cancelHostPairing(pairing.id, token);
      await refresh();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusyId(null);
    }
  };

  return (
    <div className="space-y-8">
      <header className="max-w-[760px]">
        <p className="eyebrow">{t("accessEyebrow")}</p>
        <h1 className="mt-2 font-display text-[clamp(2rem,5vw,3.25rem)] font-bold leading-[1.02] tracking-[-0.03em]">
          {t("accessTitle")}
        </h1>
        <p className="mt-3 text-[14px] leading-6 text-steel-secondary">{t("accessDescription")}</p>
      </header>

      <Card>
        <CardSection className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
          <div className="flex min-w-0 gap-3">
            <div className="flex size-10 shrink-0 items-center justify-center rounded-[12px] bg-aged-brass-surface text-aged-brass-deep">
              <ShieldCheck size={21} />
            </div>
            <div className="min-w-0">
              <p className="text-[13px] font-semibold">
                {identity?.device_name ?? t("accessIdentityUnknown")}
              </p>
              <p className="mt-1 text-[12px] text-steel-secondary">
                {identity?.kind === "device" ? t("accessDeviceIdentity") : t("accessRootIdentity")}
              </p>
              <div className="mt-2 flex flex-wrap gap-1.5">
                {(identity?.scopes ?? []).map((scope) => (
                  <ScopeChip key={scope} scope={scope} />
                ))}
              </div>
            </div>
          </div>
          {canManage ? (
            <Button tone="secondary" size="sm" disabled={loading} onClick={() => void refresh()}>
              <ArrowsClockwise size={15} className={loading ? "animate-spin" : undefined} />
              {t("accessRefresh")}
            </Button>
          ) : null}
        </CardSection>
      </Card>

      {!canManage ? (
        <Card>
          <CardSection className="flex gap-3">
            <Warning className="mt-0.5 shrink-0 text-aged-brass-deep" size={20} />
            <div>
              <p className="text-[13px] font-semibold">{t("accessLimitedTitle")}</p>
              <p className="mt-1 text-[12px] leading-5 text-steel-secondary">
                {t("accessLimitedBody")}
              </p>
            </div>
          </CardSection>
        </Card>
      ) : (
        <>
          <Card>
            <CardSection>
              <h2 className="font-display text-[20px] font-semibold">{t("accessCreateTitle")}</h2>
              <p className="mt-1 text-[13px] leading-5 text-steel-secondary">
                {t("accessCreateBody")}
              </p>
            </CardSection>
            <CardSection divided className="grid gap-5 md:grid-cols-2">
              <Field label={t("accessDeviceName")} required>
                <Input
                  value={deviceName}
                  maxLength={80}
                  placeholder={t("accessDevicePlaceholder")}
                  onChange={(event) => setDeviceName(event.target.value)}
                />
              </Field>
              <Field label={t("accessGrantDays")} helper={t("accessGrantDaysHelper")} required>
                <Input
                  type="number"
                  min={1}
                  max={365}
                  value={grantDays}
                  onChange={(event) => setGrantDays(event.target.value)}
                />
              </Field>
              <Field
                label={t("accessPublicHostUrl")}
                helper={pairingBase ? t("accessPublicHostUrlHelper") : t("accessHttpsRequired")}
                error={linkBase && !pairingBase ? t("accessHttpsRequired") : undefined}
                className="md:col-span-2"
              >
                <Input
                  type="url"
                  inputMode="url"
                  value={linkBase}
                  placeholder="https://host.example.com"
                  onChange={(event) => setLinkBase(event.target.value)}
                />
              </Field>
            </CardSection>
            <CardSection divided>
              <p className="mb-3 text-[12px] font-semibold">{t("accessPermissions")}</p>
              <div className="grid gap-3 sm:grid-cols-2">
                {SCOPE_ORDER.filter(
                  (scope) =>
                    (identity?.kind === "root" || availableScopes.has(scope)) &&
                    (scope !== "access_manage" || identity?.kind === "root"),
                ).map((scope) => (
                  <Checkbox
                    key={scope}
                    checked={selectedScopes.has(scope)}
                    disabled={scope === "observe"}
                    onCheckedChange={(checked) => toggleScope(scope, checked)}
                    className="items-start"
                    label={
                      <span>
                        <span className="block font-medium text-charcoal-ink">
                          {scopeLabel(scope, t)}
                        </span>
                        <span className="mt-0.5 block leading-5">{scopeDescription(scope, t)}</span>
                      </span>
                    }
                  />
                ))}
              </div>
            </CardSection>
            <CardSection divided className="grid gap-5 md:grid-cols-2">
              <Field label={t("accessProjectResources")} helper={t("accessProjectResourcesBody")}>
                <div className="space-y-3">
                  <Checkbox
                    checked={allProjects}
                    onCheckedChange={setAllProjects}
                    label={t("accessAllProjects")}
                  />
                  <Input
                    value={projectIds}
                    disabled={allProjects}
                    placeholder={t("accessProjectIdsPlaceholder")}
                    onChange={(event) => setProjectIds(event.target.value)}
                  />
                </div>
              </Field>
              <Field label={t("accessTargetResources")} helper={t("accessTargetResourcesBody")}>
                <div className="space-y-3">
                  <Checkbox
                    checked={allTargets}
                    onCheckedChange={setAllTargets}
                    label={t("accessAllTargets")}
                  />
                  <Input
                    value={targetIds}
                    disabled={allTargets}
                    placeholder={t("accessTargetIdsPlaceholder")}
                    onChange={(event) => setTargetIds(event.target.value)}
                  />
                </div>
              </Field>
            </CardSection>
            <CardSection divided className="flex justify-end">
              <Button
                tone="primary"
                size="lg"
                disabled={busyId === "create" || !selectedScopes.has("observe")}
                onClick={() => void createPairing()}
              >
                {busyId === "create" ? t("accessCreating") : t("accessCreateButton")}
              </Button>
            </CardSection>
          </Card>

          {pairingLink ? (
            <Card className="border-aged-brass/60">
              <CardSection>
                <p className="eyebrow">{t("accessOneTimeEyebrow")}</p>
                <h2 className="mt-2 font-display text-[20px] font-semibold">
                  {t("accessOneTimeTitle")}
                </h2>
                <p className="mt-1 text-[12px] leading-5 text-steel-secondary">
                  {t("accessOneTimeBody")}
                </p>
                <div className="mt-4 rounded-[12px] border border-whisper-border bg-warm-bone p-3 font-mono text-[11px] leading-5 break-all">
                  {pairingLink}
                </div>
                <div className="mt-4 flex flex-wrap gap-2">
                  <Button tone="primary" size="sm" onClick={() => void copyLink()}>
                    <Copy size={15} />
                    {copied ? t("accessCopied") : t("accessCopyLink")}
                  </Button>
                  {typeof navigator !== "undefined" && "share" in navigator ? (
                    <Button tone="secondary" size="sm" onClick={() => void shareLink()}>
                      {t("accessShareLink")}
                    </Button>
                  ) : null}
                </div>
              </CardSection>
            </Card>
          ) : null}

          {error ? (
            <div className="rounded-[12px] border border-deep-rust/40 bg-deep-rust-surface px-4 py-3 text-[12px] text-deep-rust">
              {error}
            </div>
          ) : null}

          <AccessRecords
            grants={grants}
            pairings={pairings}
            currentGrantId={identity?.grant_id ?? null}
            busyId={busyId}
            onRevoke={revoke}
            onCancel={cancel}
          />
        </>
      )}
    </div>
  );
}

function AccessRecords({
  grants,
  pairings,
  currentGrantId,
  busyId,
  onRevoke,
  onCancel,
}: {
  grants: HostAccessGrant[];
  pairings: HostPairing[];
  currentGrantId: string | null;
  busyId: string | null;
  onRevoke: (grant: HostAccessGrant) => Promise<void>;
  onCancel: (pairing: HostPairing) => Promise<void>;
}) {
  const t = useT();
  const pending = pairings.filter((pairing) => pairing.status === "pending");
  return (
    <div className="grid gap-6 xl:grid-cols-2">
      <Card>
        <CardSection>
          <h2 className="font-display text-[19px] font-semibold">{t("accessDevicesTitle")}</h2>
          <p className="mt-1 text-[12px] text-steel-secondary">{t("accessDevicesBody")}</p>
        </CardSection>
        <CardSection divided className="py-2">
          {grants.length === 0 ? (
            <p className="py-4 text-[12px] text-muted-tone">{t("accessNoDevices")}</p>
          ) : (
            grants.map((grant) => (
              <CardRow key={grant.id} className="items-start justify-between gap-4">
                <div className="min-w-0">
                  <p className="truncate text-[13px] font-semibold">
                    {grant.device_name}
                    {grant.id === currentGrantId ? (
                      <span className="ml-2 text-[10px] font-medium uppercase tracking-[0.08em] text-aged-brass-deep">
                        {t("accessCurrentDevice")}
                      </span>
                    ) : null}
                  </p>
                  <p className="mt-1 text-[11px] text-steel-secondary">
                    {grant.active ? t("accessStatusActive") : grant.revoked_at_ms ? t("accessStatusRevoked") : t("accessStatusExpired")}
                    {" · "}
                    {t("accessExpires", new Date(grant.expires_at_ms).toLocaleDateString())}
                  </p>
                  <div className="mt-2 flex flex-wrap gap-1">
                    {grant.scopes.map((scope) => <ScopeChip key={scope} scope={scope} />)}
                    {(grant.resources ?? []).map((resource, index) => (
                      <ResourceChip key={`${resource.kind}:${resource.id ?? "*"}:${index}`} resource={resource} />
                    ))}
                  </div>
                </div>
                {grant.active ? (
                  <Button
                    tone="destructive"
                    size="sm"
                    disabled={busyId === grant.id}
                    onClick={() => void onRevoke(grant)}
                  >
                    {t("accessRevoke")}
                  </Button>
                ) : null}
              </CardRow>
            ))
          )}
        </CardSection>
      </Card>

      <Card>
        <CardSection>
          <h2 className="font-display text-[19px] font-semibold">{t("accessPendingTitle")}</h2>
          <p className="mt-1 text-[12px] text-steel-secondary">{t("accessPendingBody")}</p>
        </CardSection>
        <CardSection divided className="py-2">
          {pending.length === 0 ? (
            <p className="py-4 text-[12px] text-muted-tone">{t("accessNoPending")}</p>
          ) : (
            pending.map((pairing) => (
              <CardRow key={pairing.id} className="items-start justify-between gap-4">
                <div className="min-w-0">
                  <p className="truncate text-[13px] font-semibold">{pairing.device_name}</p>
                  <p className="mt-1 text-[11px] text-steel-secondary">
                    {t("accessTicketExpires", new Date(pairing.expires_at_ms).toLocaleTimeString())}
                  </p>
                </div>
                <Button
                  tone="secondary"
                  size="sm"
                  disabled={busyId === pairing.id}
                  onClick={() => void onCancel(pairing)}
                >
                  {t("accessCancelTicket")}
                </Button>
              </CardRow>
            ))
          )}
        </CardSection>
      </Card>
    </div>
  );
}

function ScopeChip({ scope }: { scope: HostAccessScope }) {
  const t = useT();
  return (
    <span className="rounded-full bg-whisper-border-strong/35 px-2 py-0.5 text-[10px] font-medium text-steel-secondary">
      {scopeLabel(scope, t)}
    </span>
  );
}

function ResourceChip({ resource }: { resource: HostAccessResourceSelector }) {
  const t = useT();
  return (
    <span className="rounded-full bg-aged-brass-surface px-2 py-0.5 text-[10px] font-medium text-aged-brass-deep">
      {resourceLabel(resource.kind, resource.id, t)}
    </span>
  );
}

function parseResourceIds(value: string): string[] {
  return [...new Set(value.split(",").map((item) => item.trim()).filter(Boolean))];
}

function resourceLabel(
  kind: HostAccessResourceKind,
  id: string | null | undefined,
  t: ReturnType<typeof useT>,
): string {
  if (kind === "project") return id ? t("accessProjectResource", id) : t("accessAllProjects");
  return id ? t("accessTargetResource", id) : t("accessAllTargets");
}

function normalizePairingBase(input: string): string | null {
  try {
    const url = new URL(input.trim());
    if (
      url.protocol !== "https:" ||
      url.username ||
      url.password ||
      url.pathname !== "/" ||
      url.search ||
      url.hash
    ) return null;
    return url.origin;
  } catch {
    return null;
  }
}

function scopeLabel(scope: HostAccessScope, t: ReturnType<typeof useT>): string {
  return t({
    observe: "accessScopeObserve",
    project_operate: "accessScopeProjectOperate",
    deploy: "accessScopeDeploy",
    develop_propose: "accessScopeDevelopPropose",
    develop_approve: "accessScopeDevelopApprove",
    develop_execute: "accessScopeDevelopExecute",
    access_manage: "accessScopeManage",
  }[scope] as "accessScopeObserve");
}

function scopeDescription(scope: HostAccessScope, t: ReturnType<typeof useT>): string {
  return t({
    observe: "accessScopeObserveBody",
    project_operate: "accessScopeProjectOperateBody",
    deploy: "accessScopeDeployBody",
    develop_propose: "accessScopeDevelopProposeBody",
    develop_approve: "accessScopeDevelopApproveBody",
    develop_execute: "accessScopeDevelopExecuteBody",
    access_manage: "accessScopeManageBody",
  }[scope] as "accessScopeObserveBody");
}
