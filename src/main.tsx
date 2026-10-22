import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { SetupGate } from "./onboarding";
import "./styles.css";
import { initializeTheme } from "./theme";
import { desktop } from "./api";
import { suppressBrowserActions } from "./webview-behavior";
import { installObservability } from "./observability";
import { installLocalDiagnostics } from "./local-diagnostics";
import { AppErrorBoundary } from "./local-diagnostics-panel";
import { migrateLegacyStorage } from "./legacy-storage";

migrateLegacyStorage();
initializeTheme();
if (desktop) {
  const stopLocalDiagnostics = installLocalDiagnostics(window);
  import.meta.hot?.dispose(stopLocalDiagnostics);
  const stopObservability = installObservability(window);
  import.meta.hot?.dispose(stopObservability);
  const restoreBrowserActions = suppressBrowserActions(window);
  import.meta.hot?.dispose(restoreBrowserActions);
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <AppErrorBoundary>
      <SetupGate>
        <App />
      </SetupGate>
    </AppErrorBoundary>
  </React.StrictMode>,
);
