import { useEffect, useState } from "react";
import { FileSearch, Loader2 } from "lucide-react";
import { api, desktop } from "./api";
import type { Job } from "./types";

export function DocumentIndexPanel({
  job,
  disabled,
  changed,
  notify,
}: {
  job: Job | null;
  disabled: boolean;
  changed: () => Promise<void>;
  notify: (text: string, error?: boolean) => void;
}) {
  const [status, setStatus] = useState<Awaited<
    ReturnType<typeof api.documentStatus>
  > | null>(null);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  const [retry, setRetry] = useState(0);
  useEffect(() => {
    if (!desktop) return;
    let alive = true,
      fetching = false;
    const refresh = async () => {
      if (fetching) return;
      fetching = true;
      try {
        const status = await api.documentStatus();
        if (alive) {
          setStatus(status);
          setError("");
        }
      } catch (e) {
        if (alive) setError(String(e));
      } finally {
        fetching = false;
      }
    };
    void refresh();
    const timer = setInterval(() => void refresh(), 2500);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [retry]);
  const act = async (action: () => Promise<unknown>, message: string) => {
    setPending(true);
    try {
      await action();
      setStatus(await api.documentStatus());
      notify(message);
      await changed();
    } catch (e) {
      notify(e instanceof Error ? e.message : String(e), true);
    } finally {
      setPending(false);
    }
  };
  const running = job?.running && job.kind === "content";
  const locked = disabled || pending || !!job?.running || !desktop;
  return (
    <section className="settings-section document-index-settings">
      <div className="section-heading">
        <div>
          <h2>
            <FileSearch size={18} /> Search inside documents
          </h2>
          <p>
            Find words inside PDF, DOCX, TXT and code files in your selected
            folders. Processing and search stay on this device.
          </p>
        </div>
      </div>
      <p>
        Enable once to index existing documents, then update changed files
        automatically while the app runs. Filename search keeps working while
        content indexing is off.
      </p>
      {!desktop ? (
        <p className="settings-empty">Available in the desktop app.</p>
      ) : error ? (
        <div role="alert">
          <p>{error}</p>
          <button className="button" onClick={() => setRetry((n) => n + 1)}>
            Retry status
          </button>
        </div>
      ) : !status ? (
        <p role="status">Loading document index…</p>
      ) : (
        <>
          <div className="document-index-counts" aria-live="polite">
            <span>
              <strong>{status.indexed}</strong> indexed
            </span>
            <span>
              <strong>{status.partial}</strong> partial
            </span>
            <span>
              <strong>{status.pending}</strong> pending
            </span>
            <span>
              <strong>{status.empty}</strong> no text
            </span>
            <span>
              <strong>{status.failed}</strong> skipped
            </span>
          </div>
          {running && (
            <p role="status">
              <Loader2 className="spinning" size={14} /> {job.message}
            </p>
          )}
          <div className="document-index-actions">
            <button
              className="button"
              disabled={
                pending ||
                !desktop ||
                (status.enabled ? disabled && !running : locked)
              }
              onClick={() =>
                void act(
                  () => api.enableDocumentIndex(!status.enabled),
                  status.enabled
                    ? "Automatic content indexing stopped. Existing searchable text kept."
                    : "Document content indexing enabled.",
                )
              }
            >
              {status.enabled
                ? "Stop automatic indexing"
                : "Enable content indexing"}
            </button>
            <button
              className="button"
              disabled={locked || !status.enabled}
              onClick={() =>
                void act(
                  () => api.indexDocuments(true),
                  "Pending and skipped documents queued for indexing.",
                )
              }
            >
              Index now / retry skipped
            </button>
            {running && (
              <button
                className="button"
                disabled={pending}
                onClick={() =>
                  void act(
                    api.cancel,
                    "Stopping document indexing. Saved results kept.",
                  )
                }
              >
                Cancel indexing
              </button>
            )}
            <button
              className="button"
              disabled={locked}
              onClick={() =>
                void act(
                  api.clearDocumentIndex,
                  "Document text index cleared; automatic indexing stopped. Originals kept.",
                )
              }
            >
              Clear document text index
            </button>
          </div>
        </>
      )}
      <p className="footnote">
        Up to 16 MB per file, 200 PDF pages, and 512 KB of extracted text. Large
        text/code files use the first 2 MB. Partial results are marked above.
        Scanned PDFs need OCR; password-protected/damaged documents, legacy .doc
        and unsupported encodings are not indexed. Hidden or excluded files do
        not appear in content search. This index stays on this device.
      </p>
    </section>
  );
}
