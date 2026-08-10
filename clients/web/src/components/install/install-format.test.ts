import { detectKindFromInstallPlan, errorMessage } from "./install-format";
import type { InstallPlan } from "@/protocol/client";

function assertEqual<T>(actual: T, expected: T) {
  if (actual !== expected) {
    throw new Error(`expected ${String(expected)}, got ${String(actual)}`);
  }
}

function basePlan(installationType?: string): InstallPlan {
  return {
    root_id: "pkg/root",
    packages: [],
    ...(installationType
      ? {
          installation_descriptor: {
            schema_version: 1,
            installation: { type: installationType },
          },
        }
      : {}),
    permissions_summary: {
      new_capabilities: [],
      new_network_hosts: [],
      new_secret_refs: [],
    },
    signature_summary: {
      all_signed: false,
      unsigned_packages: [],
    },
    integrity_summary: {
      manifest_hashes_match_lockfile: true,
      drift_detected: [],
    },
  };
}

assertEqual(detectKindFromInstallPlan(basePlan("plurora_native"))?.kind, "native");
assertEqual(detectKindFromInstallPlan(basePlan("external_wrapped"))?.kind, "declared_external");
assertEqual(detectKindFromInstallPlan(basePlan("external_workspace"))?.kind, "declared_external");
assertEqual(detectKindFromInstallPlan(basePlan())?.kind, undefined);
assertEqual(errorMessage(new TypeError("Failed to fetch")), "Failed to fetch");
