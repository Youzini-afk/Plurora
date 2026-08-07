import { Bell, GearSix, Moon, SignOut, Sun } from "@/components/icons";
import { Button } from "@/components/ui/button";
import { Tooltip } from "@/components/ui/tooltip";
import { useTheme } from "@/lib/theme";
import { useAuth } from "@/lib/auth-gate";
import { useRoute, type Route } from "@/lib/router";
import { cn } from "@/lib/cn";
import { LocaleSwitcher } from "@/components/layout/locale-switcher";
import { useT } from "@/lib/locale";

const breadcrumbForRoute = (route: Route, t: ReturnType<typeof useT>): string => {
  switch (route.kind) {
    case "home":
      return t("topbarHome");
    case "settings":
      return t("topbarSettings");
    case "project":
      return t("topbarProject", route.projectId);
  }
};

export function PlatformTopbar({ route }: { route: Route }) {
  const { theme, preference, setPreference } = useTheme();
  const { token, identity, logout } = useAuth();
  const [, navigate] = useRoute();
  const t = useT();

  // Cycle: system → light → dark → system. Preserves the user's choice to
  // follow the OS instead of forcing it off after the first toggle.
  const cycleTheme = () => {
    const next =
      preference === "system" ? "light" : preference === "light" ? "dark" : "system";
    setPreference(next);
  };

  return (
    <header
      className={cn(
        "plurora-safe-topbar sticky top-0 z-30 flex items-center justify-between border-b border-whisper-border bg-warm-bone/85 backdrop-blur-[20px] sm:px-6 lg:px-8",
      )}
    >
      <nav className="flex min-w-0 items-center gap-2 text-[14px] sm:gap-3">
        <button
          type="button"
          onClick={() => navigate({ kind: "home" })}
          className="font-display text-[18px] font-bold leading-none tracking-[-0.015em] text-charcoal-ink hover:text-aged-brass-deep"
        >
          Plurora
        </button>
        <span className="hidden text-muted-tone sm:inline" aria-hidden>
          /
        </span>
        <span className="hidden truncate text-[13px] text-steel-secondary sm:inline">
          {breadcrumbForRoute(route, t)}
        </span>
      </nav>

      <div className="flex shrink-0 items-center gap-0.5 sm:gap-1">
        <span className="hidden sm:inline-flex">
          <Tooltip label={t("topbarNotifications")}>
            <Button tone="icon" size="icon" aria-label={t("topbarNotifications")} className="relative">
              <Bell size={18} />
            </Button>
          </Tooltip>
        </span>
        <Tooltip
          label={
            preference === "system"
              ? t("topbarThemeSystem", theme)
              : preference === "light"
                ? t("topbarThemeLight")
                : t("topbarThemeDark")
          }
        >
          <Button
            tone="icon"
            size="icon"
            onClick={cycleTheme}
            aria-label={t("topbarThemeAria", preference)}
            aria-pressed={preference === "dark"}
          >
            {/* Cycle indicator: filled icon for explicit choice, outline for system. */}
            {preference === "system" ? (
              theme === "dark" ? <Moon size={18} /> : <Sun size={18} />
            ) : preference === "dark" ? (
              <Moon size={18} weight="fill" />
            ) : (
              <Sun size={18} weight="fill" />
            )}
          </Button>
        </Tooltip>
        <Tooltip label={t("topbarSettings")}>
          <Button
            tone="icon"
            size="icon"
            aria-label={t("topbarSettings")}
            onClick={() => navigate({ kind: "settings", tab: "host-access" })}
          >
            <GearSix size={18} />
          </Button>
        </Tooltip>
        <div className="inline-flex">
          <LocaleSwitcher />
        </div>
        {token || identity?.kind === "device" ? (
          <Tooltip label={t("topbarLogout")}>
            <Button
              tone="icon"
              size="icon"
              aria-label={t("topbarLogout")}
              onClick={() => void logout()}
              className="text-deep-rust hover:bg-deep-rust-surface"
            >
              <SignOut size={18} />
            </Button>
          </Tooltip>
        ) : null}
      </div>
    </header>
  );
}
