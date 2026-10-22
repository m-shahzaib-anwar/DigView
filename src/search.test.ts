import { describe, it, expect } from "vitest";
import { parseSearch } from "./search";

describe("local clue search", () => {
  const now = new Date(2026, 0, 5, 12);
  it("uses calendar last month across year boundaries", () => {
    const parsed = parseSearch("screenshots from last month", now);
    expect(parsed.query.screenshot).toBe(true);
    expect(parsed.query.date_from).toBe(new Date(2025, 11, 1).getTime() / 1000);
    expect(parsed.query.date_to).toBe(new Date(2026, 0, 1).getTime() / 1000);
    expect(parsed.query.terms).toEqual([]);
  });
  it("combines file extension and prior calendar week", () => {
    const parsed = parseSearch("PDFs from last week", now);
    expect(parsed.query.terms).toEqual(["ext:pdf"]);
    expect(parsed.query.date_from).toBe(
      new Date(2025, 11, 29).getTime() / 1000,
    );
    expect(parsed.query.date_to).toBe(new Date(2026, 0, 5).getTime() / 1000);
  });
  it("converts decimal size and preserves filename terms", () => {
    const parsed = parseSearch("videos larger than 1.5 GB vacation", now);
    expect(parsed.query.category).toBe("videos");
    expect(parsed.query.min_size).toBe(1.5 * 1024 ** 3);
    expect(parsed.query.terms).toEqual(["vacation"]);
  });
  it("does not erase filename words without structured filters", () => {
    expect(
      parseSearch("the files from yesterday-report.txt", now).query.terms,
    ).toContain("yesterday-report.txt");
  });
  it("keeps quoted filename phrases", () => {
    expect(parseSearch('"quarterly report"', now).query.terms).toEqual([
      "quarterly report",
    ]);
  });
  it("keeps quoted clues literal", () => {
    const parsed = parseSearch('"screenshots from last month"', now);
    expect(parsed.chips).toEqual([]);
    expect(parsed.query.terms).toEqual(["screenshots from last month"]);
  });
  it("keeps xls as a real extension and embedded connective words intact", () => {
    expect(parseSearch("xls", now).query.terms).toEqual(["ext:xls"]);
    expect(parseSearch("JPEGs", now).query.terms).toEqual(["ext:jpeg"]);
    expect(parseSearch("images made-in-2026.jpg", now).query.terms).toEqual([
      "made-in-2026.jpg",
    ]);
  });
});
