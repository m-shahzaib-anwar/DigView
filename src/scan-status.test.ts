import { describe, expect, it } from "vitest";
import { scanSummary, type ScanStatus } from "./scan-status";
const base: ScanStatus = {
  default_exclusions: [],
  preferences: {
    paused: false,
    low_resource: false,
    pause_on_battery: false,
    exclusions: [],
  },
  battery: null,
  suspended: false,
  running: false,
  errors: 0,
  watch_error: "",
  folders: [{ id: 1, path: "selected", last_scan: 10, available: true }],
};
describe("scan status interpretation", () => {
  it("never reports a disconnected, failed or unscanned library as current", () => {
    expect(
      scanSummary({
        ...base,
        folders: [{ ...base.folders[0], available: false }],
      }),
    ).toContain("attention");
    expect(scanSummary({ ...base, errors: 1 })).toContain("errors");
    expect(
      scanSummary({ ...base, watch_error: "watch unavailable" }),
    ).toContain("attention");
    expect(
      scanSummary({
        ...base,
        folders: [{ ...base.folders[0], last_scan: null }],
      }),
    ).toContain("initial scan");
    expect(scanSummary({ ...base, folders: [] })).toContain("Choose a folder");
  });
  it("prioritizes manual and battery suspension over running state", () => {
    expect(
      scanSummary({
        ...base,
        running: true,
        suspended: true,
        preferences: { ...base.preferences, paused: true },
      }),
    ).toBe("Scanning paused");
    expect(scanSummary({ ...base, running: true, suspended: true })).toContain(
      "battery",
    );
    expect(scanSummary({ ...base, running: true })).toContain("Updating");
    expect(scanSummary(base)).toBe("Watching for changes");
  });
});
