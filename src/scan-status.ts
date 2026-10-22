export interface ScanPreferences {
  paused: boolean;
  low_resource: boolean;
  pause_on_battery: boolean;
  exclusions: string[];
}
export interface ScanStatus {
  default_exclusions: string[];
  preferences: ScanPreferences;
  battery: boolean | null;
  suspended: boolean;
  running: boolean;
  errors: number;
  watch_error: string;
  folders: {
    id: number;
    path: string;
    last_scan: number | null;
    available: boolean;
  }[];
}
export function scanSummary(s: ScanStatus): string {
  if (s.preferences.paused) return "Scanning paused";
  if (s.suspended) return "Heavy work paused on battery";
  if (s.running) return "Updating library…";
  if (s.watch_error || s.folders.some((f) => !f.available))
    return "Some folders need attention";
  if (s.errors) return "Last task finished with errors — refresh to retry";
  if (!s.folders.length) return "Choose a folder to start";
  if (s.folders.some((f) => !f.last_scan)) return "Waiting for initial scan";
  return "Watching for changes";
}
