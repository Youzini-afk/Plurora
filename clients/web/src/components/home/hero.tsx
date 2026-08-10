import { Eyebrow, HeroTitle } from "@/components/ui/typography";
import { ContinueCard, type ContinueCardLabels, type ContinueCardEntry } from "@/components/home/continue-card";

export interface HeroProps {
  greeting: string;
  summary: string;
  meta: string;
  continueEntry: ContinueCardEntry | null;
  continueLabels: ContinueCardLabels;
  hasInstalledInstallations: boolean;
  onContinue: (installationId: string) => void;
  onInstall: () => void;
  onBrowseInstallations?: () => void;
}

export function Hero({
  greeting,
  summary,
  meta,
  continueEntry,
  continueLabels,
  hasInstalledInstallations,
  onContinue,
  onInstall,
  onBrowseInstallations,
}: HeroProps) {
  return (
    <section className="grid grid-cols-1 gap-8 lg:grid-cols-[1fr_auto] lg:gap-12">
      <div className="flex flex-col gap-3">
        <Eyebrow>{meta}</Eyebrow>
        <HeroTitle>{greeting}</HeroTitle>
        <p className="max-w-[80ch] text-[15px] leading-relaxed text-steel-secondary">{summary}</p>
      </div>
      <div className="lg:flex lg:justify-end">
        <ContinueCard
          entry={continueEntry}
          labels={continueLabels}
          onContinue={onContinue}
          onInstall={onInstall}
          onBrowseInstallations={onBrowseInstallations}
          hasInstalledInstallations={hasInstalledInstallations}
        />
      </div>
    </section>
  );
}
