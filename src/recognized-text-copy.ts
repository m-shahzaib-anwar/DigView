export type RecognizedSelection = {
  source: string;
  start: number;
  end: number;
};

export function selectedRecognizedText(
  text: string,
  selection: RecognizedSelection | null,
): string {
  if (
    !selection ||
    selection.source !== text ||
    !Number.isInteger(selection.start) ||
    !Number.isInteger(selection.end)
  )
    return "";
  const start = Math.max(0, selection.start);
  const end = Math.min(text.length, selection.end);
  return end > start ? text.slice(start, end) : "";
}

export async function copyRecognizedText(
  text: string,
  selection: RecognizedSelection | null,
  scope: "selected" | "all",
  write: (value: string) => Promise<void>,
): Promise<boolean> {
  const value =
    scope === "all" ? text : selectedRecognizedText(text, selection);
  if (!value) return false;
  await write(value);
  return true;
}
