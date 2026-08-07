import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

const dist = resolve("dist");
for (const relative of [
  "index.html",
  "manifest.webmanifest",
  "sw.js",
  "icons/plurora.svg",
  "icons/plurora-maskable.svg",
]) {
  if (!existsSync(resolve(dist, relative))) throw new Error(`missing PWA artifact: dist/${relative}`);
}

const manifest = JSON.parse(readFileSync(resolve(dist, "manifest.webmanifest"), "utf8"));
if (manifest.id !== "/" || manifest.start_url !== "/" || manifest.scope !== "/") {
  throw new Error("PWA manifest must remain rooted at the Host origin");
}
if (manifest.display !== "standalone") throw new Error("PWA manifest is not installable standalone");
if (!manifest.icons?.some((icon) => icon.purpose === "maskable")) {
  throw new Error("PWA manifest has no maskable icon");
}

const index = readFileSync(resolve(dist, "index.html"), "utf8");
for (const marker of ["manifest.webmanifest", "viewport-fit=cover", "theme-color"]) {
  if (!index.includes(marker)) throw new Error(`dist/index.html is missing ${marker}`);
}

const serviceWorker = readFileSync(resolve(dist, "sw.js"), "utf8");
if (!serviceWorker.includes("index.html")) throw new Error("service worker does not precache the shell");
if (serviceWorker.includes(".map\"")) throw new Error("service worker must not precache source maps");

process.stdout.write("PWA artifact verification passed\n");
