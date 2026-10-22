import { useState } from "react";
import { api, desktop } from "./api";
import type { PersonalLibrary, SearchState } from "./personal-library";
export function SavedSearches({
  library,
  current,
  restore,
  changed,
}: {
  library: PersonalLibrary;
  current: SearchState;
  restore: (s: SearchState) => void;
  changed: () => Promise<void>;
}) {
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  async function action(work: () => Promise<unknown>) {
    setBusy(true);
    setError("");
    try {
      await work();
      await changed();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <section className="saved-searches">
      <div className="search-shortcuts-heading">
        <span>Saved searches</span>
      </div>
      {!library.searches.length && (
        <p className="search-shortcuts-empty">
          Save a search to keep its filters
        </p>
      )}
      {library.searches.map((s) => (
        <div className="saved-search-row" key={s.id}>
          <button
            disabled={busy}
            onClick={() => {
              try {
                restore(s.state);
                setError("");
              } catch (e) {
                setError(String(e));
              }
            }}
          >
            {s.name}
          </button>
          <button
            disabled={busy}
            aria-label={`Remove saved search ${s.name}`}
            onClick={() => void action(() => api.removeSearch(s.id))}
          >
            ×
          </button>
        </div>
      ))}
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void action(async () => {
            await api.saveSearch(name, current);
            setName("");
          });
        }}
      >
        <input
          maxLength={80}
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Name current search"
          aria-label="Saved search name"
          disabled={!desktop || busy}
        />
        <button
          className="button small-button"
          disabled={!desktop || busy || !name.trim()}
        >
          Save current filters
        </button>
      </form>
      {error && <p role="alert">{error}</p>}
    </section>
  );
}
