import { afterEach, expect, it, vi } from "vitest";
const mocks = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.resetModules();
  mocks.invoke.mockReset();
});

it("keeps only app source locations from stacks and drops private exception text and URLs", async () => {
  const { safeErrorFrames } = await import("./local-diagnostics");
  const error = new Error(
    "person@example.invalid secret-token C:\\Users\\Private\\file.txt",
  );
  error.stack =
    "Error: private message\n at f (http://tauri.localhost/assets/index-abc.js?token=private:4:20)\n at g (https://private.invalid/assets/private.js:1:1)\n at h (file:///C:/Users/Private/file.js:1:2)\n at f (http://tauri.localhost/src/App.tsx:8:12)";
  expect(safeErrorFrames(error, "http://tauri.localhost")).toEqual([
    "index-abc.js:4:20",
    "App.tsx:8:12",
  ]);
});
it("writes errors locally while offline and exports without a network request", async () => {
  vi.useFakeTimers();
  vi.stubGlobal("window", {
    __TAURI_INTERNALS__: {},
    location: { origin: "http://tauri.localhost" },
  });
  const network = vi.fn();
  vi.stubGlobal("fetch", network);
  vi.stubGlobal("navigator", { onLine: false });
  mocks.invoke.mockResolvedValue(true);
  const { localDiagnostics } = await import("./local-diagnostics");
  localDiagnostics.error(
    "frontend_error",
    new TypeError("secret user content"),
    "unhandled_error",
  );
  await vi.runAllTimersAsync();
  expect(mocks.invoke).toHaveBeenCalledWith("write_diagnostics", {
    entries: [
      {
        event: "frontend_error",
        code: "unhandled_error",
        error_name: "TypeError",
        frames: [],
      },
    ],
  });
  expect(await localDiagnostics.export()).toBe(true);
  expect(mocks.invoke).toHaveBeenCalledWith("export_diagnostics");
  expect(network).not.toHaveBeenCalled();
  expect(JSON.stringify(mocks.invoke.mock.calls)).not.toContain(
    "secret user content",
  );
});
it("contains logging failures and does not recursively report them", async () => {
  vi.useFakeTimers();
  vi.stubGlobal("window", {
    __TAURI_INTERNALS__: {},
    location: { origin: "http://tauri.localhost" },
  });
  mocks.invoke.mockRejectedValue(Error("disk unavailable"));
  const { localDiagnostics } = await import("./local-diagnostics");
  localDiagnostics.record({ event: "app_started" });
  await vi.runAllTimersAsync();
  expect(mocks.invoke).toHaveBeenCalledTimes(1);
});
