import { spawn } from "node:child_process";
import { writeFile } from "node:fs/promises";
import { rustEnvironment } from "./rust-env.mjs";
import os from "node:os";
const { env, cargo } = rustEnvironment();
const child = spawn(
  cargo,
  [
    "test",
    "--manifest-path",
    "native-core/Cargo.toml",
    "--lib",
    "--no-default-features",
    "benchmark_index",
    "--",
    "--ignored",
    "--nocapture",
  ],
  { env, stdio: ["ignore", "pipe", "inherit"] },
);
let output = "";
child.stdout.on("data", (data) => {
  output += data;
  process.stdout.write(data);
});
child.on("error", (error) => {
  console.error(error.message);
  process.exitCode = 1;
});
child.on("exit", async (code) => {
  if (code !== 0) {
    process.exitCode = code ?? 1;
    return;
  }
  const match = output.match(/BENCH_RESULT (\{[^\n]+\})/);
  if (!match) {
    console.error("No benchmark result was produced.");
    process.exitCode = 1;
    return;
  }
  const report = {
    measured_at: new Date().toISOString(),
    platform: process.platform,
    cpu: os.cpus()[0]?.model,
    logical_cpus: os.cpus().length,
    memory_gb: Math.round(os.totalmem() / 1024 ** 3),
    ...JSON.parse(match[1]),
  };
  await writeFile(
    "docs/performance.json",
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(
    "Saved docs/performance.json. This measures the local SQL engine, not a comparison with another application.",
  );
});
