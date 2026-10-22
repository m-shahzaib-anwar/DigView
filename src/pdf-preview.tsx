import { useEffect, useRef, useState } from "react";
import { ChevronLeft, ChevronRight, Loader2, Minus, Plus } from "lucide-react";
import type {
  PDFDocumentProxy,
  PDFDocumentLoadingTask,
  RenderTask,
} from "pdfjs-dist";
import workerUrl from "pdfjs-dist/legacy/build/pdf.worker.mjs?url";
import { localDiagnostics } from "./local-diagnostics";

export function PdfPreview({ url }: { url: string }) {
  const [pdf, setPdf] = useState<PDFDocumentProxy | null>(null);
  const [page, setPage] = useState(1);
  const [zoom, setZoom] = useState(1);
  const [width, setWidth] = useState(480);
  const [error, setError] = useState("");
  const [rendering, setRendering] = useState(false);
  const [password, setPassword] = useState("");
  const [passwordPrompt, setPasswordPrompt] = useState("");
  const unlock = useRef<((password: string) => void) | null>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const viewport = useRef<HTMLDivElement>(null);
  const renderQueue = useRef<Promise<unknown>>(Promise.resolve());
  useEffect(() => {
    let alive = true;
    let task: PDFDocumentLoadingTask | undefined;
    let worker: Worker | undefined;
    let pdfWorker: { destroy: () => void } | undefined;
    const controller = new AbortController();
    void (async () => {
      const library = await import("pdfjs-dist/legacy/build/pdf.mjs");
      if (!alive) return;
      const response = await fetch(url, {
        signal: controller.signal,
        cache: "no-store",
      });
      if (!response.ok || response.status === 206)
        throw Error(
          "The PDF could not be loaded. Reopen it or refresh the library.",
        );
      const bytes = await response.arrayBuffer();
      if (!alive) return;
      if (bytes.byteLength > 40 * 1024 * 1024)
        throw Error("This PDF exceeds the 40 MB preview limit.");
      worker = new Worker(workerUrl, { type: "module" });
      const localWorker = library.PDFWorker.create({ port: worker });
      pdfWorker = localWorker;
      const assets = new URL("pdf/", document.baseURI).href;
      task = library.getDocument({
        data: new Uint8Array(bytes),
        worker: localWorker,
        cMapUrl: `${assets}cmaps/`,
        cMapPacked: true,
        standardFontDataUrl: `${assets}standard_fonts/`,
        wasmUrl: `${assets}wasm/`,
        iccUrl: `${assets}iccs/`,
        enableXfa: false,
        maxImageSize: 16_000_000,
        canvasMaxAreaInBytes: 32 * 1024 * 1024,
      });
      task.onPassword = (
        update: (password: string) => void,
        reason: number,
      ) => {
        if (alive) {
          unlock.current = update;
          setPasswordPrompt(
            reason === 2
              ? "Incorrect password. Try again."
              : "This PDF needs a password.",
          );
        }
      };
      worker.onerror = () => {
        if (alive) {
          setError(
            "The local PDF worker could not start. Reopen the preview or update WebView2.",
          );
          void task?.destroy().catch(() => {});
        }
      };
      const pdfDocument = await task.promise;
      if (!alive) {
        await task.destroy();
        return;
      }
      setPdf(pdfDocument);
      setPasswordPrompt("");
      setPassword("");
      unlock.current = null;
    })().catch((reason) => {
      if (alive) {
        localDiagnostics.error("preview_failed", reason, "load_failed");
        setError(String(reason));
      }
    });
    return () => {
      alive = false;
      controller.abort();
      unlock.current = null;
      void task?.destroy().catch(() => {});
      pdfWorker?.destroy();
      worker?.terminate();
    };
  }, [url]);
  useEffect(() => {
    if (!viewport.current) return;
    const observer = new ResizeObserver((entries) =>
      setWidth(entries[0].contentRect.width),
    );
    observer.observe(viewport.current);
    return () => observer.disconnect();
  }, [pdf]);
  useEffect(() => {
    if (!pdf) return;
    let alive = true;
    let task: RenderTask | undefined;
    setRendering(true);
    renderQueue.current = renderQueue.current
      .catch(() => {})
      .then(async () => {
        const content = await pdf.getPage(page);
        if (!alive || !canvas.current) return;
        const base = content.getViewport({ scale: 1 });
        const scale = Math.max(
          0.1,
          (Math.max(120, width - 32) / base.width) * zoom,
        );
        const view = content.getViewport({ scale });
        const ratio = Math.min(
          window.devicePixelRatio || 1,
          2,
          Math.sqrt(8_000_000 / (view.width * view.height)),
          4096 / Math.max(view.width, view.height),
        );
        const element = canvas.current;
        element.width = Math.max(1, Math.round(view.width * ratio));
        element.height = Math.max(1, Math.round(view.height * ratio));
        element.style.width = `${view.width}px`;
        element.style.height = `${view.height}px`;
        task = content.render({
          canvas: element,
          viewport: view,
          transform: [ratio, 0, 0, ratio, 0, 0],
          background: "white",
        });
        await task.promise;
        content.cleanup();
      })
      .catch((reason) => {
        if (alive && reason?.name !== "RenderingCancelledException")
          setError(
            "This PDF page could not be rendered. Use Open original to view it.",
          );
      })
      .finally(() => {
        if (alive) setRendering(false);
      });
    return () => {
      alive = false;
      task?.cancel();
    };
  }, [pdf, page, zoom, width]);
  if (error)
    return (
      <div className="inline-preview-message" role="alert">
        <strong>PDF preview unavailable</strong>
        <span>{error}</span>
        <span>Use Open original to view this file.</span>
      </div>
    );
  if (passwordPrompt)
    return (
      <form
        className="inline-preview-message pdf-password"
        onSubmit={(event) => {
          event.preventDefault();
          unlock.current?.(password);
          setPassword("");
        }}
      >
        <label htmlFor="pdf-password">{passwordPrompt}</label>
        <input
          id="pdf-password"
          type="password"
          value={password}
          onChange={(event) => setPassword(event.target.value)}
          autoComplete="off"
        />
        <button className="button primary" disabled={!password}>
          Unlock PDF
        </button>
      </form>
    );
  if (!pdf)
    return (
      <div className="inline-preview-message" role="status">
        <Loader2 className="spinning" />
        Opening PDF locally…
      </div>
    );
  return (
    <div className="pdf-preview">
      <div className="inline-preview-toolbar">
        <div>
          <button
            className="icon-button"
            aria-label="Previous PDF page"
            disabled={page <= 1}
            onClick={() => setPage((value) => value - 1)}
          >
            <ChevronLeft size={17} />
          </button>
          <span>
            Page {page} / {pdf.numPages}
          </span>
          <button
            className="icon-button"
            aria-label="Next PDF page"
            disabled={page >= pdf.numPages}
            onClick={() => setPage((value) => value + 1)}
          >
            <ChevronRight size={17} />
          </button>
        </div>
        <div>
          <button
            className="icon-button"
            aria-label="Zoom PDF out"
            disabled={zoom <= 0.5}
            onClick={() => setZoom((value) => Math.max(0.5, value - 0.25))}
          >
            <Minus size={16} />
          </button>
          <button
            className="text-button"
            onClick={() => setZoom(1)}
            aria-label="Fit PDF to width"
          >
            {Math.round(zoom * 100)}%
          </button>
          <button
            className="icon-button"
            aria-label="Zoom PDF in"
            disabled={zoom >= 2.5}
            onClick={() => setZoom((value) => Math.min(2.5, value + 0.25))}
          >
            <Plus size={16} />
          </button>
        </div>
      </div>
      <div
        className="pdf-viewport"
        ref={viewport}
        tabIndex={0}
        aria-label={`PDF page ${page} of ${pdf.numPages}`}
      >
        <canvas ref={canvas} role="img" aria-label={`PDF page ${page}`} />
        {rendering && (
          <span className="pdf-rendering" role="status">
            Rendering…
          </span>
        )}
      </div>
    </div>
  );
}
