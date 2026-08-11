import { useMemo, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  ProtocolRpcError,
  type InstallationView,
  type PluroraProtocolClient,
} from "@/protocol/client";
import {
  foreignLaunchEntrypoints,
  foreignLaunchSecretName,
  foreignLaunchSecretRef,
  parseForeignLaunchBinding,
} from "@/client-core/foreign-work";

export function ForeignWorkWorkbench({
  client,
  installation,
  canManage,
  onChanged,
}: {
  client: PluroraProtocolClient;
  installation: InstallationView;
  canManage: boolean;
  onChanged: () => Promise<void> | void;
}) {
  const entrypoints = useMemo(
    () => foreignLaunchEntrypoints(installation.work_summary),
    [installation.work_summary],
  );
  const [launchId, setLaunchId] = useState(entrypoints[0]?.id ?? "");
  const [bindingJson, setBindingJson] = useState("");
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<{ code: string; detail: string } | null>(null);
  const keys = useRef(new Map<string, string>());
  const selected = entrypoints.find((entrypoint) => entrypoint.id === launchId) ?? entrypoints[0];
  const expectedKind = selected?.annotations?.["plurora.foreign/launch_kind"];

  const save = async () => {
    if (!selected || !canManage) return;
    setBusy(true);
    setResult(null);
    try {
      const binding = parseForeignLaunchBinding(bindingJson, selected.id);
      if (typeof expectedKind === "string" && binding.target.kind !== expectedKind) {
        throw new Error("binding_incompatible");
      }
      const reference = await foreignLaunchSecretRef(selected.id);
      const secretName = await foreignLaunchSecretName(selected.id);
      const existingSecretPolicy = installation.record.secret_policy ?? {};
      const secretPolicy = {
        ...existingSecretPolicy,
        allowed_secret_refs: [...new Set([
          ...(existingSecretPolicy.allowed_secret_refs ?? []),
          reference,
        ])].sort(),
      };
      const keyName = `${installation.record.installation_id}:${installation.revision}:${selected.id}`;
      const idempotencyKey = keys.current.get(keyName) ?? crypto.randomUUID();
      keys.current.set(keyName, idempotencyKey);
      await client.updateInstallation({
        installation_id: installation.record.installation_id,
        expected_revision: installation.revision,
        work_revision: installation.record.work_revision,
        assembly_lock: installation.record.assembly_lock,
        secret_policy: secretPolicy,
        state_action: { kind: "preserve" },
        idempotency_key: idempotencyKey,
      });
      await client.putSecret(
        secretName,
        JSON.stringify(binding),
        installation.record.installation_id,
      );
      keys.current.delete(keyName);
      setBindingJson("");
      setResult({ code: "binding_stored", detail: "The Host stored this coordinate in the Installation secret scope. Its value is not readable from the UI." });
      await onChanged();
    } catch (error) {
      setResult(safeFailure(error));
    } finally {
      setBusy(false);
    }
  };

  if (!entrypoints.length) return null;
  return (
    <section className="rounded-[16px] border border-whisper-border bg-pure-surface p-5">
      <h2 className="font-display text-lg font-bold">Foreign launch binding</h2>
      <p className="mt-1 text-xs text-steel-secondary">
        Concrete paths, URIs, image names, and entitlement input stay in this Installation&apos;s encrypted secret store; they never enter Work artifacts or event payloads.
      </p>
      <div className="mt-4 grid gap-3 sm:grid-cols-[minmax(0,220px)_1fr]">
        <label className="text-xs text-steel-secondary">
          Entrypoint
          <select
            className="mt-1 h-10 w-full rounded-[10px] border border-whisper-border bg-pure-surface px-3 text-sm text-charcoal-ink"
            value={selected?.id ?? ""}
            onChange={(event) => setLaunchId(event.target.value)}
          >
            {entrypoints.map((entrypoint) => <option key={entrypoint.id} value={entrypoint.id}>{entrypoint.id}</option>)}
          </select>
        </label>
        <div className="rounded-[10px] border border-whisper-border bg-warm-bone/40 px-3 py-2 text-xs text-steel-secondary">
          Required kind: <span className="font-mono text-charcoal-ink">{typeof expectedKind === "string" ? expectedKind : "declared by capsule"}</span>
        </div>
      </div>
      <label className="mt-3 block text-xs text-steel-secondary">
        Binding JSON
        <textarea
          className="mt-1 min-h-44 w-full rounded-[10px] border border-whisper-border bg-pure-surface p-3 font-mono text-xs text-charcoal-ink outline-none focus-visible:border-aged-brass"
          value={bindingJson}
          onChange={(event) => setBindingJson(event.target.value)}
          spellCheck={false}
          placeholder={JSON.stringify({
            schema: "plurora.foreign-launch-binding.v1",
            launch_id: selected?.id ?? "play",
            target: { kind: typeof expectedKind === "string" ? expectedKind : "local_executable", executable: "<local coordinate>" },
          }, null, 2)}
        />
      </label>
      <div className="mt-3 flex items-center gap-3">
        <Button tone="primary" size="sm" disabled={!canManage || busy || !bindingJson.trim()} onClick={() => void save()}>
          {busy ? "Storing…" : "Store local binding"}
        </Button>
        {!canManage ? <p className="text-xs text-deep-rust">installation.manage is required for this exact Installation.</p> : null}
      </div>
      {result ? <p className={`mt-3 text-xs ${result.code === "binding_stored" ? "text-steel-secondary" : "text-deep-rust"}`}><span className="font-mono">{result.code}</span> · {result.detail}</p> : null}
    </section>
  );
}

function safeFailure(error: unknown): { code: string; detail: string } {
  if (error instanceof ProtocolRpcError) {
    return {
      code: error.reasonCode,
      detail: error.nextStep ?? "Inspect the structured Host diagnostic before retrying.",
    };
  }
  if (error instanceof Error && ["binding_invalid", "binding_incompatible"].includes(error.message)) {
    return {
      code: error.message,
      detail: "Check the schema, launch ID, required kind, and non-empty target fields. Local values are not included in this diagnostic.",
    };
  }
  return { code: "outcome_unknown", detail: "Refresh the Installation before retrying with the same binding." };
}
