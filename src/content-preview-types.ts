export type PreviewKind = "pdf" | "video" | "audio" | "text";
export interface ContentPreview {
  kind: PreviewKind;
  mime: string;
  token: string | null;
  text: string | null;
  truncated: boolean;
  encoding: string | null;
}
const textExtensions = new Set(
  "txt text md markdown log csv tsv json jsonl xml yaml yml toml ini cfg conf html htm css scss js jsx ts tsx rs py java c h cpp hpp cs go sql sh ps1 bat cmd srt vtt".split(
    " ",
  ),
);
export function previewKind(
  extension: string,
  name: string,
): PreviewKind | undefined {
  const ext = extension.toLowerCase();
  if (ext === "pdf") return "pdf";
  if (["mp4", "m4v", "mov", "webm", "mkv", "avi", "wmv"].includes(ext))
    return "video";
  if (
    ["mp3", "wav", "flac", "aac", "m4a", "ogg", "oga", "opus", "wma"].includes(
      ext,
    )
  )
    return "audio";
  if (
    textExtensions.has(ext) ||
    (!ext &&
      [
        "readme",
        "license",
        "licence",
        "notice",
        "changelog",
        "makefile",
        "dockerfile",
      ].includes(name.toLowerCase()))
  )
    return "text";
}
export function validPreview(info: ContentPreview, kind: PreviewKind) {
  return (
    info.kind === kind &&
    (kind === "text"
      ? typeof info.text === "string" &&
        new TextEncoder().encode(info.text).byteLength <= 512 * 1024
      : typeof info.token === "string" && /^[a-f0-9]{64}$/.test(info.token))
  );
}
