import { resolveInstallReview } from "./use-install-flow";
import type { InstallDetectedKind, InstallPlan, InstallSource } from "@/protocol/client";

function assertEqual<T>(actual: T, expected: T) {
  if (actual !== expected) {
    throw new Error(`expected ${String(expected)}, got ${String(actual)}`);
  }
}

function plan(installationType?: string): InstallPlan {
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
    permissions_summary: { new_capabilities: [], new_network_hosts: [], new_secret_refs: [] },
    signature_summary: { all_signed: false, unsigned_packages: [] },
    integrity_summary: { manifest_hashes_match_lockfile: true, drift_detected: [] },
  };
}

const source: InstallSource = {
  root_url: "https://github.com/Youzini-afk/Plurora-Tavern",
  root_ref: "HEAD",
};

{
  let resolveCalls = 0;
  let detectCalls = 0;
  const review = await resolveInstallReview(
    {
      async resolveInstallPlan() {
        resolveCalls += 1;
        return plan("plurora_native");
      },
      async detectInstallKind(): Promise<InstallDetectedKind> {
        detectCalls += 1;
        return { kind: "native" };
      },
    },
    source,
  );
  assertEqual(resolveCalls, 1);
  assertEqual(detectCalls, 0);
  assertEqual(review.step, "plan");
  assertEqual(review.detectedKind?.kind, "native");
}

{
  let detectCalls = 0;
  const review = await resolveInstallReview(
    {
      async resolveInstallPlan() {
        throw new Error("no manifest");
      },
      async detectInstallKind(): Promise<InstallDetectedKind> {
        detectCalls += 1;
        return { kind: "external", has_manifest_yaml: false };
      },
    },
    source,
  );
  assertEqual(detectCalls, 1);
  assertEqual(review.step, "external");
  assertEqual(review.externalPlanError, "no manifest");
}
