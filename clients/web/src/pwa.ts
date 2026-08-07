export function canRegisterServiceWorker(
  locationLike: Pick<Location, "protocol" | "hostname" | "search"> = location,
  serviceWorkerSupported = typeof navigator !== "undefined" && "serviceWorker" in navigator,
): boolean {
  const isHttp = locationLike.protocol === "http:" || locationLike.protocol === "https:";
  const isTauriResourceOrigin = locationLike.hostname === "tauri.localhost";
  const isManagedDesktop = new URLSearchParams(locationLike.search).get("plurora_platform") === "desktop";
  return isHttp && !isTauriResourceOrigin && !isManagedDesktop && serviceWorkerSupported;
}

export async function registerPwa(): Promise<void> {
  if (!canRegisterServiceWorker()) return;
  try {
    const { registerSW } = await import("virtual:pwa-register");
    registerSW({
      immediate: true,
      onRegisterError(error) {
        console.warn("Plurora service worker registration failed", error);
      },
    });
  } catch (error) {
    console.warn("Plurora PWA bootstrap failed", error);
  }
}
