import { spawn, spawnSync } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

export const projectRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);

export function localBuildEnvironment(base = process.env) {
  const env = { ...base, CARGO_NET_OFFLINE: "true" };
  for (const key of Object.keys(env)) {
    if (
      [
        "DIGVIEW_UPDATER_ENDPOINT",
        "DIGVIEW_UPDATER_PUBLIC_KEY",
        "TAURI_SIGNING_PRIVATE_KEY",
        "TAURI_SIGNING_PRIVATE_KEY_PATH",
        "TAURI_SIGNING_PRIVATE_KEY_PASSWORD",
        "CARGO_TARGET_DIR",
      ].includes(key.toUpperCase())
    )
      delete env[key];
  }
  for (const key of [
    "VITE_OBSERVABILITY_ENABLED",
    "VITE_POSTHOG_ENABLED",
    "VITE_SENTRY_ENABLED",
    "VITE_DIAGNOSTICS_ENABLED",
  ])
    env[key] = "false";
  return env;
}

export function releaseConfiguration(root) {
  const config = JSON.parse(
    fs.readFileSync(path.join(root, "src-tauri/tauri.conf.json"), "utf8"),
  );
  const pkg = JSON.parse(
    fs.readFileSync(path.join(root, "package.json"), "utf8"),
  );
  const cargo = fs
    .readFileSync(path.join(root, "src-tauri/Cargo.toml"), "utf8")
    .split(/\r?\n\[/)[0];
  const nativeVersion = cargo.match(/^\s*version\s*=\s*"([^"]+)"/m)?.[1];
  if (
    !/^\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?(?:\+[A-Za-z0-9.-]+)?$/.test(
      config.version,
    ) ||
    config.version !== pkg.version ||
    config.version !== nativeVersion
  )
    throw Error(
      "Version mismatch: package.json, src-tauri/Cargo.toml and tauri.conf.json must have the same version.",
    );
  if (
    typeof config.productName !== "string" ||
    !config.productName ||
    /[<>:"/\\|?*\x00-\x1f]/.test(config.productName) ||
    /[. ]$/.test(config.productName)
  )
    throw Error("Unsafe product name in tauri.conf.json.");
  return { version: config.version, productName: config.productName };
}

export function buildSteps(python = { command: "python", prefix: [] }) {
  const node = (id, label, args, nativeTarget = false) => ({
    id,
    label,
    command: process.execPath,
    args,
    nativeTarget,
  });
  return [
    node("frontend-tests", "Frontend and build-runner tests", [
      "node_modules/vitest/vitest.mjs",
      "run",
    ]),
    node("native-tests", "Native core tests (generated fixtures only)", [
      "scripts/native.mjs",
      "test",
      "--manifest-path",
      "native-core/Cargo.toml",
      "--lib",
      "--locked",
    ]),
    {
      id: "notices",
      label: "Refresh dependency notices",
      command: python.command,
      args: [...python.prefix, "scripts/notices.py"],
    },
    node("ocr-assets", "Prepare bundled offline OCR", [
      "scripts/prepare-ocr.mjs",
    ]),
    node("pdf-assets", "Prepare bundled offline PDF renderer assets", [
      "scripts/prepare-pdf.mjs",
    ]),
    node("typecheck", "TypeScript check", [
      "node_modules/typescript/bin/tsc",
      "-b",
    ]),
    node("frontend", "Build frontend assets", [
      "node_modules/vite/bin/vite.js",
      "build",
    ]),
    node(
      "native",
      "Build optimized Windows EXE with frozen frontend",
      [
        "scripts/tauri.mjs",
        "build",
        "--no-bundle",
        "--no-sign",
        "--config",
        "scripts/windows-build.config.json",
      ],
      true,
    ),
    node(
      "bundle",
      "Package NSIS installer without changing the EXE",
      [
        "scripts/tauri.mjs",
        "bundle",
        "--bundles",
        "nsis",
        "--no-sign",
        "--config",
        "scripts/windows-bundle.config.json",
        "--no-binary-patching",
      ],
      true,
    ),
  ];
}

const alive = (pid) => {
  if (!Number.isSafeInteger(pid) || pid <= 0) return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return error.code !== "ESRCH";
  }
};

export function acquireBuildLock(root, isAlive = alive) {
  const filename = path.join(root, ".tools/windows-build.lock");
  fs.mkdirSync(path.dirname(filename), { recursive: true });
  if (fs.existsSync(filename)) {
    let old;
    try {
      old = JSON.parse(fs.readFileSync(filename, "utf8"));
    } catch {
      throw Error(
        `Unreadable build lock. Inspect ${filename}; do not remove it while a build is running.`,
      );
    }
    if (isAlive(old.pid) || isAlive(old.childPid))
      throw Error(
        `Another build/worker is still running (PID ${old.childPid || old.pid}). Wait for it; do not start a second build.`,
      );
    fs.unlinkSync(filename); // Only this runner's known stale lock; never build caches or user files.
  }
  const state = {
    id: randomUUID(),
    pid: process.pid,
    childPid: null,
    started: new Date().toISOString(),
  };
  fs.writeFileSync(filename, JSON.stringify(state), { flag: "wx" });
  return {
    child(pid) {
      state.childPid = pid;
      fs.writeFileSync(filename, JSON.stringify(state));
    },
    release() {
      if (state.childPid && isAlive(state.childPid)) return; // Retain an interrupted worker's lock.
      if (
        fs.existsSync(filename) &&
        JSON.parse(fs.readFileSync(filename, "utf8")).id === state.id
      )
        fs.unlinkSync(filename);
    },
  };
}

export function digest(filename) {
  const hash = createHash("sha256"),
    fd = fs.openSync(filename, "r"),
    buffer = Buffer.alloc(1024 * 1024);
  try {
    let length;
    while ((length = fs.readSync(fd, buffer, 0, buffer.length, null)))
      hash.update(buffer.subarray(0, length));
  } finally {
    fs.closeSync(fd);
  }
  return hash.digest("hex");
}

export function distFingerprint(root) {
  const entries = [];
  function walk(dir) {
    for (const entry of fs
      .readdirSync(dir, { withFileTypes: true })
      .sort((a, b) => a.name.localeCompare(b.name))) {
      const filename = path.join(dir, entry.name);
      if (entry.isSymbolicLink())
        throw Error("Symlinks are not allowed in frozen dist assets.");
      if (entry.isDirectory()) walk(filename);
      else if (entry.isFile())
        entries.push([path.relative(root, filename), digest(filename)]);
    }
  }
  walk(path.join(root, "dist"));
  if (!entries.length || !fs.existsSync(path.join(root, "dist/index.html")))
    throw Error("Frontend build produced no dist/index.html.");
  return createHash("sha256").update(JSON.stringify(entries)).digest("hex");
}

function atomicJSON(filename, data) {
  const temp = `${filename}.${randomUUID()}.tmp`;
  fs.writeFileSync(temp, JSON.stringify(data, null, 2) + "\n", { flag: "wx" });
  fs.renameSync(temp, filename);
}

export async function runPipeline({
  root,
  output,
  config,
  run,
  log = () => {},
  wait = (ms) => new Promise((r) => setTimeout(r, ms)),
  python,
  now = () => Date.now(),
  signal,
}) {
  let frozen, exeHash, bundleStarted;
  const sourceExe = path.join(root, "src-tauri/target/release/digview.exe");
  const sourceInstaller = path.join(
    root,
    "src-tauri/target/release/bundle/nsis",
    `${config.productName}_${config.version}_x64-setup.exe`,
  );
  for (const step of buildSteps(python)) {
    if (signal?.aborted)
      throw Error("Build cancelled. No completed release was published.");
    log(`\n=== ${step.label} ===\n`);
    if (step.id === "bundle") bundleStarted = now();
    try {
      await run(step);
    } catch (error) {
      if (
        step.id !== "bundle" ||
        signal?.aborted ||
        !/os error 32|being used by another process|sharing violation/i.test(
          String(error),
        )
      )
        throw error;
      log("Temporary packaging file lock; retrying once in 3 seconds.\n");
      await wait(3000);
      if (signal?.aborted) throw Error("Build cancelled.");
      bundleStarted = now();
      await run({ ...step, args: [...step.args, "--verbose"] });
    }
    if (step.id === "frontend") frozen = distFingerprint(root);
    if (step.id === "native") {
      if (distFingerprint(root) !== frozen)
        throw Error(
          "dist changed while Rust embedded assets. Stop other frontend/dev builds and retry.",
        );
      exeHash = digest(sourceExe);
    }
    if (step.id === "bundle") {
      if (distFingerprint(root) !== frozen || digest(sourceExe) !== exeHash)
        throw Error(
          "Frontend or EXE changed during packaging. Release was not published.",
        );
      if (fs.statSync(sourceInstaller).mtimeMs < bundleStarted - 2000)
        throw Error(
          "Installer output is stale; refusing to publish an older setup file.",
        );
    }
  }
  if (signal?.aborted) throw Error("Build cancelled.");
  const copies = [
    [sourceExe, "digview.exe"],
    [sourceInstaller, `DigView-${config.version}-windows-x64-setup.exe`],
    [
      path.join(root, "src-tauri/THIRD_PARTY_NOTICES.txt"),
      "THIRD_PARTY_NOTICES.txt",
    ],
  ];
  const catalogPath = path.join(root, "releases/checksums.json");
  const catalog = fs.existsSync(catalogPath)
    ? JSON.parse(fs.readFileSync(catalogPath, "utf8"))
    : [];
  if (!Array.isArray(catalog))
    throw Error(
      "Existing releases/checksums.json is invalid; it was not overwritten.",
    );
  const records = [];
  for (const [source, name] of copies) {
    const destination = path.join(output, name);
    fs.copyFileSync(source, destination, fs.constants.COPYFILE_EXCL);
    const sha256 = digest(destination);
    if (sha256 !== digest(source))
      throw Error(`Copy verification failed: ${name}`);
    records.push({ file: name, bytes: fs.statSync(destination).size, sha256 });
  }
  atomicJSON(path.join(output, "checksums.json"), records);
  atomicJSON(path.join(output, "build-info.json"), {
    version: config.version,
    created: new Date(now()).toISOString(),
    profile: "unsigned-local-beta",
    updater: "disabled",
    telemetry: "disabled",
    local_diagnostics: "enabled",
    dist_sha256: frozen,
  });
  atomicJSON(catalogPath, [
    ...catalog,
    ...records.map((record) => ({
      ...record,
      file: path
        .relative(path.join(root, "releases"), path.join(output, record.file))
        .replaceAll("\\", "/"),
    })),
  ]);
  fs.writeFileSync(
    path.join(output, "BUILD-SUCCESS.txt"),
    "Build and installer verification passed. Installer was not executed.\n",
  );
  fs.writeFileSync(path.join(root, "releases/LATEST-BUILD.txt"), output + "\n");
  return records;
}

function pythonCommand(root) {
  for (const candidate of [
    { command: "python", prefix: [] },
    { command: "py", prefix: ["-3"] },
  ]) {
    const result = spawnSync(
      candidate.command,
      [...candidate.prefix, "--version"],
      { cwd: root, encoding: "utf8", windowsHide: true, timeout: 10000 },
    );
    const match = `${result.stdout || ""}${result.stderr || ""}`.match(
      /Python 3\.(\d+)/,
    );
    if (result.status === 0 && match && Number(match[1]) >= 11)
      return candidate;
  }
  throw Error(
    "Python 3.11+ is required for dependency notices. Install it and reopen the launcher.",
  );
}

function preflight(root) {
  if (process.platform !== "win32" || process.arch !== "x64")
    throw Error("This launcher supports Windows x64 only.");
  const [major, minor] = process.versions.node.split(".").map(Number);
  if (!((major === 22 && minor >= 13) || major >= 24))
    throw Error("Node.js 22.13+ (22.x), or 24+ is required.");
  for (const filename of [
    "node_modules/vitest/vitest.mjs",
    "node_modules/typescript/bin/tsc",
    "node_modules/vite/bin/vite.js",
    "node_modules/@tauri-apps/cli/tauri.js",
    "node_modules/pdfjs-dist/package.json",
    ".tools/toolchain/bin/cargo.exe",
    ".tools/toolchain/bin/rustc.exe",
    ".tools/cargo/registry",
    "src-tauri/target/.tauri/NSIS/makensis.exe",
  ]) {
    if (!fs.existsSync(path.join(root, filename)))
      throw Error(
        `Missing ${filename}. Read BUILD-WINDOWS.md setup; this launcher does not download/install dependencies automatically.`,
      );
  }
  if (
    !process.env.VCINSTALLDIR &&
    !fs.existsSync(
      path.join(
        process.env["ProgramFiles(x86)"] || "C:\\Program Files (x86)",
        "Microsoft Visual Studio/Installer/vswhere.exe",
      ),
    )
  )
    throw Error(
      "Visual Studio C++ Build Tools / Windows SDK are missing. Read BUILD-WINDOWS.md.",
    );
  return pythonCommand(root);
}

function commandRunner(root, env, log, lock, signal) {
  return (step) =>
    new Promise((resolve, reject) => {
      if (signal.aborted) {
        reject(Error("Build cancelled."));
        return;
      }
      const childEnv = step.nativeTarget
        ? { ...env, CARGO_TARGET_DIR: path.join(root, "src-tauri/target") }
        : env;
      const child = spawn(step.command, step.args, {
        cwd: root,
        env: childEnv,
        shell: false,
        windowsHide: true,
        stdio: ["ignore", "pipe", "pipe"],
      });
      let tail = "",
        cancelled = false;
      lock.child(child.pid || null);
      const output = (chunk) => {
        const text = chunk.toString();
        tail = (tail + text).slice(-16000);
        log(text);
      };
      child.stdout.on("data", output);
      child.stderr.on("data", output);
      const abort = () => {
        cancelled = true;
        if (!child.pid) return;
        // Only this directly spawned build tree; no user app/process enumeration.
        spawn(
          path.join(
            process.env.SystemRoot || "C:\\Windows",
            "System32/taskkill.exe",
          ),
          ["/PID", String(child.pid), "/T", "/F"],
          { windowsHide: true, stdio: "ignore" },
        ).on("error", () => child.kill());
      };
      signal.addEventListener("abort", abort, { once: true });
      child.on("error", (error) => {
        signal.removeEventListener("abort", abort);
        lock.child(null);
        reject(Error(`${step.label}: ${error.message}`));
      });
      child.on("close", (code) => {
        signal.removeEventListener("abort", abort);
        lock.child(null);
        if (cancelled)
          reject(
            Error("Build cancelled; stop was requested for its worker tree."),
          );
        else if (code === 0) resolve();
        else reject(Error(`${step.label} failed (exit ${code}).\n${tail}`));
      });
    });
}

export async function main(args = process.argv.slice(2)) {
  if (args.includes("--help") || args.includes("--plan")) {
    console.log(
      "DigView Windows x64 local beta builder\nDouble-click Build-Windows.cmd, or: node scripts/build-windows.mjs\nNo installation, cache cleanup, production signing or publishing.\n\nSequential plan:",
    );
    for (const step of buildSteps()) console.log(`- ${step.label}`);
    console.log(
      "\nOutput: releases/builds/<unique UTC timestamp>/; latest path: releases/LATEST-BUILD.txt\nRead BUILD-WINDOWS.md for requirements, manual commands and recovery.",
    );
    return 0;
  }
  if (args.length && !(args.length === 1 && args[0] === "--check"))
    throw Error("Unknown option. Use --help, --plan or --check.");
  const root = projectRoot,
    config = releaseConfiguration(root),
    python = preflight(root);
  if (args[0] === "--check") {
    console.log(
      `Prerequisite paths, Windows x64, Node/Python and version ${config.version} checks passed. No build started.`,
    );
    return 0;
  }
  const lock = acquireBuildLock(root),
    controller = new AbortController();
  const cancel = () => controller.abort();
  process.on("SIGINT", cancel);
  process.on("SIGBREAK", cancel);
  process.on("SIGHUP", cancel);
  let log = console.log;
  try {
    const id =
      new Date()
        .toISOString()
        .replace(/[-:]/g, "")
        .replace("T", "-")
        .replace(/\.\d+Z$/, "Z") +
      "-" +
      randomUUID().slice(0, 8);
    const output = path.join(root, "releases/builds", id);
    fs.mkdirSync(output, { recursive: true });
    const logfile = path.join(output, "build.log");
    log = (text) => {
      process.stdout.write(text);
      fs.appendFileSync(logfile, text);
    };
    log(
      `DigView ${config.version} - unsigned Windows x64 local beta\nLog: ${logfile}\nDo not run dev/frontend/manual builds alongside this build. Ctrl+C cancels this build tree.\nTelemetry and live updater disabled for child builds; .env files are not changed.\n`,
    );
    await runPipeline({
      root,
      output,
      config,
      python,
      log,
      signal: controller.signal,
      run: commandRunner(
        root,
        localBuildEnvironment(),
        log,
        lock,
        controller.signal,
      ),
    });
    log(
      `\nBUILD SUCCESS\nEXE, installer, notices and checksums: ${output}\nInstaller was NOT run. Quit the old app before using Add/Reinstall.\n`,
    );
    return 0;
  } catch (error) {
    log(
      `\nBUILD FAILED: ${error.message}\nNo successful latest build was published by this run. Check the log above; earlier releases are preserved.\n`,
    );
    return 1;
  } finally {
    lock.release();
    process.off("SIGINT", cancel);
    process.off("SIGBREAK", cancel);
    process.off("SIGHUP", cancel);
  }
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href
) {
  main()
    .then((code) => {
      process.exitCode = code;
    })
    .catch((error) => {
      console.error(`BUILD FAILED: ${error.message}`);
      process.exitCode = 1;
    });
}
