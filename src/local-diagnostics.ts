import { invoke } from "@tauri-apps/api/core";

type Event =
  | "app_started"
  | "frontend_error"
  | "react_render_failed"
  | "native_action"
  | "native_failed"
  | "job_finished"
  | "window_unload"
  | "preview_failed";
type Entry = {
  event: Event;
  action?: string;
  code?: string;
  error_name?: string;
  frames?: string[];
  duration_ms?: number;
  processed?: number;
  errors?: number;
  cancelled?: boolean;
};
const names = new Set([
  "Error",
  "TypeError",
  "RangeError",
  "SyntaxError",
  "ReferenceError",
  "URIError",
  "EvalError",
  "DOMException",
]);

export function safeErrorFrames(error: unknown, origin: string): string[] {
  if (!(error instanceof Error) || !error.stack) return [];
  const frames: string[] = [];
  // Never store exception text, function arguments, URLs, queries or filesystem paths.
  for (const line of error.stack.split("\n").slice(1)) {
    const match = line.match(
      /((?:https?:\/\/|tauri:\/\/)[^\s)]+):(\d+):(\d+)\)?$/,
    );
    if (!match) continue;
    try {
      const url = new URL(match[1]);
      if (
        url.origin !== origin &&
        !(
          origin === "null" &&
          url.protocol === "tauri:" &&
          url.hostname === "localhost"
        )
      )
        continue;
      if (
        !url.pathname.startsWith("/assets/") &&
        !url.pathname.startsWith("/src/")
      )
        continue;
      const file = url.pathname.split("/").pop()!;
      if (!/^[a-zA-Z0-9_.-]{1,100}\.(?:js|mjs|ts|tsx)$/.test(file)) continue;
      frames.push(`${file}:${match[2]}:${match[3]}`);
    } catch {
      /* Untrusted error source ignored. */
    }
  }
  return frames.slice(0, 12);
}
export function nativeErrorCode(reason: unknown): string {
  const message = String(reason).toLowerCase();
  if (message.includes("changed") || message.includes("identity"))
    return "file_changed";
  if (message.includes("permission") || message.includes("access is denied"))
    return "permission_denied";
  if (message.includes("not found") || message.includes("no longer"))
    return "not_found";
  if (message.includes("cancel")) return "cancelled";
  if (message.includes("unsupported")) return "unsupported";
  if (message.includes("expired")) return "preview_expired";
  return "request_failed";
}
let queue: Entry[] = [];
let timer: ReturnType<typeof setTimeout> | undefined;
let busy = false;
const recent = new Map<string, number>();
const native = () =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
async function flush() {
  if (busy || !queue.length || !native()) return;
  busy = true;
  const entries = queue.splice(0, 30);
  try {
    await invoke("write_diagnostics", { entries });
  } catch {
    /* Diagnostics cannot interrupt app actions or recursively report themselves. */
  } finally {
    busy = false;
    if (queue.length) schedule();
  }
}
function schedule() {
  if (timer) return;
  timer = setTimeout(() => {
    timer = undefined;
    void flush();
  }, 500);
}
export const localDiagnostics = {
  record(entry: Entry) {
    if (!native()) return;
    const key = JSON.stringify(entry),
      now = Date.now();
    if (now - (recent.get(key) ?? 0) < 2000) return;
    recent.set(key, now);
    if (recent.size > 150) recent.delete(recent.keys().next().value!);
    queue.push(entry);
    queue = queue.slice(-120);
    schedule();
    if (entry.event.includes("error") || entry.event.includes("failed"))
      void flush();
  },
  error(
    event: "frontend_error" | "react_render_failed" | "preview_failed",
    reason: unknown,
    code: string,
  ) {
    this.record({
      event,
      code,
      error_name:
        reason instanceof Error && names.has(reason.name)
          ? reason.name
          : "NativeError",
      frames: safeErrorFrames(reason, window.location.origin),
    });
  },
  async export() {
    await flush();
    return invoke<boolean>("export_diagnostics");
  },
  openFolder: () => invoke<void>("open_diagnostics_folder"),
  status: () => invoke<boolean>("diagnostics_status"),
};
export function installLocalDiagnostics(target: Window) {
  const onError = (event: ErrorEvent) =>
    localDiagnostics.error("frontend_error", event.error, "unhandled_error");
  const onRejection = (event: PromiseRejectionEvent) =>
    localDiagnostics.error(
      "frontend_error",
      event.reason,
      "unhandled_rejection",
    );
  const onHide = () => {
    localDiagnostics.record({ event: "window_unload" });
    void flush();
  };
  target.addEventListener("error", onError);
  target.addEventListener("unhandledrejection", onRejection);
  target.addEventListener("pagehide", onHide);
  localDiagnostics.record({ event: "app_started" });
  return () => {
    target.removeEventListener("error", onError);
    target.removeEventListener("unhandledrejection", onRejection);
    target.removeEventListener("pagehide", onHide);
    clearTimeout(timer);
    timer = undefined;
  };
}
