import fs from "node:fs";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";
// Node build tooling is deliberately independent of the renderer TypeScript project.
import {
  acquireBuildLock,
  buildSteps,
  digest,
  localBuildEnvironment,
  projectRoot,
  releaseConfiguration,
  runPipeline,
} from "./build-windows.mjs";

const roots: string[] = [];
afterEach(() => {
  for (const root of roots.splice(0))
    fs.rmSync(root, { recursive: true, force: true });
});
function fixture() {
  const temporary = path.join(projectRoot, ".tools/build-runner-tests");
  fs.mkdirSync(temporary, { recursive: true });
  const root = fs.mkdtempSync(path.join(temporary, "fixture-"));
  roots.push(root);
  const write = (file: string, text: string) => {
    const dest = path.join(root, file);
    fs.mkdirSync(path.dirname(dest), { recursive: true });
    fs.writeFileSync(dest, text);
  };
  write("package.json", '{"version":"0.1.0"}');
  write(
    "src-tauri/tauri.conf.json",
    '{"version":"0.1.0","productName":"DigView"}',
  );
  write(
    "src-tauri/Cargo.toml",
    '[package]\nname = "digview"\nversion = "0.1.0"\n\n[dependencies]\n',
  );
  write("src-tauri/THIRD_PARTY_NOTICES.txt", "notices");
  write(
    "releases/checksums.json",
    '[{"file":"previous.exe","sha256":"preserve"}]',
  );
  write("releases/LATEST-BUILD.txt", "previous build");
  const output = path.join(root, "releases/builds/test");
  fs.mkdirSync(output, { recursive: true });
  const installer =
    "src-tauri/target/release/bundle/nsis/DigView_0.1.0_x64-setup.exe";
  const calls: string[] = [];
  const run = async (step: { id: string }) => {
    calls.push(step.id);
    if (step.id === "frontend")
      write("dist/index.html", "<html>new assets</html>");
    if (step.id === "native")
      write("src-tauri/target/release/digview.exe", "new executable");
    if (step.id === "bundle") write(installer, "new installer");
  };
  return {
    root,
    output,
    write,
    installer,
    calls,
    run,
    config: releaseConfiguration(root),
  };
}

describe("Windows build orchestration", () => {
  it("isolates local unsigned/offline environment without editing its source", () => {
    const base = {
      DIGVIEW_UPDATER_ENDPOINT: "https://example.invalid",
      TAURI_SIGNING_PRIVATE_KEY: "private-test-value",
      CARGO_TARGET_DIR: "other",
      VITE_POSTHOG_ENABLED: "true",
      KEEP: "value",
    };
    const env = localBuildEnvironment(base);
    expect(env.DIGVIEW_UPDATER_ENDPOINT).toBeUndefined();
    expect(env.TAURI_SIGNING_PRIVATE_KEY).toBeUndefined();
    expect(env.CARGO_TARGET_DIR).toBeUndefined();
    expect(env.CARGO_NET_OFFLINE).toBe("true");
    expect(env.VITE_POSTHOG_ENABLED).toBe("false");
    expect(env.KEEP).toBe("value");
    expect(base.VITE_POSTHOG_ENABLED).toBe("true");
    expect(base.CARGO_TARGET_DIR).toBe("other");
  });
  it("refuses version mismatch and path characters in product names", () => {
    const f = fixture();
    f.write("package.json", '{"version":"0.2.0"}');
    expect(() => releaseConfiguration(f.root)).toThrow("Version mismatch");
    f.write("package.json", '{"version":"0.1.0"}');
    f.write(
      "src-tauri/tauri.conf.json",
      '{"version":"0.1.0","productName":"../outside"}',
    );
    expect(() => releaseConfiguration(f.root)).toThrow("Unsafe product");
  });
  it("blocks a concurrent runner and retains the lock for a live worker", () => {
    const f = fixture();
    const live = (pid: number) => pid === process.pid || pid === 12345;
    const lock = acquireBuildLock(f.root, live);
    expect(() => acquireBuildLock(f.root, live)).toThrow("still running");
    lock.child(12345);
    lock.release();
    expect(fs.existsSync(path.join(f.root, ".tools/windows-build.lock"))).toBe(
      true,
    );
    lock.child(null);
    lock.release();
    expect(fs.existsSync(path.join(f.root, ".tools/windows-build.lock"))).toBe(
      false,
    );
  });
  it("recovers a dead runner only when its worker is also dead", () => {
    const f = fixture();
    f.write(".tools/windows-build.lock", '{"pid":111,"childPid":222}');
    expect(() =>
      acquireBuildLock(f.root, (pid: number) => pid === 222),
    ).toThrow("still running");
    const lock = acquireBuildLock(f.root, () => false);
    lock.release();
    expect(fs.existsSync(path.join(f.root, ".tools/windows-build.lock"))).toBe(
      false,
    );
  });
  it("publishes verified copies in order and preserves historical checksums", async () => {
    const f = fixture();
    const records = await runPipeline(f);
    expect(f.calls).toEqual(
      buildSteps().map((step: { id: string }) => step.id),
    );
    expect(records).toHaveLength(3);
    for (const record of records)
      expect(digest(path.join(f.output, record.file))).toBe(record.sha256);
    const catalog = JSON.parse(
      fs.readFileSync(path.join(f.root, "releases/checksums.json"), "utf8"),
    );
    expect(catalog[0].file).toBe("previous.exe");
    expect(catalog).toHaveLength(4);
    expect(
      fs
        .readFileSync(path.join(f.root, "releases/LATEST-BUILD.txt"), "utf8")
        .trim(),
    ).toBe(f.output);
    expect(fs.existsSync(path.join(f.output, "BUILD-SUCCESS.txt"))).toBe(true);
  });
  it.each(["frontend-tests", "native-tests", "native"])(
    "stops on %s failure without publishing",
    async (failure) => {
      const f = fixture();
      await expect(
        runPipeline({
          ...f,
          run: async (step: { id: string }) => {
            if (step.id === failure) throw Error("test failure");
            await f.run(step);
          },
        }),
      ).rejects.toThrow("test failure");
      expect(f.calls).not.toContain("bundle");
      expect(
        fs.readFileSync(path.join(f.root, "releases/LATEST-BUILD.txt"), "utf8"),
      ).toBe("previous build");
      expect(fs.existsSync(path.join(f.output, "BUILD-SUCCESS.txt"))).toBe(
        false,
      );
    },
  );
  it("rejects frontend changes while native embedding is running", async () => {
    const f = fixture();
    await expect(
      runPipeline({
        ...f,
        run: async (step: { id: string }) => {
          await f.run(step);
          if (step.id === "native") f.write("dist/index.html", "changed");
        },
      }),
    ).rejects.toThrow("dist changed");
    expect(f.calls).not.toContain("bundle");
  });
  it("rejects EXE changes during bundling and old installer outputs", async () => {
    const f = fixture();
    await expect(
      runPipeline({
        ...f,
        run: async (step: { id: string }) => {
          await f.run(step);
          if (step.id === "bundle")
            f.write("src-tauri/target/release/digview.exe", "changed");
        },
      }),
    ).rejects.toThrow("EXE changed");
    const stale = fixture();
    await expect(
      runPipeline({
        ...stale,
        run: async (step: { id: string }) => {
          await stale.run(step);
          if (step.id === "bundle")
            fs.utimesSync(path.join(stale.root, stale.installer), 1, 1);
        },
      }),
    ).rejects.toThrow("stale");
  });
  it("retries a transient packaging lock once and does not retry other errors", async () => {
    const f = fixture();
    let attempts = 0;
    let waits = 0;
    await runPipeline({
      ...f,
      wait: async () => {
        waits++;
      },
      run: async (step: { id: string; args: string[] }) => {
        if (step.id === "bundle" && ++attempts === 1)
          throw Error("os error 32");
        await f.run(step);
      },
    });
    expect(attempts).toBe(2);
    expect(waits).toBe(1);
    const failed = fixture();
    let failedAttempts = 0;
    await expect(
      runPipeline({
        ...failed,
        run: async (step: { id: string }) => {
          if (step.id === "bundle") {
            failedAttempts++;
            throw Error("packaging failed");
          }
          await failed.run(step);
        },
      }),
    ).rejects.toThrow("packaging failed");
    expect(failedAttempts).toBe(1);
  });
  it("honours cancellation before the next stage and does not publish", async () => {
    const f = fixture();
    const controller = new AbortController();
    await expect(
      runPipeline({
        ...f,
        signal: controller.signal,
        run: async (step: { id: string }) => {
          await f.run(step);
          if (step.id === "frontend") controller.abort();
        },
      }),
    ).rejects.toThrow("cancelled");
    expect(f.calls).not.toContain("native");
    expect(fs.existsSync(path.join(f.output, "BUILD-SUCCESS.txt"))).toBe(false);
  });
});
