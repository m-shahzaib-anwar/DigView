import { createWorker, OEM } from "tesseract.js";
import type { Worker } from "tesseract.js";

let worker: Promise<Worker> | null = null;
let idle: ReturnType<typeof setTimeout> | undefined;
let queue: Promise<unknown> = Promise.resolve();

export function recognizeRaster(source: string): Promise<string> {
  const task = queue
    .catch(() => {})
    .then(async () => {
      clearTimeout(idle);
      const base = new URL("ocr/", document.baseURI).href;
      worker ??= createWorker("eng", OEM.LSTM_ONLY, {
        workerPath: `${base}worker.min.js`,
        corePath: base,
        langPath: base,
        gzip: true,
        cacheMethod: "none",
        workerBlobURL: true,
        errorHandler: () => {
          /* Caller reports recognition failures. No telemetry. */
        },
      });
      try {
        const current = await worker;
        const result = await current.recognize(source);
        return result.data.text.trim();
      } catch (error) {
        const failed = worker;
        worker = null;
        void failed?.then((w) => w.terminate()).catch(() => {});
        throw error;
      } finally {
        idle = setTimeout(() => {
          const current = worker;
          worker = null;
          void current?.then((w) => w.terminate()).catch(() => {});
        }, 45_000);
      }
    });
  queue = task;
  return task;
}
