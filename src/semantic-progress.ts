// Completion is tied to successful task completion, never to a rounded counter.
export function taskPercent(
  completed: number,
  total: number,
  succeeded: boolean,
): number {
  if (succeeded) return 100;
  if (!Number.isFinite(completed) || !Number.isFinite(total) || total <= 0)
    return 0;
  return Math.max(0, Math.min(99, Math.floor((completed / total) * 100)));
}
