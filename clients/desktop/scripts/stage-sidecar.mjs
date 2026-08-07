import { chmodSync, copyFileSync, mkdirSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const workspaceRoot = resolve(scriptDir, "../../..");
const tauriDir = resolve(scriptDir, "../src-tauri");

const release = process.argv.includes("--release");
const requestedTarget = readArgument("--target")
  ?? process.env.PLURORA_DESKTOP_TARGET
  ?? process.env.TAURI_ENV_TARGET_TRIPLE;
const target = requestedTarget ?? detectHostTriple();

if (!/^[A-Za-z0-9_.-]+$/.test(target)) {
  throw new Error(`invalid Rust target triple: ${target}`);
}

const cargoArgs = [
  "build",
  "-p", "plurora-cli",
  "--bin", "plurora",
  "--target", target,
  "--locked",
];
if (release) cargoArgs.push("--release");

run("cargo", cargoArgs, workspaceRoot);

const windows = target.includes("windows");
const extension = windows ? ".exe" : "";
const profile = release ? "release" : "debug";
const source = resolve(workspaceRoot, "target", target, profile, `plurora${extension}`);
const destination = resolve(tauriDir, "binaries", `plurora-host-${target}${extension}`);
mkdirSync(dirname(destination), { recursive: true });
copyFileSync(source, destination);
if (!windows) chmodSync(destination, 0o755);

process.stdout.write(`staged managed Host sidecar: ${destination}\n`);

function readArgument(name) {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : undefined;
}

function detectHostTriple() {
  const result = spawnSync("rustc", ["-vV"], { encoding: "utf8", shell: false });
  if (result.status !== 0) {
    throw new Error(`rustc -vV failed: ${result.stderr || result.error || "unknown error"}`);
  }
  const hostLine = result.stdout.split(/\r?\n/).find((line) => line.startsWith("host: "));
  if (!hostLine) throw new Error("rustc -vV did not report a host triple");
  return hostLine.slice("host: ".length).trim();
}

function run(command, args, cwd) {
  const result = spawnSync(command, args, { cwd, stdio: "inherit", shell: false });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}
