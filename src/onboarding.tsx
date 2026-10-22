import { FolderSelection, type FolderChoice } from "./folder-selection";
import { AddFoldersDialog } from "./add-folders-dialog";
import { useEffect, useRef, useState, type ReactNode } from "react";
import {
  ArrowLeft,
  ArrowRight,
  Check,
  CheckCircle2,
  FileSearch,
  FolderPlus,
  Image,
  LoaderCircle,
  Monitor,
  Moon,
  Search,
  ShieldCheck,
  SlidersHorizontal,
  Sun,
} from "lucide-react";
import { BrandIcon, BrandWordmark } from "./brand";
import { api, desktop } from "./api";
import { useTheme } from "./theme";
import { LocalDiagnosticsPanel } from "./local-diagnostics-panel";
import {
  completeSetup,
  setupService,
  type BackgroundChoices,
  type SetupInitial,
} from "./setup-service";
import "./onboarding.css";

const steps = ["Welcome", "Folders", "Search", "Preferences"];
const errorText = (error: unknown) => String(error).replace(/^Error:\s*/, "");

export function SetupGate({ children }: { children: ReactNode }) {
  const preview =
    !desktop &&
    new URLSearchParams(window.location.search).get("setup") === "1";
  const [state, setState] = useState<"loading" | "setup" | "done" | "error">(
    !desktop && !preview ? "done" : "loading",
  );
  const [background, setBackground] = useState<BackgroundChoices | null>(null);
  const [initial, setInitial] = useState<SetupInitial | null>(null);
  const [error, setError] = useState("");
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    if (!desktop && !preview) return;
    let alive = true;
    setState("loading");
    setError("");
    void setupService
      .status()
      .then(async (status) => {
        if (!alive) return;
        if (status.completed) {
          setState("done");
          return;
        }
        const [preferences, existing] = await Promise.all([
          setupService.background(),
          setupService.initial(),
        ]);
        if (alive) {
          setBackground(preferences);
          setInitial(existing);
          setState("setup");
        }
      })
      .catch((e) => {
        if (alive) {
          setError(errorText(e));
          setState("error");
        }
      });
    return () => {
      alive = false;
    };
  }, [attempt, preview]);
  if (state === "done") return children;
  if (state === "setup" && background && initial)
    return (
      <Onboarding
        initialBackground={background}
        initial={initial}
        preview={preview}
        finished={() => setState("done")}
      />
    );
  return (
    <main className="setup-shell">
      <section
        className="setup-card setup-loading"
        aria-busy={state === "loading"}
      >
        <BrandWordmark />
        {state === "loading" ? (
          <>
            <LoaderCircle className="setup-spin" size={28} />
            <p role="status">Opening your workspace...</p>
          </>
        ) : (
          <>
            <h1>We couldn't open your setup</h1>
            <p role="alert">{error}</p>
            <button
              className="setup-primary"
              onClick={() => setAttempt((n) => n + 1)}
            >
              Try again
            </button>
            <LocalDiagnosticsPanel />
          </>
        )}
      </section>
    </main>
  );
}

function Onboarding({
  initialBackground,
  initial,
  preview,
  finished,
}: {
  initialBackground: BackgroundChoices;
  initial: SetupInitial;
  preview: boolean;
  finished: () => void;
}) {
  const [step, setStep] = useState(0);
  const [folders, setFolders] = useState<FolderChoice[]>([]);
  const [folderPicker, setFolderPicker] = useState(false);
  const [documentContent, setDocumentContent] = useState(
    initial.documentContent,
  );
  const [exclusions, setExclusions] = useState(initial.exclusions.join("\n"));
  const [lowResource, setLowResource] = useState(initial.lowResource);
  const [pauseOnBattery, setPauseOnBattery] = useState(initial.pauseOnBattery);
  const [background, setBackground] = useState(initialBackground);
  const theme = useTheme();
  const [busy, setBusy] = useState(false);
  const running = useRef(false);
  const [error, setError] = useState("");
  const [ready, setReady] = useState(false);
  const [warning, setWarning] = useState("");
  const [openingLibrary, setOpeningLibrary] = useState(false);
  const heading = useRef<HTMLHeadingElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  const selected = folders.filter((f) => f.selected);
  useEffect(() => {
    heading.current?.focus();
    panel.current?.scrollTo({ top: 0 });
  }, [step, ready, folderPicker, openingLibrary]);
  async function finish(skip = false) {
    if (running.current) return;
    running.current = true;
    setBusy(true);
    setError("");
    try {
      const result = await completeSetup(setupService, {
        folders: skip ? [] : selected.map((f) => f.token),
        documentContent: skip ? initial.documentContent : documentContent,
        exclusions: skip ? initial.exclusions : exclusions.split(/\r?\n/),
        lowResource: skip ? initial.lowResource : lowResource,
        pauseOnBattery: skip ? initial.pauseOnBattery : pauseOnBattery,
        background: skip ? initialBackground : background,
      });
      if (skip) setFolders([]);
      setWarning(result.scanWarning);
      if (initial.paths.length || (!skip && selected.length)) {
        setOpeningLibrary(true);
        setupService.setLibraryNotice(result.scanWarning);
        await setupService.openLibrary();
        finished();
      } else {
        setReady(true);
      }
    } catch (e) {
      setError(errorText(e));
    } finally {
      running.current = false;
      setBusy(false);
    }
  }
  async function openLibrary() {
    if (running.current) return;
    running.current = true;
    setBusy(true);
    setError("");
    try {
      await setupService.openLibrary();
      finished();
    } catch (e) {
      setError(errorText(e));
    } finally {
      running.current = false;
      setBusy(false);
    }
  }
  function next() {
    setError("");
    setStep((s) => Math.min(steps.length - 1, s + 1));
  }
  return (
    <main className="setup-shell">
      <section
        className="setup-card"
        aria-label="Set up DigView"
        aria-busy={busy}
      >
        <header className="setup-top">
          <BrandWordmark />
          <span className="setup-kicker">YOUR FILES, YOUR DEVICE</span>
        </header>
        {preview && (
          <div className="setup-preview">
            Browser preview / example folders only. Nothing is saved, installed
            or indexed.
          </div>
        )}
        {!ready && !openingLibrary && (
          <ol className="setup-steps" aria-label="Setup progress">
            {steps.map((label, i) => (
              <li
                key={label}
                aria-current={i === step ? "step" : undefined}
                className={i < step ? "complete" : i === step ? "active" : ""}
              >
                <span>{i < step ? <Check size={13} /> : i + 1}</span>
                <small>{label}</small>
              </li>
            ))}
          </ol>
        )}
        <div className="setup-body" ref={panel}>
          <fieldset className="setup-content" disabled={busy}>
            {openingLibrary ? (
              <div className="setup-opening">
                <LoaderCircle size={30} className={busy ? "setup-spin" : ""} />
                <h1 ref={heading} tabIndex={-1}>
                  {busy ? "Opening your library..." : "Your setup is saved"}
                </h1>
                <p className="setup-lead">
                  {busy
                    ? "Your selected folders will update in the library."
                    : "Try opening the library again below."}
                </p>
              </div>
            ) : folderPicker ? (
              <AddFoldersDialog
                embedded
                onClose={() => setFolderPicker(false)}
                onAdded={async (_paths, scanWarning) => {
                  setWarning(scanWarning);
                  setupService.setLibraryNotice(scanWarning);
                  setOpeningLibrary(true);
                  await openLibrary();
                }}
              />
            ) : ready ? (
              <div className="setup-ready">
                <div className="setup-ready-mark">
                  <CheckCircle2 size={42} />
                </div>
                <h1 ref={heading} tabIndex={-1}>
                  {preview ? "Preview complete" : "You're all set"}
                </h1>
                <p>Make a little space for clarity.</p>
                <div className="setup-summary">
                  <strong>Start with an empty library</strong>
                  <p>
                    Choose folders now, or open your library and add them later.
                  </p>
                </div>
                <button
                  type="button"
                  className="setup-add-folder"
                  disabled={busy}
                  onClick={() => setFolderPicker(true)}
                >
                  <FolderPlus size={18} /> Add folders
                </button>
                {warning && (
                  <p className="setup-warning" role="status">
                    {warning}
                  </p>
                )}
                <p className="setup-footnote">
                  Your originals stay in place. You can change these choices in
                  Settings.
                </p>
              </div>
            ) : (
              <>
                {step === 0 && (
                  <div className="setup-welcome">
                    <div className="setup-hero-mark">
                      <BrandIcon />
                    </div>
                    <h1 ref={heading} tabIndex={-1}>
                      A clearer view of your files.
                    </h1>
                    <p className="setup-lead">
                      Find what you need. See what you have.
                      <br />
                      Keep everything close.
                    </p>
                    {initial.paths.length > 0 && (
                      <p className="setup-footnote">
                        Your existing folders and preferences are kept. You can
                        adjust your preferences here.
                      </p>
                    )}
                    <div className="setup-features">
                      {[
                        [
                          Search,
                          "Find files quickly",
                          "Search names, types, dates and sizes.",
                        ],
                        [
                          Image,
                          "Preview before opening",
                          "View images, documents and supported media.",
                        ],
                        [
                          FileSearch,
                          "Search inside documents",
                          "Optionally index text from PDFs, DOCX and more.",
                        ],
                      ].map(([Icon, title, copy]) => {
                        const Glyph = Icon as typeof Search;
                        return (
                          <div key={String(title)}>
                            <span>
                              <Glyph size={22} />
                            </span>
                            <section>
                              <strong>{String(title)}</strong>
                              <p>{String(copy)}</p>
                            </section>
                          </div>
                        );
                      })}
                    </div>
                    <div className="setup-privacy">
                      <ShieldCheck size={18} />
                      <p>
                        Local by design. No account needed.
                        <br />
                        Your file contents stay on this computer.
                      </p>
                    </div>
                  </div>
                )}
                {step === 1 && (
                  <>
                    <h1 ref={heading} tabIndex={-1}>
                      Where should DigView look?
                    </h1>
                    <p className="setup-lead">
                      Choose new folders for your library. New choices are not
                      indexed until you finish setup.
                    </p>
                    <FolderSelection
                      suggestions={initial.suggestions}
                      existingPaths={initial.paths}
                      folders={folders}
                      setFolders={setFolders}
                      busy={busy}
                      onBusyChange={(value) => {
                        running.current = value;
                        setBusy(value);
                      }}
                      onError={setError}
                    />
                    <details className="setup-exclusions">
                      <summary>Skip subfolders or file types</summary>
                      <label>
                        One name, relative path or file extension per line
                        <textarea
                          value={exclusions}
                          maxLength={51200}
                          rows={3}
                          placeholder={"Private\nOld projects/Archive\n*.log"}
                          onChange={(e) => setExclusions(e.target.value)}
                        />
                      </label>
                      <p className="setup-footnote">
                        Rules apply inside selected folders. System and common
                        cache folders are already excluded. Choose a parent
                        folder or its subfolder, not both.
                      </p>
                    </details>
                  </>
                )}
                {step === 2 && (
                  <>
                    <h1 ref={heading} tabIndex={-1}>
                      How would you like to search?
                    </h1>
                    <p className="setup-lead">
                      Start simple. Add document text when it's useful to you.
                    </p>
                    <div className="setup-option setup-always">
                      <Search size={23} />
                      <span>
                        <strong>File names & filters</strong>
                        <small>
                          Names, types, dates, sizes and your saved filters.
                        </small>
                      </span>
                      <span className="setup-badge">Always on</span>
                    </div>
                    <Choice
                      checked={documentContent}
                      change={setDocumentContent}
                      title="Search inside documents"
                      description="Index embedded text in PDFs, DOCX, TXT and supported code files. Background processing uses extra disk space."
                    />
                    <div className="setup-note">
                      <ShieldCheck size={18} />
                      <p>
                        Document text stays on this device. Scanned PDFs need
                        separate OCR; encrypted or unsupported files may be
                        skipped. You can stop or clear indexing in Settings.
                      </p>
                    </div>
                  </>
                )}
                {step === 3 && (
                  <>
                    <h1 ref={heading} tabIndex={-1}>
                      Make yourself at home.
                    </h1>
                    <p className="setup-lead">
                      A few preferences for everyday use. Change them any time.
                    </p>
                    <fieldset className="setup-theme">
                      <legend>Appearance</legend>
                      {(
                        [
                          ["light", Sun, "Light"],
                          ["dark", Moon, "Dark"],
                          ["system", Monitor, "System"],
                        ] as const
                      ).map(([value, Icon, label]) => (
                        <label
                          key={value}
                          className={theme.preference === value ? "chosen" : ""}
                        >
                          <input
                            type="radio"
                            name="setup-theme"
                            checked={theme.preference === value}
                            onChange={() => theme.setPreference(value)}
                          />
                          <Icon size={20} />
                          {label}
                        </label>
                      ))}
                    </fieldset>
                    <Choice
                      checked={background.keepRunning}
                      change={(v) =>
                        setBackground((b) => ({ ...b, keepRunning: v }))
                      }
                      title="Keep DigView in the tray"
                      description="Closing the window keeps selected folders up to date. Use tray > Quit to exit."
                    />
                    <Choice
                      checked={background.runAtStartup}
                      disabled={!background.startupSupported}
                      change={(v) =>
                        setBackground((b) => ({ ...b, runAtStartup: v }))
                      }
                      title="Open when I sign in to Windows"
                      description={
                        background.startupSupported
                          ? "Optional. Windows startup is off until you enable it."
                          : "Startup registration is available on Windows only."
                      }
                    />
                    {background.runAtStartup && background.keepRunning && (
                      <Choice
                        checked={background.startMinimized}
                        change={(v) =>
                          setBackground((b) => ({ ...b, startMinimized: v }))
                        }
                        title="Start in the tray"
                        description="A normal manual launch still opens the window."
                      />
                    )}
                    <details className="setup-resource">
                      <summary>
                        <SlidersHorizontal size={16} />
                        Indexing resources
                      </summary>
                      <Choice
                        checked={lowResource}
                        change={setLowResource}
                        title="Use fewer resources"
                        description="Add pauses between work units. The first pass may take longer."
                      />
                      {background.startupSupported && (
                        <Choice
                          checked={pauseOnBattery}
                          change={setPauseOnBattery}
                          title="Pause indexing on battery"
                          description="Resume when plugged in. Available on Windows."
                        />
                      )}
                    </details>
                    <p className="setup-footnote">
                      Start using DigView saves your preferences.
                      {selected.length > 0
                        ? " It also starts indexing the folders you selected."
                        : " You can add folders later."}
                    </p>
                    <p className="setup-shortcut">
                      <kbd>Ctrl</kbd> + <kbd>F</kbd> focuses search inside the
                      library.
                    </p>
                  </>
                )}
              </>
            )}
            {error && (
              <div className="setup-error" role="alert">
                {error}
                <small>
                  {openingLibrary
                    ? "Your choices are saved. Retry opening the library."
                    : "Your setup remains open. Check your choices and try again."}
                </small>
                {initial.paths.length > 0 && (
                  <button
                    type="button"
                    disabled={busy}
                    onClick={() =>
                      void api
                        .cancel()
                        .then(() =>
                          setError(
                            "Current task is stopping. Retry setup once it has stopped.",
                          ),
                        )
                        .catch((e) => setError(errorText(e)))
                    }
                  >
                    Stop current indexing task
                  </button>
                )}
              </div>
            )}
          </fieldset>
        </div>
        {!folderPicker && (
          <footer className="setup-footer">
            <div>
              {!ready && !openingLibrary && step > 0 ? (
                <button
                  className="setup-back"
                  disabled={busy}
                  onClick={() => {
                    setError("");
                    setStep((s) => s - 1);
                  }}
                >
                  <ArrowLeft size={16} />
                  Back
                </button>
              ) : !ready && !openingLibrary ? (
                <button
                  className="setup-skip"
                  disabled={busy}
                  onClick={() => void finish(true)}
                >
                  Set up later
                </button>
              ) : (
                <span className="setup-local">
                  <ShieldCheck size={15} />
                  On your device
                </span>
              )}
            </div>
            <button
              className="setup-primary"
              disabled={busy}
              onClick={() =>
                ready || openingLibrary
                  ? void openLibrary()
                  : step === steps.length - 1
                    ? void finish()
                    : next()
              }
            >
              {busy ? (
                <>
                  <LoaderCircle size={17} className="setup-spin" />
                  {ready || openingLibrary ? "Opening..." : "Saving..."}
                </>
              ) : (
                <>
                  {openingLibrary
                    ? "Retry opening library"
                    : ready
                      ? "Open my library"
                      : step === steps.length - 1
                        ? "Start using DigView"
                        : "Continue"}
                  <ArrowRight size={17} />
                </>
              )}
            </button>
          </footer>
        )}
      </section>
    </main>
  );
}
function Choice({
  checked,
  change,
  title,
  description,
  disabled = false,
}: {
  checked: boolean;
  change: (value: boolean) => void;
  title: string;
  description: string;
  disabled?: boolean;
}) {
  return (
    <label className={`setup-option ${disabled ? "disabled" : ""}`}>
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(e) => change(e.target.checked)}
      />
      <span>
        <strong>{title}</strong>
        <small>{description}</small>
      </span>
    </label>
  );
}
