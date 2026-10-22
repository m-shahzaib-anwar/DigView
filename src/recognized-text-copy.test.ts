import { expect, it, vi } from "vitest";
import {
  copyRecognizedText,
  selectedRecognizedText,
} from "./recognized-text-copy";

it("copies exactly selected Unicode text, whitespace and line breaks instead of all OCR", async () => {
  const text = "First line\nاردو 🧾 text\nLast line";
  const start = text.indexOf("اردو");
  const end = text.indexOf("Last");
  const write = vi.fn().mockResolvedValue(undefined);
  expect(
    await copyRecognizedText(
      text,
      { source: text, start, end },
      "selected",
      write,
    ),
  ).toBe(true);
  expect(write).toHaveBeenCalledExactlyOnceWith("اردو 🧾 text\n");
  expect(selectedRecognizedText(text, { source: text, start: 5, end: 6 })).toBe(
    " ",
  );
});
it("never falls back to copying all text when selection is empty or belongs to an old OCR result", async () => {
  const write = vi.fn();
  for (const selection of [
    null,
    { source: "new result", start: 2, end: 2 },
    { source: "old result", start: 0, end: 10 },
  ]) {
    expect(
      await copyRecognizedText("new result", selection, "selected", write),
    ).toBe(false);
  }
  expect(write).not.toHaveBeenCalled();
  expect(await copyRecognizedText("new result", null, "all", write)).toBe(true);
  expect(write).toHaveBeenCalledExactlyOnceWith("new result");
});
it("propagates clipboard errors for the UI to report without retrying as copy-all", async () => {
  const text = "private selected portion";
  const write = vi.fn().mockRejectedValue(Error("Clipboard unavailable"));
  await expect(
    copyRecognizedText(
      text,
      { source: text, start: 8, end: 16 },
      "selected",
      write,
    ),
  ).rejects.toThrow("Clipboard unavailable");
  expect(write).toHaveBeenCalledExactlyOnceWith("selected");
});
