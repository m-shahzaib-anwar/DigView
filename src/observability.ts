import type { Job } from "./types";

export type TelemetryConfig = {
  production: boolean;
  enabled: boolean;
  analytics: boolean;
  errors: boolean;
  logs: boolean;
  token: string;
  host: string;
  dsn: string;
};
type SafeValue = string | number | boolean;
type RecordEntry = {
  id: string;
  event: string;
  timestamp: string;
  properties: Record<string, SafeValue>;
  device: string;
  analytics: boolean;
  error: boolean;
};
type StoragePort = Pick<Storage, "getItem" | "setItem" | "removeItem">;
type Dependencies = {
  storage: () => StoragePort;
  fetch: typeof fetch;
  now: () => number;
  id: () => string;
  online: () => boolean;
};
const EVENTS = new Set([
  "app_started",
  "workspace_ready",
  "window_unload",
  "screen_viewed",
  "native_action",
  "native_failed",
  "job_finished",
  "frontend_error",
]);
const ENUMS: Record<string, readonly string[]> = {
  action: [
    "bootstrap",
    "job_status",
    "choose_folder",
    "remove_root",
    "start_scan",
    "start_duplicates",
    "cancel_job",
    "query_files",
    "duplicate_groups",
    "get_thumbnail",
    "open_file",
    "reveal_file",
    "recognize_text",
    "trash_files",
    "copy_text",
  ],
  view: [
    "home",
    "all",
    "images",
    "videos",
    "music",
    "documents",
    "archives",
    "apps",
    "other",
    "sources",
    "cleanup",
    "duplicates",
    "settings",
  ],
  kind: ["scan", "duplicates", "watch"],
  code: [
    "request_failed",
    "unhandled_error",
    "unhandled_rejection",
    "react_render_failed",
  ],
  platform: ["windows", "macos", "linux", "unknown"],
};
const COUNTS = new Set(["duration_ms", "processed", "errors", "count"]);
export function safeProperties(input: Record<string, unknown>) {
  const result: Record<string, SafeValue> = {};
  for (const [key, value] of Object.entries(input)) {
    if (COUNTS.has(key) && typeof value === "number" && Number.isFinite(value))
      result[key] = Math.max(0, Math.min(1e12, Math.round(value)));
    else if (
      ["success", "cancelled"].includes(key) &&
      typeof value === "boolean"
    )
      result[key] = value;
    else if (
      key === "version" &&
      typeof value === "string" &&
      /^\d+\.\d+\.\d+$/.test(value)
    )
      result[key] = value;
    else if (Object.hasOwn(ENUMS, key) && ENUMS[key].includes(value as string))
      result[key] = value as string;
  }
  return result;
}
function posthogHost(value: string) {
  return ["https://us.i.posthog.com", "https://eu.i.posthog.com"].includes(
    value,
  )
    ? value
    : "";
}
export function sentryEndpoint(dsn: string) {
  try {
    const url = new URL(dsn);
    if (
      url.protocol !== "https:" ||
      url.password ||
      url.port ||
      url.search ||
      url.hash ||
      !/^[a-f0-9]{16,64}$/i.test(url.username) ||
      !/^\/\d+$/.test(url.pathname) ||
      !/^o\d+\.ingest(?:\.(?:us|de))?\.sentry\.io$/.test(url.hostname)
    )
      return null;
    return `https://${url.hostname}/api${url.pathname}/envelope/`;
  } catch {
    return null;
  }
}
const KEY = "digview-observability-v1";
const DEVICE = "digview-observability-device-v1";
const TTL = 7 * 86400_000;
const LIMIT = 100;
export function createObservability(
  config: TelemetryConfig,
  deps: Dependencies,
) {
  const allowed = config.production && config.enabled;
  const analytics =
    allowed &&
    config.analytics &&
    /^phc_[A-Za-z0-9_-]{8,}$/.test(config.token) &&
    !!posthogHost(config.host);
  const endpoint = sentryEndpoint(config.dsn);
  const errors = allowed && config.errors && !!endpoint;
  const logs = allowed && config.logs;
  const active = analytics || errors || logs;
  let initialized = false;
  let device = "";
  let queue: RecordEntry[] = [];
  let busy = false;
  let failures = 0;
  let retryAt = 0;
  const recentErrors = new Map<string, number>();
  let context: Record<string, SafeValue> = {};
  const now = () => deps.now();
  const prune = () => {
    queue = queue
      .filter((e) => now() - Date.parse(e.timestamp) < TTL)
      .slice(-LIMIT);
  };
  const save = () => {
    prune();
    try {
      deps.storage().setItem(KEY, JSON.stringify(queue));
    } catch {
      /* bounded memory fallback */
    }
  };
  function init() {
    if (!active || initialized) return;
    initialized = true;
    try {
      const prior = deps.storage().getItem(DEVICE);
      device = prior && /^[a-f0-9-]{36}$/i.test(prior) ? prior : deps.id();
      deps.storage().setItem(DEVICE, device);
      const parsed: unknown = JSON.parse(deps.storage().getItem(KEY) || "[]");
      if (Array.isArray(parsed))
        queue = parsed.slice(-LIMIT).flatMap((raw): RecordEntry[] => {
          if (
            !raw ||
            !EVENTS.has(raw.event) ||
            !/^[a-f0-9-]{36}$/i.test(raw.id) ||
            !Number.isFinite(Date.parse(raw.timestamp)) ||
            Date.parse(raw.timestamp) > now()
          )
            return [];
          return [
            {
              id: raw.id,
              event: raw.event,
              timestamp: new Date(raw.timestamp).toISOString(),
              properties: safeProperties(
                raw.properties && typeof raw.properties === "object"
                  ? raw.properties
                  : {},
              ),
              // Rebuild cached records without any legacy identity fields.
              device,
              analytics: analytics && raw.analytics === true,
              error: errors && raw.error === true,
            },
          ];
        });
    } catch {
      device ||= deps.id();
    }
    prune();
  }
  function capture(
    event: string,
    properties: Record<string, unknown> = {},
    isError = false,
  ) {
    if (!active || !EVENTS.has(event)) return;
    const propertiesSafe = safeProperties({ ...context, ...properties });
    if (isError) {
      const key = `${event}:${propertiesSafe.action || propertiesSafe.code || propertiesSafe.kind || "unknown"}`;
      const previous = recentErrors.get(key);
      if (previous !== undefined && now() - previous < 30000) return;
      recentErrors.set(key, now());
    }
    try {
      init();
      queue.push({
        id: deps.id(),
        event,
        timestamp: new Date(now()).toISOString(),
        properties: propertiesSafe,
        device,
        analytics,
        error: errors && isError,
      });
      save();
    } catch {
      /* optional diagnostics must not break app actions */
    }
  }
  async function post(url: string, body: string, contentType: string) {
    const response = await deps.fetch(url, {
      method: "POST",
      body,
      headers: { "Content-Type": contentType },
      credentials: "omit",
      referrerPolicy: "no-referrer",
      redirect: "error",
      signal: AbortSignal.timeout(5000),
    });
    if (!response.ok) throw new Error("telemetry_delivery_failed");
  }
  return {
    active,
    analytics,
    errors,
    logs,
    capture,
    setContext(platform: string, version: string) {
      if (active) context = safeProperties({ platform, version });
    },
    async flush() {
      if (!active || busy || !deps.online() || now() < retryAt) return;
      try {
        init();
        prune();
      } catch {
        return;
      }
      busy = true;
      let deliveryFailed = false;
      try {
        const batch = queue.filter((e) => e.analytics).slice(0, 20);
        if (analytics && batch.length) {
          try {
            await post(
              config.host + "/batch/",
              JSON.stringify({
                api_key: config.token,
                batch: batch.map((e) => ({
                  uuid: e.id,
                  event: e.event,
                  distinct_id: e.device,
                  timestamp: e.timestamp,
                  properties: {
                    ...e.properties,
                    app: "DigView",
                    $device_id: e.device,
                    $process_person_profile: false,
                    $geoip_disable: true,
                  },
                })),
              }),
              "application/json",
            );
            for (const e of batch) e.analytics = false;
            save();
          } catch {
            deliveryFailed = true;
          }
        }
        const reports = queue.filter((e) => e.error).slice(0, 5);
        for (const e of reports) {
          const eventId = e.id.replaceAll("-", "");
          const envelope =
            [
              JSON.stringify({ event_id: eventId, dsn: config.dsn }),
              JSON.stringify({ type: "event" }),
              JSON.stringify({
                event_id: eventId,
                timestamp: Date.parse(e.timestamp) / 1000,
                platform: "javascript",
                level: "error",
                logger: "digview",
                message: e.event,
                environment: "production",
                tags: Object.fromEntries(
                  Object.entries(e.properties).map(([key, value]) => [
                    key,
                    String(value),
                  ]),
                ),
                fingerprint: [
                  e.event,
                  String(e.properties.action || e.properties.code || "unknown"),
                ],
              }),
            ].join("\n") + "\n";
          try {
            await post(endpoint!, envelope, "application/x-sentry-envelope");
            e.error = false;
            save();
          } catch {
            deliveryFailed = true;
            break;
          }
        }
        if (!logs) queue = queue.filter((e) => e.analytics || e.error);
        save();
        if (deliveryFailed) {
          failures = Math.min(failures + 1, 8);
          retryAt = now() + Math.min(300000, 5000 * 2 ** failures);
        } else {
          failures = 0;
          retryAt = 0;
        }
      } catch {
        failures = Math.min(failures + 1, 8);
        retryAt = now() + Math.min(300000, 5000 * 2 ** failures);
        // Telemetry must never interrupt file actions or recursively report itself.
      } finally {
        busy = false;
      }
    },
    localRecords() {
      if (!logs) return [];
      try {
        init();
        prune();
      } catch {
        return [];
      }
      return queue.map((e) => ({
        event: e.event,
        timestamp: e.timestamp,
        properties: { ...e.properties },
      }));
    },
    clear() {
      queue = [];
      device = "";
      initialized = false;
      recentErrors.clear();
      try {
        deps.storage().removeItem(KEY);
        deps.storage().removeItem(DEVICE);
      } catch {
        /* best effort */
      }
    },
  };
}

const env = import.meta.env;
export const observability = createObservability(
  {
    production: env.PROD,
    enabled: env.VITE_OBSERVABILITY_ENABLED === "true",
    analytics: env.VITE_POSTHOG_ENABLED === "true",
    errors: env.VITE_SENTRY_ENABLED === "true",
    logs: env.VITE_DIAGNOSTICS_ENABLED === "true",
    token: env.VITE_POSTHOG_TOKEN || "",
    host: env.VITE_POSTHOG_HOST || "",
    dsn: env.VITE_SENTRY_DSN || "",
  },
  {
    storage: () => localStorage,
    fetch: (...args) => fetch(...args),
    now: Date.now,
    id: () => crypto.randomUUID(),
    online: () => navigator.onLine,
  },
);

// Poll results are observed without recording messages, roots or watch_error text.
export function jobObserver(capture: typeof observability.capture) {
  let previous: Pick<Job, "running" | "revision"> | undefined;
  return (job: Job) => {
    if (previous?.running && !job.running && job.kind)
      capture(
        "job_finished",
        {
          kind: job.kind,
          duration_ms: job.duration_ms,
          processed: job.processed,
          errors: job.errors,
          cancelled: job.cancelled,
          success: !job.errors && !job.cancelled,
        },
        job.errors > 0,
      );
    previous = { running: job.running, revision: job.revision };
  };
}
export function installObservability(target: Window) {
  if (!observability.active) return () => {};
  const onError = () =>
    observability.capture("frontend_error", { code: "unhandled_error" }, true);
  const onRejection = () =>
    observability.capture(
      "frontend_error",
      { code: "unhandled_rejection" },
      true,
    );
  const onUnload = () => observability.capture("window_unload");
  const onOnline = () => {
    void observability.flush();
  };
  target.addEventListener("error", onError);
  target.addEventListener("unhandledrejection", onRejection);
  target.addEventListener("pagehide", onUnload);
  target.addEventListener("online", onOnline);
  const timer = target.setInterval(onOnline, 15000);
  observability.capture("app_started", { duration_ms: performance.now() });
  onOnline();
  return () => {
    target.removeEventListener("error", onError);
    target.removeEventListener("unhandledrejection", onRejection);
    target.removeEventListener("pagehide", onUnload);
    target.removeEventListener("online", onOnline);
    target.clearInterval(timer);
  };
}
