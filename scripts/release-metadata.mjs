import { createHash } from "node:crypto";
import { copyFile, mkdir, readFile, stat, writeFile } from "node:fs/promises";
import { basename, join } from "node:path";

const [rawArtifactPaths, target] = process.argv.slice(2);
if (!rawArtifactPaths || !target) {
  throw new Error("usage: node scripts/release-metadata.mjs '<artifactPaths JSON>' <target>");
}
if (!/^[a-z0-9_-]+$/u.test(target)) {
  throw new Error(`invalid release target: ${target}`);
}

const artifactPaths = JSON.parse(rawArtifactPaths);
if (!Array.isArray(artifactPaths) || artifactPaths.length === 0) {
  throw new Error("Tauri did not report any release artifacts");
}

await mkdir("release-assets", { recursive: true });
await mkdir("release-metadata", { recursive: true });

const names = new Set();
const checksums = [];
for (const artifactPath of [...artifactPaths].sort()) {
  const metadata = await stat(artifactPath);
  if (!metadata.isFile()) {
    throw new Error(`release artifact is not a regular file: ${artifactPath}`);
  }
  const name = basename(artifactPath);
  if (names.has(name)) {
    throw new Error(`duplicate release artifact name: ${name}`);
  }
  names.add(name);
  const bytes = await readFile(artifactPath);
  const digest = createHash("sha256").update(bytes).digest("hex");
  await copyFile(artifactPath, join("release-assets", name));
  checksums.push(`${digest}  ${name}`);
}

await writeFile(
  join("release-metadata", `Plurora-${target}-SHA256SUMS.txt`),
  `${checksums.join("\n")}\n`,
  "utf8",
);
