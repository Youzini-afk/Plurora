import { useEffect, useState } from "react";
import { useToast } from "@/components/ui/toast";
import { usePlurora } from "@/lib/plurora-client";
import { useT } from "@/lib/locale";
import type { InstallConsent, InstallDetectedKind, InstallExecuteResult, InstallPlan, InstallSource } from "@/protocol/client";
import { detectKindFromInstallPlan, errorMessage } from "./install-format";
import type { InstallPhase, InstallStep } from "./install-types";

type InstallResolveClient = {
  resolveInstallPlan(source: InstallSource): Promise<InstallPlan>;
  detectInstallKind(source: Pick<InstallSource, "root_url" | "root_ref">): Promise<InstallDetectedKind>;
};

export async function resolveInstallReview(client: InstallResolveClient, source: InstallSource): Promise<{
  plan: InstallPlan | null;
  detectedKind: InstallDetectedKind | null;
  externalPlanError: string | null;
  step: Extract<InstallStep, "plan" | "external">;
  progressPhases: InstallPhase[];
}> {
  try {
    const plan = await client.resolveInstallPlan(source);
    return {
      plan,
      detectedKind: detectKindFromInstallPlan(plan),
      externalPlanError: null,
      step: "plan",
      progressPhases: ["resolving", "reviewed"],
    };
  } catch (planErr) {
    const planError = errorMessage(planErr);
    const detectedKind = await client.detectInstallKind(source);
    if (detectedKind.kind === "external") {
      return {
        plan: null,
        detectedKind,
        externalPlanError: planError,
        step: "external",
        progressPhases: ["resolving", "detecting", "reviewed"],
      };
    }
    throw new Error(planError);
  }
}

export function useInstallFlow({
  open,
  onClose,
  onInstalled,
}: {
  open: boolean;
  onClose: () => void;
  onInstalled?: () => void;
}) {
  const client = usePlurora();
  const toast = useToast();
  const t = useT();
  const [step, setStep] = useState<InstallStep>("url");
  const [url, setUrl] = useState("");
  const [approvedPermissions, setApprovedPermissions] = useState(false);
  const [plan, setPlan] = useState<InstallPlan | null>(null);
  const [detectedKind, setDetectedKind] = useState<InstallDetectedKind | null>(null);
  const [resolveError, setResolveError] = useState<string | null>(null);
  const [externalPlanError, setExternalPlanError] = useState<string | null>(null);
  const [isResolving, setIsResolving] = useState(false);
  const [isExecuting, setIsExecuting] = useState(false);
  const [installResult, setInstallResult] = useState<InstallExecuteResult | null>(null);
  const [progressPhases, setProgressPhases] = useState<InstallPhase[]>([]);
  const [progressError, setProgressError] = useState<string | null>(null);

  const reset = () => {
    setStep("url");
    setUrl("");
    setApprovedPermissions(false);
    setPlan(null);
    setDetectedKind(null);
    setResolveError(null);
    setExternalPlanError(null);
    setIsResolving(false);
    setIsExecuting(false);
    setInstallResult(null);
    setProgressPhases([]);
    setProgressError(null);
  };

  useEffect(() => {
    if (!open) reset();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  const handleClose = () => {
    onClose();
    setTimeout(reset, 250);
  };

  const onContinueFromUrl = async () => {
    if (!url.trim() || isResolving) return;
    const source = { root_url: url.trim(), root_ref: "HEAD" };
    setResolveError(null);
    setExternalPlanError(null);
    setIsResolving(true);
    setProgressPhases(["resolving"]);
    try {
      const review = await resolveInstallReview(client, source);
      setPlan(review.plan);
      setDetectedKind(review.detectedKind);
      setExternalPlanError(review.externalPlanError);
      setApprovedPermissions(false);
      setProgressPhases(review.progressPhases);
      setStep(review.step);
    } catch (err) {
      const message = errorMessage(err);
      setResolveError(message);
      setProgressPhases(["resolving", "failed"]);
      toast.push({ variant: "error", title: t("installPlanFailedTitle"), body: message });
    } finally {
      setIsResolving(false);
    }
  };

  const onConfirmPlan = async () => {
    if (!plan || !approvedPermissions || isExecuting || detectedKind?.kind === "external") return;
    const consent: InstallConsent = {
      approved_capabilities: plan.permissions_summary.new_capabilities,
      approved_network_hosts: plan.permissions_summary.new_network_hosts,
      approved_secret_refs: plan.permissions_summary.new_secret_refs,
    };
    setStep("progress");
    setIsExecuting(true);
    setProgressError(null);
    setProgressPhases(["resolving", "detecting", "reviewed", "executing"]);
    try {
      const result = await client.executeInstallPlan(plan, consent, "default");
      setInstallResult(result);
      setProgressPhases(["resolving", "detecting", "reviewed", "executing", "completed"]);
      toast.push({
        variant: "success",
        title: t("installCompleteTitle"),
        body: t("installCompleteBody", result.installed.length, result.project?.project_id),
      });
      onInstalled?.();
      window.setTimeout(handleClose, 700);
    } catch (err) {
      const message = errorMessage(err);
      setProgressError(message);
      setProgressPhases(["resolving", "detecting", "reviewed", "executing", "failed"]);
      toast.push({ variant: "error", title: t("installFailedTitle"), body: message });
    } finally {
      setIsExecuting(false);
    }
  };

  const onContinueExternal = () => {
    if (detectedKind?.kind === "external") {
      setStep("external");
    } else {
      setStep("plan");
    }
  };

  return {
    step,
    setStep,
    url,
    setUrl,
    approvedPermissions,
    setApprovedPermissions,
    plan,
    detectedKind,
    resolveError,
    externalPlanError,
    isResolving,
    isExecuting,
    installResult,
    progressPhases,
    progressError,
    reset,
    handleClose,
    onContinueFromUrl,
    onConfirmPlan,
    onContinueExternal,
  };
}
