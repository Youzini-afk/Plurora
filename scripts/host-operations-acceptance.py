#!/usr/bin/env python3
"""Black-box acceptance for external-project development and deployment."""

from __future__ import annotations

import json
import os
import re
import secrets
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
YGG = Path(os.environ.get("PLURORA_BIN", ROOT / "target" / "debug" / "plurora"))
REAL_SOURCE = (
    "https://github.com/mdn/beginner-html-site-styled"
    "#6c7a360ddb4a0d75be06044bf8a914f260ff10c7"
)
FIXTURE_SOURCE = ROOT / "examples" / "host-operations" / "python-service"
TERMINAL_CHANGE_FAILURES = {"failed", "recovery_required", "rejected"}
TERMINAL_DEPLOYMENT_FAILURES = {"failed", "recovery_required", "rejected"}


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


def run_checked(command: list[str], *, timeout: int = 600) -> subprocess.CompletedProcess[str]:
    note("+ " + " ".join(command))
    result = subprocess.run(
        command,
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=timeout,
        check=False,
    )
    if result.returncode != 0:
        raise AcceptanceError(
            f"command failed with exit code {result.returncode}: {' '.join(command)}\n{result.stdout}"
        )
    return result


def install_project(source: str, data_dir: Path) -> str:
    result = run_checked(
        [
            str(YGG),
            "install",
            source,
            "--profile",
            "default",
            "--data-dir",
            str(data_dir),
            "--workspace-only",
            "--yes",
            "--format",
            "json",
        ],
        timeout=300,
    )
    match = re.search(r"^Project registered: (\S+)\s*$", result.stdout, re.MULTILINE)
    require(match is not None, f"install output did not contain a registered project id\n{result.stdout}")
    project_id = match.group(1)
    note(f"registered {project_id} from {source}")
    return project_id


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
                str(YGG),
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


def project_path(project_id: str, suffix: str) -> str:
    return f"/host/v1/projects/{urllib.parse.quote(project_id, safe='')}{suffix}"


def change_path(project_id: str, change_id: str, suffix: str = "") -> str:
    return project_path(
        project_id,
        f"/changes/{urllib.parse.quote(change_id, safe='')}{suffix}",
    )


def rpc(host: Host, method: str, params: dict[str, Any] | None = None) -> Any:
    response = http_json(
        host,
        "/rpc",
        method="POST",
        payload={"id": "host-operations-acceptance", "method": method, "params": params or {}},
    )
    require(response.get("error") is None, f"{method} returned an RPC error: {response.get('error')}")
    result = response.get("result")
    require(result is not None, f"{method} response is missing its result")
    return result


def assert_public_inventory(host: Host, project_ids: set[str]) -> None:
    project_result = rpc(host, "host.project.list")
    require(isinstance(project_result, dict), "host.project.list did not return an object")
    projects = project_result.get("projects")
    require(isinstance(projects, list), "host.project.list did not return projects")
    listed = {project.get("id") for project in projects if isinstance(project, dict)}
    require(project_ids <= listed, f"public project inventory is missing {project_ids - listed}")

    targets = rpc(host, "host.target.list")
    require(isinstance(targets, list), "host.target.list did not return targets")
    local = next(
        (target for target in targets if isinstance(target, dict) and target.get("id") == "local"),
        None,
    )
    require(local is not None and local.get("status") == "available", "local target is not available")


def wait_for_change(host: Host, project_id: str, change_id: str, wanted: str, timeout: int = 600) -> dict[str, Any]:
    deadline = time.monotonic() + timeout
    previous = None
    while time.monotonic() < deadline:
        record = http_json(host, change_path(project_id, change_id))
        status = record.get("status")
        if status != previous:
            note(f"change {change_id}: {status}")
            previous = status
        if status == wanted:
            return record
        if status in TERMINAL_CHANGE_FAILURES:
            raise AcceptanceError(f"change {change_id} reached {status}: {record.get('error')}")
        time.sleep(1)
    raise AcceptanceError(f"change {change_id} did not reach {wanted} within {timeout}s")


def wait_for_deployment(
    host: Host,
    project_id: str,
    change_id: str,
    wanted: str,
    timeout: int = 600,
) -> dict[str, Any]:
    deadline = time.monotonic() + timeout
    previous = None
    while time.monotonic() < deadline:
        record = http_json(host, change_path(project_id, change_id))
        deployment = record.get("deployment")
        status = deployment.get("status") if isinstance(deployment, dict) else None
        if status != previous:
            note(f"deployment for {change_id}: {status}")
            previous = status
        if status == wanted:
            return record
        if status in TERMINAL_DEPLOYMENT_FAILURES:
            operation_diagnostics = []
            for operation_kind in ("build_operation_id", "deployment_operation_id"):
                operation_id = deployment.get(operation_kind)
                if not isinstance(operation_id, str):
                    continue
                operation = http_json(
                    host,
                    f"/host/v1/targets/{urllib.parse.quote(deployment['target_id'], safe='')}"
                    f"/operations/{urllib.parse.quote(operation_id, safe='')}",
                )
                receipt = operation.get("receipt")
                operation_diagnostics.append(
                    {
                        "role": operation_kind,
                        "status": operation.get("status"),
                        "kind": operation.get("spec", {}).get("kind"),
                        "receipt_status": receipt.get("status") if isinstance(receipt, dict) else None,
                        "diagnostics": receipt.get("diagnostics") if isinstance(receipt, dict) else [],
                    }
                )
            raise AcceptanceError(
                f"deployment for {change_id} reached {status}: {deployment.get('error')}; "
                f"target operations={json.dumps(operation_diagnostics, sort_keys=True)}"
            )
        time.sleep(1)
    raise AcceptanceError(f"deployment for {change_id} did not reach {wanted} within {timeout}s")


def wait_for_project_readiness(host: Host, project_id: str, ready: bool, timeout: int = 40) -> dict[str, Any]:
    deadline = time.monotonic() + timeout
    path = project_path(project_id, "/deployments")
    while time.monotonic() < deadline:
        status = http_json(host, path)
        if status.get("runtime_ready") is ready:
            return status
        time.sleep(1)
    raise AcceptanceError(f"project {project_id} runtime_ready did not become {ready}")


def assert_route(host: Host, route_id: str, marker: bytes) -> None:
    path = f"/p/{urllib.parse.quote(route_id, safe='')}"
    body = http_bytes(host.base_url, host.token, path, timeout=30)
    require(marker in body, f"route {route_id} response did not contain {marker!r}")


def deploy_approved_change(
    host: Host,
    project_id: str,
    *,
    goal: str,
    dockerfile: str,
    container_port: int,
    route_id: str,
    health_path: str,
    marker: bytes,
    idempotency: str,
    cleanup_routes: set[str],
    cleanup_containers: set[str],
) -> tuple[dict[str, Any], dict[str, Any]]:
    drafted = http_json(
        host,
        project_path(project_id, "/changes"),
        method="POST",
        payload={
            "goal": goal,
            "operations": [{"op": "file_write", "path": "Dockerfile", "content": dockerfile}],
            "verification": {
                "kind": "docker_build",
                "dockerfile": "Dockerfile",
                "network_mode": "none",
                "timeout_secs": 300,
            },
            "idempotency_key": f"{idempotency}-change",
        },
    )
    require(drafted.get("status") == "drafted", "new ChangeSet was not drafted")
    change_id = drafted.get("change_set", {}).get("id")
    require(isinstance(change_id, str), "drafted ChangeSet is missing its id")

    approved = http_json(
        host,
        change_path(project_id, change_id, "/approve"),
        method="POST",
        payload={"approved": True, "reason": "GitHub CI external-project acceptance"},
    )
    require(approved.get("status") == "approved", "ChangeSet approval was not recorded")
    execute = http_json(host, change_path(project_id, change_id, "/execute"), method="POST", payload={})
    require(execute.get("accepted") is True, "approved ChangeSet execution was not accepted")
    committed = wait_for_change(host, project_id, change_id, "committed")
    verification = committed.get("verification_result")
    require(isinstance(verification, dict) and verification.get("succeeded") is True, "Docker verification did not succeed")
    require(verification.get("network_mode") == "none", "Docker verification did not fail closed to network none")
    require(isinstance(verification.get("deployment_artifact_ref"), dict), "verified deployment artifact is missing")

    preview_started = http_json(
        host,
        change_path(project_id, change_id, "/deployment/preview"),
        method="POST",
        payload={
            "target_id": "local",
            "container_port": container_port,
            "port_name": "http",
            "route_id": route_id,
            "route_access": "host_authenticated",
            "health_path": health_path,
            "idempotency_key": f"{idempotency}-preview",
        },
    )
    preview_deployment = preview_started.get("deployment")
    require(isinstance(preview_deployment, dict), "deployment preview did not create a durable record")
    cleanup_routes.add(preview_deployment["preview_route_id"])
    preview_ready = wait_for_deployment(host, project_id, change_id, "preview_ready")
    preview = preview_ready["deployment"]["preview"]
    cleanup_containers.add(preview["container_id"])
    assert_route(host, preview["route_id"], marker)

    deployment_approved = http_json(
        host,
        change_path(project_id, change_id, "/deployment/approve"),
        method="POST",
        payload={"approved": True, "reason": "verified preview accepted by CI"},
    )
    require(deployment_approved["deployment"]["status"] == "approved", "deployment approval was not recorded")
    activated = http_json(
        host,
        change_path(project_id, change_id, "/deployment/activate"),
        method="POST",
        payload={},
        timeout=180,
    )
    require(activated["deployment"]["status"] == "active", "approved deployment was not activated")
    assert_route(host, route_id, marker)

    deployments = http_json(host, project_path(project_id, "/deployments"))
    active = deployments.get("active_revision")
    require(isinstance(active, dict), "project has no active deployment revision")
    require(active.get("operation") == "verified_activate", "activation did not create a verified revision")
    require(active.get("verified_change_set_id") == change_id, "revision is not bound to its verified ChangeSet")
    cleanup_containers.add(active["receipt"]["container_id"])
    return activated, active


def docker_container_id(container_ref: str) -> str:
    prefix = "docker:"
    require(container_ref.startswith(prefix), "container receipt is not a typed Docker reference")
    container_id = container_ref[len(prefix) :]
    require(
        len(container_id) == 64
        and all(character in "0123456789abcdef" for character in container_id),
        "container receipt has an invalid Docker identity",
    )
    return container_id


def remove_container(container_ref: str) -> None:
    run_checked(["docker", "rm", "--force", docker_container_id(container_ref)], timeout=60)


def cleanup_docker(routes: set[str], containers: set[str], projects: set[str]) -> None:
    for container_ref in containers:
        subprocess.run(
            ["docker", "rm", "--force", docker_container_id(container_ref)],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
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
    for project_id in projects:
        listed = subprocess.run(
            ["docker", "image", "ls", "--quiet", "--filter", f"label=plurora.project_id={project_id}"],
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
    docker_manifest = ROOT / "packages" / "plurora" / "docker-runtime-lab" / "manifest.yaml"
    path.write_text(
        "title: Host operations acceptance\n"
        "event_store:\n"
        "  kind: sqlite\n"
        "  path: events.sqlite3\n"
        "autoload:\n"
        f"  - {json.dumps(str(docker_manifest))}\n",
        encoding="utf-8",
    )


def main() -> None:
    require(
        os.environ.get("PLURORA_HOST_OPERATIONS_ACCEPTANCE") == "1",
        "set PLURORA_HOST_OPERATIONS_ACCEPTANCE=1; this Docker workload is intended for GitHub CI",
    )
    require(YGG.is_file(), f"Plurora CLI binary was not found at {YGG}")
    run_checked(["docker", "info"], timeout=60)

    output_dir = Path(
        os.environ.get("PLURORA_HOST_OPERATIONS_OUTPUT_DIR", ROOT / "target" / "host-operations-acceptance")
    )
    output_dir.mkdir(parents=True, exist_ok=True)
    token = secrets.token_hex(32)
    host: Host | None = None
    cleanup_routes = {"acceptance-mdn", "acceptance-python"}
    cleanup_containers: set[str] = set()
    project_ids: set[str] = set()
    temporary = tempfile.TemporaryDirectory(prefix="plurora-host-operations-")

    try:
        data_dir = Path(temporary.name) / "data"
        profile = data_dir / "profiles" / "default.yaml"
        write_profile(profile)

        real_project = install_project(REAL_SOURCE, data_dir)
        fixture_project = install_project(str(FIXTURE_SOURCE), data_dir)
        project_ids.update({real_project, fixture_project})

        host = start_host(data_dir, profile, token, output_dir)
        assert_public_inventory(host, project_ids)

        nginx_v1 = """FROM nginx:1.27-alpine
COPY . /usr/share/nginx/html
"""
        _, real_v1 = deploy_approved_change(
            host,
            real_project,
            goal="Add a reviewed, network-isolated deployment description",
            dockerfile=nginx_v1,
            container_port=80,
            route_id="acceptance-mdn",
            health_path="/",
            marker=b"Mozilla is cool",
            idempotency="real-v1",
            cleanup_routes=cleanup_routes,
            cleanup_containers=cleanup_containers,
        )

        nginx_v2 = """FROM nginx:1.27-alpine
LABEL org.plurora.acceptance.revision="2"
COPY . /usr/share/nginx/html
"""
        _, real_v2 = deploy_approved_change(
            host,
            real_project,
            goal="Produce a second independently verified deployment revision",
            dockerfile=nginx_v2,
            container_port=80,
            route_id="acceptance-mdn",
            health_path="/",
            marker=b"Mozilla is cool",
            idempotency="real-v2",
            cleanup_routes=cleanup_routes,
            cleanup_containers=cleanup_containers,
        )
        require(real_v2["revision_id"] != real_v1["revision_id"], "second activation reused a revision id")
        require(real_v2["parent_revision_id"] == real_v1["revision_id"], "second revision does not descend from the first")
        require(real_v2["source_commit"] != real_v1["source_commit"], "second source tree was not independently committed")

        python_dockerfile = """FROM python:3.13-alpine
WORKDIR /srv
COPY service/ /srv/
EXPOSE 8000
CMD ["python", "/srv/server.py"]
"""
        _, fixture_revision = deploy_approved_change(
            host,
            fixture_project,
            goal="Deploy the structurally different standard-library HTTP service",
            dockerfile=python_dockerfile,
            container_port=8000,
            route_id="acceptance-python",
            health_path="/healthz",
            marker=b"plurora-python-fixture",
            idempotency="python-v1",
            cleanup_routes=cleanup_routes,
            cleanup_containers=cleanup_containers,
        )

        failed_container = real_v2["receipt"]["container_id"]
        remove_container(failed_container)
        degraded = wait_for_project_readiness(host, real_project, False)
        require(degraded.get("recovery_required") is True, "target failure did not require recovery")

        recovered = http_json(
            host,
            project_path(real_project, "/deployments/recover"),
            method="POST",
            payload={},
            timeout=180,
        )
        require(recovered.get("operation") == "recover", "explicit recovery did not create a recover revision")
        recovered_revision = recovered["revision"]
        require(recovered_revision["source_commit"] == real_v2["source_commit"], "recovery rebuilt source instead of replaying the revision")
        cleanup_containers.add(recovered_revision["receipt"]["container_id"])
        wait_for_project_readiness(host, real_project, True)
        assert_route(host, "acceptance-mdn", b"Mozilla is cool")

        note("crashing Host to exercise SQLite and runtime projection recovery")
        host.stop(crash=True)
        host = None
        host = start_host(data_dir, profile, token, output_dir, retry_stale_lease=True)
        assert_public_inventory(host, project_ids)
        restarted_real = wait_for_project_readiness(host, real_project, True)
        require(
            restarted_real.get("active_revision_id") == recovered_revision["revision_id"],
            "Host restart did not restore the durable active revision",
        )
        restarted_fixture = wait_for_project_readiness(host, fixture_project, True)
        require(
            restarted_fixture.get("active_revision_id") == fixture_revision["revision_id"],
            "Host restart did not restore the second fixture revision",
        )
        assert_route(host, "acceptance-mdn", b"Mozilla is cool")
        assert_route(host, "acceptance-python", b"plurora-python-fixture")

        rolled_back = http_json(
            host,
            project_path(real_project, "/deployments/rollback"),
            method="POST",
            payload={"revision_id": real_v1["revision_id"]},
            timeout=180,
        )
        require(rolled_back.get("operation") == "rollback", "rollback did not create a rollback revision")
        rollback_revision = rolled_back["revision"]
        require(rollback_revision["source_commit"] == real_v1["source_commit"], "rollback did not replay the selected historical source")
        require(
            rollback_revision["parent_revision_id"] == recovered_revision["revision_id"],
            "rollback revision does not descend from the recovered active revision",
        )
        cleanup_containers.add(rollback_revision["receipt"]["container_id"])
        wait_for_project_readiness(host, real_project, True)
        assert_route(host, "acceptance-mdn", b"Mozilla is cool")

        summary = {
            "real_source": REAL_SOURCE,
            "real_project_id": real_project,
            "fixture_project_id": fixture_project,
            "verified_revisions": [real_v1["revision_id"], real_v2["revision_id"]],
            "recovered_revision": recovered_revision["revision_id"],
            "rollback_revision": rollback_revision["revision_id"],
            "fixture_revision": fixture_revision["revision_id"],
        }
        (output_dir / "summary.json").write_text(
            json.dumps(summary, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        note("external-project Host operations acceptance passed")
    finally:
        try:
            if host is not None:
                host.stop()
            cleanup_docker(cleanup_routes, cleanup_containers, project_ids)
        finally:
            temporary.cleanup()


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"[host-operations] FAILED: {error}", file=sys.stderr, flush=True)
        raise
