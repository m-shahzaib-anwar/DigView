import { expect, it, vi } from "vitest";
import { completeSetup, type SetupChoices, type SetupPort } from "./setup-plan";
const choices: SetupChoices = {
  folders: ["approved-token"],
  documentContent: true,
  exclusions: [" *.log "],
  lowResource: false,
  pauseOnBattery: false,
  background: {
    keepRunning: true,
    startMinimized: false,
    runAtStartup: false,
    startupSupported: true,
  },
};
function fixture() {
  const events: string[] = [];
  const port: SetupPort = {
    saveBackground: vi.fn(async () => {
      events.push("preferences");
    }),
    finish: vi.fn(async () => {
      events.push("commit");
      return { completed: true, version: 1 };
    }),
    scan: vi.fn(async () => {
      events.push("scan");
    }),
  };
  return { port, events };
}
it("starts scanning only after explicit preferences and durable completion", async () => {
  const { port, events } = fixture();
  await completeSetup(port, choices);
  expect(events).toEqual(["preferences", "commit", "scan"]);
  expect(port.finish).toHaveBeenCalledWith({
    folders: ["approved-token"],
    document_content: true,
    exclusions: ["*.log"],
    low_resource: false,
    pause_on_battery: false,
  });
});
it("does not commit or scan if saving preferences fails", async () => {
  const { port } = fixture();
  vi.mocked(port.saveBackground).mockRejectedValue(Error("preference failure"));
  await expect(completeSetup(port, choices)).rejects.toThrow(
    "preference failure",
  );
  expect(port.finish).not.toHaveBeenCalled();
  expect(port.scan).not.toHaveBeenCalled();
});
it("does not scan an unsaved or failed setup", async () => {
  const { port } = fixture();
  vi.mocked(port.finish)
    .mockRejectedValueOnce(Error("expired choice"))
    .mockResolvedValueOnce({ completed: false, version: 0 });
  await expect(completeSetup(port, choices)).rejects.toThrow("expired");
  await expect(completeSetup(port, choices)).rejects.toThrow("not saved");
  expect(port.scan).not.toHaveBeenCalled();
});
it("treats scan-start failure as recoverable after saved consent", async () => {
  const { port } = fixture();
  vi.mocked(port.scan).mockRejectedValue(Error("already running"));
  const result = await completeSetup(port, choices);
  expect(result.status.completed).toBe(true);
  expect(result.scanWarning).toContain("saved");
});
it("can finish with no folders without starting any scan", async () => {
  const { port } = fixture();
  await completeSetup(port, { ...choices, folders: [] });
  expect(port.finish).toHaveBeenCalledOnce();
  expect(port.scan).not.toHaveBeenCalled();
});
it("rejects duplicate and over-limit requests before preferences change", async () => {
  const { port } = fixture();
  await expect(
    completeSetup(port, { ...choices, folders: ["same", "same"] }),
  ).rejects.toThrow();
  await expect(
    completeSetup(port, {
      ...choices,
      folders: Array.from({ length: 33 }, (_, i) => String(i)),
    }),
  ).rejects.toThrow();
  expect(port.saveBackground).not.toHaveBeenCalled();
});
