import { resolveHostBaseUrl } from "./host-endpoint";

export type HostAccessScope =
  | "observe"
  | "installation.manage"
  | "run"
  | "binding.manage"
  | "exposure.manage"
  | "realization.plan"
  | "realization.apply"
  | "deploy"
  | "develop.propose"
  | "develop.approve"
  | "develop.execute"
  | "access_manage";

export type HostAccessIdentityKind = "root" | "device";

export type HostAccessResourceKind = "work" | "workspace" | "installation" | "run" | "target" | "exposure" | "binding" | "realization";

export interface HostAccessResourceSelector {
  kind: HostAccessResourceKind;
  id: string | null;
}

export interface HostAccessIdentity {
  kind: HostAccessIdentityKind;
  grant_id?: string | null;
  device_name: string;
  scopes: HostAccessScope[];
  resources?: HostAccessResourceSelector[];
  delegation_chain?: string[];
  expires_at_ms?: number | null;
}

export interface HostAccessGrant {
  id: string;
  device_name: string;
  scopes: HostAccessScope[];
  resources?: HostAccessResourceSelector[];
  parent_grant_id?: string | null;
  delegation_depth?: number;
  created_at_ms: number;
  expires_at_ms: number;
  revoked_at_ms?: number | null;
  active: boolean;
}

export interface HostPairing {
  id: string;
  device_name: string;
  scopes: HostAccessScope[];
  resources?: HostAccessResourceSelector[];
  parent_grant_id?: string | null;
  delegation_depth?: number;
  created_at_ms: number;
  expires_at_ms: number;
  grant_expires_at_ms: number;
  status: "pending" | "claimed" | "cancelled" | "expired" | string;
  grant_id?: string | null;
}

export interface HostAccessOverview {
  identity: HostAccessIdentity;
  grants: HostAccessGrant[];
  pairings: HostPairing[];
}

export interface CreateHostPairingInput {
  device_name: string;
  scopes: HostAccessScope[];
  resources: HostAccessResourceSelector[];
  pairing_ttl_secs?: number;
  grant_ttl_secs?: number;
}

export interface CreateHostPairingResponse {
  pairing: HostPairing;
  pairing_token: string;
}

export interface ClaimHostPairingResponse {
  grant: HostAccessGrant;
}

export class HostAccessHttpError extends Error {
  constructor(
    readonly status: number,
    readonly body: string,
  ) {
    super(`${status}: ${body || "Host access request failed"}`);
    this.name = "HostAccessHttpError";
  }
}

async function hostAccessRequest<T>(
  path: string,
  {
    method = "GET",
    body,
    accessToken,
  }: {
    method?: "GET" | "POST";
    body?: unknown;
    accessToken?: string | null;
  } = {},
): Promise<T> {
  const response = await fetch(`${resolveHostBaseUrl()}${path}`, {
    method,
    credentials: "same-origin",
    cache: "no-store",
    referrerPolicy: "no-referrer",
    headers: {
      ...(body === undefined ? {} : { "content-type": "application/json" }),
      ...(accessToken ? { authorization: `Bearer ${accessToken}` } : {}),
    },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  });
  if (!response.ok) {
    const responseBody = await response.text().catch(() => response.statusText);
    throw new HostAccessHttpError(response.status, responseBody);
  }
  if (response.status === 204) return undefined as T;
  return (await response.json()) as T;
}

export function inspectHostPairing(pairingToken: string): Promise<HostPairing> {
  return hostAccessRequest("/host/v1/access/pair/inspect", {
    method: "POST",
    body: { pairing_token: pairingToken },
  });
}

export function claimHostPairing(pairingToken: string): Promise<ClaimHostPairingResponse> {
  return hostAccessRequest("/host/v1/access/pair", {
    method: "POST",
    body: { pairing_token: pairingToken },
  });
}

export function getHostAccessIdentity(accessToken?: string | null): Promise<HostAccessIdentity> {
  return hostAccessRequest("/host/v1/access/me", { accessToken });
}

export function getHostAccessOverview(accessToken?: string | null): Promise<HostAccessOverview> {
  return hostAccessRequest("/host/v1/access", { accessToken });
}

export function createHostPairing(
  input: CreateHostPairingInput,
  accessToken?: string | null,
): Promise<CreateHostPairingResponse> {
  return hostAccessRequest("/host/v1/access/pairings", {
    method: "POST",
    body: input,
    accessToken,
  });
}

export function cancelHostPairing(
  pairingId: string,
  accessToken?: string | null,
): Promise<HostPairing> {
  return hostAccessRequest(
    `/host/v1/access/pairings/${encodeURIComponent(pairingId)}/cancel`,
    { method: "POST", body: {}, accessToken },
  );
}

export function revokeHostAccessGrant(
  grantId: string,
  accessToken?: string | null,
): Promise<HostAccessGrant> {
  return hostAccessRequest(
    `/host/v1/access/grants/${encodeURIComponent(grantId)}/revoke`,
    { method: "POST", body: {}, accessToken },
  );
}

export function logoutHostAccess(accessToken?: string | null): Promise<void> {
  return hostAccessRequest("/host/v1/access/logout", {
    method: "POST",
    body: {},
    accessToken,
  });
}
