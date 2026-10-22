import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { desktop } from "./api";
import { updateActions, type UpdateStatus } from "./update-client";
const client = updateActions(invoke);
const prefKey = "digview-update-checks",
  stampKey = "digview-update-last-check";
function read(key: string) {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}
function write(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    /* Current session still works. */
  }
}
export function useUpdates(blocked: boolean) {
  const [status, setStatus] = useState<UpdateStatus | null>(null),
    [busy, setBusy] = useState(false),
    [error, setError] = useState("");
  const [automatic, setAutomatic] = useState(() => read(prefKey) === "true");
  const checking = useRef(false);
  const check = useCallback(async (manual = true) => {
    if (checking.current || !desktop) return;
    checking.current = true;
    setBusy(true);
    if (manual) setError("");
    try {
      const next = await client.check();
      setStatus(next);
      write(stampKey, String(Date.now()));
    } catch (e) {
      setStatus((s) => (s ? { ...s, available: null, notes: null } : null));
      if (manual) setError(String(e));
    } finally {
      checking.current = false;
      setBusy(false);
    }
  }, []);
  useEffect(() => {
    if (!desktop) return;
    let alive = true;
    void client
      .status()
      .then((s) => {
        if (alive) setStatus(s);
      })
      .catch((e) => {
        if (alive) setError(String(e));
      });
    return () => {
      alive = false;
    };
  }, []);
  useEffect(() => {
    if (!automatic || !status?.configured || blocked) return;
    const tick = () => {
      const last = Number(read(stampKey));
      if (
        !Number.isFinite(last) ||
        Date.now() - last >= 86400000 ||
        last > Date.now()
      )
        void check(false);
    };
    tick();
    const timer = setInterval(tick, 3600000);
    return () => clearInterval(timer);
  }, [automatic, status?.configured, blocked, check]);
  async function install(version: string) {
    if (!status) return;
    setBusy(true);
    setError("");
    try {
      await client.install(status, version, true, blocked);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  return {
    status,
    busy,
    error,
    automatic,
    check,
    install,
    setAutomatic: (value: boolean) => {
      setAutomatic(value);
      write(prefKey, String(value));
    },
    blocked,
  };
}
export type UpdateControls = ReturnType<typeof useUpdates>;
export function UpdateSettings({ updates: u }: { updates: UpdateControls }) {
  const [confirmation, setConfirmation] = useState<string | null>(null);
  return (
    <section className="settings-section">
      <h2>App updates</h2>
      <p>Installed version: {u.status?.current ?? "0.1.0"}</p>
      {!u.status?.configured && (
        <p>
          Signed updates are not configured for this build. Release hosting and
          signing setup are required before downloads can be offered.
        </p>
      )}
      <label className="background-option">
        <span>
          <strong>Check automatically for updates</strong>
          <small>
            When enabled, check on app launch and about once a day while
            running. Downloads and installation require your confirmation.
          </small>
        </span>
        <input
          type="checkbox"
          checked={u.automatic}
          disabled={!u.status?.configured || u.busy}
          onChange={(e) => u.setAutomatic(e.target.checked)}
        />
      </label>
      <button
        className="button"
        disabled={!u.status?.configured || u.busy}
        onClick={() => void u.check()}
      >
        {" "}
        {u.busy ? "Working…" : "Check for updates"}
      </button>
      {u.status?.available && (
        <>
          <p role="status">Version {u.status.available} is available.</p>
          <p className="update-notes">{u.status.notes}</p>
          <button
            className="button"
            disabled={u.busy || u.blocked}
            onClick={() => setConfirmation(u.status!.available)}
          >
            Review update
          </button>
        </>
      )}
      {confirmation && (
        <div
          className="update-confirm"
          role="region"
          aria-label="Confirm app update"
        >
          <p>
            Install version {confirmation}? DigView will close during
            installation. A local database backup is created first; your
            selected roots, library and settings stay on this device.
          </p>
          <button
            className="button primary"
            disabled={
              u.busy || u.blocked || u.status?.available !== confirmation
            }
            onClick={() => {
              setConfirmation(null);
              void u.install(confirmation);
            }}
          >
            Download, verify and install
          </button>
          <button
            className="button"
            disabled={u.busy}
            onClick={() => setConfirmation(null)}
          >
            Cancel
          </button>
        </div>
      )}
      {u.blocked && (
        <p>Finish or cancel library/OCR work before installing an update.</p>
      )}
      {u.error && <p role="alert">{u.error}</p>}
    </section>
  );
}
