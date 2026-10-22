const key = "digview-recent-searches";
export const recentSearchLimit = 10;
export function normalizeRecentSearches(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  const seen = new Set<string>();
  return value
    .filter((text): text is string => {
      if (typeof text !== "string" || !text.trim() || text.length > 512)
        return false;
      const identity = text.trim().toLocaleLowerCase();
      if (seen.has(identity)) return false;
      seen.add(identity);
      return true;
    })
    .map((text) => text.trim())
    .slice(0, recentSearchLimit);
}
export function rememberSearch(recent: string[], text: string): string[] {
  return normalizeRecentSearches([text, ...recent]);
}
export function readRecentSearches(
  storage: Pick<Storage, "getItem">,
): string[] {
  try {
    return normalizeRecentSearches(JSON.parse(storage.getItem(key) ?? "[]"));
  } catch {
    return [];
  }
}
export function writeRecentSearches(
  storage: Pick<Storage, "setItem" | "removeItem">,
  recent: string[],
) {
  if (!recent.length) storage.removeItem(key);
  else storage.setItem(key, JSON.stringify(normalizeRecentSearches(recent)));
}
