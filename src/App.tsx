import { SemanticSettings, semanticQuery, ActivityBar } from "./semantic";
import { Modal } from "./modal";
import { AddFoldersDialog } from "./add-folders-dialog";
import { setupService } from "./setup-service";
import { BrandIcon, BrandWordmark } from "./brand";
import { indexImageBatch } from "./ocr-batch";
import { useUpdates, UpdateSettings } from "./updates";
import { SearchBox } from "./search-box";
import { emptyPersonal } from "./personal-library";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import {
  Archive,
  ArrowLeft,
  ArrowRight,
  ArrowUpRight,
  Check,
  CheckCheck,
  ChevronRight,
  CircleHelp,
  Copy,
  Download,
  ExternalLink,
  File,
  FileText,
  Folder,
  FolderPlus,
  Grid2X2,
  HardDrive,
  Image,
  LayoutDashboard,
  List,
  Loader2,
  Maximize2,
  Music2,
  Package,
  RefreshCw,
  ScanLine,
  Search,
  Settings2,
  ShieldCheck,
  Sparkles,
  Trash2,
  Video,
  X,
  Globe,
  Sun,
  Moon,
  Monitor,
  CircleAlert,
  Star,
  EyeOff,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { api, demoArt, desktop } from "./api";
import { useTheme } from "./theme";
import { BackgroundSettingsPanel } from "./background";
import { ScanSettingsPanel } from "./scan-settings";
import { observability } from "./observability";
import { formatBytes, parseSearch } from "./search";
import { fileTypeIcon } from "./file-type-icon";
import { FilePreview } from "./file-preview";
import { LocalDiagnosticsPanel } from "./local-diagnostics-panel";
import { RecognizedText } from "./recognized-text";
import { HiddenItemsPanel } from "./hidden-items";
import { DocumentIndexPanel } from "./document-index";
import type {
  Bootstrap,
  Category,
  DuplicateGroup,
  FileRecord,
  Job,
  Query,
  QueryResult,
  Root,
  View,
} from "./types";

const categories: {
  id: Category;
  label: string;
  icon: LucideIcon;
  color: string;
}[] = [
  { id: "images", label: "Images", icon: Image, color: "lavender" },
  { id: "videos", label: "Videos", icon: Video, color: "rose" },
  { id: "music", label: "Music", icon: Music2, color: "peach" },
  { id: "documents", label: "Documents", icon: FileText, color: "blue" },
  { id: "archives", label: "Archives", icon: Archive, color: "mint" },
  { id: "apps", label: "Apps", icon: Package, color: "yellow" },
  { id: "other", label: "Other files", icon: File, color: "gray" },
];
const initialResult: QueryResult = { files: [], total: 0, bytes: 0 };
function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
function shortPath(path: string): string {
  return path.replace(/^\\\\\?\\/, "");
}
function folderName(path: string): string {
  return shortPath(path).split(/[\\/]/).filter(Boolean).pop() || path;
}
function prettyDate(timestamp: number): string {
  return new Date(timestamp * 1000).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

const thumbCache = new Map<string, string>();
let activeThumbnails = 0;
const thumbnailQueue: (() => void)[] = [];
function boundedThumbnail(
  id: number,
  large: boolean,
  signal: AbortSignal,
): Promise<string> {
  return new Promise((resolve, reject) => {
    const aborted = () => {
      const index = thumbnailQueue.indexOf(run);
      if (index >= 0) thumbnailQueue.splice(index, 1);
      reject(new DOMException("Preview no longer visible", "AbortError"));
    };
    const run = () => {
      if (signal.aborted) {
        aborted();
        return;
      }
      activeThumbnails++;
      api
        .thumbnail(id, large)
        .then(resolve, reject)
        .finally(() => {
          signal.removeEventListener("abort", aborted);
          activeThumbnails--;
          thumbnailQueue.shift()?.();
        });
    };
    if (signal.aborted) {
      aborted();
      return;
    }
    signal.addEventListener("abort", aborted, { once: true });
    if (activeThumbnails < 3) run();
    else thumbnailQueue.push(run);
  });
}
function Thumbnail({
  file,
  large = false,
}: {
  file: FileRecord;
  large?: boolean;
}) {
  const key = `${file.id}:${file.path}:${file.modified_token ?? file.modified}:${file.size}:${large}`;
  const [src, setSrc] = useState<string | undefined>(() => thumbCache.get(key));
  const [failed, setFailed] = useState(false);
  const meta = categories.find((c) => c.id === file.category)!;
  const Icon = fileTypeIcon(file.extension, file.category);
  useEffect(() => {
    let alive = true;
    const controller = new AbortController();
    setFailed(false);
    setSrc(thumbCache.get(key));
    if (!["images", "videos"].includes(file.category) || thumbCache.has(key))
      return;
    if (!desktop) {
      if (file.category === "images") setSrc(demoArt(file));
      return;
    }
    boundedThumbnail(file.id, large, controller.signal)
      .then((value) => {
        if (thumbCache.size > 250)
          thumbCache.delete(thumbCache.keys().next().value!);
        thumbCache.set(key, value);
        if (alive) setSrc(value);
      })
      .catch(() => {
        if (alive) setFailed(true);
      });
    return () => {
      alive = false;
      controller.abort();
    };
  }, [key, file.category, file.id, large]);
  if (src && ["images", "videos"].includes(file.category))
    return (
      <img
        src={src}
        alt={file.name}
        draggable={false}
        className={large ? "preview-image" : "file-image"}
      />
    );
  return (
    <div className={`file-art ${meta.color}${large ? " file-art-large" : ""}`}>
      <Icon size={large ? 82 : 40} strokeWidth={1.3} />
      <span className="file-extension-badge">
        {file.extension.toUpperCase() || "FILE"}
      </span>
      {failed && large && (
        <small>Inline preview unavailable. Open the original.</small>
      )}
    </div>
  );
}

function App() {
  const theme = useTheme();
  const [folderPicker, setFolderPicker] = useState(false);
  const [personal, setPersonal] = useState(emptyPersonal);
  const [favourites, setFavourites] = useState(false);
  const [boot, setBoot] = useState<Bootstrap | null>(null);
  const [view, setView] = useState<View>("home");
  const [search, setSearch] = useState("");
  const meaning = desktop;
  const [broader, setBroader] = useState(false);
  const [semanticResults, setSemanticResults] = useState(false);
  const [meaningNotice, setMeaningNotice] = useState("");
  const [debounced, setDebounced] = useState("");
  const [result, setResult] = useState<QueryResult>(initialResult);
  const [sort, setSort] = useState("recent");
  const [layout, setLayout] = useState<"grid" | "list">(() =>
    localStorage.getItem("digview-layout") === "list" ? "list" : "grid",
  );
  const [rootId, setRootId] = useState<number | undefined>();
  const [source, setSource] = useState<string | undefined>();
  const [cleanup, setCleanup] = useState<string | undefined>();
  const [offset, setOffset] = useState(0);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [preview, setPreview] = useState<FileRecord | null>(null);
  const [fileActions, setFileActions] = useState<FileRecord | null>(null);
  const [ocr, setOcr] = useState<string | null>(null);
  const [ocrBusy, setOcrBusy] = useState(false);
  const [groups, setGroups] = useState<DuplicateGroup[]>([]);
  const [revision, setRevision] = useState(0);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [job, setJob] = useState<Job | null>(null);
  const [toast, setToast] = useState<{ text: string; error: boolean } | null>(
    null,
  );
  const [confirm, setConfirm] = useState<"trash" | Root | null>(null);
  const [help, setHelp] = useState(false);
  const [introDismissed, setIntroDismissed] = useState(
    () => localStorage.getItem("digview-intro-dismissed") === "true",
  );
  const searchRef = useRef<HTMLInputElement>(null);
  const queryEpoch = useRef(0);
  const ocrEpoch = useRef(0);
  const stopOcrBatch = useRef(false);
  const [ocrBatch, setOcrBatch] = useState("");
  const updates = useUpdates(busy || !!job?.running || ocrBusy || !!ocrBatch);
  const prevRunning = useRef(false);
  const lastLiveRefresh = useRef(0);
  const notify = useCallback(
    (text: string, error = false) => setToast({ text, error }),
    [],
  );
  useEffect(() => {
    const notice = setupService.takeLibraryNotice();
    if (notice) notify(notice);
  }, [notify]);
  useEffect(() => {
    if (!toast) return;
    const timer = setTimeout(() => setToast(null), toast.error ? 12000 : 4500);
    return () => clearTimeout(timer);
  }, [toast]);
  const refresh = useCallback(async () => {
    try {
      const data = await api.bootstrap();
      setBoot(data);
      setPersonal(await api.personal());
      setJob(data.job);
      setRevision((r) => r + 1);
    } catch (error) {
      notify(errorText(error), true);
    } finally {
      setLoading(false);
    }
  }, [notify]);
  const lastIndexRevision = useRef(0);
  const workspaceReported = useRef(false);
  const workspaceStarted = useRef(
    desktop && observability.active ? performance.now() : 0,
  );
  useEffect(() => {
    if (!loading && !workspaceReported.current) {
      workspaceReported.current = true;
      if (desktop && observability.active)
        observability.capture("workspace_ready", {
          duration_ms: performance.now() - workspaceStarted.current,
        });
    }
  }, [loading]);
  useEffect(() => {
    if (desktop) observability.capture("screen_viewed", { view });
  }, [view]);
  useEffect(() => {
    void refresh();
  }, [refresh]);
  useEffect(() => {
    const timer = setTimeout(() => setDebounced(search), 180);
    return () => clearTimeout(timer);
  }, [search, meaning]);
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "f") {
        e.preventDefault();
        searchRef.current?.focus();
      }
      if (e.key === "Escape" && !preview && !confirm && !fileActions) {
        setSelected(new Set());
        if (search) {
          setSearch("");
          searchRef.current?.blur();
        }
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [preview, confirm, fileActions, search]);
  useEffect(() => {
    if (!desktop) return;
    let alive = true;
    const timer = setInterval(() => {
      api
        .job()
        .then((next) => {
          if (!alive) return;
          setJob(next);
          const indexChanged = next.revision !== lastIndexRevision.current;
          if (indexChanged) {
            lastIndexRevision.current = next.revision;
            void refresh();
          }
          if (prevRunning.current && !next.running) {
            if (!indexChanged) void refresh();
            if (next.errors)
              notify(
                `${next.message} · ${next.errors} skipped/error(s).`,
                true,
              );
            else if (next.kind !== "watch") notify(next.message);
          }
          if (next.running && Date.now() - lastLiveRefresh.current > 3500) {
            lastLiveRefresh.current = Date.now();
            setRevision((r) => r + 1);
          }
          prevRunning.current = next.running;
        })
        .catch((error) => {
          if (alive) notify(errorText(error), true);
        });
    }, 1000);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [refresh, notify]);
  useEffect(
    () => setBroader(false),
    [debounced, view, rootId, source, cleanup, favourites],
  );
  const parsed = useMemo(() => parseSearch(debounced), [debounced]);
  const query = useMemo<Query>(
    () => ({
      terms: [],
      favourites,
      sort,
      offset,
      limit: view === "home" && !debounced ? 12 : 120,
      ...parsed.query,
      category:
        parsed.query.category ||
        (categories.some((c) => c.id === view) ? view : undefined),
      source,
      root_id: rootId,
      cleanup,
    }),
    [
      parsed,
      sort,
      offset,
      view,
      debounced,
      source,
      rootId,
      cleanup,
      favourites,
    ],
  );
  const isLibrary =
    !!debounced ||
    view === "home" ||
    view === "all" ||
    categories.some((c) => c.id === view) ||
    !!source ||
    !!cleanup;
  useEffect(() => {
    setOffset(0);
    setSelected(new Set());
  }, [debounced, view, sort, rootId, source, cleanup, favourites, meaning]);
  useEffect(() => {
    if (!boot || !isLibrary) return;
    const epoch = ++queryEpoch.current;
    setLoading(true);
    setMeaningNotice(
      meaning && debounced ? "Searching indexed text by meaning…" : "",
    );
    const request =
      meaning && debounced && desktop
        ? semanticQuery(query, broader)
        : api
            .query(query)
            .then((result) => ({
              result,
              matches: undefined,
              semantic: false,
              message: "",
            }));
    request
      .then((data) => {
        if (epoch === queryEpoch.current) {
          setResult({
            ...data.result,
            files: data.result.files.map((file) => ({
              ...file,
              search_match: data.matches?.[String(file.id)],
            })),
          });
          setSemanticResults(data.semantic);
          setMeaningNotice(data.message);
        }
      })
      .catch((error) => {
        if (epoch === queryEpoch.current) notify(errorText(error), true);
      })
      .finally(() => {
        if (epoch === queryEpoch.current) setLoading(false);
      });
    return () => {
      queryEpoch.current++;
    };
  }, [query, revision, !!boot, isLibrary, notify, meaning, debounced, broader]);
  useEffect(() => {
    if (view !== "duplicates") return;
    let alive = true;
    api
      .groups()
      .then((data) => {
        if (alive) setGroups(data);
      })
      .catch((e) => notify(errorText(e), true));
    return () => {
      alive = false;
    };
  }, [view, revision, notify]);
  useEffect(() => {
    const epoch = ++ocrEpoch.current;
    setOcr(null);
    setOcrBusy(false);
    if (preview?.category === "images" && desktop)
      void api
        .imageText(preview.id)
        .then((text) => {
          if (epoch === ocrEpoch.current) setOcr(text);
        })
        .catch(() => {});
  }, [preview?.id]);
  const navigate = (next: View) => {
    setView(next);
    setFavourites(false);
    setSearch("");
    setDebounced("");
    setSource(undefined);
    setCleanup(undefined);
    setSelected(new Set());
    setOffset(0);
  };
  const run = async (action: () => Promise<unknown>) => {
    setBusy(true);
    try {
      await action();
    } catch (e) {
      notify(errorText(e), true);
    } finally {
      setBusy(false);
    }
  };
  const startJob = async (kind: "scan" | "duplicates") => {
    setSelected(new Set());
    await (kind === "scan" ? api.scan() : api.duplicates());
    prevRunning.current = true;
    setJob(await api.job());
  };
  const visibilityChanged = async () => {
    queryEpoch.current++;
    ocrEpoch.current++;
    setPreview(null);
    setFileActions(null);
    setOcr(null);
    setSelected(new Set());
    setResult(initialResult);
    setGroups([]);
    setOffset(0);
    await refresh();
  };
  const hideItems = (ids: number[], folders = false) =>
    run(async () => {
      await api.hideFiles(ids, folders);
      await visibilityChanged();
      notify(
        folders
          ? "Folder and its contents hidden. Restore from Settings → Hidden items."
          : "Files hidden from app. Restore from Settings → Hidden items.",
      );
    });
  const addFolder = () => setFolderPicker(true);
  const toggleSelected = (id: number) =>
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  const allSelected =
    result.files.length > 0 && result.files.every((f) => selected.has(f.id));
  const openPreview = (file: FileRecord) => {
    setPreview(file);
    setOcr(null);
  };
  const chooseSource = (host: string) => {
    navigate("all");
    setSource(host);
  };
  const chooseCleanup = (filter: string) => {
    navigate("all");
    setCleanup(filter);
    if (filter === "large") setSort("size");
  };
  const changeLayout = (next: "grid" | "list") => {
    setLayout(next);
    localStorage.setItem("digview-layout", next);
  };
  const doTrash = () =>
    run(async () => {
      const response = await api.trash(selectedFiles, view === "duplicates");
      setConfirm(null);
      setSelected(new Set());
      if (response.trashed.length)
        notify(
          `${response.trashed.length} file${response.trashed.length === 1 ? "" : "s"} moved to system trash. Restore using your OS trash.`,
        );
      if (response.failed.length)
        notify(
          `${response.failed.length} file(s) were kept: ${response.failed[0].reason}`,
          true,
        );
      await refresh();
    });
  const selectedFiles =
    view === "duplicates"
      ? groups.flatMap((g) => g.files).filter((f) => selected.has(f.id))
      : result.files.filter((f) => selected.has(f.id));
  const selectedBytes = selectedFiles.reduce((n, f) => n + f.size, 0);
  const running = !!job?.running;
  const hideLocked = busy || running || ocrBusy || !!ocrBatch || !desktop;
  const title = favourites
    ? "Favourites"
    : debounced
      ? "Search results"
      : source
        ? `From ${source}`
        : cleanup
          ? {
              large: "Large files",
              screenshots: "Old screenshots",
              downloads: "Forgotten downloads",
            }[cleanup] || "Clean up"
          : view === "all"
            ? "All files"
            : categories.find((c) => c.id === view)?.label || "";

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="sidebar-scroll">
          <button
            className="brand"
            onClick={() => navigate("home")}
            aria-label="DigView home"
          >
            <BrandWordmark />
          </button>
          <div className="sidebar-caption">YOUR SPACE</div>
          <nav aria-label="Main navigation">
            <Nav
              icon={LayoutDashboard}
              label="Overview"
              active={view === "home" && !debounced}
              onClick={() => navigate("home")}
            />
            <Nav
              icon={Grid2X2}
              label="All files"
              count={boot?.stats.count}
              active={
                view === "all" &&
                !source &&
                !cleanup &&
                !debounced &&
                !favourites
              }
              onClick={() => navigate("all")}
            />
            <Nav
              icon={Globe}
              label="Downloaded from"
              active={view === "sources" || !!source}
              onClick={() => navigate("sources")}
            />
          </nav>
          <Nav
            icon={Star}
            label="Favourites"
            active={favourites}
            onClick={() => {
              navigate("all");
              setFavourites(true);
            }}
          />
          <div className="sidebar-caption category-caption">LIBRARY</div>
          <nav aria-label="File categories">
            {categories.map((c) => (
              <Nav
                key={c.id}
                icon={c.icon}
                label={c.label}
                count={
                  boot?.stats.categories.find((s) => s.category === c.id)
                    ?.count || undefined
                }
                active={view === c.id && !debounced}
                onClick={() => navigate(c.id)}
              />
            ))}
          </nav>
          <div className="nav-separator" />
          <Nav
            icon={Sparkles}
            label="Clean up"
            active={view === "cleanup" || !!cleanup}
            onClick={() => navigate("cleanup")}
          />
          <Nav
            icon={Copy}
            label="Duplicates"
            count={boot?.stats.duplicate_groups || undefined}
            active={view === "duplicates"}
            onClick={() => navigate("duplicates")}
          />
        </div>
        <div className="sidebar-bottom">
          <div className="storage-card">
            <div className="storage-icon">
              <HardDrive size={17} />
            </div>
            <div>
              <strong>{formatBytes(boot?.stats.bytes || 0)}</strong>
              <span>in your library</span>
            </div>
            <span className="local-dot" />
          </div>
          <div className="privacy-line">
            <ShieldCheck size={13} /> On your device. Always.
          </div>
          <Nav
            icon={Settings2}
            label="Settings & folders"
            active={view === "settings"}
            onClick={() => navigate("settings")}
          />
        </div>
      </aside>

      <div className="workspace">
        <header className="topbar">
          <div className="breadcrumb">
            <span>Your library</span>
            <ChevronRight size={14} />
            <strong>
              {debounced
                ? "Search"
                : title ||
                  (
                    {
                      home: "Overview",
                      sources: "Downloaded from",
                      cleanup: "Clean up",
                      duplicates: "Duplicates",
                      settings: "Settings",
                    } as Record<string, string>
                  )[view]}
            </strong>
          </div>
          <SearchBox inputRef={searchRef} value={search} onChange={setSearch} />
          <div className="topbar-actions">
            <button
              className={`icon-button ${running ? "spinning" : ""}`}
              aria-label="Refresh library"
              title="Rescan selected folders"
              disabled={running || busy || !boot?.roots.length}
              onClick={() => void run(() => startJob("scan"))}
            >
              <RefreshCw size={18} />
            </button>
            <button
              className="icon-button theme-toggle"
              aria-label={`Switch to ${theme.resolved === "dark" ? "light" : "dark"} mode`}
              title={`Switch to ${theme.resolved === "dark" ? "light" : "dark"} mode`}
              onClick={() =>
                theme.setPreference(
                  theme.resolved === "dark" ? "light" : "dark",
                )
              }
            >
              {theme.resolved === "dark" ? (
                <Sun size={18} />
              ) : (
                <Moon size={18} />
              )}
            </button>
            <button
              className="icon-button help-button"
              aria-label="Help and keyboard shortcuts"
              onClick={() => setHelp(true)}
            >
              <CircleHelp size={19} />
            </button>
          </div>
        </header>
        {meaningNotice && (
          <div className="semantic-search-notice" role="status">
            {meaningNotice}
          </div>
        )}
        {!desktop && (
          <div className="demo-banner">
            <span>
              <span className="demo-dot" /> Browser preview · sample library
            </span>
            <span>Your real files are only available in the desktop app.</span>
          </div>
        )}
        {updates.status?.available && (
          <div className="update-banner" role="status">
            Update {updates.status.available} is available.{" "}
            <button
              className="button small-button"
              onClick={() => navigate("settings")}
            >
              Review update
            </button>
          </div>
        )}
        {ocrBatch && (
          <div className="job-banner" role="status">
            <span>Indexing image text · {ocrBatch}</span>
            <button
              onClick={() => {
                stopOcrBatch.current = true;
              }}
            >
              Stop after current image
            </button>
          </div>
        )}
        {running && (
          <div className="job-banner" role="status">
            <Loader2 size={16} className="spinning" />
            <span>
              {job?.kind === "update" ? "Installing update…" : job?.message}
            </span>
            {job?.kind !== "update" && (
              <strong>{job?.processed.toLocaleString()} checked</strong>
            )}
            <button
              disabled={job?.kind === "update"}
              onClick={() => void run(() => api.cancel())}
            >
              Cancel
            </button>
          </div>
        )}
        {boot?.roots.length === 0 && view !== "settings" ? (
          <main className="onboarding">
            <span className="onboarding-icon">
              <FolderPlus size={42} strokeWidth={1.3} />
            </span>
            <div className="eyebrow">
              A LITTLE LESS SEARCHING. A LITTLE MORE SPACE.
            </div>
            <h1>Your files, finally in view.</h1>
            <p>
              Bring the folders you choose into one calm, visual library.
              <br />
              Find a file, copy text from an image, or make room for what’s
              next.
            </p>
            <button
              className="button primary"
              disabled={busy}
              onClick={() => void addFolder()}
            >
              <FolderPlus size={17} /> Choose your first folder
            </button>
            <div className="onboarding-points">
              <span>
                <ShieldCheck size={16} /> Entirely on your device
              </span>
              <span>
                <Check size={16} /> Originals stay where they are
              </span>
              <span>
                <Trash2 size={16} /> You approve every cleanup
              </span>
            </div>
          </main>
        ) : (
          <main
            className={`main-content ${isLibrary && view !== "home" ? "library-main" : ""}`}
          >
            {view === "home" && !debounced && (
              <>
                <div className="page-heading">
                  <div>
                    <div className="eyebrow">
                      A FRIENDLIER HOME FOR YOUR FILES
                    </div>
                    <h1>
                      A little order. A lot of clarity
                      <span className="accent-period">.</span>
                    </h1>
                    <p>Everything you need, right where you can find it.</p>
                  </div>
                  <button
                    className="button"
                    disabled={busy || running}
                    onClick={() => void addFolder()}
                  >
                    <FolderPlus size={16} /> Add folder
                  </button>
                </div>
                {!introDismissed && (
                  <section className="welcome-card">
                    <div className="welcome-copy">
                      <span className="pill">
                        <Sparkles size={12} /> LESS CLUTTER, MORE POSSIBILITY
                      </span>
                      <h2>Make room for a fresh start.</h2>
                      <p>
                        Find the large files, old screenshots, and extra copies
                        <br className="desktop-break" /> hiding in your folders.
                        You choose what goes.
                      </p>
                      <button
                        className="text-button"
                        onClick={() => navigate("cleanup")}
                      >
                        Explore clean up <ArrowRight size={16} />
                      </button>
                    </div>
                    <div className="welcome-art" aria-hidden="true">
                      <div className="art-orbit orbit-one" />
                      <div className="art-orbit orbit-two" />
                      <div className="art-folder back">
                        <Folder size={84} strokeWidth={1} />
                      </div>
                      <div className="art-document">
                        <FileText size={49} strokeWidth={1.2} />
                        <span />
                        <span />
                      </div>
                      <div className="art-folder front">
                        <Folder size={102} strokeWidth={1} />
                      </div>
                      <span className="art-star star-one">✦</span>
                      <span className="art-star star-two">✧</span>
                      <span className="art-badge">
                        <Check size={17} />
                      </span>
                    </div>
                    <button
                      className="icon-button dismiss-welcome"
                      aria-label="Dismiss welcome card"
                      onClick={() => {
                        setIntroDismissed(true);
                        localStorage.setItem("digview-intro-dismissed", "true");
                      }}
                    >
                      <X size={16} />
                    </button>
                  </section>
                )}
                <section className="categories-section">
                  <div className="section-heading">
                    <h2>Your library, at a glance</h2>
                    <span>
                      {boot?.stats.count.toLocaleString() || 0} files ·{" "}
                      {formatBytes(boot?.stats.bytes || 0)}
                    </span>
                  </div>
                  <div className="category-grid">
                    {categories.slice(0, 6).map((c) => {
                      const stat = boot?.stats.categories.find(
                        (s) => s.category === c.id,
                      );
                      const Icon = c.icon;
                      return (
                        <button
                          key={c.id}
                          className="category-card"
                          onClick={() => navigate(c.id)}
                        >
                          <span className={`category-icon ${c.color}`}>
                            <Icon size={22} strokeWidth={1.6} />
                          </span>
                          <strong>{c.label}</strong>
                          <span>
                            {(stat?.count || 0).toLocaleString()} files
                          </span>
                          <small>{formatBytes(stat?.bytes || 0)}</small>
                          <ArrowUpRight className="category-arrow" size={15} />
                        </button>
                      );
                    })}
                  </div>
                </section>
              </>
            )}

            {isLibrary && (
              <>
                {view === "home" && !debounced ? (
                  <div className="section-heading recent-heading">
                    <h2>Recently modified</h2>
                    <button
                      className="text-button muted"
                      onClick={() => navigate("all")}
                    >
                      View all files <ArrowRight size={15} />
                    </button>
                  </div>
                ) : (
                  <div className="page-heading library-heading">
                    <div>
                      <h1>{title}</h1>
                      <p>
                        {result.total.toLocaleString()} files{" "}
                        <span className="middle-dot">·</span>{" "}
                        {formatBytes(result.bytes)}
                        {debounced && semanticResults && (
                          <button
                            className="text-button related-results-control"
                            onClick={() => {
                              setOffset(0);
                              setBroader((value) => !value);
                            }}
                          >
                            {broader
                              ? "Best matches only"
                              : "More related results"}
                          </button>
                        )}
                        {cleanup === "screenshots"
                          ? " · Older than 30 days"
                          : cleanup === "downloads"
                            ? " · Older than 90 days"
                            : cleanup === "large"
                              ? " · 100 MB and above"
                              : ""}
                      </p>
                    </div>
                    <button
                      className="button"
                      onClick={() => void addFolder()}
                      disabled={running || busy}
                    >
                      <FolderPlus size={16} /> Add folder
                    </button>
                  </div>
                )}
                {parsed.chips.length > 0 && (
                  <div className="filter-chips">
                    <span>
                      <Sparkles size={13} /> Understood as
                    </span>
                    {parsed.chips.map((chip) => (
                      <span className="filter-chip" key={chip}>
                        {chip}
                      </span>
                    ))}
                    {(query.date_from || query.date_to) && (
                      <small>Using modified date</small>
                    )}
                  </div>
                )}
                {(source || cleanup) && (
                  <div className="filter-chips">
                    <span className="filter-chip">
                      {source || title}
                      <button
                        aria-label="Remove filter"
                        onClick={() => {
                          setSource(undefined);
                          setCleanup(undefined);
                        }}
                      >
                        <X size={12} />
                      </button>
                    </span>
                  </div>
                )}
                <div className="library-toolbar">
                  <div className="toolbar-left">
                    <button
                      className={`selection-toggle ${allSelected ? "checked" : ""}`}
                      aria-label={
                        allSelected
                          ? "Deselect page"
                          : "Select files on this page"
                      }
                      onClick={() =>
                        setSelected(
                          allSelected
                            ? new Set()
                            : new Set(result.files.map((f) => f.id)),
                        )
                      }
                    >
                      {allSelected && <Check size={12} />}
                    </button>
                    <select
                      aria-label="Filter by folder"
                      value={rootId ?? ""}
                      onChange={(e) =>
                        setRootId(
                          e.target.value ? Number(e.target.value) : undefined,
                        )
                      }
                    >
                      <option value="">All folders</option>
                      {boot?.roots.map((root) => (
                        <option key={root.id} value={root.id}>
                          {folderName(root.path)}
                        </option>
                      ))}
                    </select>
                  </div>
                  <div className="toolbar-right">
                    {selectedFiles.some((f) => f.category === "images") &&
                      !ocrBatch && (
                        <button
                          className="button small-button"
                          disabled={busy || running || !desktop}
                          onClick={() =>
                            void run(async () => {
                              stopOcrBatch.current = false;
                              setOcrBatch("Starting…");
                              try {
                                const outcome = await indexImageBatch(
                                  selectedFiles,
                                  (id) => api.ocr(id),
                                  () => stopOcrBatch.current,
                                  (done, total) =>
                                    setOcrBatch(`${done}/${total}`),
                                );
                                notify(
                                  `${outcome.indexed} images indexed, ${outcome.failed} failed${outcome.cancelled ? " · stopped" : ""}.`,
                                  outcome.failed > 0,
                                );
                                setRevision((r) => r + 1);
                              } finally {
                                setOcrBatch("");
                              }
                            })
                          }
                        >
                          Index image text
                        </button>
                      )}
                    {ocrBatch && (
                      <button
                        className="button small-button"
                        onClick={() => {
                          stopOcrBatch.current = true;
                        }}
                      >
                        Stop OCR after current image · {ocrBatch}
                      </button>
                    )}
                    <span className="sort-label">Sort by</span>
                    <select
                      aria-label="Sort files"
                      value={sort}
                      onChange={(e) => setSort(e.target.value)}
                    >
                      <option value="recent">Newest first</option>
                      <option value="oldest">Oldest first</option>
                      <option value="name">Name</option>
                      <option value="size">Largest first</option>
                    </select>
                    <div className="layout-toggle">
                      <button
                        aria-label="Grid view"
                        aria-pressed={layout === "grid"}
                        className={layout === "grid" ? "active" : ""}
                        onClick={() => changeLayout("grid")}
                      >
                        <Grid2X2 size={16} />
                      </button>
                      <button
                        aria-label="List view"
                        aria-pressed={layout === "list"}
                        className={layout === "list" ? "active" : ""}
                        onClick={() => changeLayout("list")}
                      >
                        <List size={17} />
                      </button>
                    </div>
                  </div>
                </div>
                {loading && result.files.length === 0 ? (
                  <div className="loading-state">
                    <Loader2 size={25} className="spinning" />
                    <p>Bringing your library into view…</p>
                  </div>
                ) : result.files.length === 0 ? (
                  <Empty
                    icon={Search}
                    title={
                      running && job?.kind === "scan"
                        ? "Building your library…"
                        : "No files found"
                    }
                    description={
                      running && job?.kind === "scan"
                        ? "Your first files will appear as the scan progresses. You can keep browsing."
                        : debounced
                          ? "Try a different filename, file type, date, or size."
                          : "Add a folder or adjust your filters to see files here."
                    }
                    action={
                      running ? undefined : debounced ? (
                        <button
                          className="button"
                          onClick={() => setSearch("")}
                        >
                          Clear search
                        </button>
                      ) : (
                        <button
                          className="button"
                          onClick={() => void addFolder()}
                        >
                          <FolderPlus size={16} /> Add folder
                        </button>
                      )
                    }
                  />
                ) : (
                  <>
                    <FileBrowser
                      files={result.files}
                      layout={layout}
                      compact={view === "home" && !debounced}
                      selected={selected}
                      toggle={toggleSelected}
                      preview={openPreview}
                      actions={setFileActions}
                    />
                    {!(view === "home" && !debounced) && (
                      <div className="pagination">
                        <span>
                          Showing {offset + 1}–
                          {Math.min(offset + result.files.length, result.total)}{" "}
                          of {result.total.toLocaleString()}
                        </span>
                        <div>
                          <button
                            className="button small-button"
                            disabled={offset === 0 || loading}
                            onClick={() =>
                              setOffset((n) => Math.max(0, n - 120))
                            }
                          >
                            <ArrowLeft size={14} /> Previous
                          </button>
                          <button
                            className="button small-button"
                            disabled={offset + 120 >= result.total || loading}
                            onClick={() => setOffset((n) => n + 120)}
                          >
                            Next <ArrowRight size={14} />
                          </button>
                        </div>
                      </div>
                    )}
                  </>
                )}
              </>
            )}

            {view === "sources" && !debounced && (
              <>
                <div className="page-heading">
                  <div>
                    <div className="eyebrow">
                      A LITTLE CONTEXT GOES A LONG WAY
                    </div>
                    <h1>Remember where it came from.</h1>
                    <p>
                      Your downloads, grouped by the websites saved in their
                      metadata.
                    </p>
                  </div>
                </div>
                <div className="info-note">
                  <ShieldCheck size={17} />
                  <span>
                    Read from your files, on this device. These are source
                    websites, not connected accounts.
                  </span>
                </div>
                <div className="source-grid">
                  {boot?.stats.sources.map((s) => (
                    <button
                      className="source-card"
                      key={s.source}
                      onClick={() => chooseSource(s.source)}
                    >
                      <span className="source-monogram">
                        {s.source.charAt(0).toUpperCase()}
                      </span>
                      <ArrowUpRight size={17} />
                      <h3>{s.source}</h3>
                      <p>
                        {s.count.toLocaleString()} files{" "}
                        <span>· {formatBytes(s.bytes)}</span>
                      </p>
                    </button>
                  ))}
                </div>
                {!boot?.stats.sources.length && (
                  <Empty
                    icon={Globe}
                    title="Every file has a story. Some keep it private."
                    description="Download sources appear when your browser saved origin metadata. Older files and some platforms may not include it."
                  />
                )}
                <p className="footnote">
                  Source information depends on your browser and filesystem.
                  Files with no saved source stay in your library.
                </p>
              </>
            )}

            {view === "cleanup" && !debounced && (
              <>
                <div className="page-heading">
                  <div>
                    <div className="eyebrow">ROOM TO BREATHE</div>
                    <h1>
                      A fresh start for your folders
                      <span className="accent-period">.</span>
                    </h1>
                    <p>A few places to look. You decide what stays.</p>
                  </div>
                </div>
                <div className="cleanup-summary">
                  <span className="cleanup-summary-icon">
                    <Sparkles size={34} strokeWidth={1.3} />
                  </span>
                  <div>
                    <h2>Your space, your say.</h2>
                    <p>
                      Review files before moving anything to the system trash.
                      <br />
                      Your originals are never deleted automatically.
                    </p>
                  </div>
                  <ShieldCheck size={34} strokeWidth={1.2} />
                </div>
                <div className="cleanup-grid">
                  {[
                    {
                      id: "duplicates",
                      title: "Duplicate files",
                      description: "Keep your favorite copy. Clear the extras.",
                      count: boot?.stats.duplicate_groups || 0,
                      unit: "groups",
                      icon: Copy,
                      color: "lavender",
                      action: () => navigate("duplicates"),
                    },
                    {
                      id: "large",
                      title: "Large files",
                      description:
                        "The big things taking up a little too much room.",
                      count: boot?.stats.large || 0,
                      unit: "files · 100 MB+",
                      icon: HardDrive,
                      color: "blue",
                      action: () => chooseCleanup("large"),
                    },
                    {
                      id: "screenshots",
                      title: "Old screenshots",
                      description:
                        "Little captures from more than 30 days ago.",
                      count: boot?.stats.screenshots || 0,
                      unit: "files",
                      icon: ScanLine,
                      color: "peach",
                      action: () => chooseCleanup("screenshots"),
                    },
                    {
                      id: "downloads",
                      title: "Forgotten downloads",
                      description: "Downloads untouched for more than 90 days.",
                      count: boot?.stats.downloads || 0,
                      unit: "files",
                      icon: Download,
                      color: "mint",
                      action: () => chooseCleanup("downloads"),
                    },
                  ].map((c) => (
                    <button
                      className="cleanup-card"
                      key={c.id}
                      onClick={c.action}
                    >
                      <span className={`category-icon ${c.color}`}>
                        <c.icon size={24} strokeWidth={1.5} />
                      </span>
                      <h3>{c.title}</h3>
                      <p>{c.description}</p>
                      <div>
                        <strong>
                          {c.count.toLocaleString()} <span>{c.unit}</span>
                        </strong>
                        <span className="review-link">
                          Review <ArrowRight size={15} />
                        </span>
                      </div>
                    </button>
                  ))}
                </div>
              </>
            )}

            {view === "duplicates" && !debounced && (
              <>
                <div className="page-heading">
                  <div>
                    <div className="eyebrow">ONE GOOD COPY IS ENOUGH</div>
                    <h1>Keep one. Clear the rest.</h1>
                    <p>
                      Exact matches verified with SHA-256. Review every copy
                      before cleanup.
                    </p>
                  </div>
                  <button
                    className="button primary"
                    disabled={running || busy || !boot?.roots.length}
                    onClick={() => void run(() => startJob("duplicates"))}
                  >
                    <ScanLine size={16} />{" "}
                    {running && job?.kind === "duplicates"
                      ? "Checking…"
                      : "Check for duplicates"}
                  </button>
                </div>
                <div className="info-note">
                  <ShieldCheck size={17} />
                  <span>
                    At least one unchanged, verified copy must stay. Empty files
                    are excluded.
                  </span>
                  <strong>
                    {formatBytes(boot?.stats.duplicate_bytes || 0)} extra copies
                  </strong>
                </div>
                {groups.length === 0 ? (
                  <Empty
                    icon={Copy}
                    title={
                      running
                        ? "Looking for exact matches…"
                        : "A little less repetition."
                    }
                    description="Run a duplicate check to compare files in your selected folders. Same size is only a clue; matching contents are verified."
                  />
                ) : (
                  <div className="duplicate-list">
                    {groups.map((group, index) => (
                      <section className="duplicate-group" key={group.hash}>
                        <div className="duplicate-heading">
                          <div>
                            <span className="group-number">
                              {String(index + 1).padStart(2, "0")}
                            </span>
                            <strong>
                              {group.files.length} identical files
                            </strong>
                            <span>{formatBytes(group.size)} each</span>
                          </div>
                          <button
                            className="text-button"
                            onClick={() =>
                              setSelected((prev) => {
                                const next = new Set(prev);
                                group.files.forEach((f, i) => {
                                  if (i) next.add(f.id);
                                  else next.delete(f.id);
                                });
                                return next;
                              })
                            }
                          >
                            Keep newest, select extras <CheckCheck size={15} />
                          </button>
                        </div>
                        {group.files.map((file, i) => (
                          <div
                            className={`duplicate-row ${selected.has(file.id) ? "selected" : ""}`}
                            key={file.id}
                            onContextMenu={(e) => {
                              e.preventDefault();
                              setFileActions(file);
                            }}
                          >
                            <button
                              className={`selection-toggle ${selected.has(file.id) ? "checked" : ""}`}
                              aria-label={`Select ${file.name} for trash`}
                              onClick={() => toggleSelected(file.id)}
                            >
                              {selected.has(file.id) && <Check size={12} />}
                            </button>
                            <div
                              className="duplicate-thumb"
                              onClick={() => openPreview(file)}
                            >
                              <Thumbnail file={file} />
                            </div>
                            <button
                              className="duplicate-file"
                              onClick={() => openPreview(file)}
                            >
                              <strong>{file.name}</strong>
                              <span>{shortPath(file.path)}</span>
                            </button>
                            <span className="file-date">
                              {prettyDate(file.modified)}
                            </span>
                            {!selected.has(file.id) ? (
                              <span className="keep-tag">
                                <ShieldCheck size={12} /> Keep
                                {i === 0 ? " · newest" : ""}
                              </span>
                            ) : (
                              <span className="trash-tag">To trash</span>
                            )}
                          </div>
                        ))}
                      </section>
                    ))}
                  </div>
                )}
              </>
            )}

            {view === "settings" && !debounced && (
              <>
                <div className="page-heading">
                  <div>
                    <div className="eyebrow">MAKE YOURSELF AT HOME</div>
                    <h1>Your library, your way.</h1>
                    <p>
                      Choose where DigView looks. Your files stay where they
                      are.
                    </p>
                  </div>
                </div>
                <section className="settings-section">
                  <div className="section-heading">
                    <div>
                      <h2>Appearance</h2>
                      <p>Choose a theme, or follow your device.</p>
                    </div>
                  </div>
                  <fieldset className="theme-options">
                    <legend className="visually-hidden">Color theme</legend>
                    {(
                      [
                        ["light", "Light", "A bright, clear workspace", Sun],
                        ["dark", "Dark", "Comfortable in low light", Moon],
                        ["system", "System", "Match your device", Monitor],
                      ] as const
                    ).map(([value, label, description, Icon]) => (
                      <label
                        className={`theme-option ${theme.preference === value ? "selected" : ""}`}
                        key={value}
                      >
                        <input
                          type="radio"
                          name="theme"
                          value={value}
                          checked={theme.preference === value}
                          onChange={() => theme.setPreference(value)}
                        />
                        <Icon size={20} aria-hidden="true" />
                        <span>
                          <strong>{label}</strong>
                          <small>{description}</small>
                        </span>
                        {theme.preference === value && (
                          <Check size={15} aria-hidden="true" />
                        )}
                      </label>
                    ))}
                  </fieldset>
                </section>
                <BackgroundSettingsPanel />
                <UpdateSettings updates={updates} />
                <ScanSettingsPanel />
                <DocumentIndexPanel
                  job={job}
                  disabled={busy || ocrBusy || !!ocrBatch}
                  changed={refresh}
                  notify={notify}
                />
                <SemanticSettings />
                <HiddenItemsPanel
                  disabled={hideLocked}
                  revision={revision}
                  changed={visibilityChanged}
                  notify={notify}
                />
                <section className="settings-section">
                  <h2>Searchable image text</h2>
                  <p>
                    Recognize text from an image preview, or select up to 50
                    images and choose Index image text. Extracted text stays in
                    this device's local search index. Changed files lose old
                    text on rescan; recognize them again to refresh it.
                  </p>
                  <button
                    className="button"
                    disabled={busy || !!ocrBatch || !desktop}
                    onClick={() =>
                      void run(async () => {
                        await api.clearImageText();
                        setRevision((r) => r + 1);
                        notify(
                          "Recognized text index cleared. Original images kept.",
                        );
                      })
                    }
                  >
                    Clear recognized text index
                  </button>
                </section>
                {desktop && <LocalDiagnosticsPanel />}
                <section className="settings-section">
                  <div className="section-heading">
                    <div>
                      <h2>Selected folders</h2>
                      <p>
                        Subfolders are included. Removing a folder only removes
                        it from this library.
                      </p>
                    </div>
                    <button
                      className="button"
                      onClick={() => void addFolder()}
                      disabled={running || busy}
                    >
                      <FolderPlus size={16} /> Add folder
                    </button>
                  </div>
                  {boot?.roots.map((root) => (
                    <div className="root-row" key={root.id}>
                      <span className="folder-avatar">
                        <Folder size={22} />
                      </span>
                      <div>
                        <strong>{folderName(root.path)}</strong>
                        <span>{shortPath(root.path)}</span>
                        <small>
                          {root.last_scan
                            ? `Last scanned ${prettyDate(root.last_scan)}`
                            : "Not scanned yet"}
                        </small>
                      </div>
                      <button
                        className="icon-button"
                        aria-label={`Hide ${folderName(root.path)} from app`}
                        title="Hide folder and everything inside"
                        disabled={hideLocked}
                        onClick={() =>
                          void run(async () => {
                            await api.hideRoot(root.id);
                            await visibilityChanged();
                            notify(
                              "Folder hidden. Restore from Settings → Hidden items.",
                            );
                          })
                        }
                      >
                        <EyeOff size={17} />
                      </button>
                      <button
                        className="icon-button"
                        aria-label={`Remove ${folderName(root.path)} from library`}
                        disabled={running || busy}
                        onClick={() => setConfirm(root)}
                      >
                        <X size={17} />
                      </button>
                    </div>
                  ))}
                  {!boot?.roots.length && (
                    <p className="settings-empty">
                      No folders selected yet. Add one to get started.
                    </p>
                  )}
                </section>
                <section className="settings-section">
                  <h2>Private by design</h2>
                  <div className="setting-detail">
                    <ShieldCheck size={20} />
                    <div>
                      <strong>Entirely on your device</strong>
                      <p>
                        No file uploads, cloud search, analytics, or remote OCR.
                        Scanning and cleanup work offline.
                      </p>
                    </div>
                  </div>
                  <div className="setting-detail">
                    <ScanLine size={20} />
                    <div>
                      <strong>Text recognition</strong>
                      <p>{boot?.ocr || "Checking local capabilities…"}</p>
                    </div>
                  </div>
                  <div className="setting-detail">
                    <RefreshCw size={20} />
                    <div>
                      <strong>Automatic folder updates</strong>
                      <p>
                        {job?.watch_error ||
                          "Selected folders update automatically while the desktop app is running. Downloads settle before indexing. Use Refresh to reconcile missed changes."}{" "}
                        The index preserves unchanged file identities.
                      </p>
                    </div>
                  </div>
                </section>
                <section className="settings-section about-section">
                  <BrandIcon />
                  <div>
                    <h2>
                      DigView <span>v{boot?.version || "0.1.0"}</span>
                    </h2>
                    <p>Built for clarity. Designed to keep your files close.</p>
                    <small>{boot?.platform}</small>
                  </div>
                </section>
              </>
            )}
            <footer className="main-footer">
              <span>
                <ShieldCheck size={13} /> A private home for your files.
              </span>
              <span>
                DigView <span className="footer-dot">✦</span>
              </span>
            </footer>
          </main>
        )}
      </div>
      <ActivityBar job={job} ocr={ocrBusy || !!ocrBatch} updatesBusy={busy} />

      {selected.size > 0 && (
        <div className="selection-bar">
          <span className="selection-count">{selected.size}</span>
          <strong>selected</strong>
          <span>{formatBytes(selectedBytes)}</span>
          <button
            className="text-button"
            onClick={() => setSelected(new Set())}
          >
            Clear selection
          </button>
          <div className="selection-divider" />
          <button
            className="button"
            disabled={hideLocked}
            onClick={() => void hideItems(selectedFiles.map((f) => f.id))}
          >
            <EyeOff size={16} /> Hide from app
          </button>
          <button
            className="button danger"
            disabled={busy || running}
            onClick={() => setConfirm("trash")}
          >
            <Trash2 size={16} /> Move to trash
          </button>
        </div>
      )}
      {folderPicker && (
        <AddFoldersDialog
          onClose={() => setFolderPicker(false)}
          onAdded={async (_paths, warning) => {
            await refresh();
            notify(warning || "Folders added to your library.");
          }}
        />
      )}
      {preview && (
        <Modal title={preview.name} wide onClose={() => setPreview(null)}>
          <div className="preview-body">
            <div className="preview-stage">
              <FilePreview
                key={`${preview.id}:${preview.path}:${preview.modified_token ?? preview.modified}:${preview.size}`}
                file={preview}
                fallback={<Thumbnail file={preview} large />}
              />
            </div>
            <div className="preview-details">
              <div className="preview-badges">
                <span>{preview.extension.toUpperCase() || "FILE"}</span>
                <span>{formatBytes(preview.size)}</span>
              </div>
              <button
                className="button full-width"
                disabled={busy || !desktop}
                onClick={() =>
                  void run(async () => {
                    await api.favourite(
                      preview.id,
                      !personal.favourite_ids.includes(preview.id),
                    );
                    setPersonal(await api.personal());
                    setRevision((r) => r + 1);
                  })
                }
              >
                <Star size={16} />
                {personal.favourite_ids.includes(preview.id)
                  ? "Remove favourite"
                  : "Add to favourites"}
              </button>
              <h3>File details</h3>
              <dl>
                <dt>Location</dt>
                <dd>{shortPath(preview.path)}</dd>
                <dt>Modified</dt>
                <dd>{prettyDate(preview.modified)}</dd>
                <dt>Source</dt>
                <dd>{preview.source || "No saved source information"}</dd>
              </dl>
              <button
                className="button full-width"
                disabled={hideLocked}
                onClick={() => void hideItems([preview.id])}
              >
                <EyeOff size={16} /> Hide from app
              </button>
              <button
                className="button full-width"
                disabled={hideLocked}
                onClick={() => {
                  setFileActions(preview);
                  setPreview(null);
                }}
              >
                <Folder size={16} /> Hide containing folder…
              </button>
              <button
                className="button primary full-width"
                onClick={() => void run(() => api.open(preview.id))}
              >
                <ExternalLink size={16} /> Open original
              </button>
              <button
                className="button full-width"
                onClick={() => void run(() => api.reveal(preview.id))}
              >
                <Folder size={16} /> Show containing folder
              </button>
              {preview.category === "images" && (
                <button
                  className="button full-width"
                  disabled={ocrBusy || busy}
                  onClick={() => {
                    const id = preview.id;
                    const epoch = ++ocrEpoch.current;
                    setOcrBusy(true);
                    api
                      .ocr(id)
                      .then((text) => {
                        if (epoch !== ocrEpoch.current) return;
                        setOcr(text || "");
                        setRevision((r) => r + 1);
                        if (!text)
                          notify("No text was recognized in this image.");
                      })
                      .catch((e) => {
                        if (epoch === ocrEpoch.current)
                          notify(errorText(e), true);
                      })
                      .finally(() => {
                        if (epoch === ocrEpoch.current) setOcrBusy(false);
                      });
                  }}
                >
                  {ocrBusy ? (
                    <Loader2 size={16} className="spinning" />
                  ) : (
                    <ScanLine size={16} />
                  )}{" "}
                  {ocrBusy ? "Reading on your device…" : "Recognize text"}
                </button>
              )}
              <p className="preview-privacy">
                <ShieldCheck size={12} /> Your file stays on this device.
                Recognized text is saved locally and included in library search.
              </p>
            </div>
          </div>
          {ocr !== null && (
            <RecognizedText
              key={preview.id}
              text={ocr}
              write={api.copyText}
              copied={(scope) =>
                notify(
                  scope === "selected"
                    ? "Selected recognized text copied."
                    : "All recognized text copied.",
                )
              }
              failed={(error) => notify(errorText(error), true)}
            />
          )}
        </Modal>
      )}
      {fileActions && (
        <Modal title={fileActions.name} onClose={() => setFileActions(null)}>
          <div className="hide-actions">
            <p>
              Hide items from DigView. Your originals stay in place. Restore
              them from Settings → Hidden items.
            </p>
            <button
              className="button full-width"
              disabled={hideLocked}
              onClick={() => void hideItems([fileActions.id])}
            >
              <EyeOff size={16} /> Hide this file from app
            </button>
            <div className="hide-folder-detail">
              <strong>Containing folder</strong>
              <span>
                {shortPath(fileActions.path).replace(/[\\/][^\\/]+$/, "")}
              </span>
              <p>Hides every file and subfolder inside this folder.</p>
            </div>
            <button
              className="button full-width"
              disabled={hideLocked}
              onClick={() => void hideItems([fileActions.id], true)}
            >
              <Folder size={16} /> Hide containing folder and contents
            </button>
            <button
              className="button full-width"
              onClick={() => setFileActions(null)}
            >
              Cancel
            </button>
          </div>
        </Modal>
      )}
      {confirm && (
        <Modal
          title={
            confirm === "trash"
              ? "Move selected files to trash?"
              : "Remove folder from library?"
          }
          onClose={() => {
            if (!busy) setConfirm(null);
          }}
        >
          {confirm === "trash" ? (
            <>
              <div className="confirm-icon">
                <Trash2 size={27} />
              </div>
              <p className="confirm-description">
                Move{" "}
                <strong>
                  {selected.size} file{selected.size === 1 ? "" : "s"}
                </strong>{" "}
                ({formatBytes(selectedBytes)}) to your system trash. You can
                restore them using your operating system.
              </p>
              <div className="confirm-files">
                {selectedFiles.slice(0, 5).map((f) => (
                  <div key={f.id}>
                    <File size={14} />
                    <span>{f.name}</span>
                    <small>{formatBytes(f.size)}</small>
                  </div>
                ))}
                {selected.size > 5 && (
                  <small>and {selected.size - 5} more</small>
                )}
              </div>
              {view === "duplicates" && (
                <div className="info-note compact">
                  <ShieldCheck size={16} /> Contents are checked again before
                  trashing. A verified copy must remain.
                </div>
              )}
              <div className="modal-actions">
                <button
                  className="button"
                  disabled={busy}
                  onClick={() => setConfirm(null)}
                >
                  Keep files
                </button>
                <button
                  className="button danger"
                  disabled={busy}
                  onClick={() => void doTrash()}
                >
                  {busy ? (
                    <Loader2 size={16} className="spinning" />
                  ) : (
                    <Trash2 size={16} />
                  )}{" "}
                  Move to trash
                </button>
              </div>
            </>
          ) : (
            <>
              <p className="confirm-description">
                Remove <strong>{folderName(confirm.path)}</strong> from this
                library? All original files stay on disk.
              </p>
              <div className="modal-actions">
                <button
                  className="button"
                  disabled={busy}
                  onClick={() => setConfirm(null)}
                >
                  Cancel
                </button>
                <button
                  className="button primary"
                  disabled={busy}
                  onClick={() =>
                    void run(async () => {
                      await api.removeRoot(confirm.id);
                      setConfirm(null);
                      setRootId(undefined);
                      await refresh();
                    })
                  }
                >
                  Remove from library
                </button>
              </div>
            </>
          )}
        </Modal>
      )}
      {help && (
        <Modal title="A little help" onClose={() => setHelp(false)}>
          <div className="help-content">
            <p>
              Start by choosing folders in <strong>Settings & folders</strong>.
              Search names, saved image text and indexed document contents, or
              use a few clues.
            </p>
            <div className="help-example">screenshots from last month</div>
            <div className="help-example">PDFs from last week</div>
            <div className="help-example">videos larger than 500 MB</div>
            <p>
              Search dates use the file’s modified date. “Last week” means the
              previous Monday–Sunday calendar week.
            </p>
            <div className="shortcut">
              <span>Focus search</span>
              <kbd>Ctrl / ⌘ F</kbd>
            </div>
            <div className="shortcut">
              <span>Close preview / clear selection</span>
              <kbd>Esc</kbd>
            </div>
            <p className="footnote">
              Cleanup always asks first. To undo a cleanup, restore files from
              your system trash, then refresh the library.
            </p>
          </div>
        </Modal>
      )}
      {toast && (
        <div
          className={`toast ${toast.error ? "error" : ""}`}
          role={toast.error ? "alert" : "status"}
        >
          {toast.error ? <CircleAlert size={18} /> : <Check size={18} />}
          <span>{toast.text}</span>
          <button
            aria-label="Dismiss notification"
            onClick={() => setToast(null)}
          >
            <X size={16} />
          </button>
        </div>
      )}
    </div>
  );
}

function Nav({
  icon: Icon,
  label,
  count,
  active,
  onClick,
}: {
  icon: LucideIcon;
  label: string;
  count?: number;
  active?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      className={`nav-item ${active ? "active" : ""}`}
      onClick={onClick}
      aria-current={active ? "page" : undefined}
    >
      <Icon size={18} strokeWidth={1.7} />
      <span>{label}</span>
      {count !== undefined && <small>{count.toLocaleString()}</small>}
    </button>
  );
}
function Empty({
  icon: Icon,
  title,
  description,
  action,
}: {
  icon: LucideIcon;
  title: string;
  description: string;
  action?: React.ReactNode;
}) {
  return (
    <div className="empty-state">
      <span>
        <Icon size={31} strokeWidth={1.3} />
      </span>
      <h2>{title}</h2>
      <p>{description}</p>
      {action}
    </div>
  );
}
function FileCard({
  file,
  selected,
  toggle,
  preview,
}: {
  file: FileRecord;
  selected: boolean;
  toggle: () => void;
  preview: () => void;
}) {
  return (
    <article
      data-file-id={file.id}
      className={`file-card ${selected ? "selected" : ""} ${file.search_match ? "has-match" : ""}`}
    >
      <button
        className="file-preview"
        onClick={preview}
        aria-label={`Preview ${file.name}`}
      >
        <Thumbnail file={file} />
        <span className="preview-affordance">
          <Maximize2 size={15} />
        </span>
      </button>
      <button
        className={`card-checkbox selection-toggle ${selected ? "checked" : ""}`}
        aria-label={`Select ${file.name}`}
        aria-pressed={selected}
        onClick={toggle}
      >
        {selected && <Check size={12} />}
      </button>
      <button className="file-label" onClick={preview} title={file.name}>
        {file.name}
      </button>
      {file.search_match && (
        <button
          className="match-snippet"
          onClick={preview}
          title={file.search_match.snippet}
        >
          {file.search_match.snippet}
        </button>
      )}
      <div className="file-meta">
        <span>{formatBytes(file.size)}</span>
        <span>
          {new Date(file.modified * 1000).toLocaleDateString(undefined, {
            month: "short",
            day: "numeric",
          })}
        </span>
      </div>
    </article>
  );
}
function FileBrowser({
  files,
  layout,
  compact,
  selected,
  toggle,
  preview,
  actions,
}: {
  files: FileRecord[];
  layout: "grid" | "list";
  compact: boolean;
  selected: Set<number>;
  toggle: (id: number) => void;
  preview: (f: FileRecord) => void;
  actions: (f: FileRecord) => void;
}) {
  const scroll = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(900);
  useEffect(() => {
    if (!scroll.current) return;
    const observer = new ResizeObserver((entries) =>
      setWidth(entries[0].contentRect.width),
    );
    observer.observe(scroll.current);
    return () => observer.disconnect();
  }, []);
  const columns = layout === "list" ? 1 : Math.max(2, Math.floor(width / 205));
  const hasMatches = files.some((file) => !!file.search_match);
  const rowHeight =
    layout === "list" ? (hasMatches ? 100 : 66) : hasMatches ? 275 : 220;
  const virtualizer = useVirtualizer({
    count: Math.ceil(files.length / columns),
    getScrollElement: () => scroll.current,
    estimateSize: () => rowHeight,
    overscan: 2,
  });
  const renderRow = (row: number) =>
    files.slice(row * columns, (row + 1) * columns).map((file) =>
      layout === "grid" ? (
        <FileCard
          key={file.id}
          file={file}
          selected={selected.has(file.id)}
          toggle={() => toggle(file.id)}
          preview={() => preview(file)}
        />
      ) : (
        <div
          key={file.id}
          className={`file-list-row ${selected.has(file.id) ? "selected" : ""} ${hasMatches ? "has-match" : ""}`}
          data-file-id={file.id}
        >
          <button
            className={`selection-toggle ${selected.has(file.id) ? "checked" : ""}`}
            aria-label={`Select ${file.name}`}
            onClick={() => toggle(file.id)}
          >
            {selected.has(file.id) && <Check size={12} />}
          </button>
          <div className="list-thumb">
            <Thumbnail file={file} />
          </div>
          <button
            className="list-filename"
            onClick={() => preview(file)}
            title={file.name}
          >
            {file.name}
            <small>{file.extension.toUpperCase()}</small>
            {file.search_match && (
              <span className="match-snippet" title={file.search_match.snippet}>
                {file.search_match.snippet}
              </span>
            )}
          </button>
          <span>{prettyDate(file.modified)}</span>
          <span>{formatBytes(file.size)}</span>
          <button
            className="icon-button"
            aria-label={`Preview ${file.name}`}
            onClick={() => preview(file)}
          >
            <ArrowUpRight size={16} />
          </button>
        </div>
      ),
    );
  useEffect(() => {
    scroll.current?.scrollTo({ top: 0 });
  }, [files, layout]);
  return (
    <div
      ref={scroll}
      className={`file-browser ${compact ? "compact-browser" : ""} ${layout === "list" ? "list-browser" : ""}`}
      aria-label="Files"
      onContextMenu={(e) => {
        const element = (e.target as HTMLElement).closest<HTMLElement>(
          "[data-file-id]",
        );
        const file = files.find(
          (f) => f.id === Number(element?.dataset.fileId),
        );
        if (file) {
          e.preventDefault();
          actions(file);
        }
      }}
      onKeyDown={(e) => {
        if (e.key !== "ContextMenu" && !(e.shiftKey && e.key === "F10")) return;
        const element = (e.target as HTMLElement).closest<HTMLElement>(
          "[data-file-id]",
        );
        const file = files.find(
          (f) => f.id === Number(element?.dataset.fileId),
        );
        if (file) {
          e.preventDefault();
          actions(file);
        }
      }}
    >
      {layout === "list" && (
        <div className="list-heading">
          <span />
          <span>Name</span>
          <span>Modified</span>
          <span>Size</span>
          <span />
        </div>
      )}
      {compact ? (
        <div
          className={layout === "grid" ? "static-grid" : "static-list"}
          style={
            layout === "grid"
              ? { gridTemplateColumns: `repeat(${columns},minmax(0,1fr))` }
              : undefined
          }
        >
          {files.map((file) =>
            layout === "grid" ? (
              <FileCard
                key={file.id}
                file={file}
                selected={selected.has(file.id)}
                toggle={() => toggle(file.id)}
                preview={() => preview(file)}
              />
            ) : (
              <div key={file.id}>{renderRow(files.indexOf(file))}</div>
            ),
          )}
        </div>
      ) : (
        <div
          style={{ height: virtualizer.getTotalSize(), position: "relative" }}
        >
          {virtualizer.getVirtualItems().map((row) => (
            <div
              key={row.key}
              className={
                layout === "grid" ? "virtual-grid-row" : "virtual-list-row"
              }
              style={{
                position: "absolute",
                top: 0,
                left: 0,
                width: "100%",
                height: row.size,
                transform: `translateY(${row.start}px)`,
                gridTemplateColumns: `repeat(${columns},minmax(0,1fr))`,
              }}
            >
              {renderRow(row.index)}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
export default App;
