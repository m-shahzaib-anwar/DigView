export interface UpdateStatus {
  configured: boolean;
  current: string;
  available: string | null;
  notes: string | null;
}
export type UpdateInvoke = <T>(
  command: string,
  args?: Record<string, unknown>,
) => Promise<T>;
export function updateActions(invoke: UpdateInvoke) {
  return {
    status: () => invoke<UpdateStatus>("update_status"),
    check: async () => {
      const status = await invoke<UpdateStatus>("update_status");
      if (!status.configured)
        throw Error("Signed updates are not configured for this build.");
      return invoke<UpdateStatus>("check_update");
    },
    install: async (
      status: UpdateStatus,
      version: string,
      confirmed: boolean,
      blocked: boolean,
    ) => {
      if (
        !confirmed ||
        blocked ||
        !status.configured ||
        !status.available ||
        status.available !== version
      )
        throw Error(
          "Confirm the available update while library tasks are idle.",
        );
      await invoke<void>("install_update", { confirmedVersion: version });
    },
  };
}
