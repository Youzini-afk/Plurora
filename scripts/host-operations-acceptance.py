#!/usr/bin/env python3
"""Black-box acceptance for external Work, Installation, and Realization lifecycle."""

from __future__ import annotations

import json
import os
import secrets
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
PLURORA_BIN = Path(os.environ.get("PLURORA_BIN", ROOT / "target" / "debug" / "plurora"))
REAL_SOURCE = (
    "https://github.com/mdn/beginner-html-site-styled"
    "#6c7a360ddb4a0d75be06044bf8a914f260ff10c7"
)
FIXTURE_SOURCE = ROOT / "examples" / "host-operations" / "python-service"


class AcceptanceError(RuntimeError):
    pass


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):  # noqa: N802
        return None


HTTP = urllib.request.build_opener(NoRedirect)


def note(message: str) -> None:
    print(f"[host-operations] {message}", flush=True)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AcceptanceError(message)


def run_checked(
    command: list[str],
    *,
    timeout: int = 600,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    note("+ " + " ".join(command))
    result = subprocess.run(
        command,
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=timeout,
        check=False,
        env=env,
    )
    if result.returncode != 0:
        raise AcceptanceError(
            f"command failed with exit code {result.returncode}: {' '.join(command)}\n{result.stdout}"
        )
    return result


def reserve_loopback_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
        listener.bind(("127.0.0.1", 0))
        return int(listener.getsockname()[1])


def http_bytes(
    base_url: str,
    token: str,
    path: str,
    *,
    method: str = "GET",
    payload: Any | None = None,
    timeout: int = 60,
) -> bytes:
    body = None if payload is None else json.dumps(payload, separators=(",", ":")).encode()
    request = urllib.request.Request(
        base_url + path,
        data=body,
        method=method,
        headers={
            "accept": "application/json",
            "authorization": f"Bearer {token}",
            **({"content-type": "application/json"} if body is not None else {}),
        },
    )
    try:
        with HTTP.open(request, timeout=timeout) as response:
            return response.read()
    except urllib.error.HTTPError as error:
        response_body = error.read().decode(errors="replace")
        raise AcceptanceError(f"{method} {path} returned HTTP {error.code}: {response_body}") from error
    except OSError as error:
        raise AcceptanceError(f"{method} {path} failed: {error}") from error


def http_json(
    host: "Host",
    path: str,
    *,
    method: str = "GET",
    payload: Any | None = None,
    timeout: int = 60,
) -> dict[str, Any]:
    raw = http_bytes(host.base_url, host.token, path, method=method, payload=payload, timeout=timeout)
    try:
        value = json.loads(raw)
    except json.JSONDecodeError as error:
        raise AcceptanceError(f"{method} {path} returned invalid JSON: {raw[:500]!r}") from error
    require(isinstance(value, dict), f"{method} {path} did not return a JSON object")
    return value


@dataclass
class Host:
    process: subprocess.Popen[str]
    log_path: Path
    log_handle: Any
    base_url: str
    token: str

    def stop(self, *, crash: bool = False) -> None:
        if self.process.poll() is None:
            if crash:
                self.process.kill()
            else:
                self.process.terminate()
            try:
                self.process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=10)
        self.log_handle.close()


def read_log(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return ""


def start_host(
    data_dir: Path,
    profile: Path,
    token: str,
    output_dir: Path,
    *,
    retry_stale_lease: bool = False,
) -> Host:
    retry_deadline = time.monotonic() + (50 if retry_stale_lease else 0)
    retry_delay = 2
    while True:
        port = reserve_loopback_port()
        log_path = output_dir / f"host-{time.time_ns()}.log"
        log_handle = log_path.open("w", encoding="utf-8")
        process = subprocess.Popen(
            [
                str(PLURORA_BIN),
                "host",
                "serve",
                "--http",
                f"127.0.0.1:{port}",
                "--profile",
                str(profile),
                "--data-dir",
                str(data_dir),
                "--access-token",
                token,
            ],
            cwd=ROOT,
            env={**os.environ, "RUST_LOG": os.environ.get("RUST_LOG", "plurora_service=warn")},
            text=True,
            stdout=log_handle,
            stderr=subprocess.STDOUT,
        )
        base_url = f"http://127.0.0.1:{port}"
        started = False
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline:
            if process.poll() is not None:
                break
            try:
                if http_bytes(base_url, token, "/livez", timeout=1) == b"ok":
                    started = True
                    break
            except AcceptanceError:
                time.sleep(0.2)
        if started:
            note(f"Host ready at {base_url}")
            return Host(process, log_path, log_handle, base_url, token)

        if process.poll() is None:
            process.kill()
            process.wait(timeout=10)
        log_handle.close()
        log = read_log(log_path)
        if (
            retry_stale_lease
            and "another Host currently owns the development control-plane lease" in log
            and time.monotonic() < retry_deadline
        ):
            note(f"waiting for the crashed Host lease to expire ({retry_delay}s)")
            time.sleep(retry_delay)
            retry_delay = min(retry_delay * 2, 8)
            continue
        raise AcceptanceError(f"Host failed to start; log: {log_path}\n{log[-4000:]}")


def rpc(
    host: Host,
    method: str,
    params: dict[str, Any] | None = None,
    *,
    timeout: int = 60,
) -> Any:
    response = http_json(
        host,
        "/rpc",
        method="POST",
        payload={"id": "host-operations-acceptance", "method": method, "params": params or {}},
        timeout=timeout,
    )
    require(response.get("error") is None, f"{method} returned an RPC error: {response.get('error')}")
    result = response.get("result")
    require(result is not None, f"{method} response is missing its result")
    return result


def invoke_capability(host: Host, capability_id: str, input_value: dict[str, Any]) -> dict[str, Any]:
    invoked = rpc(
        host,
        "capability.invoke",
        {
            "capability_id": capability_id,
            "provider_package_id": "plurora/install-lab",
            "input": input_value,
        },
    )
    require(isinstance(invoked, dict), f"{capability_id} did not return an invocation object")
    output = invoked.get("output")
    require(isinstance(output, dict), f"{capability_id} did not return an object output")
    return output


def prepare_external_work(
    host: Host,
    source: str,
    data_dir: Path,
) -> tuple[str, dict[str, Any]]:
    intake = invoke_capability(
        host,
        "plurora/install-lab/prepare_external_intake",
        {"source": source, "data_dir": str(data_dir)},
    )
    workspace = intake.get("workspace")
    plan = intake.get("plan")
    require(isinstance(workspace, dict), "external intake is missing its Workspace record")
    require(isinstance(plan, dict), "external intake is missing its Work candidate plan")
    workspace_id = workspace.get("workspace_id")
    require(isinstance(workspace_id, str), "external intake is missing workspace_id")
    try:
        uuid.UUID(workspace_id)
    except ValueError as error:
        raise AcceptanceError("external intake returned a non-UUID workspace_id") from error
    require(workspace.get("ownership") == "managed", "external intake did not create a managed Workspace")

    executed = invoke_capability(
        host,
        "plurora/install-lab/execute_plan",
        {
            "plan": plan,
            "consent": {},
            "workspace_id": workspace_id,
            "data_dir": str(data_dir),
        },
    )
    require(
        executed.get("next_step") == "host.installation.create",
        "Install Lab did not hand authority to host.installation.create",
    )
    candidate = executed.get("installation_candidate")
    require(isinstance(candidate, dict), "Install Lab did not return an Installation candidate")
    for field in (
        "work_revision",
        "assembly_lock",
        "display_name",
        "source",
        "state_bindings",
        "secret_policy",
    ):
        require(field in candidate, f"Installation candidate is missing {field}")
    note(f"prepared managed Workspace {workspace_id} from {source}")
    return workspace_id, candidate


def work_id_from_candidate(candidate: dict[str, Any]) -> str:
    work_revision = candidate.get("work_revision")
    require(
        isinstance(work_revision, dict),
        "Installation candidate is missing its WorkRevision descriptor",
    )
    require(
        work_revision.get("artifact_type_uri") == "urn:plurora:work-revision:v1",
        "Installation candidate does not contain a WorkRevision descriptor",
    )
    annotations = work_revision.get("annotations")
    require(
        isinstance(annotations, dict),
        "WorkRevision descriptor is missing typed annotations",
    )
    work_id = annotations.get("work_id")
    require(
        isinstance(work_id, str) and work_id,
        "WorkRevision descriptor is missing its typed work_id",
    )
    return work_id


def create_installation(
    host: Host,
    candidate: dict[str, Any],
    idempotency_key: str,
) -> dict[str, Any]:
    work_id = work_id_from_candidate(candidate)
    created = rpc(
        host,
        "host.installation.create",
        {
            "work_id": work_id,
            "source": candidate["source"],
            "work_revision": candidate["work_revision"],
            "assembly_lock": candidate["assembly_lock"],
            "display_name": candidate["display_name"],
            "state_bindings": candidate["state_bindings"],
            "secret_policy": candidate["secret_policy"],
            "idempotency_key": idempotency_key,
        },
    )
    require(isinstance(created, dict), "host.installation.create did not return an object")
    view = created.get("installation")
    require(isinstance(view, dict), "host.installation.create is missing the Installation view")
    record = view.get("record")
    require(isinstance(record, dict), "host.installation.create is missing the Installation record")
    installation_id = record.get("installation_id")
    require(isinstance(installation_id, str), "Installation record is missing installation_id")
    try:
        uuid.UUID(installation_id)
    except ValueError as error:
        raise AcceptanceError("host.installation.create returned a non-UUID identity") from error
    require(record.get("status") == "ready", "new Installation is not ready")
    require(view.get("revision") == 1, "new Installation did not start at revision 1")
    note(f"created Installation {installation_id}")
    return view


def create_installation_from_work(
    host: Host,
    source: Path,
    data_dir: Path,
    idempotency_key: str,
) -> dict[str, Any]:
    created = run_checked(
        [
            str(PLURORA_BIN),
            "installation",
            "create",
            str(source),
            "--idempotency-key",
            idempotency_key,
            "--format",
            "json",
            "--data-dir",
            str(data_dir),
            "--endpoint",
            host.base_url,
        ],
        timeout=300,
        env={**os.environ, "PLURORA_HTTP_ACCESS_TOKEN": host.token},
    )
    try:
        result = json.loads(created.stdout)
    except json.JSONDecodeError as error:
        raise AcceptanceError(
            f"installation create returned invalid JSON: {created.stdout[:500]!r}"
        ) from error
    require(isinstance(result, dict), "installation create did not return an object")
    view = result.get("installation")
    require(isinstance(view, dict), "installation create is missing the Installation view")
    record = view.get("record")
    require(isinstance(record, dict), "installation create is missing the Installation record")
    installation_id = record.get("installation_id")
    require(isinstance(installation_id, str), "Installation record is missing installation_id")
    try:
        uuid.UUID(installation_id)
    except ValueError as error:
        raise AcceptanceError("installation create returned a non-UUID identity") from error
    require(record.get("status") == "ready", "new Work Installation is not ready")
    require(view.get("revision") == 1, "new Work Installation did not start at revision 1")
    note(f"created Work Installation {installation_id} through the public CLI")
    return view


def exercise_installation_mutations(
    host: Host,
    candidate: dict[str, Any],
) -> str:
    created = create_installation(host, candidate, "lifecycle-create")
    installation_id = created["record"]["installation_id"]
    updated = rpc(
        host,
        "host.installation.update",
        {
            "installation_id": installation_id,
            "expected_revision": created["revision"],
            "work_revision": candidate["work_revision"],
            "assembly_lock": candidate["assembly_lock"],
            "display_name": "Acceptance lifecycle updated",
            "state_action": {"kind": "preserve"},
            "idempotency_key": "lifecycle-update",
        },
    )
    require(isinstance(updated, dict), "host.installation.update did not return an object")
    updated_view = updated.get("installation")
    require(isinstance(updated_view, dict), "host.installation.update is missing its view")
    require(updated_view.get("revision") == 2, "Installation update did not advance CAS revision")
    require(updated_view.get("record", {}).get("display_name") == "Acceptance lifecycle updated", "Installation update did not change display_name")
    require(updated.get("diff", {}).get("display_name_changed") is True, "Installation update did not report its display-name diff")

    removed = rpc(
        host,
        "host.installation.remove",
        {
            "installation_id": installation_id,
            "expected_revision": updated_view["revision"],
            "state_disposition": "keep",
            "idempotency_key": "lifecycle-remove",
        },
    )
    require(isinstance(removed, dict), "host.installation.remove did not return an object")
    removed_view = removed.get("installation")
    require(isinstance(removed_view, dict), "host.installation.remove is missing its view")
    require(removed_view.get("revision") == 3, "Installation removal did not advance CAS revision")
    require(removed_view.get("record", {}).get("status") == "removed", "Installation was not retired")
    fetched = rpc(host, "host.installation.get", {"installation_id": installation_id})
    require(isinstance(fetched, dict) and fetched.get("record", {}).get("status") == "removed", "removed Installation did not remain auditable")
    return installation_id


def assert_public_inventory(host: Host, installation_ids: set[str]) -> None:
    installations = rpc(host, "host.installation.list")
    require(isinstance(installations, list), "host.installation.list did not return a list")
    listed = {
        view.get("record", {}).get("installation_id")
        for view in installations
        if isinstance(view, dict) and isinstance(view.get("record"), dict)
    }
    require(
        installation_ids <= listed,
        f"public Installation inventory is missing {installation_ids - listed}",
    )
    for installation_id in installation_ids:
        view = rpc(host, "host.installation.get", {"installation_id": installation_id})
        require(
            isinstance(view, dict)
            and view.get("record", {}).get("installation_id") == installation_id,
            f"host.installation.get did not return {installation_id}",
        )

    targets = rpc(host, "host.target.list")
    require(isinstance(targets, list), "host.target.list did not return targets")
    local = next(
        (target for target in targets if isinstance(target, dict) and target.get("id") == "local"),
        None,
    )
    require(local is not None and local.get("status") == "available", "local target is not available")


def assert_route(host: Host, route_id: str, marker: bytes) -> None:
    path = f"/p/{urllib.parse.quote(route_id, safe='')}"
    body = http_bytes(host.base_url, host.token, path, timeout=30)
    require(marker in body, f"route {route_id} response did not contain {marker!r}")


def write_managed_realization_work(root: Path) -> None:
    package_dir = root / "packages" / "server"
    package_dir.mkdir(parents=True, exist_ok=True)
    (root / "work.yaml").write_text(
        """schema: plurora.work-source.v1
work:
  id: acceptance/managed-realization
  title: Managed Realization Acceptance
  description: Exercises the public managed Realization lifecycle.
  assembly: assembly.yaml
  operational_intent: operation.yaml
""",
        encoding="utf-8",
    )
    (root / "assembly.yaml").write_text(
        """schema: plurora.assembly-source.v1
assembly:
  id: acceptance/managed-realization-main
  nodes:
    - id: server
      component: packages/server/manifest.yaml
""",
        encoding="utf-8",
    )
    (root / "operation.yaml").write_text(
        """schema: plurora.operational-intent.v1
workloads:
  - workload_id: web
    node_id: server
    execution_classes: [oci-container.v1]
    replicas: {min: 1, max: 1}
    restart_policy: on_failure
""",
        encoding="utf-8",
    )
    (package_dir / "manifest.yaml").write_text(
        """schema_version: 1
id: acceptance/server-component
version: 1.0.0
license: MIT
entry:
  kind: rust_inproc
  crate_ref: acceptance-server-component
  symbol: register
  abi_version: 1
provides: []
consumes: []
requires: []
contributes: {}
permissions: {}
sandbox_policy: {}
""",
        encoding="utf-8",
    )


def immutable_nginx_image() -> str:
    run_checked(["docker", "pull", "nginx:1.27-alpine"], timeout=300)
    inspected = run_checked(
        [
            "docker",
            "image",
            "inspect",
            "--format",
            "{{index .RepoDigests 0}}",
            "nginx:1.27-alpine",
        ],
        timeout=60,
    )
    image = inspected.stdout.strip()
    name, separator, digest = image.rpartition("@")
    require(separator == "@" and bool(name), "nginx did not resolve to an immutable image name")
    require(
        len(digest) == 71
        and digest.startswith("sha256:")
        and all(character in "0123456789abcdef" for character in digest[7:]),
        "nginx resolved to an invalid content digest",
    )
    return image


def realization_approval(planned: dict[str, Any]) -> dict[str, Any]:
    plan_ref = planned.get("plan_ref")
    plan = planned.get("plan")
    require(isinstance(plan_ref, dict), "Realization plan is missing plan_ref")
    require(isinstance(plan, dict), "Realization plan is missing its typed plan")
    risks = plan.get("risk_summary")
    require(isinstance(risks, list), "Realization plan is missing risk_summary")
    return {
        "plan_digest": plan_ref["digest"],
        "decision": "approved",
        "accepted_risks": risks,
        "decided_at": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
    }


def plan_realization(
    host: Host,
    installation: dict[str, Any],
    image: str,
    route_id: str,
    idempotency_key: str,
) -> dict[str, Any]:
    record = installation.get("record")
    require(isinstance(record, dict), "Installation view is missing its record")
    request = {
        "installation_id": record["installation_id"],
        "expected_installation_revision": installation["revision"],
        "target_id": "local",
        "backends": [
            {
                "kind": "oci_image",
                "workload_id": "web",
                "execution_class": "oci-container.v1",
                "image": image,
                "container_port": 80,
                "port_name": "http",
                "route_id": route_id,
                "route_access": "host_authenticated",
                "health_path": "/",
                "pull_if_missing": False,
            }
        ],
        "idempotency_key": idempotency_key,
    }
    planned = rpc(host, "host.realization.plan", request)
    require(isinstance(planned, dict), "host.realization.plan did not return an object")
    require(planned.get("gaps", []) == [], "Realization planning returned structured gaps")
    realization = planned.get("realization")
    require(isinstance(realization, dict), "Realization planning did not create a durable record")
    require(realization.get("status") == "planned", "new Realization is not Planned")
    require(isinstance(planned.get("plan_ref"), dict), "Realization planning omitted plan_ref")
    require(isinstance(planned.get("plan"), dict), "Realization planning omitted the typed plan")
    require(planned.get("replayed") is False, "first Realization plan was unexpectedly replayed")

    replay = rpc(host, "host.realization.plan", request)
    require(replay.get("replayed") is True, "Realization plan idempotency did not replay")
    require(
        replay.get("realization", {}).get("realization_id") == realization["realization_id"]
        and replay.get("plan_ref", {}).get("digest") == planned["plan_ref"]["digest"],
        "Realization plan replay changed its durable identity",
    )
    note(f"planned Realization {realization['realization_id']} for route {route_id}")
    return planned


def apply_realization(
    host: Host,
    installation_id: str,
    planned: dict[str, Any],
    idempotency_key: str,
) -> dict[str, Any]:
    planned_revision = planned["realization"]
    request = {
        "installation_id": installation_id,
        "target_id": "local",
        "realization_id": planned_revision["realization_id"],
        "expected_revision": planned_revision["revision"],
        "plan_ref": planned["plan_ref"],
        "approval": realization_approval(planned),
        "idempotency_key": idempotency_key,
    }
    applied = rpc(host, "host.realization.apply", request, timeout=300)
    require(isinstance(applied, dict), "host.realization.apply did not return an object")
    require(applied.get("gaps", []) == [], "Realization apply returned structured gaps")
    realization = applied.get("realization")
    require(isinstance(realization, dict), "Realization apply omitted its revision")
    require(realization.get("status") == "active", "Realization did not become Active")
    require(len(realization.get("actual_resources", [])) == 1, "Realization did not record its managed resource")
    require(applied.get("replayed") is False, "first Realization apply was unexpectedly replayed")

    replay = rpc(host, "host.realization.apply", request, timeout=300)
    require(replay.get("replayed") is True, "Realization apply idempotency did not replay")
    require(
        replay.get("realization", {}).get("realization_id") == realization["realization_id"],
        "Realization apply replay changed its identity",
    )
    note(f"activated Realization {realization['realization_id']}")
    return realization


def stop_realization(
    host: Host,
    installation_id: str,
    realization: dict[str, Any],
    idempotency_key: str,
) -> dict[str, Any]:
    request = {
        "installation_id": installation_id,
        "target_id": "local",
        "realization_id": realization["realization_id"],
        "expected_revision": realization["revision"],
        "idempotency_key": idempotency_key,
    }
    stopped = rpc(host, "host.realization.stop", request, timeout=300)
    terminal = stopped.get("realization")
    require(isinstance(terminal, dict), "Realization stop omitted its revision")
    require(terminal.get("status") == "stopped", "Realization did not become Stopped")
    require(stopped.get("replayed") is False, "first Realization stop was unexpectedly replayed")
    replay = rpc(host, "host.realization.stop", request, timeout=300)
    require(replay.get("replayed") is True, "Realization stop idempotency did not replay")
    require(replay.get("realization") == terminal, "Realization stop replay changed its terminal record")
    note(f"stopped Realization {terminal['realization_id']}")
    return terminal


def reconcile_realization(
    host: Host,
    installation_id: str,
    realization: dict[str, Any],
    idempotency_key: str,
) -> dict[str, Any]:
    request = {
        "installation_id": installation_id,
        "target_id": "local",
        "realization_id": realization["realization_id"],
        "expected_revision": realization["revision"],
        "idempotency_key": idempotency_key,
    }
    reconciled = rpc(host, "host.realization.reconcile", request, timeout=180)
    current = reconciled.get("realization")
    require(isinstance(current, dict), "Realization reconcile omitted its revision")
    require(current.get("status") == "active", "reconciled Realization is not Active")
    require(reconciled.get("replayed") is False, "first Realization reconcile was unexpectedly replayed")
    replay = rpc(host, "host.realization.reconcile", request, timeout=180)
    require(replay.get("replayed") is True, "Realization reconcile idempotency did not replay")
    require(replay.get("realization") == current, "Realization reconcile replay changed its record")
    return current


def rollback_realization(
    host: Host,
    installation_id: str,
    current: dict[str, Any],
    historic_plan: dict[str, Any],
    idempotency_key: str,
) -> dict[str, Any]:
    historic = historic_plan["realization"]
    request = {
        "installation_id": installation_id,
        "target_id": "local",
        "realization_id": current["realization_id"],
        "expected_revision": current["revision"],
        "rollback_to_realization_id": historic["realization_id"],
        "approval": realization_approval(historic_plan),
        "idempotency_key": idempotency_key,
    }
    rolled_back = rpc(host, "host.realization.rollback", request, timeout=300)
    replacement = rolled_back.get("realization")
    require(isinstance(replacement, dict), "Realization rollback omitted its child revision")
    require(replacement.get("status") == "active", "rollback child did not become Active")
    require(
        replacement.get("parent_realization_id") == current["realization_id"]
        and replacement.get("plan_ref") == historic["plan_ref"],
        "rollback child is not pinned to its current parent and historic plan",
    )
    require(
        replacement.get("realization_id") not in {current["realization_id"], historic["realization_id"]},
        "rollback did not create a fresh Host-owned Realization identity",
    )
    require(rolled_back.get("replayed") is False, "first Realization rollback was unexpectedly replayed")
    replay = rpc(host, "host.realization.rollback", request, timeout=300)
    require(replay.get("replayed") is True, "Realization rollback idempotency did not replay")
    require(replay.get("realization") == replacement, "Realization rollback replay changed its child")
    note(f"rolled back {current['realization_id']} as {replacement['realization_id']}")
    return replacement


def cleanup_docker(routes: set[str], installations: set[str]) -> None:
    for route_id in routes:
        listed = subprocess.run(
            ["docker", "ps", "--all", "--quiet", "--filter", f"label=plurora.route_id={route_id}"],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            check=False,
        )
        for container_id in listed.stdout.split():
            subprocess.run(
                ["docker", "rm", "--force", container_id],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                check=False,
            )
    for installation_id in installations:
        listed = subprocess.run(
            [
                "docker",
                "image",
                "ls",
                "--quiet",
                "--filter",
                f"label=plurora.installation_id={installation_id}",
            ],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            check=False,
        )
        image_ids = sorted(set(listed.stdout.split()))
        if image_ids:
            subprocess.run(
                ["docker", "image", "rm", "--force", *image_ids],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                check=False,
            )


def write_profile(path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    package_root = ROOT / "packages" / "plurora"
    manifests = [
        package_root / "git-tools-lab" / "manifest.yaml",
        package_root / "integrity-lab" / "manifest.yaml",
        package_root / "install-lab" / "manifest.yaml",
        package_root / "docker-runtime-lab" / "manifest.yaml",
    ]
    path.write_text(
        "title: Host operations acceptance\n"
        "event_store:\n"
        "  kind: sqlite\n"
        "  path: events.sqlite3\n"
        "autoload:\n"
        + "".join(f"  - {json.dumps(str(manifest))}\n" for manifest in manifests),
        encoding="utf-8",
    )


def main() -> None:
    require(
        os.environ.get("PLURORA_HOST_OPERATIONS_ACCEPTANCE") == "1",
        "set PLURORA_HOST_OPERATIONS_ACCEPTANCE=1; this Docker workload is intended for GitHub CI",
    )
    require(PLURORA_BIN.is_file(), f"Plurora CLI binary was not found at {PLURORA_BIN}")
    run_checked(["docker", "info"], timeout=60)

    output_dir = Path(
        os.environ.get("PLURORA_HOST_OPERATIONS_OUTPUT_DIR", ROOT / "target" / "host-operations-acceptance")
    )
    output_dir.mkdir(parents=True, exist_ok=True)
    token = secrets.token_hex(32)
    host: Host | None = None
    cleanup_routes = {"acceptance-realization-v1", "acceptance-realization-v2"}
    installation_ids: set[str] = set()
    temporary = tempfile.TemporaryDirectory(prefix="plurora-host-operations-")

    try:
        data_dir = Path(temporary.name) / "data"
        profile = data_dir / "profiles" / "default.yaml"
        write_profile(profile)
        managed_source = Path(temporary.name) / "managed-realization"
        write_managed_realization_work(managed_source)

        host = start_host(data_dir, profile, token, output_dir)
        real_workspace, real_candidate = prepare_external_work(host, REAL_SOURCE, data_dir)
        fixture_workspace, fixture_candidate = prepare_external_work(
            host,
            str(FIXTURE_SOURCE),
            data_dir,
        )
        real_installation_view = create_installation(host, real_candidate, "real-create")
        fixture_installation_view = create_installation(host, fixture_candidate, "fixture-create")
        managed_installation_view = create_installation_from_work(
            host,
            managed_source,
            Path(temporary.name) / "cli-data",
            "managed-realization-create",
        )
        real_installation = real_installation_view["record"]["installation_id"]
        fixture_installation = fixture_installation_view["record"]["installation_id"]
        managed_installation = managed_installation_view["record"]["installation_id"]
        installation_ids.update(
            {real_installation, fixture_installation, managed_installation}
        )
        retired_installation = exercise_installation_mutations(host, fixture_candidate)
        assert_public_inventory(host, installation_ids | {retired_installation})

        nginx_image = immutable_nginx_image()
        planned_v1 = plan_realization(
            host,
            managed_installation_view,
            nginx_image,
            "acceptance-realization-v1",
            "realization-v1-plan",
        )
        active_v1 = apply_realization(
            host,
            managed_installation,
            planned_v1,
            "realization-v1-apply",
        )
        assert_route(host, "acceptance-realization-v1", b"Welcome to nginx")
        stopped_v1 = stop_realization(
            host,
            managed_installation,
            active_v1,
            "realization-v1-stop",
        )

        planned_v2 = plan_realization(
            host,
            managed_installation_view,
            nginx_image,
            "acceptance-realization-v2",
            "realization-v2-plan",
        )
        active_v2 = apply_realization(
            host,
            managed_installation,
            planned_v2,
            "realization-v2-apply",
        )
        require(
            active_v2["realization_id"] != stopped_v1["realization_id"],
            "second managed activation reused the first Realization identity",
        )
        assert_route(host, "acceptance-realization-v2", b"Welcome to nginx")

        note("crashing Host to exercise durable Realization and runtime projection recovery")
        host.stop(crash=True)
        host = None
        host = start_host(data_dir, profile, token, output_dir, retry_stale_lease=True)
        assert_public_inventory(host, installation_ids | {retired_installation})
        retired_after_restart = rpc(
            host,
            "host.installation.get",
            {"installation_id": retired_installation},
        )
        require(
            retired_after_restart.get("record", {}).get("status") == "removed",
            "Host restart did not rehydrate the removed Installation terminal state",
        )
        restarted_v2 = rpc(
            host,
            "host.realization.get",
            {
                "installation_id": managed_installation,
                "realization_id": active_v2["realization_id"],
            },
        )
        require(
            isinstance(restarted_v2, dict) and restarted_v2.get("status") == "active",
            "Host restart did not rehydrate the active Realization",
        )
        assert_route(host, "acceptance-realization-v2", b"Welcome to nginx")
        reconciled_v2 = reconcile_realization(
            host,
            managed_installation,
            restarted_v2,
            "realization-v2-reconcile",
        )
        assert_route(host, "acceptance-realization-v2", b"Welcome to nginx")

        rollback_child = rollback_realization(
            host,
            managed_installation,
            reconciled_v2,
            planned_v1,
            "realization-rollback-v1",
        )
        assert_route(host, "acceptance-realization-v1", b"Welcome to nginx")
        former_current = rpc(
            host,
            "host.realization.get",
            {
                "installation_id": managed_installation,
                "realization_id": reconciled_v2["realization_id"],
            },
        )
        require(
            isinstance(former_current, dict) and former_current.get("status") == "stopped",
            "rollback did not stop its former active parent",
        )
        realizations = rpc(
            host,
            "host.realization.list",
            {"installation_id": managed_installation, "target_id": "local"},
        )
        require(isinstance(realizations, list), "host.realization.list did not return a list")
        realization_ids = {
            value.get("realization_id") for value in realizations if isinstance(value, dict)
        }
        require(
            {
                stopped_v1["realization_id"],
                reconciled_v2["realization_id"],
                rollback_child["realization_id"],
            }
            <= realization_ids,
            "public Realization inventory omitted durable history",
        )
        stopped_rollback = stop_realization(
            host,
            managed_installation,
            rollback_child,
            "realization-rollback-stop",
        )

        summary = {
            "real_source": REAL_SOURCE,
            "real_workspace_id": real_workspace,
            "fixture_workspace_id": fixture_workspace,
            "real_installation_id": real_installation,
            "fixture_installation_id": fixture_installation,
            "managed_installation_id": managed_installation,
            "retired_installation_id": retired_installation,
            "historic_realization_id": stopped_v1["realization_id"],
            "reconciled_realization_id": reconciled_v2["realization_id"],
            "rollback_realization_id": stopped_rollback["realization_id"],
        }
        (output_dir / "summary.json").write_text(
            json.dumps(summary, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        note("external Work, Installation, and Realization acceptance passed")
    finally:
        try:
            if host is not None:
                host.stop()
            cleanup_docker(cleanup_routes, installation_ids)
        finally:
            temporary.cleanup()


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"[host-operations] FAILED: {error}", file=sys.stderr, flush=True)
        raise
