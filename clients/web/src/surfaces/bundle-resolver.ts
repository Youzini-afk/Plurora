import type { PluroraProtocolClient } from "../protocol/client";

export interface ResolvedSurfaceBundle {
  surfaceId: string;
  bundleUrl: string;
  bundleFingerprint?: string;
  exportName: string;
  stylesheets: string[];
  wrapperClass?: string;
  installationId?: string;
  source: "installed_installation" | "dev_path";
}

export async function resolveSurfaceBundle(
  client: PluroraProtocolClient,
  surfaceId: string,
): Promise<ResolvedSurfaceBundle> {
  const result = await client.invoke("host.surface.bundle.resolve", { surface_id: surfaceId });
  const r = result as {
    surface_id: string;
    bundle_url: string;
    bundle_fingerprint?: string;
    export_name: string;
    stylesheets?: string[];
    wrapper_class?: string;
    installation_id?: string;
    source: "installed_installation" | "dev_path";
  };
  const hostOrigin = new URL(client.baseUrl).origin;
  const resolveAsset = (value: string) => {
    const url = new URL(value, `${hostOrigin}/`);
    if (url.origin !== hostOrigin || (url.protocol !== "http:" && url.protocol !== "https:")) {
      throw new Error("Resolved surface asset is outside the selected Host");
    }
    return url.toString();
  };
  return {
    surfaceId: r.surface_id,
    bundleUrl: resolveAsset(r.bundle_url),
    bundleFingerprint: r.bundle_fingerprint,
    exportName: r.export_name,
    stylesheets: (r.stylesheets ?? []).map(resolveAsset),
    wrapperClass: r.wrapper_class,
    installationId: r.installation_id,
    source: r.source,
  };
}
