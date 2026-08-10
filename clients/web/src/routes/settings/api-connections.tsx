import { useEffect, useMemo, useRef, useState, type FormEvent } from "react";
import { motion } from "motion/react";
import { Copy, DotsThree, Eye, EyeSlash, Key, Plus } from "@/components/icons";
import { Button } from "@/components/ui/button";
import { Card, CardSection } from "@/components/ui/card";
import { Eyebrow, EyebrowSm, PageTitle } from "@/components/ui/typography";
import { StatusPill } from "@/components/ui/status-pill";
import { Tooltip } from "@/components/ui/tooltip";
import { Field, Input } from "@/components/ui/input";
import { Modal, ModalHeader, ModalFooter } from "@/components/ui/modal";
import {
  Dropdown,
  DropdownTrigger,
  DropdownMenu,
  DropdownItem,
  DropdownSeparator,
} from "@/components/ui/dropdown";
import { Skeleton } from "@/components/ui/skeleton";
import { EmptyState } from "@/components/ui/empty-state";
import { useToast } from "@/components/ui/toast";
import { useAsync, usePlurora } from "@/lib/plurora-client";
import { useT } from "@/lib/locale";
import { cn } from "@/lib/cn";

interface DraftSecret {
  name: string;
  value: string;
}

interface SecretView {
  name: string;
  scope: string;
  scopeLabel: string;
  provider: string;
}

const PROVIDERS: Array<{ id: string; label: string }> = [
  { id: "OpenAI", label: "OpenAI" },
  { id: "Anthropic", label: "Anthropic" },
  { id: "Google", label: "Google" },
  { id: "OpenRouter", label: "OpenRouter" },
  { id: "DeepSeek", label: "DeepSeek" },
  { id: "xAI", label: "xAI" },
  { id: "Fireworks", label: "Fireworks" },
  { id: "Custom", label: "Custom" },
];

const PROVIDER_HINTS: Record<string, string> = {
  OPENAI_API_KEY: "OpenAI",
  ANTHROPIC_API_KEY: "Anthropic",
  GEMINI_API_KEY: "Google",
  OPENROUTER_API_KEY: "OpenRouter",
  DEEPSEEK_API_KEY: "DeepSeek",
  XAI_API_KEY: "xAI",
  FIREWORKS_API_KEY: "Fireworks",
};

export function ApiConnectionsPanel() {
  const client = usePlurora();
  const toast = useToast();
  const t = useT();

  const installations = useAsync(() => client.listInstallations().catch(() => []), [client]);
  const [scope, setScope] = useState<"platform" | "installation">("platform");
  const [selectedInstallationId, setSelectedInstallationId] = useState<string | null>(null);
  const selectedScopeId = scope === "installation" ? selectedInstallationId ?? undefined : undefined;
  const platform = useAsync(() => client.listSecrets(selectedScopeId), [client, selectedScopeId]);
  const health = useAsync(() => client.secretsHealth().catch(() => null), [client]);
  const [showAdd, setShowAdd] = useState(false);

  const secrets = useMemo<SecretView[]>(() => {
    return (platform.data ?? []).map((name) => ({
      name,
      scope: selectedScopeId ? "installation" : "platform",
      scopeLabel: selectedScopeId ? "INSTALLATION" : "PLATFORM",
      provider: PROVIDER_HINTS[name] ?? "Custom",
    }));
  }, [platform.data, selectedScopeId]);

  const handleDelete = async (name: string) => {
    try {
      await client.deleteSecret(name, selectedScopeId);
      platform.refresh();
      health.refresh();
      toast.push({ variant: "info", title: t("apiRemoved", name) });
    } catch (err) {
      toast.push({
        variant: "error",
        title: t("apiDeleteFailedTitle"),
        body: t("apiDeleteFailedBody"),
      });
    }
  };

  const handleCopy = (name: string) => {
    navigator.clipboard?.writeText(name);
    toast.push({ variant: "success", title: t("apiCopiedSecretName"), duration: 2000 });
  };

  const handleSave = async (entry: DraftSecret) => {
    if (!entry.name || !entry.value) return;
    try {
      await client.putSecret(entry.name, entry.value, selectedScopeId);
      platform.refresh();
      health.refresh();
      setShowAdd(false);
      toast.push({ variant: "success", title: t("apiStored", entry.name) });
    } catch (err) {
      toast.push({
        variant: "error",
        title: t("apiSaveFailedTitle"),
        body: t("apiSaveFailedBody"),
      });
    }
  };

  return (
    <>
      <header className="mb-8">
        <Eyebrow>
          {platform.loading
            ? t("apiEyebrowLoading")
            : t("apiEyebrowCount", secrets.length)}
        </Eyebrow>
        <PageTitle className="mt-2">{t("apiTitle")}</PageTitle>
        <p className="mt-2 max-w-[60ch] text-[13px] leading-relaxed text-steel-secondary">
          {t("apiDescription")}
        </p>
      </header>

      <div className="grid grid-cols-1 gap-6 lg:grid-cols-[7fr_3fr]">
        <section className="flex flex-col gap-3">
          <div className="flex items-center justify-between">
            <Eyebrow>{t("apiStoredSecrets")}</Eyebrow>
            <Button tone="tertiary" size="sm" onClick={() => setShowAdd(true)}>
              <Plus size={14} />
              {t("apiAddSecret")}
            </Button>
          </div>
          <Card>
            {platform.error ? (
              <EmptyState
                icon={<Key />}
                title={t("apiLoadErrorTitle")}
                body={t("apiLoadErrorBody")}
                action={{ label: t("retry"), onClick: () => platform.refresh() }}
              />
            ) : platform.loading ? (
              <ul className="divide-y divide-whisper-border">
                {Array.from({ length: 3 }).map((_, idx) => (
                  <li key={idx} className="flex items-center gap-4 px-5 py-4">
                    <Skeleton className="size-9 rounded-full" />
                    <div className="flex-1 space-y-1.5">
                      <Skeleton className="h-3 w-40" />
                      <Skeleton className="h-2.5 w-60" />
                    </div>
                    <Skeleton className="h-7 w-7 rounded" />
                    <Skeleton className="h-7 w-7 rounded" />
                  </li>
                ))}
              </ul>
            ) : secrets.length === 0 ? (
              <EmptyState
                icon={<Key />}
                title={t("apiEmptyTitle")}
                body={t("apiEmptyBody")}
                action={{ label: t("apiAddSecret"), onClick: () => setShowAdd(true) }}
              />
            ) : (
              <ul className="divide-y divide-whisper-border">
                {secrets.map((secret) => (
                  <SecretRow
                    key={secret.name}
                    secret={secret}
                    onCopy={() => handleCopy(secret.name)}
                    onDelete={() => handleDelete(secret.name)}
                    labels={{
                      hideName: t("apiHideName"),
                      revealName: t("apiRevealName"),
                      toggleReveal: t("apiToggleReveal"),
                      copyName: t("apiCopyName"),
                      copy: t("apiCopy"),
                      more: t("apiMore"),
                      rotate: t("apiRotate"),
                      delete: t("apiDelete"),
                    }}
                  />
                ))}
              </ul>
            )}
          </Card>
        </section>

        <aside className="flex flex-col gap-4">
          <Card>
            <CardSection>
              <EyebrowSm>{t("apiStoreStatus")}</EyebrowSm>
              <dl className="mt-3 space-y-2 text-[12px]">
                {[
                  [t("apiEncryption"), "age (X25519)"],
                  [t("apiMasterKey"), health.data?.key_source ?? "—"],
                  [t("apiStorage"), health.data?.exists ? t("apiConfigured") : t("apiNotCreated")],
                  [
                    t("apiTotal"),
                    health.data
                      ? t("apiSecretsCount", health.data.secret_count)
                      : platform.loading
                        ? "—"
                        : t("apiSecretsCount", secrets.length),
                  ],
                ].map(([label, value]) => (
                  <div key={label} className="flex items-center justify-between gap-3">
                    <dt className="text-steel-secondary">{label}</dt>
                    <dd className="truncate font-mono text-charcoal-ink">{value}</dd>
                  </div>
                ))}
              </dl>
            </CardSection>
          </Card>
          <Card>
            <CardSection>
              <EyebrowSm>{t("apiHowUsed")}</EyebrowSm>
              <p className="mt-3 text-[12px] leading-relaxed text-steel-secondary">
                Installations reference keys with{" "}
                <span className="font-mono text-charcoal-ink">secret_ref:store:NAME</span>. {t("apiHowUsedBody")}
              </p>
              <button
                type="button"
                className="mt-3 text-[12px] font-medium text-charcoal-ink underline underline-offset-4 decoration-1 hover:decoration-aged-brass"
              >
                {t("apiOpenAuditLog")}
              </button>
            </CardSection>
          </Card>
          <Card>
            <CardSection>
              <EyebrowSm>{t("apiBackup")}</EyebrowSm>
              <div className="mt-3 flex flex-col gap-1.5">
                <Button
                  tone="secondary"
                  size="sm"
                  className="justify-start"
                  onClick={() =>
                    toast.push({ variant: "info", title: t("apiExportToast") })
                  }
                >
                  {t("apiExportFile")}
                </Button>
                <Button
                  tone="secondary"
                  size="sm"
                  className="justify-start"
                  onClick={() =>
                    toast.push({ variant: "info", title: t("apiImportToast") })
                  }
                >
                  {t("apiImportFile")}
                </Button>
              </div>
            </CardSection>
          </Card>
        </aside>
      </div>

      <AddSecretModal
        open={showAdd}
        onClose={() => setShowAdd(false)}
        onSave={handleSave}
        scope={scope}
        selectedInstallationId={selectedInstallationId}
        installations={installations.data ?? []}
        onScopeChange={(nextScope, installationId) => {
          setScope(nextScope);
          setSelectedInstallationId(installationId);
        }}
      />
    </>
  );
}

function SecretRow({
  secret,
  onCopy,
  onDelete,
  labels,
}: {
  secret: SecretView;
  onCopy: () => void;
  onDelete: () => void;
  labels: {
    hideName: string;
    revealName: string;
    toggleReveal: string;
    copyName: string;
    copy: string;
    more: string;
    rotate: string;
    delete: string;
  };
}) {
  const [revealed, setRevealed] = useState(false);
  return (
    <motion.li layout className="flex items-center gap-4 px-5 py-4">
      <span className="rounded-full border border-whisper-border bg-aged-brass-surface-soft p-2 text-aged-brass">
        <Key size={16} />
      </span>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="truncate font-mono text-[13px] text-charcoal-ink">
            {revealed ? secret.name : "••••••••••••••"}
          </span>
          <StatusPill
            tone={secret.scope === "platform" ? "accent" : "neutral"}
            label={secret.scopeLabel}
            showDot={false}
          />
        </div>
        <p className="mt-1 text-[11px] text-steel-secondary">
          <span>{secret.provider}</span>
          <span className="mx-1.5 text-whisper-border-strong/70">·</span>
          <span className="font-mono text-muted-tone">secret_ref:store:{secret.name}</span>
        </p>
      </div>
      <Tooltip label={revealed ? labels.hideName : labels.revealName}>
        <Button
          tone="icon"
          size="icon-sm"
          onClick={() => setRevealed((v) => !v)}
          aria-label={labels.toggleReveal}
        >
          {revealed ? <EyeSlash size={14} /> : <Eye size={14} />}
        </Button>
      </Tooltip>
      <Tooltip label={labels.copyName}>
        <Button tone="icon" size="icon-sm" onClick={onCopy} aria-label={labels.copy}>
          <Copy size={14} />
        </Button>
      </Tooltip>
      <Dropdown>
        <DropdownTrigger asChild>
          <Button tone="icon" size="icon-sm" aria-label={labels.more}>
            <DotsThree size={16} />
          </Button>
        </DropdownTrigger>
        <DropdownMenu>
          <DropdownItem>{labels.rotate}</DropdownItem>
          <DropdownSeparator />
          <DropdownItem destructive onSelect={onDelete}>
            {labels.delete}
          </DropdownItem>
        </DropdownMenu>
      </Dropdown>
    </motion.li>
  );
}

function AddSecretModal({
  open,
  onClose,
  onSave,
  scope,
  selectedInstallationId,
  installations,
  onScopeChange,
}: {
  open: boolean;
  onClose: () => void;
  onSave: (entry: DraftSecret) => void;
  scope: "platform" | "installation";
  selectedInstallationId: string | null;
  installations: Array<{ record: { installation_id: string; display_name: string } }>;
  onScopeChange: (scope: "platform" | "installation", installationId: string | null) => void;
}) {
  const t = useT();
  const [name, setName] = useState("");
  const [provider, setProvider] = useState("OpenAI");
  const valueRef = useRef<HTMLInputElement>(null);

  const wipeValueInput = () => {
    if (valueRef.current) valueRef.current.value = "";
  };

  // Always wipe the raw secret from React memory when the modal closes —
  // success path, cancel path, esc path, anywhere. The host has the
  // encrypted copy; the UI must never retain raw values past submission.
  useEffect(() => {
    if (!open) {
      wipeValueInput();
      setName("");
      setProvider("OpenAI");
      onScopeChange("platform", null);
    }
  }, [open]);

  const handleSave = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const data = new FormData(event.currentTarget);
    const value = String(data.get("secret-value") ?? "");
    if (!name || !value) return;
    onSave({ name, value });
    // Wipe immediately on submit; do not wait for the close effect.
    wipeValueInput();
    setName("");
  };

  return (
    <Modal open={open} onOpenChange={onClose} contentLabel={t("apiAddContentLabel")}>
      <ModalHeader
        eyebrow={t("apiAddEyebrow")}
        title={t("apiAddTitle")}
        description={t("apiAddDescription")}
      />
      <form onSubmit={handleSave} className="flex flex-col gap-4">
        <Field label={t("apiProvider")} required>
          <select
            value={provider}
            onChange={(e) => setProvider(e.target.value)}
            className="h-10 rounded-[10px] border border-whisper-border bg-transparent px-3 text-[13px] outline-none transition focus-visible:border-aged-brass focus-visible:ring-2 focus-visible:ring-aged-brass/40"
          >
            {PROVIDERS.map((p) => (
              <option key={p.id}>{p.label}</option>
            ))}
          </select>
        </Field>
        <Field
          label={t("apiSecretName")}
          helper={t("apiSecretNameHelper")}
          required
        >
          <Input
            value={name}
            onChange={(e) => setName(e.target.value.toUpperCase())}
            placeholder="OPENAI_API_KEY"
            spellCheck={false}
          />
        </Field>
        <Field label={t("apiValue")} helper={t("apiValueHelper")} required>
          <Input
            type="password"
            ref={valueRef}
            name="secret-value"
            placeholder="sk-…"
            autoComplete="new-password"
          />
        </Field>
        <Field label={t("apiScope")}>
          <div className="flex flex-wrap gap-2">
            {[
              { id: "platform", label: t("apiScopePlatform") },
              { id: "installation", label: t("apiScopeInstallation") },
            ].map((option) => (
              <button
                key={option.id}
                type="button"
                onClick={() => {
                  if (option.id === "platform") onScopeChange("platform", null);
                  else if (installations[0]) onScopeChange("installation", selectedInstallationId ?? installations[0].record.installation_id);
                }}
                disabled={option.id === "installation" && installations.length === 0}
                className={cn(
                  "rounded-full border px-3 py-1 text-[12px] font-medium transition disabled:opacity-50",
                  scope === option.id
                    ? "border-aged-brass-border bg-aged-brass-surface text-charcoal-ink"
                    : "border-whisper-border text-steel-secondary hover:bg-whisper-border-strong/30",
                )}
              >
                {option.label}
              </button>
            ))}
          </div>
          {scope === "installation" ? (
            <select
              className="mt-3 h-10 w-full rounded-[10px] border border-whisper-border bg-transparent px-3 text-[13px] outline-none focus-visible:border-aged-brass"
              value={selectedInstallationId ?? ""}
              onChange={(event) => onScopeChange("installation", event.target.value || null)}
              required
            >
              <option value="" disabled>Select an Installation</option>
              {installations.map((installation) => <option key={installation.record.installation_id} value={installation.record.installation_id}>{installation.record.display_name} ({installation.record.installation_id})</option>)}
            </select>
          ) : null}
        </Field>
        <ModalFooter className="justify-end">
          <Button tone="secondary" type="button" onClick={onClose}>
            {t("cancel")}
          </Button>
          <Button tone="primary" type="submit" disabled={!name}>
            {t("apiSaveKey")}
          </Button>
        </ModalFooter>
      </form>
    </Modal>
  );
}
