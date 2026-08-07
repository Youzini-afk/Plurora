import { canRegisterServiceWorker } from "./pwa";

function assertEqual<T>(actual: T, expected: T) {
  if (actual !== expected) throw new Error(`expected ${String(expected)}, got ${String(actual)}`);
}

assertEqual(canRegisterServiceWorker({ protocol: "https:", hostname: "host.test", search: "" }, true), true);
assertEqual(canRegisterServiceWorker({ protocol: "http:", hostname: "127.0.0.1", search: "?plurora_platform=desktop" }, true), false);
assertEqual(canRegisterServiceWorker({ protocol: "http:", hostname: "tauri.localhost", search: "" }, true), false);
assertEqual(canRegisterServiceWorker({ protocol: "file:", hostname: "", search: "" }, true), false);
assertEqual(canRegisterServiceWorker({ protocol: "https:", hostname: "host.test", search: "" }, false), false);
