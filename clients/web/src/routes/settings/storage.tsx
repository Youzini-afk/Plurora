import { Folder } from "@/components/icons";
import { Card, CardSection } from "@/components/ui/card";
import { Eyebrow, EyebrowSm, PageTitle } from "@/components/ui/typography";
import { Skeleton } from "@/components/ui/skeleton";
import { useAsync, usePlurora } from "@/lib/plurora-client";
import { useT } from "@/lib/locale";

export function StoragePanel() {
  const client = usePlurora();
  const t = useT();
  const diagnostics = useAsync(() => client.diagnostics().catch(() => null), [client]);

  const eventStoreKind =
    (diagnostics.data as { event_store?: { kind?: string } } | null)?.event_store?.kind ?? "sqlite";

  const storageAreas = [
    { label: t("storageAreaInstallationData"), description: t("storageAreaInstallationDataDesc") },
    { label: t("storageAreaPackageStore"), description: t("storageAreaPackageStoreDesc") },
    { label: t("storageAreaProfiles"), description: t("storageAreaProfilesDesc") },
    { label: t("storageAreaSecrets"), description: t("storageAreaSecretsDesc") },
    { label: t("storageAreaCache"), description: t("storageAreaCacheDesc") },
  ];

  return (
    <>
      <header className="mb-8">
        <Eyebrow>{t("storageTitleEyebrow")}</Eyebrow>
        <PageTitle className="mt-2">{t("storageTitle")}</PageTitle>
        <p className="mt-2 max-w-[60ch] text-[13px] leading-relaxed text-steel-secondary">
          {t("storageDescription")}
        </p>
      </header>

      <Card>
        <CardSection>
          <EyebrowSm>{t("storageAreas")}</EyebrowSm>
          {diagnostics.loading ? (
            <ul className="mt-3 space-y-2">
              {Array.from({ length: 6 }).map((_, idx) => (
                <li key={idx} className="flex items-center justify-between gap-4 py-2">
                  <Skeleton className="h-3 w-20" />
                  <Skeleton className="h-3 w-72" />
                </li>
              ))}
            </ul>
          ) : (
            <ul className="mt-3 divide-y divide-whisper-border">
              {storageAreas.map((entry) => (
                <li
                  key={entry.label}
                  className="flex items-start justify-between gap-4 py-2.5 text-[13px]"
                >
                  <span className="text-steel-secondary">{entry.label}</span>
                  <span className="flex min-w-0 max-w-[42ch] items-start gap-2 text-right text-charcoal-ink">
                    <Folder size={12} className="shrink-0 text-steel-secondary" />
                    <span>{entry.description}</span>
                  </span>
                </li>
              ))}
            </ul>
          )}
        </CardSection>
        <CardSection divided>
          <EyebrowSm>{t("storageEventStore")}</EyebrowSm>
          <p className="mt-2 text-[13px] text-charcoal-ink">
            <span className="font-mono">{eventStoreKind}</span>
            <span className="mx-2 text-muted-tone">·</span>
            <span className="text-steel-secondary">
              {eventStoreKind === "sqlite"
                ? t("storageSqliteDesc")
                : eventStoreKind === "postgres"
                  ? t("storagePostgresDesc")
                  : eventStoreKind === "memory"
                    ? t("storageMemoryDesc")
                    : t("storageCustomDesc")}
            </span>
          </p>
        </CardSection>
        <CardSection divided>
          <EyebrowSm>{t("storageBackendNeutrality")}</EyebrowSm>
          <p className="mt-2 text-[12px] leading-relaxed text-steel-secondary">
            {t("storageBackendBody")}
          </p>
        </CardSection>
      </Card>
    </>
  );
}
