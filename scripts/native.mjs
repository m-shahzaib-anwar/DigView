import { spawn } from "node:child_process";
import { rustEnvironment } from "./rust-env.mjs";
try {
  const { env, cargo } = rustEnvironment();
  const child = spawn(cargo, process.argv.slice(2), { env, stdio: "inherit" });
  child.on("exit", (code) => process.exit(code ?? 1));
  child.on("error", (error) => {
    console.error(error.message);
    process.exit(1);
  });
} catch (error) {
  console.error(`Native build environment is not ready: ${error.message}`);
  process.exit(1);
}
