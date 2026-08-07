import { PendingPairingCredentialLease } from "./pairing-credential";

function assertEqual<T>(actual: T, expected: T) {
  if (actual !== expected) {
    throw new Error(`expected ${String(expected)}, got ${String(actual)}`);
  }
}

let replacedUrl: string | undefined;
const browserWindow = {
  location: {
    pathname: "/pair",
    search: "?from=phone&pairing_token=plurora_pair.secret-once",
    hash: "#details",
  },
  history: {
    state: { navigation: 1 },
    replaceState(_data: unknown, _unused: string, url?: string | URL | null) {
      replacedUrl = url === undefined || url === null ? undefined : String(url);
    },
  },
};

const lease = new PendingPairingCredentialLease(browserWindow);
assertEqual(lease.resolve(), "plurora_pair.secret-once");
assertEqual(replacedUrl, "/pair?from=phone#details");
assertEqual(replacedUrl?.includes("secret-once"), false);

// React StrictMode can remount the page; resolving again must return the
// in-memory credential without reading or reintroducing it into the URL.
browserWindow.location.search = "?from=phone";
assertEqual(lease.resolve(), "plurora_pair.secret-once");

lease.clear();
assertEqual(lease.resolve(), undefined);

const missing = new PendingPairingCredentialLease({
  location: { pathname: "/pair", search: "?from=phone", hash: "" },
  history: browserWindow.history,
});
assertEqual(missing.resolve(), undefined);
