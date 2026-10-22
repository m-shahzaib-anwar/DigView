import { afterEach, expect, it, vi } from "vitest";
const mocks = vi.hoisted(() => ({ invoke: vi.fn(), recognize: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("./ocr", () => ({ recognizeRaster: mocks.recognize }));
afterEach(() => {
  vi.unstubAllGlobals();
  vi.resetModules();
  vi.clearAllMocks();
});
async function setup(rejectSave = false) {
  vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
  const network = vi.fn();
  vi.stubGlobal("fetch", network);
  const identity = {
    id: 7,
    path: "selected/image.png",
    size: 10,
    modified_token: "precise",
    digest: "sha256",
  };
  mocks.invoke.mockImplementation(async (command: string) => {
    if (command === "bootstrap")
      return { platform: "windows", version: "0.1.0" };
    if (command === "recognize_text") throw Error("Native OCR unavailable");
    if (command === "ocr_identity") return identity;
    if (command === "get_thumbnail") return "data:image/png;base64,fixture";
    if (command === "save_image_text") {
      if (rejectSave) throw Error("Image changed");
      return;
    }
    throw Error(`Unexpected command ${command}`);
  });
  mocks.recognize.mockResolvedValue("Recognized local content");
  const { api } = await import("./api");
  await api.bootstrap();
  return { api, identity, network };
}
it("bundled OCR fallback persists text against the pre-recognition identity without content HTTP", async () => {
  const { api, identity, network } = await setup();
  expect(await api.ocr(7)).toBe("Recognized local content");
  expect(mocks.invoke).toHaveBeenCalledWith("save_image_text", {
    identity,
    text: "Recognized local content",
  });
  expect(network).not.toHaveBeenCalled();
});
it("failed stale-image persistence is reported and failed OCR does not enter the index", async () => {
  const { api } = await setup(true);
  await expect(api.ocr(7)).rejects.toThrow("Image changed");
  mocks.invoke.mockClear();
  mocks.recognize.mockRejectedValue(Error("worker failed"));
  await expect(api.ocr(7)).rejects.toThrow("worker failed");
  expect(
    mocks.invoke.mock.calls.some((call) => call[0] === "save_image_text"),
  ).toBe(false);
});
