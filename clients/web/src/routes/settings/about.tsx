import { Bug, ChatCircle, GithubLogo, BookOpen, Newspaper } from "@/components/icons";
import { Card, CardSection } from "@/components/ui/card";
import { Eyebrow, EyebrowSm, PageTitle } from "@/components/ui/typography";
import { useT } from "@/lib/locale";

export function AboutPanel() {
  const t = useT();
  return (
    <>
      <header className="mb-8">
        <Eyebrow>{t("aboutEyebrow")}</Eyebrow>
        <PageTitle className="mt-3">Plurora</PageTitle>
        <p className="mt-3 text-[15px] text-steel-secondary">{t("aboutSubtitle")}</p>
      </header>

      <div className="grid grid-cols-1 gap-6 lg:grid-cols-[2fr_1fr]">
        <div className="flex flex-col gap-4">
          <Card>
            <CardSection>
              <dl className="grid grid-cols-2 gap-x-6 gap-y-3 sm:grid-cols-4">
                {[
                  [t("aboutVersion"), "v0.1.0-alpha"],
                  [t("aboutBuild"), "2a47e5c"],
                  [t("aboutReleased"), "2026-05-14"],
                  [t("aboutChannel"), "alpha"],
                ].map(([label, value]) => (
                  <div key={label}>
                    <dt className="font-mono text-[10px] uppercase tracking-[0.12em] text-muted-tone">
                      {label}
                    </dt>
                    <dd className="mt-1 font-mono text-[14px] text-charcoal-ink">{value}</dd>
                  </div>
                ))}
              </dl>
            </CardSection>
          </Card>

          <Card>
            <CardSection>
              <EyebrowSm>{t("aboutWhat")}</EyebrowSm>
              <div className="mt-3 max-w-[60ch] space-y-3 text-[14px] leading-relaxed text-charcoal-ink">
                <p>
                  {t("aboutPara1")}
                </p>
                <p>
                  {t("aboutPara2")}
                </p>
                <p className="text-steel-secondary">
                  {t("aboutPara3")}
                </p>
              </div>
            </CardSection>
          </Card>

          <Card>
            <CardSection>
              <EyebrowSm>{t("aboutCredits")}</EyebrowSm>
              <ul className="mt-3 divide-y divide-whisper-border text-[12px]">
                {[
                  [t("aboutBuiltOn"), "Rust 1.84 · TypeScript 5.7 · Tauri 2.x", "mono"],
                  [t("aboutFonts"), "Bricolage Grotesque · Geist · JetBrains Mono"],
                  [t("aboutIcons"), "Phosphor Icons (1.4 · MIT)"],
                ].map(([label, value, font]) => (
                  <li key={label} className="flex items-center justify-between gap-4 py-2.5">
                    <span className="text-steel-secondary">{label}</span>
                    <span
                      className={
                        font === "mono"
                          ? "font-mono text-charcoal-ink"
                          : "text-charcoal-ink"
                      }
                    >
                      {value}
                    </span>
                  </li>
                ))}
              </ul>
            </CardSection>
          </Card>
        </div>

        <div className="flex flex-col gap-4">
          <Card>
            <CardSection>
              <EyebrowSm>{t("aboutLicense")}</EyebrowSm>
              <h3 className="mt-2 font-display text-[17px] font-bold text-charcoal-ink">
                AGPL-3.0-only
              </h3>
              <p className="mt-2 max-w-[32ch] text-[12px] leading-relaxed text-steel-secondary">
                {t("aboutLicenseBody")}
              </p>
              <a
                href="https://github.com/Youzini-afk/Plurora/blob/main/LICENSE"
                target="_blank"
                rel="noopener noreferrer"
                className="mt-3 inline-block text-[12px] font-medium text-charcoal-ink underline underline-offset-4 decoration-1 hover:decoration-aged-brass"
              >
                {t("aboutReadLicense")}
              </a>
            </CardSection>
          </Card>

          <Card>
            <CardSection>
              <EyebrowSm>{t("aboutLinks")}</EyebrowSm>
              <ul className="mt-3 divide-y divide-whisper-border">
                {[
                  { Icon: GithubLogo, label: t("aboutSourceCode"), href: "https://github.com/Youzini-afk/Plurora" },
                  { Icon: BookOpen, label: t("aboutDocumentation"), href: "https://github.com/Youzini-afk/Plurora/tree/main/docs" },
                  { Icon: Bug, label: t("aboutReportIssue"), href: "https://github.com/Youzini-afk/Plurora/issues" },
                  { Icon: ChatCircle, label: t("aboutCommunity"), href: "#" },
                  { Icon: Newspaper, label: t("aboutChangelog"), href: "https://github.com/Youzini-afk/Plurora/blob/main/CHANGELOG.md" },
                ].map(({ Icon, label, href }) => (
                  <li key={label}>
                    <a
                      href={href}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="flex items-center justify-between gap-2 py-2.5 text-[13px] font-medium text-charcoal-ink hover:text-aged-brass-deep"
                    >
                      <span className="flex items-center gap-2">
                        <Icon size={16} className="text-steel-secondary" />
                        {label}
                      </span>
                      <span className="font-mono text-[10px] text-muted-tone">↗</span>
                    </a>
                  </li>
                ))}
              </ul>
            </CardSection>
          </Card>

          <Card>
            <CardSection>
              <EyebrowSm>{t("aboutGratitude")}</EyebrowSm>
              <p className="mt-3 max-w-[36ch] text-[12px] leading-relaxed text-steel-secondary">
                {t("aboutGratitudeBody")}
              </p>
            </CardSection>
          </Card>
        </div>
      </div>
    </>
  );
}
