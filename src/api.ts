import {
  emptyPersonal,
  type PersonalLibrary,
  type SearchState,
} from "./personal-library";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import type { ContentPreview } from "./content-preview-types";
import { localDiagnostics, nativeErrorCode } from "./local-diagnostics";
import { observability, jobObserver } from "./observability";
import type {
  Bootstrap,
  Category,
  DuplicateGroup,
  FileRecord,
  HiddenItem,
  Job,
  LibraryStats,
  Query,
  QueryResult,
  Root,
  TrashResult,
} from "./types";

export const desktop = "__TAURI_INTERNALS__" in window;
let nativePlatform = "";
const observeJob = jobObserver(observability.capture);
let lastLocalJob: Pick<Job, "running" | "revision"> | undefined;
const emptyJob: Job = {
  revision: 0,
  watch_error: "",
  running: false,
  kind: "",
  processed: 0,
  errors: 0,
  message: "",
  cancelled: false,
  duration_ms: 0,
};
const now = Math.floor(Date.now() / 1000);
const seeds: [string, Category, string, number, string | null][] = [
  ["Coastal morning.jpg", "images", "jpg", 3245000, null],
  ["Brand guidelines.pdf", "documents", "pdf", 2450000, "drive.google.com"],
  ["Screenshot — dashboard.png", "images", "png", 1845000, null],
  ["Alpine escape.jpg", "images", "jpg", 5260000, null],
  ["Product walkthrough.mp4", "videos", "mp4", 728000000, "dropbox.com"],
  ["Quarterly report.xlsx", "documents", "xlsx", 348000, "gmail.com"],
  ["Desert light.jpg", "images", "jpg", 4280000, null],
  ["Design assets.zip", "archives", "zip", 48200000, "github.com"],
  ["Quiet hours.mp3", "music", "mp3", 12400000, null],
  ["City after rain.jpg", "images", "jpg", 3850000, null],
  ["Project notes.md", "documents", "md", 12400, null],
  ["Creative tools.dmg", "apps", "dmg", 168000000, "github.com"],
];
const demoFiles: FileRecord[] = Array.from({ length: 84 }, (_, i) => {
  const [name, category, extension, size, source] = seeds[i % seeds.length];
  return {
    id: i + 1,
    root_id: 1,
    path: `/Demo library/${i >= 12 ? `${Math.floor(i / 12)} — ` : ""}${name}`,
    name: i >= 12 ? `${Math.floor(i / 12)} — ${name}` : name,
    category,
    extension,
    size: size + i * 113,
    modified: now - i * 86400,
    created: now - i * 86400,
    source,
    screenshot: name.startsWith("Screenshot"),
    hash: null,
  };
});
function stats(): LibraryStats {
  return {
    count: demoFiles.length,
    bytes: demoFiles.reduce((n, f) => n + f.size, 0),
    categories: [
      "images",
      "videos",
      "music",
      "documents",
      "archives",
      "apps",
      "other",
    ].map((category) => ({
      category: category as Category,
      count: demoFiles.filter((f) => f.category === category).length,
      bytes: demoFiles
        .filter((f) => f.category === category)
        .reduce((n, f) => n + f.size, 0),
    })),
    sources: [
      ...new Set(
        demoFiles.map((f) => f.source).filter((s): s is string => !!s),
      ),
    ].map((source) => ({
      source,
      count: demoFiles.filter((f) => f.source === source).length,
      bytes: demoFiles
        .filter((f) => f.source === source)
        .reduce((n, f) => n + f.size, 0),
    })),
    large: demoFiles.filter((f) => f.size >= 104857600).length,
    screenshots: demoFiles.filter(
      (f) => f.screenshot && f.modified < now - 30 * 86400,
    ).length,
    downloads: demoFiles.filter(
      (f) => f.source && f.modified < now - 90 * 86400,
    ).length,
    duplicate_groups: 0,
    duplicate_bytes: 0,
  };
}
function demoQuery(query: Query): QueryResult {
  if (query.favourites) return { files: [], total: 0, bytes: 0 };
  const files = demoFiles.filter(
    (f) =>
      (!query.category || f.category === query.category) &&
      (!query.source || f.source === query.source) &&
      (!query.screenshot || f.screenshot) &&
      (!query.root_id || f.root_id === query.root_id) &&
      (query.min_size === undefined || f.size >= query.min_size) &&
      (query.max_size === undefined || f.size <= query.max_size) &&
      (query.date_from === undefined || f.modified >= query.date_from) &&
      (query.date_to === undefined || f.modified < query.date_to) &&
      query.terms.every((t) =>
        t.startsWith("ext:")
          ? f.extension === t.slice(4)
          : f.name.toLowerCase().includes(t.toLowerCase()),
      ) &&
      (!query.cleanup ||
        (query.cleanup === "large" && f.size >= 104857600) ||
        (query.cleanup === "screenshots" &&
          f.screenshot &&
          f.modified < now - 30 * 86400) ||
        (query.cleanup === "downloads" &&
          !!f.source &&
          f.modified < now - 90 * 86400)),
  );
  files.sort((a, b) =>
    query.sort === "name"
      ? a.name.localeCompare(b.name)
      : query.sort === "size"
        ? b.size - a.size
        : query.sort === "oldest"
          ? a.modified - b.modified
          : b.modified - a.modified,
  );
  return {
    files: files.slice(query.offset, query.offset + query.limit),
    total: files.length,
    bytes: files.reduce((n, f) => n + f.size, 0),
  };
}

// Browser mode is a labeled preview. It never simulates a successful filesystem action.
async function call<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (desktop) {
    const started = performance.now();
    try {
      const result = await invoke<T>(command, args);
      if (
        ![
          "job_status",
          "get_thumbnail",
          "query_files",
          "close_preview",
        ].includes(command)
      )
        localDiagnostics.record({
          event: "native_action",
          action: command,
          duration_ms: Math.round(performance.now() - started),
        });
      if (command === "job_status") observeJob(result as Job);
      if (command === "job_status") {
        const job = result as Job;
        if (lastLocalJob?.running && !job.running)
          localDiagnostics.record({
            event: "job_finished",
            action: job.kind,
            duration_ms: job.duration_ms,
            processed: job.processed,
            errors: job.errors,
            cancelled: job.cancelled,
          });
        lastLocalJob = { running: job.running, revision: job.revision };
      } else if (
        !["get_thumbnail", "query_files", "bootstrap"].includes(command)
      )
        observability.capture("native_action", {
          action: command,
          duration_ms: performance.now() - started,
          success: true,
        });
      return result;
    } catch (error) {
      localDiagnostics.record({
        event: "native_failed",
        action: command,
        code: nativeErrorCode(error),
        duration_ms: Math.round(performance.now() - started),
      });
      observability.capture(
        "native_failed",
        {
          action: command,
          code: "request_failed",
          duration_ms: performance.now() - started,
        },
        true,
      );
      throw error;
    }
  }
  if (command === "bootstrap")
    return {
      roots: [{ id: 1, path: "/Demo library", added_at: now, last_scan: now }],
      stats: stats(),
      job: emptyJob,
      platform: "Browser preview",
      ocr: "Desktop app required",
      version: "0.1.0",
    } as T;
  if (command === "job_status") return emptyJob as T;
  if (command === "query_files") return demoQuery(args?.query as Query) as T;
  if (command === "duplicate_groups") return [] as T;
  throw new Error(
    "This is the browser preview. Use the desktop app to work with your real files.",
  );
}
export const api = {
  bootstrap: async () => {
    const data = await call<Bootstrap>("bootstrap");
    nativePlatform = data.platform;
    if (desktop) observability.setContext(data.platform, data.version);
    return data;
  },
  job: () => call<Job>("job_status"),
  chooseFolder: () => call<Root | null>("choose_folder"),
  removeRoot: (id: number) => call<void>("remove_root", { id }),
  hiddenItems: () =>
    desktop ? call<HiddenItem[]>("hidden_items") : Promise.resolve([]),
  hideFiles: (ids: number[], containingFolders = false) =>
    call<void>("hide_files", { ids, containingFolders }),
  hideRoot: (id: number) => call<void>("hide_root", { id }),
  chooseHiddenFolder: () => call<boolean>("choose_hidden_folder"),
  unhide: (id: number) => call<void>("unhide_item", { id }),
  scan: () => call<void>("start_scan"),
  documentStatus: () =>
    call<{
      enabled: boolean;
      indexed: number;
      partial: number;
      empty: number;
      failed: number;
      pending: number;
    }>("document_index_status"),
  enableDocumentIndex: (enabled: boolean) =>
    call<void>("set_document_index_enabled", { enabled }),
  indexDocuments: (retry = false) =>
    call<void>("start_document_index", { retry }),
  clearDocumentIndex: () => call<void>("clear_document_index"),
  personal: () =>
    desktop
      ? call<PersonalLibrary>("personal_library")
      : Promise.resolve(emptyPersonal),
  favourite: (id: number, enabled: boolean) =>
    call<void>("set_favourite", { id, enabled }),
  saveSearch: (name: string, filters: SearchState) =>
    call<void>("save_search", { name, filters }),
  removeSearch: (id: number) => call<void>("remove_search", { id }),
  clearImageText: () => call<void>("clear_image_text"),
  imageText: (id: number) =>
    desktop
      ? call<string | null>("get_image_text", { id })
      : Promise.resolve(null),
  duplicates: () => call<void>("start_duplicates"),
  cancel: () => call<void>("cancel_job"),
  query: (query: Query) => call<QueryResult>("query_files", { query }),
  groups: () => call<DuplicateGroup[]>("duplicate_groups"),
  thumbnail: (id: number, large = false) =>
    call<string>("get_thumbnail", { id, large }),
  openPreview: (id: number) => call<ContentPreview>("open_preview", { id }),
  closePreview: (token: string) => call<void>("close_preview", { token }),
  previewUrl: (token: string) => {
    if (!/^[a-f0-9]{64}$/.test(token))
      throw Error("Invalid local preview session.");
    return convertFileSrc(token, "digview-preview");
  },
  open: (id: number) => call<void>("open_file", { id }),
  reveal: (id: number) => call<void>("reveal_file", { id }),
  copyText: async (text: string) => {
    if (desktop) {
      const { writeText } =
        await import("@tauri-apps/plugin-clipboard-manager");
      await writeText(text);
    } else {
      await navigator.clipboard.writeText(text);
    }
  },
  ocr: async (id: number) => {
    if (!desktop)
      throw new Error("Use the desktop app to recognize text from your files.");
    if (nativePlatform === "windows") {
      try {
        return await call<string>("recognize_text", { id });
      } catch {
        /* Bundled local engine also works when native language packs are absent. */
      }
    }
    const identity = await call<{
      id: number;
      path: string;
      size: number;
      modified_token: string;
      digest: string;
    }>("ocr_identity", { id });
    const raster = await api.thumbnail(id, true);
    const { recognizeRaster } = await import("./ocr");
    const text = await recognizeRaster(raster);
    await call<void>("save_image_text", { identity, text });
    return text;
  },
  trash: (files: FileRecord[], duplicateOnly: boolean) =>
    call<TrashResult>("trash_files", {
      files: files.map((file) => ({
        id: file.id,
        path: file.path,
        size: file.size,
        modified_token: file.modified_token ?? "",
      })),
      duplicateOnly,
    }),
};

// Original lightweight artwork for the explicitly labeled preview, never remote assets.
export function demoArt(file: FileRecord): string {
  const palettes = [
    ["#d7e8e3", "#82aaa1", "#305d58", "#efcca4"],
    ["#e4d8ef", "#aca4c5", "#655f87", "#f6d5a4"],
    ["#eee0c9", "#d6a677", "#a87049", "#f8d999"],
    ["#d1dce8", "#8d9fb1", "#455969", "#f5d1a6"],
  ];
  const p = palettes[file.id % 4];
  const screenshot = file.screenshot;
  const shapes = screenshot
    ? `<rect x="35" y="22" width="330" height="210" rx="10" fill="#fff"/><rect x="35" y="22" width="62" height="210" rx="10" fill="#f0f3f9"/><rect x="113" y="42" width="110" height="9" rx="4" fill="${p[1]}"/><rect x="113" y="70" width="100" height="65" rx="7" fill="${p[0]}"/><rect x="225" y="70" width="122" height="65" rx="7" fill="${p[0]}"/><path d="M120 200 L150 170 L180 185 L215 150 L250 170 L290 140 L340 155" fill="none" stroke="${p[2]}" stroke-width="4"/>`
    : `<circle cx="300" cy="65" r="27" fill="${p[3]}"/><path d="M0 180L105 68L220 190L300 100L400 180V260H0Z" fill="${p[1]}"/><path d="M0 205L160 130L270 213L400 165V260H0Z" fill="${p[2]}"/><path d="M0 229Q100 204 200 230T400 220V260H0Z" fill="${p[0]}" opacity=".6"/>`;
  return `data:image/svg+xml,${encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" width="400" height="260" viewBox="0 0 400 260"><rect width="400" height="260" fill="${p[0]}"/>${shapes}</svg>`)}`;
}
