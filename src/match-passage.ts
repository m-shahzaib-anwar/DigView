// Normalize PDF/text whitespace and punctuation while retaining original UTF-16 offsets.
function indexed(text: string) {
  let value = "";
  const positions: number[] = [];
  let offset = 0;
  for (const char of text) {
    if (/[\p{L}\p{N}]/u.test(char)) {
      const lower = char.toLowerCase();
      value += lower;
      for (let j = 0; j < lower.length; j++) positions.push(offset);
    }
    offset += char.length;
  }
  return { value, positions };
}
export function findPassage(
  text: string,
  passage: string,
): { start: number; end: number } | null {
  const source = indexed(text);
  const needle = indexed(passage).value;
  if (!needle) return null;
  const at = source.value.indexOf(needle);
  if (at < 0) return null;
  const start = source.positions[at];
  const last = source.positions[at + needle.length - 1];
  const code = text.codePointAt(last)!;
  return { start, end: last + (code > 0xffff ? 2 : 1) };
}
export function passageAnchors(passage: string): string[] {
  // Full passage first. Smaller anchors locate a passage split over PDF pages.
  const chars = Array.from(passage.trim());
  if (chars.length < 48) return [passage];
  return [
    passage,
    chars.slice(0, 96).join(""),
    chars
      .slice(
        Math.max(0, Math.floor(chars.length / 2) - 48),
        Math.floor(chars.length / 2) + 48,
      )
      .join(""),
    chars.slice(-96).join(""),
  ];
}
