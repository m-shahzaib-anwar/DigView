import { describe, expect, it } from "vitest";
import { taskPercent } from "./semantic-progress";
describe("semantic task percentages", () => {
  it("uses bytes/files and floors incomplete progress", () => {
    expect(taskPercent(1, 3, false)).toBe(33);
    expect(taskPercent(2, 3, false)).toBe(66);
    expect(taskPercent(9999, 10000, false)).toBe(99);
  });
  it("never claims completion before verification or successful indexing", () => {
    expect(taskPercent(100, 100, false)).toBe(99);
    expect(taskPercent(500, 100, false)).toBe(99);
    expect(taskPercent(100, 100, true)).toBe(100);
  });
  it("handles empty jobs, unknown totals and invalid counters", () => {
    expect(taskPercent(0, 0, false)).toBe(0);
    expect(taskPercent(0, 0, true)).toBe(100);
    expect(taskPercent(-5, 100, false)).toBe(0);
    expect(taskPercent(Infinity, 100, false)).toBe(0);
    expect(taskPercent(1, NaN, false)).toBe(0);
  });
});
