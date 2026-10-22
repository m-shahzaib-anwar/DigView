import { useEffect, useState } from "react";

export type ThemePreference = "light" | "dark" | "system";
type ResolvedTheme = "light" | "dark";
const storageKey = "digview-theme";

function readPreference(): ThemePreference {
  try {
    const value = localStorage.getItem(storageKey);
    if (value === "light" || value === "dark") return value;
  } catch {
    // Restricted storage must not prevent the library from opening.
  }
  return "system";
}

function applyAppearance(preference: ThemePreference): ResolvedTheme {
  const resolved =
    preference === "system"
      ? matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light"
      : preference;
  document.documentElement.dataset.theme = resolved;
  document.documentElement.style.colorScheme = resolved;
  document
    .querySelector('meta[name="theme-color"]')
    ?.setAttribute("content", resolved === "dark" ? "#131a17" : "#f8f9fb");
  return resolved;
}

// Apply the saved choice before mounting React, including after a reload.
export function initializeTheme() {
  applyAppearance(readPreference());
}

let nativeUpdates = Promise.resolve();
function syncWindowTheme(preference: ThemePreference) {
  if (!("__TAURI_INTERNALS__" in window)) return;
  // Preserve order when the user changes the theme rapidly.
  nativeUpdates = nativeUpdates
    .then(async () => {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().setTheme(
        preference === "system" ? null : preference,
      );
    })
    .catch((error: unknown) => {
      console.warn("Native window appearance could not be updated", error);
    });
}

export function useTheme() {
  const [preference, updatePreference] =
    useState<ThemePreference>(readPreference);
  const [resolved, updateResolved] = useState<ResolvedTheme>(() =>
    document.documentElement.dataset.theme === "dark" ? "dark" : "light",
  );

  useEffect(() => {
    const media = matchMedia("(prefers-color-scheme: dark)");
    const apply = () => updateResolved(applyAppearance(preference));
    apply();
    syncWindowTheme(preference);
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [preference]);

  useEffect(() => {
    const changed = (event: StorageEvent) => {
      if (event.key === storageKey || event.key === null)
        updatePreference(readPreference());
    };
    window.addEventListener("storage", changed);
    return () => window.removeEventListener("storage", changed);
  }, []);

  const setPreference = (next: ThemePreference) => {
    try {
      localStorage.setItem(storageKey, next);
    } catch {
      /* Session choice still works. */
    }
    updatePreference(next);
  };
  return { preference, resolved, setPreference };
}
