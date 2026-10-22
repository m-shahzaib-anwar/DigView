import { useRef, type Dispatch, type SetStateAction } from "react";
import { FolderOpen, FolderPlus, X } from "lucide-react";
import {
  setupService,
  type SetupFolder,
  type SetupSuggestion,
} from "./setup-service";
import "./onboarding.css";

export type FolderChoice = SetupFolder & {
  selected: boolean;
  suggestionKey?: string;
};
const errorText = (error: unknown) => String(error).replace(/^Error:\s*/, "");
export const displayPath = (path: string) =>
  path.startsWith("\\\\?\\UNC\\")
    ? "\\\\" + path.slice(8)
    : path.startsWith("\\\\?\\")
      ? path.slice(4)
      : path;

export function FolderSelection({
  suggestions,
  existingPaths,
  folders,
  setFolders,
  busy,
  onBusyChange,
  onError,
}: {
  suggestions: SetupSuggestion[];
  existingPaths: string[];
  folders: FolderChoice[];
  setFolders: Dispatch<SetStateAction<FolderChoice[]>>;
  busy: boolean;
  onBusyChange: (busy: boolean) => void;
  onError: (message: string) => void;
}) {
  const pending = useRef(false);
  async function addFolder() {
    if (pending.current) return;
    pending.current = true;
    onBusyChange(true);
    onError("");
    try {
      const folder = await setupService.chooseFolder();
      if (folder)
        setFolders((prior) =>
          prior.some((f) => f.token === folder.token)
            ? prior.map((f) =>
                f.token === folder.token ? { ...f, selected: true } : f,
              )
            : [...prior, { ...folder, selected: true }],
        );
    } catch (e) {
      onError(errorText(e));
    } finally {
      pending.current = false;
      onBusyChange(false);
    }
  }
  async function toggleSuggestion(
    suggestion: SetupSuggestion,
    checked: boolean,
  ) {
    if (
      pending.current ||
      !suggestion.available ||
      suggestion.covered ||
      suggestion.overlap
    )
      return;
    const chosen = folders.find(
      (f) => f.suggestionKey === suggestion.key || f.path === suggestion.path,
    );
    if (chosen) {
      setFolders((prior) =>
        prior.map((f) =>
          f.token === chosen.token ? { ...f, selected: checked } : f,
        ),
      );
      return;
    }
    if (!checked) return;
    pending.current = true;
    onBusyChange(true);
    onError("");
    try {
      const folder = await setupService.chooseSuggestion(suggestion);
      setFolders((prior) =>
        prior.some((f) => f.token === folder.token)
          ? prior.map((f) =>
              f.token === folder.token ? { ...f, selected: true } : f,
            )
          : [
              ...prior,
              { ...folder, selected: true, suggestionKey: suggestion.key },
            ],
      );
    } catch (e) {
      onError(errorText(e));
    } finally {
      pending.current = false;
      onBusyChange(false);
    }
  }

  return (
    <div className="folder-selection">
      <fieldset className="setup-content" disabled={busy}>
        {existingPaths.length > 0 && (
          <div className="setup-note">
            <div>
              <strong>Already in your library</strong>
              <ul>
                {existingPaths.map((path) => (
                  <li key={path}>{displayPath(path)}</li>
                ))}
              </ul>
              <p>
                These stay selected and continue using your existing indexing
                settings. Manage existing folders in Settings.
              </p>
            </div>
          </div>
        )}
        <div className="setup-folders">
          {suggestions.map((suggestion) => {
            const chosen = folders.find(
              (f) =>
                f.suggestionKey === suggestion.key ||
                f.path === suggestion.path,
            );
            return (
              <div className="setup-folder" key={suggestion.key}>
                <label>
                  <input
                    type="checkbox"
                    checked={suggestion.covered || !!chosen?.selected}
                    disabled={
                      busy ||
                      !suggestion.available ||
                      suggestion.covered ||
                      suggestion.overlap ||
                      (!chosen && folders.length >= 32)
                    }
                    onChange={(event) =>
                      void toggleSuggestion(suggestion, event.target.checked)
                    }
                  />
                  <FolderOpen size={22} />
                  <span>
                    <strong>{suggestion.label}</strong>
                    <small>
                      {displayPath(chosen?.path || suggestion.path) ||
                        "Unavailable on this device"}
                    </small>
                    {suggestion.covered && (
                      <small>Already in your library</small>
                    )}
                    {suggestion.overlap && (
                      <small>
                        Contains an existing library folder. Manage selected
                        folders in Settings.
                      </small>
                    )}
                    {!suggestion.available && suggestion.path && (
                      <small>Folder unavailable</small>
                    )}
                  </span>
                </label>
              </div>
            );
          })}
          {folders
            .filter(
              (f) =>
                !suggestions.some(
                  (s) => s.key === f.suggestionKey || s.path === f.path,
                ),
            )
            .map((f) => (
              <div className="setup-folder" key={f.token}>
                <label>
                  <input
                    type="checkbox"
                    checked={f.selected}
                    disabled={busy}
                    onChange={(e) =>
                      setFolders((prior) =>
                        prior.map((p) =>
                          p.token === f.token
                            ? { ...p, selected: e.target.checked }
                            : p,
                        ),
                      )
                    }
                  />
                  <FolderOpen size={22} />
                  <span>
                    <strong>
                      {f.path.split(/[\\/]/).filter(Boolean).at(-1) || f.path}
                    </strong>
                    <small title={displayPath(f.path)}>
                      {displayPath(f.path)}
                    </small>
                  </span>
                </label>
                <button
                  type="button"
                  aria-label={`Remove ${f.path}`}
                  disabled={busy}
                  onClick={() => {
                    setFolders((prior) =>
                      prior.filter((p) => p.token !== f.token),
                    );
                    void setupService.discardFolders([f.token]).catch(() => {});
                  }}
                >
                  <X size={17} />
                </button>
              </div>
            ))}
        </div>
        {!folders.length && !suggestions.length && (
          <div className="setup-empty">
            <FolderOpen size={30} />
            <strong>Your library starts with your choice.</strong>
            <p>Add a folder, or continue and choose one later.</p>
          </div>
        )}
        <button
          className="setup-add-folder"
          type="button"
          disabled={busy || folders.length >= 32}
          onClick={() => void addFolder()}
        >
          <FolderPlus size={18} />
          Choose custom folder
        </button>
      </fieldset>
    </div>
  );
}
