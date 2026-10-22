import {
  Component,
  useEffect,
  useState,
  type ErrorInfo,
  type ReactNode,
} from "react";
import { localDiagnostics } from "./local-diagnostics";

export function LocalDiagnosticsPanel() {
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [enabled, setEnabled] = useState<boolean | null>(null);
  useEffect(() => {
    let alive = true;
    void localDiagnostics
      .status()
      .then((value) => {
        if (alive) setEnabled(value);
      })
      .catch(() => {
        if (alive) setEnabled(false);
      });
    return () => {
      alive = false;
    };
  }, []);
  async function run(action: () => Promise<unknown>) {
    setBusy(true);
    setMessage("");
    try {
      await action();
    } catch {
      setMessage(
        "Logs could not be opened or exported. Check the chosen location and try again.",
      );
    } finally {
      setBusy(false);
    }
  }
  return (
    <section className="settings-section local-diagnostics-panel">
      <h2>Local diagnostics</h2>
      <p>
        {enabled === null
          ? "Checking local log storage…"
          : enabled
            ? "Local logging is enabled."
            : "Local log storage is unavailable. Check disk space and app folder permissions."}{" "}
        Errors and crash markers stay on this device. Export a log to share when
        reporting an issue. File contents, filenames, searches and credentials
        are excluded.
      </p>
      <div className="diagnostic-actions">
        <button
          className="button"
          disabled={busy}
          onClick={() =>
            void run(async () => {
              const saved = await localDiagnostics.export();
              if (saved)
                setMessage("Log exported. You can share the saved file.");
            })
          }
        >
          Export logs
        </button>
        <button
          className="button"
          disabled={busy}
          onClick={() => void run(localDiagnostics.openFolder)}
        >
          Open logs folder
        </button>
      </div>
      {message && <p role="status">{message}</p>}
    </section>
  );
}
export class AppErrorBoundary extends Component<
  { children: ReactNode },
  { failed: boolean }
> {
  state = { failed: false };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  componentDidCatch(error: Error, _info: ErrorInfo) {
    localDiagnostics.error("react_render_failed", error, "render_failed");
  }
  render() {
    if (!this.state.failed) return this.props.children;
    return (
      <main className="crash-screen">
        <h1>The app could not display this screen.</h1>
        <p>
          Export the local log to help diagnose the problem, then reload the
          app.
        </p>
        <button
          className="button primary"
          onClick={() => window.location.reload()}
        >
          Reload app
        </button>
        <LocalDiagnosticsPanel />
      </main>
    );
  }
}
