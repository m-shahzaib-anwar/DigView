import { useState } from "react";
import { Clipboard, Copy } from "lucide-react";
import {
  copyRecognizedText,
  selectedRecognizedText,
  type RecognizedSelection,
} from "./recognized-text-copy";

export function RecognizedText({
  text,
  write,
  copied,
  failed,
}: {
  text: string;
  write: (value: string) => Promise<void>;
  copied: (scope: "selected" | "all") => void;
  failed: (error: unknown) => void;
}) {
  const [selection, setSelection] = useState<RecognizedSelection | null>(null);
  const [copying, setCopying] = useState(false);
  const selected = selectedRecognizedText(text, selection);
  async function copy(scope: "selected" | "all") {
    if (copying) return;
    setCopying(true);
    try {
      if (await copyRecognizedText(text, selection, scope, write))
        copied(scope);
    } catch (error) {
      failed(error);
    } finally {
      setCopying(false);
    }
  }
  return (
    <section className="ocr-panel">
      <div className="section-heading">
        <h3>Recognized text</h3>
        <div className="ocr-copy-actions">
          <button
            className="button small-button"
            disabled={!selected || copying}
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => void copy("selected")}
          >
            <Copy size={14} />
            Copy selected
          </button>
          <button
            className="button small-button"
            disabled={!text || copying}
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => void copy("all")}
          >
            <Clipboard size={14} />
            Copy all
          </button>
        </div>
      </div>
      <textarea
        aria-label="Recognized text"
        aria-describedby="recognized-text-copy-hint"
        value={text}
        readOnly
        spellCheck={false}
        onSelect={(event) =>
          setSelection({
            source: text,
            start: event.currentTarget.selectionStart,
            end: event.currentTarget.selectionEnd,
          })
        }
        onKeyDown={(event) => {
          if (
            (event.ctrlKey || event.metaKey) &&
            !event.altKey &&
            !event.shiftKey &&
            event.key.toLowerCase() === "c" &&
            selected
          ) {
            event.preventDefault();
            void copy("selected");
          }
        }}
        placeholder="No text found in this image."
      />
      <p className="ocr-copy-hint" id="recognized-text-copy-hint">
        Select a portion to copy it with Copy selected or Ctrl/Cmd+C.
      </p>
    </section>
  );
}
