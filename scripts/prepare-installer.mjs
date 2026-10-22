import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

// Invoke local build-time commands without changing machine execution policy.
const root = fileURLToPath(new URL("../", import.meta.url));
if (process.platform !== "win32")
  throw new Error(
    "Installer bitmap generation requires Windows; checked-in bitmaps remain usable elsewhere.",
  );
execFileSync(
  "powershell.exe",
  [
    "-NoProfile",
    "-NonInteractive",
    "-Command",
    readFileSync(new URL("./prepare-installer.ps1", import.meta.url), "utf8"),
  ],
  { cwd: root, stdio: "inherit", windowsHide: true },
);
