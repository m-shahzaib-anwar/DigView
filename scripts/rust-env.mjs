import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { resolve, delimiter, join } from "node:path";

export function rustEnvironment({ windowsTools = true } = {}) {
  let env = { ...process.env };
  const inheritedPath = process.env.PATH || process.env.Path || "";
  // A copied JS object loses Windows' case-insensitive environment lookup.
  for (const key of Object.keys(env))
    if (key.toLowerCase() === "path") delete env[key];
  env.PATH = inheritedPath;
  env.CARGO_BUILD_JOBS ??= "4";
  const localBin = resolve(".tools/toolchain/bin");
  const cargoName = process.platform === "win32" ? "cargo.exe" : "cargo";
  const local = existsSync(join(localBin, cargoName));
  if (local) {
    env.CARGO_HOME = resolve(".tools/cargo");
    env.PATH = `${localBin}${delimiter}${env.PATH}`;
  }
  if (process.platform === "win32" && windowsTools && !env.VCINSTALLDIR) {
    const programFiles = env["ProgramFiles(x86)"] || "C:\\Program Files (x86)";
    const vswhere = join(
      programFiles,
      "Microsoft Visual Studio/Installer/vswhere.exe",
    );
    if (existsSync(vswhere)) {
      const installations = execFileSync(
        vswhere,
        [
          "-all",
          "-products",
          "*",
          "-requires",
          "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
          "-property",
          "installationPath",
        ],
        { encoding: "utf8", windowsHide: true },
      )
        .trim()
        .split(/\r?\n/);
      const installation = installations.find((path) =>
        existsSync(join(path, "VC/Auxiliary/Build/vcvars64.bat")),
      );
      if (installation) {
        const batch = join(installation, "VC/Auxiliary/Build/vcvars64.bat");
        // Capture the official developer environment without displaying environment values.
        const commandProcessor = join(
          process.env.SystemRoot || "C:\\Windows",
          "System32/cmd.exe",
        );
        const output = execFileSync(
          commandProcessor,
          ["/d", "/s", "/c", `""${batch}" >nul 2>nul && set"`],
          {
            encoding: "utf8",
            windowsVerbatimArguments: true,
            windowsHide: true,
            env,
          },
        );
        for (const line of output.split(/\r?\n/)) {
          const index = line.indexOf("=");
          if (index > 0) {
            const name = line.slice(0, index);
            env[name.toLowerCase() === "path" ? "PATH" : name] = line.slice(
              index + 1,
            );
          }
        }
      }
    }
  }
  if (local) env.PATH = `${localBin}${delimiter}${env.PATH}`;
  return { env, cargo: local ? join(localBin, cargoName) : cargoName };
}
