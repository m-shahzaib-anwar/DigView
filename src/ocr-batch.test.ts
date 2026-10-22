import { expect, it } from "vitest";
import { indexImageBatch } from "./ocr-batch";
import type { FileRecord } from "./types";
const file = (id: number, category = "images") =>
  ({ id, category }) as FileRecord;
it("indexes selected images sequentially, skips other files and reports partial failure", async () => {
  let active = 0,
    maxActive = 0;
  const seen: number[] = [];
  const result = await indexImageBatch(
    [file(1), file(2, "documents"), file(3)],
    async (id) => {
      active++;
      maxActive = Math.max(maxActive, active);
      await Promise.resolve();
      seen.push(id);
      active--;
      if (id === 3) throw Error();
      return "text";
    },
    () => false,
    () => {},
  );
  expect(maxActive).toBe(1);
  expect(seen).toEqual([1, 3]);
  expect(result).toEqual({ indexed: 1, failed: 1, cancelled: false });
});
it("stops before the next image on cancellation and rejects oversized batches", async () => {
  let stop = false;
  const seen: number[] = [];
  const result = await indexImageBatch(
    [file(1), file(2)],
    async (id) => {
      seen.push(id);
      stop = true;
      return "";
    },
    () => stop,
    () => {},
  );
  expect(seen).toEqual([1]);
  expect(result.cancelled).toBe(true);
  await expect(
    indexImageBatch(
      Array.from({ length: 51 }, (_, i) => file(i)),
      async () => "",
      () => false,
      () => {},
    ),
  ).rejects.toThrow("50");
});
