// Serialize native searches and discard intermediate typing requests.
export function latestSearchQueue() {
  let sequence = 0;
  let tail: Promise<void> = Promise.resolve();
  return {
    invalidate() {
      sequence++;
    },
    run<T>(work: () => Promise<T>): Promise<T> {
      const ticket = ++sequence;
      const request = tail.then(() => {
        if (ticket !== sequence) throw new Error("Superseded semantic query");
        return work();
      });
      tail = request.then(
        () => undefined,
        () => undefined,
      );
      return request;
    },
  };
}
