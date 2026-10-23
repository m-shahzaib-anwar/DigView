import { latestSearchQueue } from "./search-queue";
import { taskPercent } from "./semantic-progress";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { desktop } from "./api";
import type { Query, QueryResult } from "./types";

interface SemanticStatus {
  supported: boolean;
  enabled: boolean;
  installed: boolean;
  busy: boolean;
  indexed: number;
  pending: number;
  download_bytes: number;
  required_bytes: number;
  available_bytes: number | null;
  progress: {
    phase: string;
    downloaded: number;
    processed: number;
    total: number;
    indexed: number;
    skipped: number;
    task: string;
    finished: boolean;
    message: string;
  };
}
export interface SemanticResult {
  result: QueryResult;
  matches?: Record<string, import("./types").MatchPassage>;
  semantic: boolean;
  message: string;
}
const searchQueue = latestSearchQueue();
export function invalidateSemanticQuery() {
  searchQueue.invalidate();
}
export function semanticQuery(
  query: Query,
  broader = false,
): Promise<SemanticResult> {
  return searchQueue.run(() =>
    invoke<SemanticResult>("semantic_query", { query, broader }),
  );
}
const mib = (n: number) => `${Math.ceil(n / 1024 / 1024)} MiB`;

function useSemanticStatus() {
  const [status, setStatus] = useState<SemanticStatus | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    if (!desktop) return;
    let alive = true,
      reading = false;
    const refresh = async () => {
      if (reading) return;
      reading = true;
      try {
        const value = await invoke<SemanticStatus>("semantic_status");
        if (alive) {
          setStatus(value);
          setError("");
        }
      } catch {
        if (alive) setError("Activity unavailable. Trying again…");
      } finally {
        reading = false;
      }
    };
    void refresh();
    const timer = setInterval(() => void refresh(), 1500);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, []);
  return { status, error };
}
function stateLabel(s: SemanticStatus): string {
  if (s.progress.phase === "error")
    return `Needs attention: ${s.progress.message}`;
  if (!s.progress.phase && s.installed && s.enabled)
    return "Checking selected folders…";
  if (s.progress.phase === "downloaded") return "Preparing your files…";
  if (s.progress.phase === "paused") return s.progress.message || "Paused";
  if (s.busy) return s.progress.message || "Working…";
  if (!s.installed) return "Not downloaded";
  if (!s.enabled) return "Paused";
  if (s.progress.phase === "waiting") return "Choose a folder to start";
  if (s.progress.phase === "documents" || s.pending > 0)
    return "Preparing your files…";
  return "Ready to search";
}
export function SemanticSettings({
  onboarding = false,
}: {
  onboarding?: boolean;
}) {
  const { status, error } = useSemanticStatus();
  const [working, setWorking] = useState(false);
  const [actionError, setActionError] = useState("");
  const act = async (action: string) => {
    setWorking(true);
    setActionError("");
    try {
      await invoke("semantic_action", { action });
    } catch (e) {
      setActionError(String(e));
    } finally {
      setWorking(false);
    }
  };
  return (
    <section
      className="settings-section semantic-settings"
      aria-label="AI search"
    >
      <h2>
        AI search <span className="semantic-badge">Optional</span>
      </h2>
      <p>
        Find documents by what you remember about them. Download once; DigView
        prepares your selected folders and keeps them up to date automatically.
      </p>
      <p className="footnote">
        {status ? mib(status.download_bytes) : "556 MiB"} download · 1 GiB free
        space needed · Windows x64. Your files stay on this computer.
      </p>
      {onboarding && (
        <p className="footnote">
          You can skip this. Regular search works without it.
        </p>
      )}
      <p role="status">
        {status
          ? stateLabel(status)
          : desktop
            ? "Checking model…"
            : "Available in the desktop app"}
      </p>
      {status && (
        <div className="document-index-actions">
          {!status.installed && (
            <button
              className="button"
              type="button"
              disabled={
                working ||
                status.busy ||
                !status.supported ||
                (status.available_bytes !== null &&
                  status.available_bytes < status.required_bytes)
              }
              onClick={() => void act("download")}
            >
              {status.progress.phase === "error"
                ? "Retry download"
                : "Download AI search"}
            </button>
          )}
          {status.installed && (
            <button
              className="button"
              type="button"
              disabled={working || (!status.enabled && status.busy)}
              onClick={() => void act(status.enabled ? "pause" : "enable")}
            >
              {status.enabled ? "Pause" : "Resume"}
            </button>
          )}
          {status.busy && !status.installed && (
            <button
              className="button"
              type="button"
              disabled={working}
              onClick={() => void act("pause")}
            >
              Pause download
            </button>
          )}
          {status.installed && status.progress.phase === "error" && (
            <button
              className="button"
              type="button"
              disabled={working || status.busy}
              onClick={() => void act("enable")}
            >
              Retry
            </button>
          )}
          {status.installed && (
            <button
              className="button"
              type="button"
              disabled={working || status.busy}
              onClick={() => void act("remove")}
            >
              Remove model
            </button>
          )}
        </div>
      )}
      {(actionError || error) && <p role="alert">{actionError || error}</p>}
      <details>
        <summary>Details</summary>
        <p>
          EmbeddingGemma 2 from Google DeepMind searches text in PDFs, DOCX,
          text and supported code files. For example, words about getting your
          money back may find a refund policy. Images, audio and video are
          excluded. Some long, scanned, encrypted or unreadable documents may be
          skipped or only partly covered.
        </p>
        <p>
          After download, document reading and AI preparation run automatically.
          Use the normal search bar. Exact matches stay available while AI
          prepares files or is paused. Related results may not be relevant; open
          the document to check.
        </p>
        <p>
          Common Urdu and Roman Urdu search words have local keyword assistance.
          Unfamiliar wording may work better in English. This is not full
          translation.
        </p>
        <p>
          Progress and current tasks appear in Activity at the bottom. Pause
          keeps completed work; Resume continues automatically. Pausing a
          download may require downloading the incomplete part again. Remove
          deletes the model and its AI index; your original files and regular
          document search stay.
        </p>
        <p>
          Download connects to Hugging Face and GitHub. After installation, AI
          processing works offline on your processor. No document contents or
          search words are uploaded. Keep DigView running for background
          updates.
        </p>
        {status?.available_bytes !== null &&
          status?.available_bytes !== undefined && (
            <p>
              Available disk space:{" "}
              {(status.available_bytes / 1_000_000_000).toFixed(2)} GB
            </p>
          )}
        <p>
          Model: ggml-org text conversion of EmbeddingGemma 2 (Apache 2.0).
          Engine: llama.cpp (MIT).{" "}
          <a
            href="https://ai.google.dev/gemma/docs/embeddinggemma/model_card_2"
            target="_blank"
            rel="noreferrer"
          >
            Model details
          </a>
        </p>
      </details>
    </section>
  );
}
export function ActivityBar({
  job,
  ocr,
  updatesBusy,
}: {
  job: import("./types").Job | null;
  ocr: boolean;
  updatesBusy: boolean;
}) {
  const { status, error } = useSemanticStatus();
  const [notice, setNotice] = useState("");
  const act = async (action: string) => {
    try {
      await invoke("semantic_action", { action });
      setNotice("");
    } catch (e) {
      setNotice(String(e));
    }
  };
  const tasks: Array<{ title: string; message: string; percent?: number }> = [];
  if (job?.running)
    tasks.push({
      title:
        job.kind === "content"
          ? "Document indexing"
          : job.kind === "duplicates"
            ? "Duplicate review"
            : job.kind === "update"
              ? "App update"
              : "Library",
      message: `${job.message} · ${job.processed} processed`,
    });
  if (
    status &&
    (status.installed ||
      status.busy ||
      status.progress.phase === "error" ||
      status.progress.phase === "paused")
  ) {
    const progress = status.progress;
    const percent =
      status.busy && progress.task === "download"
        ? taskPercent(
            progress.downloaded,
            status.download_bytes,
            progress.finished,
          )
        : progress.phase === "indexing"
          ? taskPercent(progress.processed, progress.total, progress.finished)
          : undefined;
    tasks.push({ title: "AI search", message: stateLabel(status), percent });
  }
  if (ocr) tasks.push({ title: "Image text", message: "Recognizing text…" });
  if (updatesBusy) tasks.push({ title: "File action", message: "Working…" });
  if (job?.watch_error)
    tasks.push({ title: "Folder watcher", message: job.watch_error });
  if (job && !job.running && job.errors)
    tasks.push({
      title: "Last task",
      message: `${job.errors} files need attention`,
    });
  return (
    <nav className="activity-bar" aria-label="Activity">
      <strong className="activity-heading">Activity</strong>
      <div className="activity-tasks" role="status" aria-live="polite">
        {tasks.length ? (
          tasks.map((task, i) => (
            <div className="activity-task" key={`${task.title}-${i}`}>
              <strong>{task.title}</strong>
              <span title={task.message}>{task.message}</span>
              {task.percent !== undefined && (
                <>
                  <progress
                    max={100}
                    value={task.percent}
                    aria-label={`${task.title} progress`}
                  />
                  <b>{task.percent}%</b>
                </>
              )}
            </div>
          ))
        ) : (
          <span>{desktop ? "No active tasks" : "Browser preview"}</span>
        )}
        {(notice || error) && <span role="alert">{notice || error}</span>}
      </div>
      {status && (status.installed || status.busy) && (
        <button
          className="button"
          type="button"
          disabled={!status.enabled && status.busy && status.installed}
          onClick={() =>
            void act(
              status.enabled || status.busy
                ? "pause"
                : status.installed
                  ? "enable"
                  : "download",
            )
          }
        >
          {status.enabled || status.busy ? "Pause AI" : "Resume AI"}
        </button>
      )}
    </nav>
  );
}
