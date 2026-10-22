import { invoke } from "@tauri-apps/api/core";
import { api, desktop } from "./api";
import type { ScanStatus } from "./scan-status";
import type { Root } from "./types";

export type SetupInitial = {
  suggestions: SetupSuggestion[];
  paths: string[];
  documentContent: boolean;
  exclusions: string[];
  lowResource: boolean;
  pauseOnBattery: boolean;
};
export type SetupSuggestion = {
  key: string;
  label: string;
  path: string;
  available: boolean;
  covered: boolean;
  overlap: boolean;
};

import type {
  SetupStatus,
  BackgroundChoices,
  SetupFolder,
  SetupRequest,
} from "./setup-plan";
export * from "./setup-plan";

let demoCompleted = false;
let demoFolder = 0;
let pendingLibraryNotice = "";
const demoChoices = new Map<string, string>();
const demoRoots = new Set<string>();
function demoChoice(token: string, path: string): SetupFolder {
  demoChoices.set(token, path);
  return { token, path };
}
function addDemoChoices(tokens: string[]) {
  const paths = tokens.map((token) => {
    const path = demoChoices.get(token);
    if (!path) throw new Error("A folder choice expired. Choose it again.");
    return path;
  });
  paths.forEach((path) => demoRoots.add(path));
  return paths;
}
export const setupService = {
  setLibraryNotice: (message: string) => {
    pendingLibraryNotice = message;
  },
  takeLibraryNotice: () => {
    const message = pendingLibraryNotice;
    pendingLibraryNotice = "";
    return message;
  },
  initial: async (): Promise<SetupInitial> => {
    if (!desktop)
      return {
        suggestions: [
          "Documents",
          "Pictures",
          "Videos",
          "Music",
          "Desktop",
          "Downloads",
        ].map((label) => ({
          key: label.toLowerCase(),
          label,
          path: `C:\\Example user\\${label}`,
          available: true,
          covered: demoRoots.has(`C:\\Example user\\${label}`),
          overlap: false,
        })),
        paths: [...demoRoots],
        documentContent: false,
        exclusions: [],
        lowResource: false,
        pauseOnBattery: false,
      };
    const [library, scan, documents, suggestions] = await Promise.all([
      api.bootstrap(),
      invoke<ScanStatus>("scan_status"),
      api.documentStatus(),
      invoke<SetupSuggestion[]>("setup_suggestions"),
    ]);
    return {
      suggestions,
      paths: library.roots.map((root) => root.path),
      documentContent: documents.enabled,
      exclusions: scan.preferences.exclusions,
      lowResource: scan.preferences.low_resource,
      pauseOnBattery: scan.preferences.pause_on_battery,
    };
  },
  status: (): Promise<SetupStatus> =>
    desktop
      ? invoke("setup_status")
      : Promise.resolve({
          completed: demoCompleted,
          version: demoCompleted ? 1 : 0,
        }),
  background: (): Promise<BackgroundChoices> =>
    desktop
      ? invoke("background_settings")
      : Promise.resolve({
          keepRunning: true,
          startMinimized: false,
          runAtStartup: false,
          startupSupported: true,
        }),
  chooseFolder: (): Promise<SetupFolder | null> =>
    desktop
      ? invoke("choose_setup_folder")
      : Promise.resolve(
          demoChoice(`demo-${++demoFolder}`, `Example folder ${demoFolder}`),
        ),
  chooseSuggestion: (suggestion: SetupSuggestion): Promise<SetupFolder> =>
    desktop
      ? invoke("choose_suggested_setup_folder", { key: suggestion.key })
      : Promise.resolve(
          demoChoice(`suggested-${suggestion.key}`, suggestion.path),
        ),
  addFolders: async (tokens: string[]): Promise<string[]> => {
    if (desktop)
      return (await invoke<Root[]>("add_chosen_folders", { tokens })).map(
        (root) => root.path,
      );
    return addDemoChoices(tokens);
  },
  discardFolders: (tokens: string[]) =>
    desktop
      ? invoke<void>("discard_folder_choices", { tokens })
      : Promise.resolve(tokens.forEach((token) => demoChoices.delete(token))),
  saveBackground: (choices: BackgroundChoices) =>
    desktop
      ? invoke("set_background_settings", {
          preferences: {
            keepRunning: choices.keepRunning,
            startMinimized: choices.keepRunning && choices.startMinimized,
          },
          runAtStartup: choices.startupSupported && choices.runAtStartup,
        })
      : Promise.resolve(),
  finish: (request: SetupRequest): Promise<SetupStatus> => {
    if (desktop) return invoke("finish_setup", { request });
    addDemoChoices(request.folders);
    demoCompleted = true;
    return Promise.resolve({ completed: true, version: 1 });
  },
  scan: () => (desktop ? api.scan() : Promise.resolve()),
  openLibrary: () => (desktop ? invoke("open_library") : Promise.resolve()),
};
