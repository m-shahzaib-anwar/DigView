import { copyFile, cp, mkdir } from "node:fs/promises";
import { resolve } from "node:path";

const source = resolve("node_modules/pdfjs-dist");
const destination = resolve("public/pdf");
await mkdir(destination, { recursive: true });
for (const folder of ["cmaps", "standard_fonts", "wasm", "iccs"]) {
  await cp(resolve(source, folder), resolve(destination, folder), {
    recursive: true,
  });
}
await copyFile(
  resolve(source, "LICENSE"),
  resolve(destination, "LICENSE-pdfjs.txt"),
);
console.log(
  "Offline PDF fonts, CMaps and decoders prepared. No runtime CDN requests.",
);
