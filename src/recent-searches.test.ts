import { expect, it } from "vitest";
import {
  normalizeRecentSearches,
  readRecentSearches,
  recentSearchLimit,
  rememberSearch,
  writeRecentSearches,
} from "./recent-searches";

function memoryStorage() {
  const values = new Map<string, string>();
  return {
    values,
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => {
      values.set(key, value);
    },
    removeItem: (key: string) => {
      values.delete(key);
    },
  };
}
it("remembers latest submitted query first without duplicate case/spacing entries", () => {
  expect(rememberSearch(["PDFs", "images"], "  pdfs  ")).toEqual([
    "pdfs",
    "images",
  ]);
  expect(rememberSearch(["PDFs"], "  ")).toEqual(["PDFs"]);
  expect(rememberSearch(["PDFs"], "x".repeat(513))).toEqual(["PDFs"]);
});
it("bounds history and rejects malformed persisted values", () => {
  expect(
    normalizeRecentSearches([
      null,
      42,
      {},
      "",
      " files ",
      "FILES",
      "<script>text</script>",
    ]),
  ).toEqual(["files", "<script>text</script>"]);
  expect(
    normalizeRecentSearches(Array.from({ length: 20 }, (_, i) => `query ${i}`)),
  ).toHaveLength(recentSearchLimit);
  expect(normalizeRecentSearches({ text: "invalid" })).toEqual([]);
  expect(readRecentSearches({ getItem: () => "{broken JSON" })).toEqual([]);
  expect(
    readRecentSearches({
      getItem: () => {
        throw Error("storage disabled");
      },
    }),
  ).toEqual([]);
});
it("persists and clears history across reloads without touching other preferences", () => {
  const storage = memoryStorage();
  storage.setItem("digview-layout", "grid");
  writeRecentSearches(storage, rememberSearch([], "images from last week"));
  expect(readRecentSearches(storage)).toEqual(["images from last week"]);
  writeRecentSearches(storage, []);
  expect(readRecentSearches(storage)).toEqual([]);
  expect(storage.values.size).toBe(1);
  expect(storage.getItem("digview-layout")).toBe("grid");
});
it("reports persistence failures so the UI can show that clearing was not saved", () => {
  const storage = {
    setItem: () => {
      throw Error("unavailable");
    },
    removeItem: () => {
      throw Error("unavailable");
    },
  };
  expect(() => writeRecentSearches(storage, ["PDFs"])).toThrow("unavailable");
  expect(() => writeRecentSearches(storage, [])).toThrow("unavailable");
});
