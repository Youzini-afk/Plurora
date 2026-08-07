import { Suspense, lazy, useCallback, useState } from "react";
import { Hero } from "@/components/home/hero";
import { UtilityStrip } from "@/components/home/utility-strip";
import { ProjectCard } from "@/components/home/project-card";
import { InstallCard } from "@/components/home/install-card";
import { ActivityTimeline } from "@/components/home/activity-timeline";
import { WorkshopUtilities } from "@/components/home/workshop-utilities";
import { Eyebrow } from "@/components/ui/typography";
import { Skeleton } from "@/components/ui/skeleton";
import { EmptyState } from "@/components/ui/empty-state";
import { Plus, Warning } from "@/components/icons";
import { useKernel } from "@/lib/kernel-client";
import { useRoute, type Route } from "@/lib/router";
import { useToast } from "@/components/ui/toast";
import { formatGreetingTime } from "@/lib/format";
import { useLocale } from "@/lib/locale";
import { openProjectInTab } from "@/lib/project-launcher";
import type { FailureDetail } from "@/components/install/failure-modal";
import { HomeCapabilityCards } from "@/surfaces/shell-contribution-renderers";
import type {
  HomeCardContribution,
  QuickActionContribution,
  WorkshopCardContribution,
} from "@/surfaces/shell-contributions";
import { useSurfaceContributions } from "@/surfaces/use-surface-contributions";
import { useHomeProjects } from "./home/use-home-projects";
import { useProjectActions } from "./home/use-project-actions";
import {
  HOME_BUILTIN_QUICK_ACTION_PACKAGE_ID,
  HOME_CAPABILITY_CARD_LIMIT,
  HOME_WORKSHOP_CARD_LIMIT,
  createHomeBuiltinQuickActions,
  limitHomeCapabilityCards,
  limitHomeWorkshopCards,
  mergeHomeQuickActions,
  type HomeBuiltinQuickActionId,
} from "./home/shell-actions";

const InstallModal = lazy(() =>
  import("@/components/install/install-modal").then((module) => ({ default: module.InstallModal })),
);
const FailureModal = lazy(() =>
  import("@/components/install/failure-modal").then((module) => ({ default: module.FailureModal })),
);

export function HomePage() {
  const client = useKernel();
  const toast = useToast();
  const [, navigate] = useRoute();
  const { locale, t } = useLocale();

  const [search, setSearch] = useState("");
  const [activeFilter, setActiveFilter] = useState("all");
  const [showInstall, setShowInstall] = useState(false);
  const [failureProjectId, setFailureProjectId] = useState<string | null>(null);
  const [failureDetail, setFailureDetail] = useState<FailureDetail>();
  const packageQuickActions = useSurfaceContributions<QuickActionContribution>("quick_action", locale);
  const packageWorkshopCards = useSurfaceContributions<WorkshopCardContribution>("workshop_card", locale);
  const packageHomeCards = useSurfaceContributions<HomeCardContribution>("home_card", locale);
  const navigateTo = useCallback((route: Route) => navigate(route), [navigate]);
  const launchProject = useCallback(
    (projectId: string) => {
      const outcome = openProjectInTab(projectId);
      if (outcome === "failed" || outcome === "invalid") {
        toast.push({
          variant: "warning",
          title: t("homeProjectPopupBlockedTitle"),
          body: t("homeProjectPopupBlockedBody"),
        });
      }
      if (outcome === "tab") navigate({ kind: "project", projectId });
    },
    [navigate, t, toast],
  );
  const relativeAgeLabels = {
    now: t("homeContinueAgeNow"),
    minutesAgo: (count: number) => t("homeTimeMinutesAgo", count),
    hoursAgo: (count: number) => t("homeTimeHoursAgo", count),
    daysAgo: (count: number) => t("homeTimeDaysAgo", count),
    weeksAgo: (count: number) => t("homeTimeWeeksAgo", count),
    monthsAgo: (count: number) => t("homeTimeMonthsAgo", count),
    yearsAgo: (count: number) => t("homeTimeYearsAgo", count),
  };

  const {
    projects,
    lifecycleEvents,
    projectList,
    counts,
    filters,
    filtered,
    diskSegments,
    totalDisk,
    diskCapacity,
    continueEntry,
    timelineRows,
  } = useHomeProjects({
    client,
    search,
    activeFilter,
    labels: {
      all: t("homeFilterAll"),
      running: t("homeFilterRunning"),
      stopped: t("homeFilterStopped"),
      failed: t("homeFilterFailed"),
      relativeAge: relativeAgeLabels,
    },
  });

  const { onStop, onUninstall, onInstallClick, onShowFailure, onCardLaunch } = useProjectActions({
    client,
    onLaunch: launchProject,
    pushToast: toast.push,
    refreshProjects: () => projects.refresh(),
    setShowInstall,
    setFailureProjectId,
    setFailureDetail,
    labels: {
      stoppedToast: (title) => t("homeStoppedToast", title),
      stopFailedTitle: t("homeStopFailedTitle"),
      stopFailedBody: t("homeStopFailedBody"),
      uninstallTitle: (title) => t("homeUninstallTitle", title),
      uninstallingBody: t("homeUninstallingBody"),
      uninstalledTitle: (title) => t("homeUninstalledTitle", title),
      uninstalledBody: t("homeUninstalledBody"),
      uninstallFailedTitle: t("homeUninstallFailedTitle"),
      uninstallFailedBody: t("homeUninstallFailedBody"),
      loadingDiagnostics: t("homeLoadingDiagnostics"),
      loadingDiagnosticsSummary: t("homeLoadingDiagnosticsSummary"),
      descriptorNoPackages: t("homeDescriptorNoPackages"),
      noPackageStatus: t("homeNoPackageStatus"),
      diagnosticsUnavailable: t("homeDiagnosticsUnavailable"),
      noDiagnosticAvailable: t("homeNoDiagnosticAvailable"),
      diagnosticsUnavailableCause: t("homeDiagnosticsUnavailableCause"),
      packageFailureTitle: (packageId, state) => t("homePackageFailureTitle", packageId, state),
      packageDegradedSummary: t("homePackageDegradedSummary"),
      relativeAge: relativeAgeLabels,
    },
  });

  const onContinue = useCallback(
    (projectId: string) => {
      const project = projectList.find((p) => p.id === projectId);
      if (project) {
        onCardLaunch(project);
      }
    },
    [projectList, onCardLaunch],
  );

  const onBrowseProjects = useCallback(() => {
    const el = document.getElementById("home-projects");
    if (el) {
      el.scrollIntoView({ behavior: "smooth", block: "start" });
    }
  }, []);

  const hasInstalledProjects = projectList.length > 0;
  const builtinQuickActions = createHomeBuiltinQuickActions([
    { id: "install", title: t("homeQuickInstallUrl"), iconHint: "plus" },
    { id: "open-folder", title: t("homeQuickDataFolder"), iconHint: "folder" },
    { id: "settings", title: t("homeQuickSettings"), iconHint: "settings" },
    { id: "switch-profile", title: t("homeQuickSwitchProfile"), iconHint: "terminal" },
  ]);
  const quickActions = mergeHomeQuickActions({
    builtin: builtinQuickActions,
    packageActions: packageQuickActions.items,
  });
  const workshopCards = limitHomeWorkshopCards(packageWorkshopCards.items, HOME_WORKSHOP_CARD_LIMIT);
  const homeCards = limitHomeCapabilityCards(packageHomeCards.items, HOME_CAPABILITY_CARD_LIMIT);

  const onPackageContributionClick = useCallback(
    (item: QuickActionContribution | WorkshopCardContribution | HomeCardContribution) => {
      toast.push({
        variant: "info",
        title: t("homePackageActionFoundTitle", item.title),
        body: item.surfaceId ? t("homePackageActionFoundSurfaceBody") : t("homePackageActionFoundBody"),
      });
    },
    [t, toast],
  );

  const onQuickActionClick = useCallback(
    (action: QuickActionContribution) => {
      if (action.packageId !== HOME_BUILTIN_QUICK_ACTION_PACKAGE_ID) {
        onPackageContributionClick(action);
        return;
      }
      switch (action.id as HomeBuiltinQuickActionId) {
        case "install":
          onInstallClick();
          break;
        case "open-folder":
          toast.push({ variant: "info", title: t("homeOpenDataFolderToast") });
          break;
        case "settings":
          navigateTo({ kind: "settings", tab: "api-connections" });
          break;
        case "switch-profile":
          navigateTo({ kind: "settings", tab: "profiles" });
          break;
      }
    },
    [navigateTo, onInstallClick, onPackageContributionClick, t, toast],
  );

  return (
    <div className="mx-auto flex min-h-[calc(100dvh-60px)] w-full max-w-[1920px] flex-col gap-7 px-4 pt-6 pb-8 sm:px-6 sm:pb-10 lg:gap-8 lg:px-8 lg:pt-8 lg:pb-12 2xl:px-12 2xl:pb-14">
      <Hero
        meta={formatGreetingTime(locale, undefined, t("homeWorkshop"))}
        greeting={t("homeGreeting")}
        summary={
          projects.loading
            ? t("homeReading")
            : counts.all > 0
              ? t("homeShelfSummary", counts.all, counts.running, counts.stopped, counts.failed)
              : t("homeEmptyWorkshop")
        }
        continueEntry={continueEntry}
        continueLabels={{
          title: t("homeContinueTitle"),
          running: t("homeContinueRunning"),
          stopped: t("homeContinueStopped"),
          failed: t("homeContinueFailed"),
          continueAction: t("homeContinueResumeAction"),
          openAction: t("homeContinueOpenAction"),
          diagnoseAction: t("homeContinueDiagnoseAction"),
          ageNow: t("homeContinueAgeNow"),
          ageMinutes: (count) => t("homeTimeMinutesAgo", count),
          ageHours: (count) => t("homeTimeHoursAgo", count),
          ageDays: (count) => t("homeTimeDaysAgo", count),
          emptyTitle: t("homeContinueEmptyTitle"),
          emptyBody: t("homeContinueEmptyBody"),
          emptyInstall: t("homeContinueEmptyInstall"),
          emptyTryYdltavern: t("homeContinueEmptyTryYdltavern"),
          pickInstalled: t("homeContinuePickInstalled"),
        }}
        hasInstalledProjects={hasInstalledProjects}
        onContinue={onContinue}
        onInstall={onInstallClick}
        onBrowseProjects={onBrowseProjects}
      />

      <HomeCapabilityCards
        items={homeCards}
        onCardClick={onPackageContributionClick}
        ariaLabel={t("homeCapabilityCards")}
        maxItems={HOME_CAPABILITY_CARD_LIMIT}
        className="-mt-3"
      />

      <div className="grid flex-1 grid-cols-1 gap-8 lg:min-h-0 lg:grid-cols-[1fr_380px] xl:grid-cols-[1fr_420px] 2xl:grid-cols-[1fr_460px]">
        <div id="home-projects" className="flex min-h-0 flex-col gap-5">
          <UtilityStrip
            search={search}
            onSearchChange={setSearch}
            filters={filters}
            activeFilter={activeFilter}
            onFilterChange={setActiveFilter}
            sortLabel={t("homeSortRecent")}
            sortPrefix={t("homeSortPrefix")}
            searchPlaceholder={t("homeSearchPlaceholder")}
          />

          <div className="flex items-center justify-between">
            <Eyebrow>{t("homeInstalledEyebrow", counts.all)}</Eyebrow>
          </div>

          {projects.error ? (
            <EmptyState
              icon={<Warning />}
              title={t("homeErrorTitle")}
              body={t("homeErrorBody")}
              action={{ label: t("retry"), onClick: () => projects.refresh() }}
            />
          ) : projects.loading ? (
            <div
              className="grid gap-5"
              style={{ gridTemplateColumns: "repeat(auto-fill, minmax(280px, 1fr))" }}
            >
              {Array.from({ length: 4 }).map((_, idx) => (
                <Skeleton key={idx} className="h-[220px] rounded-[20px]" />
              ))}
            </div>
          ) : projectList.length === 0 ? (
            <div className="flex min-h-[320px] flex-1 items-center justify-center rounded-[24px] border border-dashed border-whisper-border/50 bg-pure-surface/20 px-4 py-8 lg:min-h-[clamp(360px,42vh,620px)]">
              <EmptyState
                icon={<Plus />}
                title={t("homeEmptyTitle")}
                body={t("homeEmptyBody")}
                action={{ label: t("homeInstallLabel"), onClick: onInstallClick }}
                className="border-0 bg-transparent py-8 shadow-none"
              />
            </div>
          ) : (
            <div
              className="grid gap-5"
              style={{ gridTemplateColumns: "repeat(auto-fill, minmax(280px, 1fr))" }}
            >
              {filtered.map((project, idx) => (
                <ProjectCard
                  key={project.id}
                  index={idx}
                  data={{
                    id: project.id,
                    title: project.title,
                    description: project.description,
                    state: project.state,
                    type: project.type,
                    source: project.type === "plurora_native" ? "github" : "local",
                  }}
                  actions={{
                    onLaunch: () => onCardLaunch(project),
                    onStop: () => onStop(project.id, project.title),
                    onRestart: () => onCardLaunch(project),
                    onUninstall: () => void onUninstall(project),
                    onConfigure: () => navigateTo({ kind: "settings", tab: "installed-packages" }),
                    onViewLogs:
                      project.state === "failed" ? () => void onShowFailure(project) : undefined,
                  }}
                  labels={{
                    restart: t("homeActionRestart"),
                    resume: t("homeActionResume"),
                    play: t("homeActionPlay"),
                    more: t("homeMore"),
                    actionsAria: (title) => t("homeActionsAria", title),
                    stop: t("homeActionStop"),
                    open: t("homeActionOpen"),
                    configure: t("homeActionConfigure"),
                    viewLogs: t("homeActionViewLogs"),
                    uninstall: t("homeActionUninstall"),
                  }}
                />
              ))}
              <InstallCard
                onClick={onInstallClick}
                index={filtered.length}
                title={t("homeInstallLabel")}
                hint={t("homeInstallHint")}
              />
            </div>
          )}
        </div>

        <div className="flex min-h-0 flex-col gap-6 lg:h-full">
          <ActivityTimeline
            rows={timelineRows.slice(0, 6)}
            loading={lifecycleEvents.loading && timelineRows.length === 0}
            onViewAll={() => navigateTo({ kind: "settings", tab: "installed-packages" })}
            title={t("homeActivityLast24h")}
            emptyLabel={t("homeActivityNo24h")}
            viewAllLabel={t("homeViewFullAuditLog")}
          />
          <WorkshopUtilities
            updates={[]}
            totalDisk={totalDisk}
            diskCapacity={diskCapacity}
            diskSegments={diskSegments}
            labels={{
              workshop: t("homeWorkshop"),
              updates: t("homeUpdates"),
              updatesAvailable: (count) => t("homeUpdatesAvailable", count),
              everythingUpToDate: t("homeEverythingUpToDate"),
              update: t("homeUpdate"),
              updateAll: t("homeUpdateAll"),
              diskUsage: t("homeDiskUsage"),
              diskUsed: (value) => t("homeDiskUsed", value),
              unknown: t("homeUnknown"),
              measuring: t("homeMeasuring"),
              noStorageMeasured: t("homeNoStorageMeasured"),
              manageStorage: t("homeManageStorage"),
              workshopCards: t("homeWorkshopCards"),
              categoryTool: t("homeWorkshopCategoryTool"),
              categoryTemplate: t("homeWorkshopCategoryTemplate"),
              categoryExample: t("homeWorkshopCategoryExample"),
              quickActions: t("homeQuickActions"),
            }}
            quickActions={quickActions}
            workshopCards={workshopCards}
            onQuickActionClick={onQuickActionClick}
            onWorkshopCardClick={onPackageContributionClick}
          />
        </div>
      </div>

      {showInstall ? (
        <Suspense fallback={null}>
          <InstallModal open={showInstall} onClose={() => setShowInstall(false)} onInstalled={projects.refresh} />
        </Suspense>
      ) : null}
      {failureProjectId !== null ? (
        <Suspense fallback={null}>
          <FailureModal
            open={failureProjectId !== null}
            onClose={() => setFailureProjectId(null)}
            onRestart={() => {
              if (failureProjectId) navigateTo({ kind: "project", projectId: failureProjectId });
            }}
            onUninstall={() => {
              const project = projectList.find((entry) => entry.id === failureProjectId);
              if (project) void onUninstall(project);
            }}
            detail={failureDetail}
          />
        </Suspense>
      ) : null}
    </div>
  );
}
