import { copyFile, mkdir, readdir } from "node:fs/promises";
import { resolve } from "node:path";

const destination = resolve("public/ocr");
await mkdir(destination, { recursive: true });
await copyFile(
  resolve("node_modules/tesseract.js/dist/worker.min.js"),
  resolve(destination, "worker.min.js"),
);
const core = resolve("node_modules/tesseract.js-core");
for (const name of await readdir(core)) {
  if (/^tesseract-core-(simd-)?lstm\.wasm(\.js)?$/.test(name))
    await copyFile(resolve(core, name), resolve(destination, name));
}
await copyFile(
  resolve(
    "node_modules/@tesseract.js-data/eng/4.0.0_best_int/eng.traineddata.gz",
  ),
  resolve(destination, "eng.traineddata.gz"),
);
await copyFile(
  resolve("node_modules/tesseract.js/LICENSE.md"),
  resolve(destination, "LICENSE-tesseract-js.txt"),
);
await copyFile(
  resolve("node_modules/tesseract.js-core/LICENSE"),
  resolve(destination, "LICENSE-tesseract-core.txt"),
);
await copyFile(
  resolve("node_modules/@tesseract.js-data/eng/package.json"),
  resolve(destination, "language-package-notice.json"),
);
await copyFile(
  resolve("notices/tessdata-LICENSE.txt"),
  resolve(destination, "LICENSE-language-model.txt"),
);
console.log("Offline OCR assets prepared. No runtime CDN requests.");
