import { useEffect, useRef, useState } from "react";
import { FolderPlus, LoaderCircle } from "lucide-react";
import { desktop } from "./api";
import { FolderSelection, type FolderChoice } from "./folder-selection";
import { Modal } from "./modal";
import { setupService, type SetupInitial } from "./setup-service";
import "./add-folders-dialog.css";

const errorText = (error: unknown) => String(error).replace(/^Error:\s*/, "");

export function AddFoldersDialog({
  onClose,
  onAdded,
  embedded = false,
}: {
  embedded?: boolean;
  onClose: () => void;
  onAdded: (paths: string[], warning: string) => void | Promise<void>;
}) {
  const heading = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    if (embedded) heading.current?.focus();
  }, [embedded]);
  const [initial, setInitial] = useState<SetupInitial | null>(null);
  const [folders, setFolders] = useState<FolderChoice[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [attempt, setAttempt] = useState(0);
  const [loading, setLoading] = useState(true);
  const pending = useRef(false);
  const draft = useRef<FolderChoice[]>([]);
  draft.current = folders;
  const selected = folders.filter((folder) => folder.selected);
  useEffect(() => {
    let alive = true;
    setLoading(true);
    setError("");
    void setupService
      .initial()
      .then((next) => {
        if (alive) setInitial(next);
      })
      .catch((e) => {
        if (alive) setError(errorText(e));
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [attempt]);
  useEffect(
    () => () => {
      void setupService
        .discardFolders(draft.current.map((folder) => folder.token))
        .catch(() => {});
    },
    [],
  );

  async function confirm() {
    if (pending.current || !selected.length) return;
    pending.current = true;
    setBusy(true);
    setError("");
    let added: string[];
    try {
      added = await setupService.addFolders(
        selected.map((folder) => folder.token),
      );
    } catch (e) {
      setError(errorText(e));
      pending.current = false;
      setBusy(false);
      return;
    }
    // Saved roots are durable. A watcher race or refresh failure must not resubmit them.
    let warning = "";
    try {
      await setupService.scan();
    } catch {
      warning =
        "Folders are saved. Use Refresh in the library if indexing has not started.";
    }
    try {
      await onAdded(added, warning);
    } catch {
      /* Roots are saved; opening the library can refresh its display. */
    } finally {
      pending.current = false;
      onClose();
    }
  }

  const content = (
    <div className="add-folders-dialog" aria-busy={busy || loading}>
      <p className="add-folders-intro">
        Choose suggested locations or a custom folder. New choices are added
        only when you confirm.
      </p>
      {!desktop && (
        <p className="setup-preview">
          Browser preview / example folders only. No real files are indexed.
        </p>
      )}
      {loading ? (
        <p role="status">
          <LoaderCircle size={17} className="setup-spin" /> Loading folders...
        </p>
      ) : (
        initial && (
          <FolderSelection
            suggestions={initial.suggestions}
            existingPaths={initial.paths}
            folders={folders}
            setFolders={setFolders}
            busy={busy}
            onBusyChange={(value) => {
              pending.current = value;
              setBusy(value);
            }}
            onError={setError}
          />
        )
      )}
      {error && (
        <div className="setup-error" role="alert">
          {error}
        </div>
      )}
      {!loading && !initial && (
        <button className="button" onClick={() => setAttempt((n) => n + 1)}>
          Retry
        </button>
      )}
      <div className="add-folders-footer">
        <button className="button" disabled={busy} onClick={onClose}>
          Cancel
        </button>
        <button
          className="button primary"
          disabled={busy || loading || !initial || !selected.length}
          onClick={() => void confirm()}
        >
          {busy ? (
            <LoaderCircle size={16} className="setup-spin" />
          ) : (
            <FolderPlus size={16} />
          )}
          {busy
            ? "Adding..."
            : `Add ${selected.length || "selected"} folder${selected.length === 1 ? "" : "s"}`}
        </button>
      </div>
    </div>
  );
  if (embedded)
    return (
      <section className="add-folders-page" aria-label="Choose folders">
        <h1 ref={heading} tabIndex={-1}>
          Where should DigView look?
        </h1>
        {content}
      </section>
    );
  return (
    <Modal
      title="Where should DigView look?"
      wide
      closeDisabled={busy}
      onClose={() => {
        if (!pending.current) onClose();
      }}
    >
      {content}
    </Modal>
  );
}
