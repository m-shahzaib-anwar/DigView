import { useEffect, useState } from "react";
import { Eye, EyeOff, Folder, File, Search } from "lucide-react";
import { api, desktop } from "./api";
import type { HiddenItem } from "./types";

export function HiddenItemsPanel({
  disabled,
  revision,
  changed,
  notify,
}: {
  disabled: boolean;
  revision: number;
  changed: () => Promise<void>;
  notify: (text: string, error?: boolean) => void;
}) {
  const [items, setItems] = useState<HiddenItem[]>([]);
  const [pending, setPending] = useState(false);
  const [loading, setLoading] = useState(true);
  const [filter, setFilter] = useState("");
  const [error, setError] = useState("");
  const [retry, setRetry] = useState(0);
  useEffect(() => {
    let alive = true;
    setLoading(true);
    api
      .hiddenItems()
      .then((items) => {
        if (alive) {
          setItems(items);
          setError("");
        }
      })
      .catch((e) => {
        if (alive) setError(String(e));
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [revision, retry]);
  const act = async (
    action: () => Promise<boolean | void>,
    message: string,
  ) => {
    setPending(true);
    try {
      if ((await action()) === false) return;
      setItems(await api.hiddenItems());
      await changed();
      notify(message);
    } catch (e) {
      notify(e instanceof Error ? e.message : String(e), true);
    } finally {
      setPending(false);
    }
  };
  const locked = disabled || pending || !desktop;
  const visible = items.filter((item) =>
    item.path.toLocaleLowerCase().includes(filter.toLocaleLowerCase()),
  );
  return (
    <section className="settings-section hidden-settings">
      <div className="section-heading">
        <div>
          <h2>
            <EyeOff size={18} /> Hidden items
          </h2>
          <p>
            Hidden files and folders stay out of this app's library, search,
            favourites, duplicates and cleanup. Originals stay in place.
          </p>
        </div>
        <button
          className="button"
          disabled={locked}
          onClick={() =>
            void act(
              api.chooseHiddenFolder,
              "Folder hidden. Restore it from Hidden items.",
            )
          }
        >
          <Folder size={16} /> Hide folder…
        </button>
      </div>
      <p>
        Folder rules include every file and subfolder inside. These settings
        apply to everyone using this device's local library.
      </p>
      {error ? (
        <div role="alert">
          <p>{error}</p>
          <button
            className="button"
            disabled={pending}
            onClick={() => setRetry((n) => n + 1)}
          >
            Retry hidden items
          </button>
        </div>
      ) : loading ? (
        <p role="status">Loading hidden items…</p>
      ) : items.length === 0 ? (
        <p className="settings-empty">
          Nothing hidden. Right-click a file, use its preview actions, or select
          files and choose Hide from app.
        </p>
      ) : (
        <>
          <label className="hidden-filter">
            <Search size={16} />
            <input
              aria-label="Find hidden items"
              type="search"
              placeholder="Find a hidden file or folder…"
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
            />
            <span>{items.length} hidden rules</span>
          </label>
          <div className="hidden-items-list">
            {visible.map((item) => (
              <div className="root-row" key={item.id}>
                <span className="folder-avatar">
                  {item.folder ? <Folder size={20} /> : <File size={20} />}
                </span>
                <div>
                  <strong>
                    {item.path
                      .replace(/^\\\\\?\\/, "")
                      .split(/[\\/]/)
                      .filter(Boolean)
                      .pop() || item.path}
                  </strong>
                  <span>{item.path.replace(/^\\\\\?\\/, "")}</span>
                  <small>
                    {item.folder ? "Folder and everything inside" : "File"}
                  </small>
                </div>
                <button
                  className="button small-button"
                  disabled={locked}
                  onClick={() =>
                    void act(
                      () => api.unhide(item.id),
                      "Hide rule removed. Any remaining parent folder rule still applies.",
                    )
                  }
                >
                  <Eye size={15} /> Unhide
                  <span className="visually-hidden"> {item.path}</span>
                </button>
              </div>
            ))}
          </div>
          {visible.length === 0 && (
            <p className="settings-empty">No hidden items match.</p>
          )}
          <p className="footnote">
            If a file stays hidden after Unhide, remove its parent folder's hide
            rule too. Moved or renamed files need a new hide rule.
          </p>
        </>
      )}
      {!desktop && (
        <p className="settings-empty">
          Use the desktop app to hide items in your library.
        </p>
      )}
    </section>
  );
}
