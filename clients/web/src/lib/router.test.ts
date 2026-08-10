import { isValidInstallationId, parseHash, parseInstallationPath, installationPath, serializeRoute } from "./router";

function assertDeepEqual(actual: unknown, expected: unknown) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(`expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
  }
}

function assertEqual<T>(actual: T, expected: T) {
  if (actual !== expected) throw new Error(`expected ${String(expected)}, got ${String(actual)}`);
}

assertDeepEqual(parseHash("#/"), { kind: "home" });
assertDeepEqual(parseHash("#/settings/storage"), { kind: "settings", tab: "storage" });
assertDeepEqual(parseHash("#/installation/youzini-afk__YdlTavern__2a47e5c"), {
  kind: "installation",
  installationId: "youzini-afk__YdlTavern__2a47e5c",
});
assertDeepEqual(parseHash("#/installation/%"), { kind: "home" });
assertDeepEqual(parseHash("#/installation/bad%2Fid"), { kind: "home" });

assertEqual(serializeRoute({ kind: "settings", tab: "about" }), "#/settings/about");
assertEqual(installationPath("demo.installation-1"), "/installation/demo.installation-1");
assertDeepEqual(parseInstallationPath("/installation/demo.installation-1"), { kind: "installation", installationId: "demo.installation-1" });
assertDeepEqual(parseInstallationPath("/installation/bad%2Fid"), null);
assertDeepEqual(parseInstallationPath("/installation/demo/extra"), null);
assertEqual(isValidInstallationId("demo_installation-1.x"), true);
assertEqual(isValidInstallationId("bad/id"), false);
assertEqual(isValidInstallationId(".."), false);
assertEqual(isValidInstallationId("a..b"), false);
assertEqual(isValidInstallationId(".hidden"), false);
assertEqual(isValidInstallationId("foo:bar"), false);
assertEqual(isValidInstallationId("foo@bar"), false);
assertEqual(isValidInstallationId(""), false);
