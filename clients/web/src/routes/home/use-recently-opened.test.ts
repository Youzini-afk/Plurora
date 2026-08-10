import { loadStore, saveStore, recordOpen, clearStore } from "./use-recently-opened";
import type { StorageLike } from "./use-recently-opened";

function assertEqual<T>(actual: T, expected: T) {
  if (actual !== expected) {
    throw new Error(`expected ${String(expected)}, got ${String(actual)}`);
  }
}

function assertDeepEqual(actual: unknown, expected: unknown) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(`expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
  }
}

function createMockStorage(): StorageLike {
  const map = new Map<string, string>();
  return {
    getItem: (key: string) => map.get(key) ?? null,
    setItem: (key: string, value: string) => map.set(key, value),
  };
}

// Test 1: empty initial
const s1 = createMockStorage();
assertDeepEqual(loadStore(s1), []);

// Test 2: recordOpen pushes front
const s2 = createMockStorage();
recordOpen("p1", s2);
const listAfterOne = loadStore(s2);
assertEqual(listAfterOne.length, 1);
assertEqual(listAfterOne[0].installationId, "p1");

// Test 3: dedupes existing id by moving to front
const s3 = createMockStorage();
recordOpen("p1", s3);
recordOpen("p2", s3);
recordOpen("p3", s3);
recordOpen("p1", s3); // p1 was last, should move to front
const listAfterDedup = loadStore(s3);
assertEqual(listAfterDedup.length, 3);
assertEqual(listAfterDedup[0].installationId, "p1");
assertEqual(listAfterDedup[1].installationId, "p3");
assertEqual(listAfterDedup[2].installationId, "p2");

// Test 4: cap at 8
const s4 = createMockStorage();
recordOpen("p1", s4);
recordOpen("p2", s4);
recordOpen("p3", s4);
for (let i = 4; i <= 10; i++) {
  recordOpen(`p${i}`, s4);
}
const listAfterCap = loadStore(s4);
assertEqual(listAfterCap.length, 8);
assertEqual(listAfterCap[0].installationId, "p10");
assertEqual(listAfterCap[7].installationId, "p3");

// Test 5: corrupt storage falls back to empty
const s5 = createMockStorage();
s5.setItem("plurora-recently-opened", "not json");
assertDeepEqual(loadStore(s5), []);

// Test 6: clear empties storage
const s6 = createMockStorage();
recordOpen("x1", s6);
clearStore(s6);
assertDeepEqual(loadStore(s6), []);

// Test 7: null storage best-effort (no throw)
recordOpen("z1", null);
assertDeepEqual(loadStore(null), []);
