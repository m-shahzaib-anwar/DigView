import { expect, it, vi } from "vitest";
import {
  createObservability,
  safeProperties,
  sentryEndpoint,
  jobObserver,
  type TelemetryConfig,
} from "./observability";
import type { Job } from "./types";
const config: TelemetryConfig = {
  production: true,
  enabled: true,
  analytics: true,
  errors: true,
  logs: true,
  token: "phc_fixture1234",
  host: "https://us.i.posthog.com",
  dsn: "https://0123456789abcdef0123456789abcdef@o123.ingest.us.sentry.io/456",
};
function setup(overrides: Partial<TelemetryConfig> = {}) {
  const values = new Map<string, string>();
  const storage = {
    getItem: vi.fn((key: string) => values.get(key) ?? null),
    setItem: vi.fn((key: string, value: string) => {
      values.set(key, value);
    }),
    removeItem: vi.fn((key: string) => {
      values.delete(key);
    }),
  };
  let time = Date.UTC(2026, 9, 20);
  let online = true;
  let counter = 0;
  const fetch = vi
    .fn<typeof globalThis.fetch>()
    .mockResolvedValue(new Response("{}", { status: 200 }));
  const id = vi.fn(
    () => `00000000-0000-4000-8000-${String(++counter).padStart(12, "0")}`,
  );
  const deps = {
    storage: () => storage,
    fetch,
    now: () => time,
    id,
    online: () => online,
  };
  const client = createObservability({ ...config, ...overrides }, deps);
  return {
    client,
    storage,
    values,
    fetch,
    id,
    deps,
    advance: (n: number) => {
      time += n;
    },
    offline: () => {
      online = false;
    },
    online: () => {
      online = true;
    },
  };
}
it.each([
  { enabled: false },
  { production: false },
  { analytics: false, errors: false, logs: false },
])("does not collect, persist or send when disabled: %o", async (flags) => {
  const f = setup(flags);
  f.client.capture("app_started");
  f.client.capture("frontend_error", { code: "unhandled_error" }, true);
  await f.client.flush();
  expect(f.client.active).toBe(false);
  expect(f.storage.getItem).not.toHaveBeenCalled();
  expect(f.storage.setItem).not.toHaveBeenCalled();
  expect(f.id).not.toHaveBeenCalled();
  expect(f.fetch).not.toHaveBeenCalled();
});
it("rejects unsupported endpoints and personal keys without side effects", async () => {
  const f = setup({
    token: "phx_secret",
    host: "http://localhost",
    dsn: "https://secret@evil.test/1",
    logs: false,
  });
  f.client.capture("app_started");
  await f.client.flush();
  expect(f.client.active).toBe(false);
  expect(f.fetch).not.toHaveBeenCalled();
  for (const dsn of [
    "https://0123456789abcdef@o1.ingest.sentry.io/1?token=secret",
    "https://0123456789abcdef:secret@o1.ingest.sentry.io/1",
    "https://0123456789abcdef@evil.ingest.sentry.io/1",
  ])
    expect(sentryEndpoint(dsn)).toBeNull();
});
it("keeps only explicit numeric, boolean and enum metadata, never raw errors or files", () => {
  const input = {
    action: "open_file",
    duration_ms: 12.4,
    errors: Infinity,
    count: -1,
    cancelled: true,
    path: "C:/private/secret.pdf",
    email: "me@example.com",
    otp: "123456",
    token: "bearer-secret",
    message: "secret",
    stack: "private-path",
    view: "secret-query",
    version: "0.1.0",
  };
  expect(safeProperties(input)).toEqual({
    action: "open_file",
    duration_ms: 12,
    count: 0,
    cancelled: true,
    version: "0.1.0",
  });
});
it("sends bounded PostHog events and sanitized Sentry envelopes without cookies", async () => {
  const f = setup();
  f.client.setContext("windows", "0.1.0");
  f.client.capture(
    "native_failed",
    {
      action: "open_file",
      code: "request_failed",
      path: "SECRET",
      message: "SECRET",
    },
    true,
  );
  await f.client.flush();
  expect(f.fetch).toHaveBeenCalledTimes(2);
  const [url, request] = f.fetch.mock.calls[0];
  expect(url).toBe(config.host + "/batch/");
  expect(request).toMatchObject({
    credentials: "omit",
    referrerPolicy: "no-referrer",
    redirect: "error",
  });
  const batch = JSON.parse(request!.body as string).batch;
  expect(batch[0].distinct_id).toBe(
    f.values.get("digview-observability-device-v1"),
  );
  expect(batch[0].properties.$process_person_profile).toBe(false);
  expect(batch[0].properties.platform).toBe("windows");
  const [errorUrl, errorRequest] = f.fetch.mock.calls[1];
  expect(errorUrl).toBe("https://o123.ingest.us.sentry.io/api/456/envelope/");
  const envelope = (errorRequest!.body as string)
    .trim()
    .split("\n")
    .map((line) => JSON.parse(line));
  expect(envelope[1]).toEqual({ type: "event" });
  expect(envelope[2].message).toBe("native_failed");
  expect(envelope[2].user).toBeUndefined();
  expect(envelope[2].request).toBeUndefined();
  expect(JSON.stringify(f.fetch.mock.calls)).not.toContain("SECRET");
  await f.client.flush();
  expect(f.fetch).toHaveBeenCalledTimes(2);
});
it("retains failed events offline across reload and retries with backoff", async () => {
  const f = setup();
  f.offline();
  f.client.capture("app_started");
  await f.client.flush();
  expect(f.fetch).not.toHaveBeenCalled();
  const reloaded = createObservability(config, f.deps);
  f.online();
  f.fetch.mockRejectedValueOnce(new Error("offline"));
  await reloaded.flush();
  expect(f.fetch).toHaveBeenCalledTimes(1);
  await reloaded.flush();
  expect(f.fetch).toHaveBeenCalledTimes(1);
  f.advance(10000);
  await reloaded.flush();
  expect(f.fetch).toHaveBeenCalledTimes(2);
  await reloaded.flush();
  expect(f.fetch).toHaveBeenCalledTimes(2);
});
it("does not duplicate successful analytics delivery when the error sink fails", async () => {
  const f = setup();
  f.client.capture("frontend_error", { code: "unhandled_error" }, true);
  f.fetch
    .mockResolvedValueOnce(new Response("{}"))
    .mockRejectedValueOnce(new Error("sentry down"));
  await f.client.flush();
  f.advance(10000);
  await f.client.flush();
  expect(f.fetch.mock.calls.map((c) => c[0])).toEqual([
    config.host + "/batch/",
    sentryEndpoint(config.dsn),
    sentryEndpoint(config.dsn),
  ]);
});
it("bounds local logs and offline backlog to 100 records and expires them after 7 days", async () => {
  const f = setup();
  f.offline();
  for (let i = 0; i < 110; i++) f.client.capture("native_action", { count: i });
  expect(f.client.localRecords()).toHaveLength(100);
  expect(f.client.localRecords()[0].properties.count).toBe(10);
  f.advance(8 * 86400_000);
  expect(f.client.localRecords()).toHaveLength(0);
  f.online();
  await f.client.flush();
  expect(f.fetch).not.toHaveBeenCalled();
});
it("sanitizes corrupted persisted records and clears queued history", async () => {
  const f = setup();
  f.values.set(
    "digview-observability-v1",
    JSON.stringify([
      {
        event: "frontend_error",
        id: "00000000-0000-4000-8000-000000000001",
        timestamp: new Date(f.deps.now()).toISOString(),
        properties: { code: "unhandled_error", message: "SECRET" },
        person: "user_legacy",
        analytics: true,
        error: true,
      },
    ]),
  );
  await f.client.flush();
  expect(JSON.stringify(f.fetch.mock.calls)).not.toMatch(/SECRET|user_legacy/);
  expect(f.values.get("digview-observability-v1")).not.toContain("user_legacy");
  f.client.clear();
  expect(f.values.size).toBe(0);
});
it("tracks terminal job transition once using native duration, excluding messages", () => {
  const capture = vi.fn();
  const observe = jobObserver(capture);
  const job: Job = {
    revision: 1,
    running: true,
    kind: "scan",
    processed: 1,
    errors: 0,
    message: "SECRET PATH",
    watch_error: "SECRET",
    cancelled: false,
    duration_ms: 1,
  };
  observe(job);
  observe({ ...job, running: false, processed: 5, duration_ms: 1200 });
  observe({ ...job, running: false });
  expect(capture).toHaveBeenCalledOnce();
  expect(capture).toHaveBeenCalledWith(
    "job_finished",
    {
      kind: "scan",
      duration_ms: 1200,
      processed: 5,
      errors: 0,
      cancelled: false,
      success: true,
    },
    false,
  );
});
it("logging alone records metadata without sending", async () => {
  const f = setup({ analytics: false, errors: false });
  f.client.capture("screen_viewed", { view: "all" });
  await f.client.flush();
  expect(f.fetch).not.toHaveBeenCalled();
  expect(f.client.localRecords()).toHaveLength(1);
});
it("deduplicates repeated error codes without masking unrelated failures", async () => {
  const f = setup();
  f.client.capture(
    "native_failed",
    { action: "job_status", code: "request_failed" },
    true,
  );
  f.client.capture(
    "native_failed",
    { action: "job_status", code: "request_failed" },
    true,
  );
  f.client.capture(
    "native_failed",
    { action: "open_file", code: "request_failed" },
    true,
  );
  expect(f.client.localRecords()).toHaveLength(2);
  f.advance(30000);
  f.client.capture(
    "native_failed",
    { action: "job_status", code: "request_failed" },
    true,
  );
  expect(f.client.localRecords()).toHaveLength(3);
});
it("serializes concurrent flushes and keeps failed HTTP responses for retry", async () => {
  const f = setup();
  f.client.capture("app_started");
  let release!: (response: Response) => void;
  f.fetch.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        release = resolve;
      }),
  );
  const flushing = f.client.flush();
  await f.client.flush();
  expect(f.fetch).toHaveBeenCalledOnce();
  release(new Response("{}", { status: 429 }));
  await flushing;
  f.advance(10000);
  await f.client.flush();
  expect(f.fetch).toHaveBeenCalledTimes(2);
});
it("keeps the error sink independent when analytics delivery is unavailable", async () => {
  const f = setup();
  f.client.capture("frontend_error", { code: "unhandled_error" }, true);
  f.fetch.mockRejectedValueOnce(new Error("posthog down"));
  await f.client.flush();
  expect(f.fetch.mock.calls.map((c) => c[0])).toEqual([
    config.host + "/batch/",
    sentryEndpoint(config.dsn),
  ]);
  f.advance(10000);
  await f.client.flush();
  expect(f.fetch.mock.calls.map((c) => c[0])).toEqual([
    config.host + "/batch/",
    sentryEndpoint(config.dsn),
    config.host + "/batch/",
  ]);
});
it("fails quietly if storage or device ID support is unavailable", async () => {
  const f = setup();
  f.storage.getItem.mockImplementation(() => {
    throw new Error("storage blocked");
  });
  f.storage.setItem.mockImplementation(() => {
    throw new Error("storage full");
  });
  f.client.capture("app_started");
  await f.client.flush();
  expect(f.fetch).toHaveBeenCalledOnce();
  const broken = createObservability(config, {
    ...f.deps,
    id: () => {
      throw new Error("crypto unavailable");
    },
  });
  expect(() => broken.capture("app_started")).not.toThrow();
  await expect(broken.flush()).resolves.toBeUndefined();
});
it("rejects prototype-named fields in untrusted cached properties", () => {
  expect(
    safeProperties(
      JSON.parse(
        '{"constructor":"SECRET","toString":"SECRET","__proto__":{"path":"SECRET"}}',
      ),
    ),
  ).toEqual({});
});
