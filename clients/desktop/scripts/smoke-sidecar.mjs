import { mkdtempSync, rmSync } from "node:fs";
import { spawn, spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const workspaceRoot = resolve(scriptDir, "../../..");
const target = process.env.PLURORA_DESKTOP_TARGET ?? detectHostTriple();
const extension = target.includes("windows") ? ".exe" : "";
const binary = resolve(scriptDir, "../src-tauri/binaries", `plurora-host-${target}${extension}`);
const staticDir = resolve(workspaceRoot, "clients/web/dist");
const dataDir = mkdtempSync(join(tmpdir(), "plurora-desktop-smoke-"));
const token = "desktop-sidecar-smoke-token";
const bootstrapNonce = "desktop-sidecar-smoke-bootstrap-nonce";

const child = spawn(binary, [
  "host", "serve",
  "--http", "127.0.0.1:0",
  "--static-dir", staticDir,
  "--data-dir", dataDir,
], {
  env: {
    ...process.env,
    PLURORA_HTTP_ACCESS_TOKEN: token,
    PLURORA_HTTP_BOOTSTRAP_TOKEN: bootstrapNonce,
  },
  stdio: ["ignore", "pipe", "pipe"],
  windowsHide: true,
});

let stderr = "";
child.stderr.setEncoding("utf8");
child.stderr.on("data", (chunk) => { stderr += chunk; });

try {
  const address = await listenAddress(child);
  await waitForHealth(address);

  const body = JSON.stringify({ id: "smoke", method: "host.info", params: {} });
  const denied = await fetch(`http://${address}/rpc`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body,
  });
  if (denied.status !== 401) throw new Error(`unauthenticated RPC returned ${denied.status}`);

  const allowed = await fetch(`http://${address}/rpc`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      authorization: `Bearer ${token}`,
    },
    body,
  });
  const response = await allowed.json();
  if (!allowed.ok || response.id !== "smoke" || !response.result) {
    throw new Error(`authenticated RPC failed: ${JSON.stringify(response)}`);
  }

  const bootstrap = await fetch(
    `http://${address}/host/bootstrap?nonce=${encodeURIComponent(bootstrapNonce)}`,
    { redirect: "manual" },
  );
  if (bootstrap.status !== 303) throw new Error(`bootstrap returned ${bootstrap.status}`);
  if (bootstrap.headers.get("location") !== "/?plurora_platform=desktop") {
    throw new Error(`bootstrap returned an unexpected location: ${bootstrap.headers.get("location")}`);
  }
  const sessionCookie = bootstrap.headers.get("set-cookie")?.split(";", 1)[0];
  if (!sessionCookie?.startsWith("plurora_host_session=")) {
    throw new Error("bootstrap did not issue the Host session cookie");
  }
  const cookieAllowed = await fetch(`http://${address}/rpc`, {
    method: "POST",
    headers: { "content-type": "application/json", cookie: sessionCookie },
    body,
  });
  if (!cookieAllowed.ok) throw new Error(`cookie-authenticated RPC returned ${cookieAllowed.status}`);

  const replay = await fetch(
    `http://${address}/host/bootstrap?nonce=${encodeURIComponent(bootstrapNonce)}`,
    { redirect: "manual" },
  );
  if (replay.status !== 401) throw new Error(`bootstrap replay returned ${replay.status}`);

  process.stdout.write(`managed Host sidecar smoke passed at ${address}\n`);
} finally {
  child.kill();
  await Promise.race([
    new Promise((resolveExit) => child.once("exit", resolveExit)),
    new Promise((resolveTimeout) => setTimeout(resolveTimeout, 2_000)),
  ]);
  if (dataDir.startsWith(tmpdir())) rmSync(dataDir, { recursive: true, force: true });
}

function listenAddress(processHandle) {
  return new Promise((resolveAddress, reject) => {
    const lines = createInterface({ input: processHandle.stdout });
    const timeout = setTimeout(() => {
      lines.close();
      reject(new Error(`listen handshake timed out: ${stderr}`));
    }, 20_000);
    processHandle.once("exit", (code, signal) => {
      clearTimeout(timeout);
      reject(new Error(`sidecar exited before handshake (code=${code}, signal=${signal}): ${stderr}`));
    });
    lines.on("line", (line) => {
      const match = line.match(/^PLURORA_HOST_LISTEN_ADDR=(127\.0\.0\.1:[1-9][0-9]*)$/);
      if (!match) return;
      clearTimeout(timeout);
      lines.close();
      resolveAddress(match[1]);
    });
  });
}

async function waitForHealth(address) {
  const deadline = Date.now() + 20_000;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://${address}/healthz`);
      if (response.ok && (await response.text()).trim() === "ok") return;
    } catch {
      // Host may have bound the socket but not installed the router yet.
    }
    await new Promise((resolveDelay) => setTimeout(resolveDelay, 100));
  }
  throw new Error(`health check timed out at ${address}: ${stderr}`);
}

function detectHostTriple() {
  const result = spawnSync("rustc", ["-vV"], { encoding: "utf8", shell: false });
  if (result.status !== 0) throw new Error(`rustc -vV failed: ${result.stderr}`);
  const hostLine = result.stdout.split(/\r?\n/).find((line) => line.startsWith("host: "));
  if (!hostLine) throw new Error("rustc -vV did not report a host triple");
  return hostLine.slice("host: ".length).trim();
}
