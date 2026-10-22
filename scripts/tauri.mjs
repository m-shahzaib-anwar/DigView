import { spawn } from "node:child_process";
import { resolve } from "node:path";
import { rustEnvironment } from "./rust-env.mjs";
const { env } = rustEnvironment({
  windowsTools: ["build", "dev"].includes(process.argv[2]),
});
const child = spawn(
  process.execPath,
  [resolve("node_modules/@tauri-apps/cli/tauri.js"), ...process.argv.slice(2)],
  { env, stdio: "inherit" },
);
child.on("exit", (code) => process.exit(code ?? 1));
child.on("error", (error) => {
  console.error(error.message);
  process.exit(1);
});
