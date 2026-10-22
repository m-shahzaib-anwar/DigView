export type SetupFolder = { token: string; path: string };
export type SetupStatus = { completed: boolean; version: number };
export type BackgroundChoices = {
  keepRunning: boolean;
  startMinimized: boolean;
  runAtStartup: boolean;
  startupSupported: boolean;
};
export type SetupChoices = {
  folders: string[];
  documentContent: boolean;
  exclusions: string[];
  lowResource: boolean;
  pauseOnBattery: boolean;
  background: BackgroundChoices;
};
export type SetupRequest = {
  folders: string[];
  document_content: boolean;
  exclusions: string[];
  low_resource: boolean;
  pause_on_battery: boolean;
};
export interface SetupPort {
  saveBackground: (choices: BackgroundChoices) => Promise<unknown>;
  finish: (request: SetupRequest) => Promise<SetupStatus>;
  scan: () => Promise<unknown>;
}
// Completion is durable before any scan. Scan failure cannot undo a saved setup.
export async function completeSetup(port: SetupPort, choices: SetupChoices) {
  if (
    choices.folders.length > 32 ||
    new Set(choices.folders).size !== choices.folders.length
  )
    throw new Error("Choose at most 32 different folders.");
  const exclusions = choices.exclusions.map((s) => s.trim()).filter(Boolean);
  if (exclusions.length > 100) throw new Error("Use at most 100 exclusions.");
  await port.saveBackground(choices.background);
  const status = await port.finish({
    folders: choices.folders,
    document_content: choices.documentContent,
    exclusions,
    low_resource: choices.lowResource,
    pause_on_battery: choices.pauseOnBattery,
  });
  if (!status.completed)
    throw new Error("Setup was not saved. Please try again.");
  let scanWarning = "";
  if (choices.folders.length) {
    try {
      await port.scan();
    } catch {
      scanWarning =
        "Your folders are saved. If indexing has not started, use Refresh in the library.";
    }
  }
  return { status, scanWarning };
}
