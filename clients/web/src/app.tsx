import { Suspense, lazy, type ReactNode } from "react";
import { MotionConfig } from "motion/react";
import { IconContext } from "@phosphor-icons/react";
import { ThemeProvider } from "@/lib/theme";
import { LocaleProvider } from "@/lib/locale";
import { AuthProvider, useAuth } from "@/lib/auth-gate";
import { PluroraProvider } from "@/lib/plurora-client";
import { ToastProvider } from "@/components/ui/toast";
import { TooltipProvider } from "@/components/ui/tooltip";
import { AuthGateScreen, AuthChecking, HostUnavailable } from "@/components/auth-gate";
import { Shell } from "@/components/layout/shell";
import { Skeleton } from "@/components/ui/skeleton";
import { usePathInstallationRoute } from "@/lib/router";

const InstallationFrame = lazy(() =>
  import("@/routes/installation-frame").then((module) => ({ default: module.InstallationFrame })),
);
const PairingPage = lazy(() =>
  import("@/routes/pairing").then((module) => ({ default: module.PairingPage })),
);

const iconDefaults = {
  color: "currentColor",
  size: 18,
  weight: "regular" as const,
  mirrored: false,
};

export function App({ children }: { children?: ReactNode }) {
  return (
    <ThemeProvider>
      <LocaleProvider>
        <IconContext.Provider value={iconDefaults}>
          <AuthProvider>
            <AppInner>{children}</AppInner>
          </AuthProvider>
        </IconContext.Provider>
      </LocaleProvider>
    </ThemeProvider>
  );
}

function AppInner({ children }: { children?: ReactNode }) {
  const { status, token } = useAuth();
  const pathInstallationRoute = usePathInstallationRoute();
  const isPairingPath = typeof window !== "undefined" && window.location.pathname === "/pair";

  if (isPairingPath) {
    return (
      <Suspense fallback={<AuthChecking />}>
        <PairingPage />
      </Suspense>
    );
  }

  if (status === "checking") {
    return <AuthChecking />;
  }

  if (status === "unavailable") {
    return <HostUnavailable />;
  }

  const showGate = status === "required" || status === "invalid";
  if (showGate) {
    return <AuthGateScreen />;
  }

  return (
    <PluroraProvider accessToken={token}>
      <MotionConfig reducedMotion="user">
        <TooltipProvider>
          <ToastProvider>
            {children ?? (pathInstallationRoute ? (
              <Suspense fallback={<InstallationTabSkeleton />}>
                <InstallationFrame installationId={pathInstallationRoute.installationId} chrome="none" />
              </Suspense>
            ) : (
              <Shell />
            ))}
          </ToastProvider>
        </TooltipProvider>
      </MotionConfig>
    </PluroraProvider>
  );
}

function InstallationTabSkeleton() {
  return (
    <div className="flex min-h-[100dvh] flex-col gap-4 bg-warm-bone p-6">
      <Skeleton className="h-5 w-44" />
      <Skeleton className="min-h-0 flex-1 rounded-[24px]" />
    </div>
  );
}
