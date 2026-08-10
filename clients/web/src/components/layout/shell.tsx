import { Suspense, lazy } from "react";
import { useRoute, type Route } from "@/lib/router";
import { PlatformTopbar } from "@/components/layout/platform-topbar";
import { Skeleton } from "@/components/ui/skeleton";

const HomePage = lazy(() => import("@/routes/home").then((module) => ({ default: module.HomePage })));
const SettingsPage = lazy(() =>
  import("@/routes/settings").then((module) => ({ default: module.SettingsPage })),
);
const InstallationFrame = lazy(() =>
  import("@/routes/installation-frame").then((module) => ({ default: module.InstallationFrame })),
);

export function Shell() {
  const [route] = useRoute();

  return (
    <div className="flex min-h-[100dvh] flex-col bg-warm-bone text-charcoal-ink">
      <PlatformTopbar route={route} />
      <main className="flex flex-1 flex-col">
        <Suspense fallback={<RouteSkeleton />}>{renderRoute(route)}</Suspense>
      </main>
    </div>
  );
}

function RouteSkeleton() {
  return (
    <div className="mx-auto flex w-full max-w-[1920px] flex-1 flex-col gap-6 px-4 py-6 sm:px-6 lg:px-8 lg:py-10 2xl:px-12">
      <Skeleton className="h-5 w-32" />
      <Skeleton className="h-16 w-full max-w-[760px] rounded-[18px]" />
      <div className="grid gap-6 lg:grid-cols-[1fr_380px] xl:grid-cols-[1fr_420px] 2xl:grid-cols-[1fr_460px]">
        <Skeleton className="h-[420px] rounded-[24px]" />
        <Skeleton className="h-[420px] rounded-[24px]" />
      </div>
    </div>
  );
}

function renderRoute(route: Route) {
  switch (route.kind) {
    case "home":
      return <HomePage />;
    case "settings":
      return <SettingsPage tab={route.tab} />;
    case "installation":
      return <InstallationFrame installationId={route.installationId} />;
  }
}
