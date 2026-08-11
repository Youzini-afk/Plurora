import { useRef, useState } from "react";
import { affordanceForAction, type LibraryAuthorityInfo } from "@/client-core/library-affordance";
import { Button } from "@/components/ui/button";
import {
  ProtocolRpcError,
  type ArtifactDescriptor,
  type InstallationView,
  type PluroraProtocolClient,
} from "@/protocol/client";

export function StateBackupPanel({
  client,
  installation,
  authority,
  onChanged,
}: {
  client: PluroraProtocolClient;
  installation: InstallationView;
  authority: LibraryAuthorityInfo;
  onChanged: () => Promise<void> | void;
}) {
  const [snapshot, setSnapshot] = useState<ArtifactDescriptor | null>(null);
  const [busy, setBusy] = useState<"backup" | "download" | null>(null);
  const [failure, setFailure] = useState<{ code: string; detail: string } | null>(null);
  const keys = useRef(new Map<number, string>());
  const affordance = affordanceForAction({
    installation,
    work_summary: installation.work_summary,
    authority,
  }, "backup");
  const canExport = installation.work_summary.rights_declaration?.export_state === "allowed";

  const backup = async () => {
    if (!affordance.available) return;
    setBusy("backup");
    setFailure(null);
    const key = keys.current.get(installation.revision) ?? crypto.randomUUID();
    keys.current.set(installation.revision, key);
    try {
      const result = await client.updateInstallation({
        installation_id: installation.record.installation_id,
        expected_revision: installation.revision,
        work_revision: installation.record.work_revision,
        assembly_lock: installation.record.assembly_lock,
        state_action: { kind: "backup" },
        idempotency_key: key,
      });
      const nextSnapshot = (result.receipts ?? []).find((receipt) => receipt.artifact_type_uri === "urn:plurora:installation-state-snapshot:v1");
      if (!nextSnapshot) throw new Error("snapshot_missing");
      keys.current.delete(installation.revision);
      setSnapshot(nextSnapshot);
      await onChanged();
    } catch (error) {
      setFailure(safeFailure(error));
    } finally {
      setBusy(null);
    }
  };

  const download = async () => {
    if (!snapshot) return;
    setBusy("download");
    setFailure(null);
    try {
      const artifact = await client.getInstallationStateArtifact(
        installation.record.installation_id,
        snapshot,
      );
      const bytes = artifact.content_encoding === "hex"
        ? decodeHex(artifact.content)
        : new TextEncoder().encode(artifact.content);
      const buffer = new ArrayBuffer(bytes.byteLength);
      new Uint8Array(buffer).set(bytes);
      const url = URL.createObjectURL(new Blob([buffer], { type: snapshot.media_type }));
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = `plurora-state-${snapshot.digest.replace("sha256:", "")}.json`;
      anchor.click();
      URL.revokeObjectURL(url);
    } catch (error) {
      setFailure(safeFailure(error));
    } finally {
      setBusy(null);
    }
  };

  return (
    <section className="rounded-[16px] border border-whisper-border bg-pure-surface p-5">
      <h2 className="font-display text-lg font-bold">Opaque state backup</h2>
      <p className="mt-1 text-xs text-steel-secondary">Backup snapshots the current state tree without replacing, migrating, or clearing it. Restore remains an explicit Installation update.</p>
      <div className="mt-4 flex flex-wrap items-center gap-3">
        <Button tone="secondary" size="sm" disabled={!affordance.available || busy !== null} onClick={() => void backup()}>
          {busy === "backup" ? "Backing up…" : "Back up state"}
        </Button>
        {snapshot ? <Button tone="tertiary" size="sm" disabled={busy !== null || !canExport} onClick={() => void download()}>{busy === "download" ? "Preparing…" : "Download latest snapshot"}</Button> : null}
      </div>
      {!affordance.available ? <p className="mt-3 text-xs text-deep-rust"><span className="font-mono">{affordance.reason_code}</span> · {affordance.next_step}</p> : null}
      {snapshot ? <p className="mt-3 break-all font-mono text-[11px] text-steel-secondary">snapshot={snapshot.digest}</p> : null}
      {snapshot && !canExport ? <p className="mt-3 text-xs text-deep-rust"><span className="font-mono">rights_unspecified</span> · Exporting the backup requires an explicit Allowed export-state Right.</p> : null}
      {failure ? <p className="mt-3 text-xs text-deep-rust"><span className="font-mono">{failure.code}</span> · {failure.detail}</p> : null}
    </section>
  );
}

export function decodeHex(value: string): Uint8Array {
  if (value.length % 2 !== 0 || !/^[0-9a-f]*$/i.test(value)) throw new Error("artifact_invalid");
  return Uint8Array.from(value.match(/.{2}/g) ?? [], (pair) => Number.parseInt(pair, 16));
}

function safeFailure(error: unknown): { code: string; detail: string } {
  if (error instanceof ProtocolRpcError) {
    return { code: error.reasonCode, detail: error.nextStep ?? "Inspect the structured Host diagnostic before retrying." };
  }
  if (error instanceof Error && ["snapshot_missing", "artifact_invalid"].includes(error.message)) {
    return { code: error.message, detail: "The Host did not return one valid, journal-issued state snapshot." };
  }
  return { code: "outcome_unknown", detail: "Refresh the Installation and inspect its current revision before retrying." };
}
