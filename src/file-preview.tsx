import { useEffect, useRef, useState, type ReactNode } from "react";
import { Loader2, Music2 } from "lucide-react";
import { api, desktop } from "./api";
import type { FileRecord } from "./types";
import {
  previewKind,
  validPreview,
  type ContentPreview,
} from "./content-preview-types";
import { PdfPreview } from "./pdf-preview";
import { localDiagnostics } from "./local-diagnostics";

export function FilePreview({
  file,
  fallback,
}: {
  file: FileRecord;
  fallback: ReactNode;
}) {
  const kind = previewKind(file.extension, file.name);
  const [info, setInfo] = useState<ContentPreview | null>(null);
  const [error, setError] = useState("");
  const [wrap, setWrap] = useState(true);
  const media = useRef<HTMLMediaElement | null>(null);
  const active = useRef(true);
  useEffect(() => {
    if (!kind || !desktop) return;
    let alive = true;
    active.current = true;
    let token: string | null = null;
    setInfo(null);
    setError("");
    void api
      .openPreview(file.id)
      .then((result) => {
        token = result.token;
        if (!alive) {
          if (token) void api.closePreview(token).catch(() => {});
          return;
        }
        if (!validPreview(result, kind))
          throw Error("The preview response was invalid. Reopen the file.");
        setInfo(result);
      })
      .catch((reason) => {
        if (alive) {
          localDiagnostics.error("preview_failed", reason, "load_failed");
          setError(String(reason));
        }
      });
    return () => {
      alive = false;
      active.current = false;
      const element = media.current;
      if (element) {
        element.pause();
        element.removeAttribute("src");
        element.load();
      }
      if (token) void api.closePreview(token).catch(() => {});
    };
  }, [file.id, kind]);
  if (!kind) return <>{fallback}</>;
  if (!desktop)
    return (
      <div className="inline-preview-message">
        Open the desktop app to preview files from your library.
      </div>
    );
  if (error)
    return (
      <div className="inline-preview-message" role="alert">
        <strong>Preview unavailable</strong>
        <span>{error}</span>
        <span>Use Open original to view this file in its app.</span>
      </div>
    );
  if (!info)
    return (
      <div className="inline-preview-message" role="status">
        <Loader2 className="spinning" size={24} />
        Loading local preview…
      </div>
    );
  if (info.kind === "text")
    return (
      <div className="text-preview">
        <div className="inline-preview-toolbar">
          <span>{info.encoding} · Read only</span>
          <label>
            <input
              type="checkbox"
              checked={wrap}
              onChange={(event) => setWrap(event.target.checked)}
            />
            Wrap lines
          </label>
        </div>
        <pre
          className={wrap ? "wrap-text" : ""}
          tabIndex={0}
          aria-label={`Text preview of ${file.name}`}
        >
          {info.text || "(Empty file)"}
        </pre>
        {info.truncated && (
          <p className="inline-preview-note">
            Showing the first 256 KB. Open original to read the full file.
          </p>
        )}
      </div>
    );
  const url = api.previewUrl(info.token!);
  if (info.kind === "pdf") return <PdfPreview url={url} />;
  const mediaError = () => {
    if (!active.current) return;
    localDiagnostics.record({ event: "preview_failed", code: "decode_failed" });
    setError(
      "This media format or codec could not be played by your system. The file may also have changed or become unavailable.",
    );
  };
  return (
    <div className="media-preview">
      {info.kind === "video" ? (
        <video
          ref={(element) => {
            if (element) media.current = element;
          }}
          controls
          preload="metadata"
          src={url}
          onError={mediaError}
          aria-label={`Video preview of ${file.name}`}
        />
      ) : (
        <>
          <div className="audio-preview-art">
            <Music2 size={64} strokeWidth={1.4} />
            <span>{file.extension.toUpperCase()} audio</span>
          </div>
          <audio
            ref={(element) => {
              if (element) media.current = element;
            }}
            controls
            preload="metadata"
            src={url}
            onError={mediaError}
            aria-label={`Audio preview of ${file.name}`}
          />
        </>
      )}
      <p className="inline-preview-note">
        Local playback · Codec support depends on your system
      </p>
    </div>
  );
}
