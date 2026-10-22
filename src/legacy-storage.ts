// Read old keys once; active consumers write only DigView keys.
// Keep compatibility strings isolated here rather than in product components.
export function migrateLegacyStorage() {
  const keys = [
    ["filo-theme", "digview-theme"],
    ["filo-layout", "digview-layout"],
    ["filo-intro-dismissed", "digview-intro-dismissed"],
    ["zee-recent-searches", "digview-recent-searches"],
    ["zee-update-checks", "digview-update-checks"],
    ["zee-update-last-check", "digview-update-last-check"],
    ["zee-observability-v1", "digview-observability-v1"],
    ["zee-observability-device-v1", "digview-observability-device-v1"],
  ];
  for (const [oldKey, newKey] of keys) {
    try {
      if (localStorage.getItem(newKey) === null) {
        const value = localStorage.getItem(oldKey);
        if (value !== null) localStorage.setItem(newKey, value);
      }
      localStorage.removeItem(oldKey);
    } catch {
      // Restricted storage must not prevent the app from opening; retry next launch.
    }
  }
}
