import { expect, it, vi } from "vitest";
import {
  updateActions,
  type UpdateInvoke,
  type UpdateStatus,
} from "./update-client";
const available: UpdateStatus = {
  configured: true,
  current: "0.1.0",
  available: "0.2.0",
  notes: "Changes",
};
it("unconfigured update checks make no network or install request", async () => {
  const invoke = vi.fn().mockResolvedValue({ ...available, configured: false });
  const c = updateActions(invoke as UpdateInvoke);
  await expect(c.check()).rejects.toThrow("not configured");
  expect(invoke.mock.calls.map((c) => c[0])).toEqual(["update_status"]);
});
it("installation requires explicit matching-version confirmation and idle library", async () => {
  const invoke = vi.fn().mockResolvedValue(undefined);
  const c = updateActions(invoke as UpdateInvoke);
  for (const [version, confirmed, blocked] of [
    ["0.2.0", false, false],
    ["0.3.0", true, false],
    ["0.2.0", true, true],
  ] as const)
    await expect(
      c.install(available, version, confirmed, blocked),
    ).rejects.toThrow("Confirm");
  expect(invoke).not.toHaveBeenCalled();
  await c.install(available, "0.2.0", true, false);
  expect(invoke).toHaveBeenCalledWith("install_update", {
    confirmedVersion: "0.2.0",
  });
});
it("check failures propagate and native signature errors cannot turn into success", async () => {
  const invoke = vi
    .fn()
    .mockResolvedValueOnce(available)
    .mockRejectedValueOnce(Error("offline"))
    .mockRejectedValueOnce(Error("bad signature"));
  const c = updateActions(invoke as UpdateInvoke);
  await expect(c.check()).rejects.toThrow("offline");
  await expect(c.install(available, "0.2.0", true, false)).rejects.toThrow(
    "signature",
  );
});
