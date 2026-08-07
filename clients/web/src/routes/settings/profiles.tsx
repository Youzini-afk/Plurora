import { useMemo, useState, type FormEvent } from "react";
import { ArrowsLeftRight, GitBranch, Globe, Plus } from "@/components/icons";
import { Button } from "@/components/ui/button";
import { Card, CardSection } from "@/components/ui/card";
import { Field, Input } from "@/components/ui/input";
import { Modal, ModalFooter, ModalHeader } from "@/components/ui/modal";
import { Eyebrow, EyebrowSm, PageTitle } from "@/components/ui/typography";
import { StatusPill } from "@/components/ui/status-pill";
import { Skeleton } from "@/components/ui/skeleton";
import {
  browserAccessTokenStorageKey,
  BrowserHostConnectionStore,
  CURRENT_HOST_CONNECTION_ID,
  type HostConnectionProfile,
} from "@/client-core";
import { useAsync, usePlurora } from "@/lib/plurora-client";
import { useT } from "@/lib/locale";
import { cn } from "@/lib/cn";

interface ConnectionView extends HostConnectionProfile {
  active: boolean;
  currentOrigin: boolean;
}

function currentOrigin(): string {
  if (typeof location !== "undefined" && location.origin && location.origin !== "null") {
    return location.origin;
  }
  return "http://127.0.0.1:8787";
}

export function ProfilesPanel() {
  const client = usePlurora();
  const t = useT();
  const store = useMemo(() => new BrowserHostConnectionStore(), []);
  const [saved, setSaved] = useState(() => store.list());
  const [createOpen, setCreateOpen] = useState(false);
  const [name, setName] = useState("");
  const [endpoint, setEndpoint] = useState("");
  const [formError, setFormError] = useState<string | null>(null);
  const diagnostics = useAsync(() => client.diagnostics().catch(() => null), [client]);
  const activeSaved = store.active();

  const connections = useMemo<ConnectionView[]>(
    () => [
      {
        id: CURRENT_HOST_CONNECTION_ID,
        name: t("hostConnectionsCurrent"),
        baseUrl: currentOrigin(),
        active: !activeSaved,
        currentOrigin: true,
      },
      ...saved.map((profile) => ({
        ...profile,
        active: activeSaved?.id === profile.id,
        currentOrigin: false,
      })),
    ],
    [activeSaved, saved, t],
  );
  const active = connections.find((connection) => connection.active) ?? connections[0];

  const reloadWith = (profileId?: string) => {
    store.select(profileId);
    window.location.reload();
  };

  const submitConnection = (event: FormEvent) => {
    event.preventDefault();
    setFormError(null);
    try {
      const profile = store.save(name, endpoint);
      reloadWith(profile.id);
    } catch (error) {
      setFormError(error instanceof Error ? error.message : String(error));
    }
  };

  const removeConnection = (profile: ConnectionView) => {
    if (profile.currentOrigin || !window.confirm(t("hostConnectionsRemoveConfirm", profile.name))) {
      return;
    }
    try {
      window.localStorage.removeItem(browserAccessTokenStorageKey(profile.id));
    } catch {
      // The profile can still be removed when browser storage is partially unavailable.
    }
    store.remove(profile.id);
    if (profile.active) window.location.reload();
    else setSaved(store.list());
  };

  const runtime = diagnostics.data as
    | {
        profile_name?: string;
        packages_loaded?: number;
        network_allowlist?: { hosts?: string[] };
      }
    | null;

  return (
    <>
      <header className="mb-8">
        <Eyebrow>{t("hostConnectionsEyebrowActive", active.name)}</Eyebrow>
        <PageTitle className="mt-2">{t("hostConnectionsTitle")}</PageTitle>
        <p className="mt-2 max-w-[64ch] text-[13px] leading-relaxed text-steel-secondary">
          {t("hostConnectionsDescription")}
        </p>
      </header>

      <div className="grid grid-cols-1 gap-6 lg:grid-cols-[7fr_4fr]">
        <section className="flex flex-col gap-3">
          <div className="flex items-center justify-between">
            <Eyebrow>{t("hostConnectionsSaved")}</Eyebrow>
            <Button tone="tertiary" size="sm" onClick={() => setCreateOpen(true)}>
              <Plus size={14} />
              {t("hostConnectionsNew")}
            </Button>
          </div>
          <Card>
            <ul className="divide-y divide-whisper-border">
              {connections.map((profile) => (
                <li
                  key={profile.id}
                  onClick={() =>
                    profile.active
                      ? undefined
                      : reloadWith(profile.currentOrigin ? undefined : profile.id)
                  }
                  className={cn(
                    "flex items-center gap-4 px-5 py-4 transition",
                    profile.active &&
                      "border-l-[3px] border-l-aged-brass bg-aged-brass-surface-soft",
                    !profile.active && "cursor-pointer hover:bg-whisper-border-strong/30",
                  )}
                >
                  {profile.currentOrigin ? (
                    <GitBranch
                      size={18}
                      className={profile.active ? "text-aged-brass" : "text-steel-secondary"}
                    />
                  ) : (
                    <Globe
                      size={18}
                      className={profile.active ? "text-aged-brass" : "text-steel-secondary"}
                    />
                  )}
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="font-display text-[16px] font-bold text-charcoal-ink">
                        {profile.name}
                      </span>
                      {profile.active ? (
                        <StatusPill tone="accent" label={t("profilesActive")} showDot={false} />
                      ) : null}
                    </div>
                    <p className="mt-1 truncate font-mono text-[11px] text-steel-secondary">
                      {profile.baseUrl}
                    </p>
                  </div>
                  {!profile.currentOrigin ? (
                    <Button
                      tone="tertiary"
                      size="sm"
                      onClick={(event) => {
                        event.stopPropagation();
                        removeConnection(profile);
                      }}
                    >
                      {t("hostConnectionsRemove")}
                    </Button>
                  ) : null}
                </li>
              ))}
            </ul>
          </Card>
        </section>

        <Card>
          <CardSection>
            <EyebrowSm>{t("profilesActive")}</EyebrowSm>
            <h3 className="mt-3 font-display text-[20px] font-bold text-charcoal-ink">
              {active.name}
            </h3>
            <p className="mt-2 break-all font-mono text-[11px] text-steel-secondary">
              {active.baseUrl}
            </p>
            <p className="mt-3 text-[11px] leading-relaxed text-muted-tone">
              {t("hostConnectionsCredentialHint")}
            </p>
          </CardSection>

          <CardSection divided>
            <EyebrowSm>{t("profilesLoadedPackages")}</EyebrowSm>
            {diagnostics.loading ? (
              <Skeleton className="mt-3 h-4 w-16" />
            ) : (
              <p className="mt-2 font-mono text-[13px] text-charcoal-ink">
                {runtime?.packages_loaded ?? 0}
              </p>
            )}
            <p className="mt-1 text-[11px] text-steel-secondary">
              {runtime?.profile_name ?? t("profilesEyebrowNone")}
            </p>
          </CardSection>

          <CardSection divided>
            <EyebrowSm>{t("profilesNetworkAllowlist")}</EyebrowSm>
            {runtime?.network_allowlist?.hosts?.length ? (
              <ul className="mt-3 space-y-1.5">
                {runtime.network_allowlist.hosts.slice(0, 6).map((host) => (
                  <li key={host} className="flex items-center gap-2 font-mono text-[12px]">
                    <Globe size={12} className="text-steel-secondary" />
                    <span className="truncate">{host}</span>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="mt-2 text-[12px] text-muted-tone">
                {diagnostics.error
                  ? t("profilesDiagnosticsErrorBody")
                  : t("profilesOutboundBlocked")}
              </p>
            )}
          </CardSection>

          {!active.currentOrigin ? (
            <CardSection divided>
              <Button
                tone="secondary"
                className="w-full"
                onClick={() => reloadWith(undefined)}
              >
                <ArrowsLeftRight size={14} />
                {t("hostConnectionsReturnCurrent")}
              </Button>
            </CardSection>
          ) : null}
        </Card>
      </div>

      <Modal open={createOpen} onOpenChange={setCreateOpen} size="sm">
        <ModalHeader
          eyebrow={t("hostConnectionsSaved")}
          title={t("hostConnectionsNewTitle")}
          description={t("hostConnectionsNewBody")}
        />
        <form onSubmit={submitConnection}>
          <div className="space-y-4">
            <Field label={t("hostConnectionsName")} required>
              <Input
                value={name}
                onChange={(event) => setName(event.target.value)}
                maxLength={64}
                autoFocus
              />
            </Field>
            <Field
              label={t("hostConnectionsEndpoint")}
              helper={t("hostConnectionsEndpointHint")}
              error={formError ?? undefined}
              required
            >
              <Input
                value={endpoint}
                onChange={(event) => setEndpoint(event.target.value)}
                placeholder="https://host.example"
                className="font-mono"
                spellCheck={false}
              />
            </Field>
          </div>
          <ModalFooter className="justify-end">
            <Button tone="secondary" type="button" onClick={() => setCreateOpen(false)}>
              {t("cancel")}
            </Button>
            <Button tone="primary" type="submit" disabled={!name.trim() || !endpoint.trim()}>
              {t("hostConnectionsConnect")}
            </Button>
          </ModalFooter>
        </form>
      </Modal>
    </>
  );
}
