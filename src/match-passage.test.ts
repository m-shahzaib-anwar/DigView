import { describe, it, expect } from "vitest";
import { findPassage, passageAnchors } from "./match-passage";
describe("passage navigation", () => {
  it("finds the original range through PDF line breaks and punctuation", () => {
    const text =
      "Header. A refund\nreturns to your-card within five days. Footer.";
    const range = findPassage(
      text,
      "A refund returns to your card within five days",
    );
    expect(range).not.toBeNull();
    expect(text.slice(range!.start, range!.end)).toBe(
      "A refund\nreturns to your-card within five days",
    );
  });
  it("retains UTF-16 offsets around supplementary characters and case expansion", () => {
    const text = "😀 Intro: İSTANBUL café report. End.";
    const range = findPassage(text, "İstanbul café");
    expect(text.slice(range!.start, range!.end)).toBe("İSTANBUL café");
  });
  it("does not claim a missing or empty passage was found", () => {
    expect(findPassage("some text", "unknown passage")).toBeNull();
    expect(findPassage("some text", "...")).toBeNull();
  });
  it("offers bounded anchors for passages crossing page boundaries", () => {
    const text =
      "Long opening section about the repayment process. ".repeat(6) +
      "Closing note.";
    const anchors = passageAnchors(text);
    expect(anchors[0]).toBe(text);
    expect(anchors).toHaveLength(4);
    expect(Array.from(anchors[1]).length).toBeLessThanOrEqual(96);
    expect(findPassage(text.slice(0, 110), anchors[1])).not.toBeNull();
  });
});
