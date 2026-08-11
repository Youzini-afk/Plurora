import type { InstallationWorkSummary, WorkEntrypoint } from "@/protocol/generated-types";

export const FOREIGN_LAUNCH_BINDING_SCHEMA = "plurora.foreign-launch-binding.v1";

export type ForeignLaunchKind =
  | "external_uri"
  | "local_executable"
  | "managed_artifact"
  | "oci_image"
  | "remote_service"
  | "entitlement_adapter";

export interface ForeignLaunchBinding {
  schema: typeof FOREIGN_LAUNCH_BINDING_SCHEMA;
  launch_id: string;
  target: Record<string, unknown> & { kind: ForeignLaunchKind };
  entitlement?: Record<string, unknown>;
}

export function isForeignWork(summary: InstallationWorkSummary): boolean {
  return summary.annotations?.["plurora.normalization/kind"] === "foreign_capsule";
}

export function foreignLaunchEntrypoints(summary: InstallationWorkSummary): WorkEntrypoint[] {
  return summary.entrypoints.filter((entrypoint) => entrypoint.target.kind === "foreign_launch");
}

export async function foreignLaunchSecretName(launchId: string): Promise<string> {
  const bytes = new TextEncoder().encode(launchId);
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  const hex = [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
  return `foreign-launch-${hex}`;
}

export async function foreignLaunchSecretRef(launchId: string): Promise<string> {
  return `secret_ref:installation:${await foreignLaunchSecretName(launchId)}`;
}

export function parseForeignLaunchBinding(
  text: string,
  expectedLaunchId: string,
): ForeignLaunchBinding {
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    throw new Error("binding_invalid");
  }
  if (!isRecord(value)
    || value.schema !== FOREIGN_LAUNCH_BINDING_SCHEMA
    || value.launch_id !== expectedLaunchId
    || !isRecord(value.target)
    || !isLaunchKind(value.target.kind)) {
    throw new Error("binding_invalid");
  }
  validateTarget(value.target);
  if (value.entitlement !== undefined && !isEntitlementAdapter(value.entitlement)) {
    throw new Error("binding_invalid");
  }
  return value as unknown as ForeignLaunchBinding;
}

function validateTarget(target: Record<string, unknown>): void {
  const requiredText: Record<ForeignLaunchKind, string | null> = {
    external_uri: "uri",
    local_executable: "executable",
    managed_artifact: null,
    oci_image: "image",
    remote_service: "endpoint",
    entitlement_adapter: null,
  };
  const key = requiredText[target.kind as ForeignLaunchKind];
  if (key && !nonEmptyText(target[key])) throw new Error("binding_invalid");
  if (target.kind === "managed_artifact" && !isRecord(target.artifact)) {
    throw new Error("binding_invalid");
  }
  if (target.kind === "entitlement_adapter" && !isEntitlementAdapter(target.adapter)) {
    throw new Error("binding_invalid");
  }
  if (target.args !== undefined
    && (!Array.isArray(target.args) || target.args.some((item) => typeof item !== "string" || item.includes("\0")))) {
    throw new Error("binding_invalid");
  }
}

function isEntitlementAdapter(value: unknown): value is Record<string, unknown> {
  return isRecord(value) && nonEmptyText(value.package_id) && nonEmptyText(value.capability_id);
}

function nonEmptyText(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0 && !value.includes("\0");
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function isLaunchKind(value: unknown): value is ForeignLaunchKind {
  return typeof value === "string" && [
    "external_uri",
    "local_executable",
    "managed_artifact",
    "oci_image",
    "remote_service",
    "entitlement_adapter",
  ].includes(value);
}
