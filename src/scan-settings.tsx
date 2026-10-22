import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { api, desktop } from "./api";
import {
  scanSummary,
  type ScanStatus,
  type ScanPreferences,
} from "./scan-status";

export function ScanSettingsPanel() {
  const [status, setStatus] = useState<ScanStatus | null>(null);
  const [rules, setRules] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const initialized = useRef(false);
  useEffect(() => {
    if (!desktop) return;
    let alive = true;
    let pending = false;
    const refresh = async () => {
      if (pending) return;
      pending = true;
      try {
        const next = await invoke<ScanStatus>("scan_status");
        if (!alive) return;
        setStatus(next);
        if (!initialized.current) {
          initialized.current = true;
          setRules(next.preferences.exclusions.join("\n"));
        }
      } catch (e) {
        if (alive) setError(String(e));
      } finally {
        pending = false;
      }
    };
    void refresh();
    const timer = setInterval(() => void refresh(), 2000);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, []);
  async function save(patch: Partial<ScanPreferences>) {
    if (!status || busy) return;
    setBusy(true);
    setError("");
    try {
      await invoke("set_scan_preferences", {
        preferences: { ...status.preferences, ...patch },
      });
      setStatus(await invoke<ScanStatus>("scan_status"));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <section className="settings-section">
      <div className="section-heading">
        <div>
          <h2>Scanning & resources</h2>
          <p>
            Controls apply to selected folders, including background updates.
          </p>
        </div>
      </div>
      {!desktop && <p>Scan controls are available in the desktop app.</p>}
      {desktop && !status && !error && <p>Loading scan status…</p>}
      {status && (
        <>
          <p role="status">
            <strong>{scanSummary(status)}</strong>
          </p>
          <div className="background-options" aria-busy={busy}>
            <button
              className="button"
              disabled={busy}
              onClick={() => void save({ paused: !status.preferences.paused })}
            >
              {status.preferences.paused ? "Resume scanning" : "Pause scanning"}
            </button>
            <label className="background-option">
              <span>
                <strong>Low-resource mode</strong>
                <small>
                  Add small breaks between scanned entries and hash chunks to
                  reduce sustained disk/CPU activity.
                </small>
              </span>
              <input
                type="checkbox"
                checked={status.preferences.low_resource}
                disabled={busy}
                onChange={(e) => void save({ low_resource: e.target.checked })}
              />
            </label>
            <label className="background-option">
              <span>
                <strong>Pause heavy work on battery</strong>
                <small>
                  {status.battery === null
                    ? "Battery detection unavailable on this device. Manual pause remains available."
                    : "Automatically resume scans and duplicate hashing when AC power returns."}
                </small>
              </span>
              <input
                type="checkbox"
                checked={status.preferences.pause_on_battery}
                disabled={busy || status.battery === null}
                onChange={(e) =>
                  void save({ pause_on_battery: e.target.checked })
                }
              />
            </label>
          </div>
          <details className="scan-defaults">
            <summary>Default exclusions (always enabled)</summary>
            <p>
              Development dependencies, generated caches, system metadata and
              temporary downloads are skipped automatically. Your custom rules
              below add to these defaults.
            </p>
            <p className="scan-default-rules">
              {status.default_exclusions.join(", ")}
            </p>
          </details>
          <label className="scan-exclusions">
            <strong>Exclude folders and files</strong>
            <span>
              One rule per line: a name (Private), a path relative to each
              selected folder (Work/Private), or an extension (*.log). Default
              system, development and temporary-file exclusions stay enabled.
            </span>
            <textarea
              rows={5}
              maxLength={51200}
              value={rules}
              onChange={(e) => setRules(e.target.value)}
              placeholder={"Private\nWork/Archive\n*.log"}
            />
          </label>
          <p>
            Save while idle; cancel a paused task before changing rules. Saving
            schedules a recovery scan. Excluded items leave the index after a
            successful scan; original files stay in place.
          </p>
          <button
            className="button"
            disabled={busy || status.running}
            onClick={() =>
              void save({
                exclusions: rules
                  .split(/\r?\n/)
                  .map((s) => s.trim())
                  .filter(Boolean),
              })
            }
          >
            Save exclusions
          </button>
          <h3>Folder availability</h3>
          <button
            className="button small-button"
            disabled={
              busy ||
              status.running ||
              status.suspended ||
              !status.folders.length
            }
            onClick={() =>
              void (async () => {
                setBusy(true);
                setError("");
                try {
                  await api.scan();
                  setStatus(await invoke<ScanStatus>("scan_status"));
                } catch (e) {
                  setError(String(e));
                } finally {
                  setBusy(false);
                }
              })()
            }
          >
            Retry selected folders
          </button>
          {!status.folders.length && <p>No folders selected.</p>}
          {status.folders.map((folder) => (
            <div className="scan-folder-status" key={folder.id}>
              <strong>{folder.path}</strong>
              <span>
                {folder.available
                  ? "Available"
                  : "Unavailable — reconnect drive or check access"}{" "}
                ·{" "}
                {folder.last_scan
                  ? `Last completed scan: ${new Date(folder.last_scan * 1000).toLocaleString()}`
                  : "No completed scan yet"}
              </span>
              {!folder.available && (
                <small>
                  Indexed files are preserved. Recovery retries automatically
                  while the app runs; redirected folders are refused.
                </small>
              )}
            </div>
          ))}
        </>
      )}
      {error && (
        <p role="alert" className="scan-error">
          {error}
        </p>
      )}
    </section>
  );
}
