export type RouteAccess = "host_authenticated" | "public";

export interface DockerDeploymentDescriptor {
  image: string;
  container_port: number;
  port_name: string;
  route_id: string;
  route_access: RouteAccess;
  health_path?: string;
  pull_if_missing: boolean;
}

export interface DockerDeploymentParseResult {
  descriptor: DockerDeploymentDescriptor | null;
  error?: string;
}

export type BuildDeployStrategy = "dockerfile" | "nixpacks";

export interface BuildDeployEnvDescriptor {
  name: string;
  value?: string;
  secret_ref?: string;
}

export interface BuildDeployMountDescriptor {
  source_host_path: string;
  container_path: string;
  mode: "ro" | "rw";
  approved: boolean;
  high_risk_approved: boolean;
  reason: string;
}

export interface BuildDeployDescriptor {
  source_url: string;
  ref_name: string;
  strategy: BuildDeployStrategy;
  dockerfile?: string;
  container_port: number;
  port_name: string;
  route_id: string;
  route_access: RouteAccess;
  health_path?: string;
  runtime_env: BuildDeployEnvDescriptor[];
  runtime_mounts: BuildDeployMountDescriptor[];
}

export interface BuildDeployParseResult {
  descriptor: BuildDeployDescriptor | null;
  error?: string;
}

// Explicit web deploy metadata lives at installation.metadata.deployment.docker.
// This intentionally accepts only one prebuilt Docker HTTP image and host-owned
// port/proxy metadata; env, volumes, mounts, and secrets are not part of Phase 5.
export function parseDockerDeploymentDescriptor(
  installationId: string,
  metadata: unknown,
): DockerDeploymentParseResult {
  const docker = readDockerDeploymentMetadata(metadata);
  if (docker === undefined) return { descriptor: null };
  if (!isRecord(docker)) return { descriptor: null, error: "deployment.docker must be an object" };

  if ("env" in docker || "environment" in docker || "secrets" in docker || "volumes" in docker || "mounts" in docker || "binds" in docker) {
    return { descriptor: null, error: "deployment.docker cannot declare env, secrets, volumes, mounts, or binds" };
  }

  const image = docker.image;
  if (typeof image !== "string" || !isSafeDockerImage(image.trim())) {
    return { descriptor: null, error: "deployment.docker.image must be a safe Docker image reference" };
  }

  const rawContainerPort = docker.container_port;
  if (typeof rawContainerPort !== "number" || !Number.isInteger(rawContainerPort) || rawContainerPort < 1 || rawContainerPort > 65_535) {
    return { descriptor: null, error: "deployment.docker.container_port must be an integer in 1..65535" };
  }
  const containerPort = rawContainerPort;

  const portName = docker.port_name === undefined ? "web" : docker.port_name;
  if (typeof portName !== "string" || !isSafeRouteToken(portName)) {
    return { descriptor: null, error: "deployment.docker.port_name must be label-safe" };
  }

  const routeId = docker.route_id === undefined ? `${installationId}-web` : docker.route_id;
  if (typeof routeId !== "string" || !isSafeRouteToken(routeId)) {
    return { descriptor: null, error: "deployment.docker.route_id must be label-safe" };
  }

  const routeAccess = docker.route_access ?? "host_authenticated";
  if (routeAccess !== "host_authenticated" && routeAccess !== "public") {
    return { descriptor: null, error: "deployment.docker.route_access must be host_authenticated or public" };
  }

  const healthPath = docker.health_path;
  if (healthPath !== undefined && (typeof healthPath !== "string" || !healthPath.startsWith("/") || healthPath.length > 256)) {
    return { descriptor: null, error: "deployment.docker.health_path must start with /" };
  }

  const pullIfMissing = docker.pull_if_missing ?? false;
  if (typeof pullIfMissing !== "boolean") {
    return { descriptor: null, error: "deployment.docker.pull_if_missing must be a boolean" };
  }

  return {
    descriptor: {
      image: image.trim(),
      container_port: containerPort,
      port_name: portName,
      route_id: routeId,
      route_access: routeAccess,
      ...(healthPath ? { health_path: healthPath } : {}),
      pull_if_missing: pullIfMissing,
    },
  };
}

export function parseBuildDeployDescriptor(installationId: string, metadata: unknown): BuildDeployParseResult {
  const raw = readBuildDeployMetadata(metadata);
  if (raw === undefined) return { descriptor: null };
  if (!isRecord(raw)) return { descriptor: null, error: "deployment.build_deploy must be an object" };

  for (const denied of ["env", "environment", "secrets", "volumes", "mounts", "binds", "build_env", "build_secrets"]) {
    if (denied in raw) return { descriptor: null, error: `deployment.build_deploy cannot declare ${denied}; use runtime_env/runtime_mounts` };
  }

  const sourceUrl = raw.source_url;
  const normalizedSourceUrl = typeof sourceUrl === "string" ? normalizeBuildDeploySourceUrl(sourceUrl) : null;
  if (!normalizedSourceUrl) {
    return { descriptor: null, error: "deployment.build_deploy.source_url must be HTTPS without userinfo" };
  }
  const refName = raw.ref_name === undefined ? "HEAD" : raw.ref_name;
  if (typeof refName !== "string" || refName.trim().length === 0 || refName.length > 256 || refName.includes("\0")) {
    return { descriptor: null, error: "deployment.build_deploy.ref_name is invalid" };
  }
  const strategy = raw.strategy === undefined ? "dockerfile" : raw.strategy;
  if (strategy !== "dockerfile" && strategy !== "nixpacks") {
    return { descriptor: null, error: "deployment.build_deploy.strategy must be dockerfile or nixpacks" };
  }
  const dockerfile = raw.dockerfile_path ?? raw.dockerfile;
  if (dockerfile !== undefined && (typeof dockerfile !== "string" || !isSafeRelativePath(dockerfile))) {
    return { descriptor: null, error: "deployment.build_deploy.dockerfile_path must be relative and safe" };
  }

  const rawContainerPort = raw.container_port;
  if (typeof rawContainerPort !== "number" || !Number.isInteger(rawContainerPort) || rawContainerPort < 1 || rawContainerPort > 65_535) {
    return { descriptor: null, error: "deployment.build_deploy.container_port must be an integer in 1..65535" };
  }
  const portName = raw.port_name === undefined ? "web" : raw.port_name;
  if (typeof portName !== "string" || !isSafeRouteToken(portName)) return { descriptor: null, error: "deployment.build_deploy.port_name must be label-safe" };
  const routeId = raw.route_id === undefined ? `${installationId}-web` : raw.route_id;
  if (typeof routeId !== "string" || !isSafeRouteToken(routeId)) return { descriptor: null, error: "deployment.build_deploy.route_id must be label-safe" };
  const routeAccess = raw.route_access ?? "host_authenticated";
  if (routeAccess !== "host_authenticated" && routeAccess !== "public") {
    return { descriptor: null, error: "deployment.build_deploy.route_access must be host_authenticated or public" };
  }
  const healthPath = raw.health_path;
  if (healthPath !== undefined && (typeof healthPath !== "string" || !healthPath.startsWith("/") || healthPath.length > 256)) {
    return { descriptor: null, error: "deployment.build_deploy.health_path must start with /" };
  }

  const env = parseRuntimeEnv(raw.runtime_env);
  if ("error" in env) return { descriptor: null, error: env.error };
  const mounts = parseRuntimeMounts(raw.runtime_mounts);
  if ("error" in mounts) return { descriptor: null, error: mounts.error };

  return { descriptor: {
    source_url: normalizedSourceUrl,
    ref_name: refName.trim(),
    strategy,
    ...(dockerfile ? { dockerfile } : {}),
    container_port: rawContainerPort,
    port_name: portName,
    route_id: routeId,
    route_access: routeAccess,
    ...(healthPath ? { health_path: healthPath } : {}),
    runtime_env: env.value,
    runtime_mounts: mounts.value,
  } };
}

function readDockerDeploymentMetadata(metadata: unknown): unknown {
  if (!isRecord(metadata)) return undefined;
  const deployment = metadata.deployment;
  if (!isRecord(deployment)) return undefined;
  return deployment.docker;
}

function readBuildDeployMetadata(metadata: unknown): unknown {
  if (!isRecord(metadata)) return undefined;
  const deployment = metadata.deployment;
  if (!isRecord(deployment)) return undefined;
  return deployment.build_deploy;
}

function parseRuntimeEnv(raw: unknown): { value: BuildDeployEnvDescriptor[] } | { error: string } {
  if (raw === undefined) return { value: [] };
  if (!Array.isArray(raw) || raw.length > 128) return { error: "deployment.build_deploy.runtime_env must be an array of at most 128 entries" };
  const seen = new Set<string>();
  const value: BuildDeployEnvDescriptor[] = [];
  for (const entry of raw) {
    if (!isRecord(entry)) return { error: "deployment.build_deploy.runtime_env entries must be objects" };
    const name = entry.name;
    if (typeof name !== "string" || !/^[A-Za-z_][A-Za-z0-9_]{0,127}$/.test(name) || seen.has(name)) return { error: "deployment.build_deploy.runtime_env has invalid or duplicate name" };
    seen.add(name);
    const hasValue = typeof entry.value === "string";
    const hasSecret = typeof entry.secret_ref === "string";
    if (hasValue === hasSecret) return { error: "deployment.build_deploy.runtime_env requires exactly one of value or secret_ref" };
    if (hasValue) {
      const v = entry.value as string;
      if (v.length > 8192 || v.includes("\0") || /^secret_ref:/i.test(v) || looksSecretLike(v)) return { error: "deployment.build_deploy.runtime_env contains an unsafe plain value" };
      value.push({ name, value: v });
    } else {
      const ref = entry.secret_ref as string;
      if (!isAllowedSecretRef(ref)) return { error: "deployment.build_deploy.runtime_env secret_ref is invalid" };
      value.push({ name, secret_ref: ref });
    }
  }
  return { value };
}

function parseRuntimeMounts(raw: unknown): { value: BuildDeployMountDescriptor[] } | { error: string } {
  if (raw === undefined) return { value: [] };
  if (!Array.isArray(raw) || raw.length > 32) return { error: "deployment.build_deploy.runtime_mounts must be an array of at most 32 entries" };
  const targets = new Set<string>();
  const value: BuildDeployMountDescriptor[] = [];
  for (const entry of raw) {
    if (!isRecord(entry)) return { error: "deployment.build_deploy.runtime_mounts entries must be objects" };
    const source = entry.host_path ?? entry.source_host_path;
    const target = entry.container_path;
    const mode = entry.mode === undefined ? "ro" : entry.mode;
    const reason = entry.reason;
    if (typeof source !== "string" || !source.startsWith("/") || source.includes("\0")) return { error: "runtime mount host_path must be absolute" };
    if (typeof target !== "string" || !target.startsWith("/") || target.includes("..") || target.includes("\0") || targets.has(target)) return { error: "runtime mount container_path is invalid or duplicate" };
    targets.add(target);
    if (mode !== "ro" && mode !== "rw") return { error: "runtime mount mode must be ro or rw" };
    if (typeof reason !== "string" || reason.trim().length === 0 || reason.length > 512) return { error: "runtime mount reason is required" };
    value.push({
      source_host_path: source,
      container_path: target,
      mode,
      approved: entry.approved === true,
      high_risk_approved: entry.high_risk_approved === true,
      reason,
    });
  }
  return { value };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isSafeDockerImage(image: string): boolean {
  return image.length > 0
    && image.length <= 255
    && !image.startsWith("-")
    && !image.includes("..")
    && /^[A-Za-z0-9./:_@-]+$/.test(image);
}

function isSafeRouteToken(value: string): boolean {
  return value.length > 0
    && value.length <= 128
    && !value.includes("..")
    && /^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(value);
}

function isSafeRelativePath(value: string): boolean {
  return value.length > 0 && value.length <= 256 && !value.startsWith("/") && !value.includes("..") && !value.includes("\0");
}

function looksSecretLike(value: string): boolean {
  return /(password|api[_-]?key|secret|token)=/i.test(value) || value.length > 256;
}

function normalizeBuildDeploySourceUrl(value: string): string | null {
  try {
    const url = new URL(value.trim());
    if (url.protocol !== "https:" || url.username || url.password || url.search || url.hash) return null;
    if (!url.hostname || url.hostname.includes("..")) return null;
    return url.toString();
  } catch {
    return null;
  }
}

function isAllowedSecretRef(value: string): boolean {
  if (!value || value.length > 512 || value.includes("\0")) return false;
  return /^(store|installation|env):[A-Za-z0-9_.:-]+$/.test(value);
}
