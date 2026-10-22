import { expect, it } from "vitest";
import {
  previewKind,
  validPreview,
  type ContentPreview,
} from "./content-preview-types";

it("routes only supported files to local viewers and treats code/HTML as text", () => {
  expect(previewKind("PDF", "report.PDF")).toBe("pdf");
  expect(previewKind("webm", "movie.webm")).toBe("video");
  expect(previewKind("flac", "song.flac")).toBe("audio");
  expect(previewKind("html", "untrusted.html")).toBe("text");
  expect(previewKind("", "README")).toBe("text");
  expect(previewKind("docx", "report.docx")).toBeUndefined();
  expect(previewKind("exe", "program.exe")).toBeUndefined();
});
it("accepts opaque preview sessions and refuses paths, remote URLs and kind mismatches", () => {
  const info: ContentPreview = {
    kind: "video",
    mime: "video/mp4",
    token: "ab".repeat(32),
    text: null,
    truncated: false,
    encoding: null,
  };
  expect(validPreview(info, "video")).toBe(true);
  for (const token of [
    "../../private.mp4",
    "https://example.invalid",
    "C:\\private.mp4",
  ])
    expect(validPreview({ ...info, token }, "video")).toBe(false);
  expect(validPreview(info, "pdf")).toBe(false);
  expect(
    validPreview(
      {
        ...info,
        kind: "text",
        token: null,
        text: "<script>literal text</script>",
      },
      "text",
    ),
  ).toBe(true);
});
