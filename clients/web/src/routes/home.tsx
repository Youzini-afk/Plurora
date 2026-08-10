import { useMemo, useState } from "react";
import { Plus, ArrowsClockwise } from "@/components/icons";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { Skeleton } from "@/components/ui/skeleton";
import { useT } from "@/lib/locale";
import { openInstallationInTab } from "@/lib/installation-launcher";
import { InstallationCard } from "@/components/home/installation-card";
import { InstallModal } from "@/components/install/install-modal";
import { filterInstallations } from "./home/home-filtering";
import { useHomeInstallations } from "./home/use-home-installations";

export function HomePage() {
  const t = useT();
  const [installOpen, setInstallOpen] = useState(false);
  const [search, setSearch] = useState("");
  const { installations, loading, error, refresh } = useHomeInstallations();
  const visible = useMemo(() => filterInstallations(installations, search), [installations, search]);

  return (
    <div className="mx-auto flex w-full max-w-[1440px] flex-1 flex-col gap-8 px-4 py-8 sm:px-6 lg:px-10 lg:py-12">
      <header className="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
        <div>
          <p className="font-mono text-[11px] uppercase tracking-[0.18em] text-muted-tone">{t("homeInstalledEyebrow", installations.length)}</p>
          <h1 className="mt-2 font-display text-4xl font-bold text-charcoal-ink">{t("homeGreeting")}</h1>
          <p className="mt-2 max-w-xl text-sm text-steel-secondary">Installations are durable records. Run lifecycle is intentionally unavailable until Phase 4.</p>
        </div>
        <Button tone="primary" onClick={() => setInstallOpen(true)}><Plus size={16} /> {t("homeInstallLabel")}</Button>
      </header>

      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <input
          value={search}
          onChange={(event) => setSearch(event.target.value)}
          placeholder={t("homeSearchPlaceholder")}
          className="h-10 w-full max-w-md rounded-[10px] border border-whisper-border bg-pure-surface px-3 text-sm outline-none focus-visible:border-aged-brass"
        />
        <Button tone="tertiary" size="sm" onClick={() => void refresh()} disabled={loading}><ArrowsClockwise size={14} /> {t("retry")}</Button>
      </div>

      {error ? <p className="rounded-[12px] border border-deep-rust/30 bg-deep-rust-surface px-4 py-3 text-sm text-deep-rust">{error}</p> : null}
      {loading ? (
        <div className="grid gap-5 md:grid-cols-2 xl:grid-cols-3"><Skeleton className="h-64 rounded-[20px]" /><Skeleton className="h-64 rounded-[20px]" /></div>
      ) : visible.length === 0 ? (
        <EmptyState title={t("homeEmptyTitle")} body={t("homeEmptyBody")} action={{ label: t("homeInstallLabel"), onClick: () => setInstallOpen(true) }} />
      ) : (
        <div className="grid gap-5 md:grid-cols-2 xl:grid-cols-3">
          {visible.map((installation, index) => (
            <InstallationCard
              key={installation.installationId}
              index={index}
              data={installation}
              onOpen={() => { openInstallationInTab(installation.installationId); }}
            />
          ))}
        </div>
      )}

      <InstallModal open={installOpen} onClose={() => setInstallOpen(false)} onInstalled={() => { setInstallOpen(false); void refresh(); }} />
    </div>
  );
}
