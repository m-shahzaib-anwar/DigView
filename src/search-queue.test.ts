import { describe, expect, it } from "vitest";
import { latestSearchQueue } from "./search-queue";
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}
describe("latest semantic search queue", () => {
  it("runs only the newest waiting query after the active query finishes", async () => {
    const queue = latestSearchQueue(),
      active = deferred<string>(),
      calls: string[] = [];
    const first = queue.run(() => {
      calls.push("first");
      return active.promise;
    });
    await Promise.resolve();
    const middle = queue.run(async () => {
      calls.push("middle");
      return "middle";
    });
    const skipped = expect(middle).rejects.toThrow("Superseded");
    const last = queue.run(async () => {
      calls.push("last");
      return "last";
    });
    expect(calls).toEqual(["first"]);
    active.resolve("first");
    await first;
    await skipped;
    expect(await last).toBe("last");
    expect(calls).toEqual(["first", "last"]);
  });
  it("invalidates queued work immediately while the new text is still debouncing", async () => {
    const queue = latestSearchQueue(),
      active = deferred<void>();
    const first = queue.run(() => active.promise);
    await Promise.resolve();
    let invoked = false;
    const pending = queue.run(async () => {
      invoked = true;
    });
    const skipped = expect(pending).rejects.toThrow("Superseded");
    queue.invalidate();
    active.resolve();
    await first;
    await skipped;
    expect(invoked).toBe(false);
    expect(await queue.run(async () => "new text")).toBe("new text");
  });
  it("allows the next search after a native failure", async () => {
    const queue = latestSearchQueue();
    await expect(
      queue.run(async () => {
        throw new Error("offline");
      }),
    ).rejects.toThrow("offline");
    expect(await queue.run(async () => "fallback recovered")).toBe(
      "fallback recovered",
    );
  });
});
