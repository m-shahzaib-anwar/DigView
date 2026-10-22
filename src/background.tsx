import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { desktop } from "./api";

type BackgroundSettings = {
  keepRunning: boolean;
  startMinimized: boolean;
  runAtStartup: boolean;
  startupSupported: boolean;
};

export function BackgroundSettingsPanel() {
  const [settings, setSettings] = useState<BackgroundSettings | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    if (!desktop) return;
    void invoke<BackgroundSettings>("background_settings")
      .then(setSettings)
      .catch((e: unknown) => setError(String(e)));
  }, []);

  async function update(
    key: "keepRunning" | "startMinimized" | "runAtStartup",
    value: boolean,
  ) {
    if (!settings || busy) return;
    setBusy(true);
    setError("");
    const next = { ...settings, [key]: value };
    try {
      setSettings(
        await invoke<BackgroundSettings>("set_background_settings", {
          preferences: {
            keepRunning: next.keepRunning,
            startMinimized: next.startMinimized,
          },
          runAtStartup: next.runAtStartup,
        }),
      );
    } catch (e) {
      setError(String(e));
      // Refresh actual OS state if saving or registration was interrupted.
      try {
        setSettings(await invoke<BackgroundSettings>("background_settings"));
      } catch {
        /* Keep the last confirmed settings. */
      }
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="settings-section">
      <div className="section-heading">
        <div>
          <h2>Background & startup</h2>
          <p>
            Keep your selected folders up to date while the window is closed.
          </p>
        </div>
      </div>
      {!desktop && <p>These settings are available in the desktop app.</p>}
      {desktop && !settings && !error && <p>Loading background settings…</p>}
      {settings && (
        <div className="background-options" aria-busy={busy}>
          {(
            [
              [
                "keepRunning",
                "Keep running when closed",
                "The close button hides the window. Use the tray icon to reopen or Quit.",
              ],
              [
                "runAtStartup",
                "Run at startup",
                settings.startupSupported
                  ? "Launch DigView when you sign in to Windows. Keep the app in a permanent location."
                  : "Startup registration is currently available on Windows only.",
              ],
              [
                "startMinimized",
                "Start minimized at login",
                "Start in the system tray when signing in. Opening the app yourself always shows the window.",
              ],
            ] as const
          ).map(([key, label, description]) => (
            <label className="background-option" key={key}>
              <span>
                <strong>{label}</strong>
                <small>{description}</small>
              </span>
              <input
                type="checkbox"
                checked={settings[key]}
                disabled={
                  busy ||
                  (key === "runAtStartup" && !settings.startupSupported) ||
                  (key === "startMinimized" &&
                    (!settings.keepRunning || !settings.runAtStartup))
                }
                onChange={(event) => void update(key, event.target.checked)}
              />
            </label>
          ))}
          <p className="background-note">
            Only selected folders are indexed. Quit stops background work.
            Startup is off until you enable it.
          </p>
        </div>
      )}
      {error && (
        <p role="alert">Could not update background settings: {error}</p>
      )}
    </section>
  );
}
