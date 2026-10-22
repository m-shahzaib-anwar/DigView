import type { FileRecord } from "./types";
export async function indexImageBatch(
  files: FileRecord[],
  recognize: (id: number) => Promise<string>,
  cancelled: () => boolean,
  progress: (done: number, total: number) => void,
) {
  const images = files.filter((f) => f.category === "images");
  if (!images.length || images.length > 50)
    throw new Error("Select between 1 and 50 images to index text.");
  let indexed = 0,
    failed = 0,
    done = 0;
  for (const image of images) {
    if (cancelled()) break;
    try {
      await recognize(image.id);
      indexed++;
    } catch {
      failed++;
    }
    progress(++done, images.length);
  }
  return { indexed, failed, cancelled: cancelled() };
}
