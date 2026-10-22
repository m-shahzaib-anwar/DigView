import { useEffect, useRef, useState, type RefObject } from "react";
import { Clock3, Search, X } from "lucide-react";
import {
  readRecentSearches,
  rememberSearch,
  writeRecentSearches,
} from "./recent-searches";

export function SearchBox({
  inputRef,
  value,
  onChange,
}: {
  inputRef: RefObject<HTMLInputElement | null>;
  value: string;
  onChange: (text: string) => void;
}) {
  const container = useRef<HTMLDivElement>(null);
  const pending = useRef(false);
  const [open, setOpen] = useState(false);
  const [recent, setRecent] = useState(() => {
    try {
      return readRecentSearches(localStorage);
    } catch {
      return [];
    }
  });
  const [error, setError] = useState("");
  function persist(next: string[]) {
    setRecent(next);
    try {
      writeRecentSearches(localStorage, next);
      setError("");
    } catch {
      setError("Search history could not be saved on this device.");
    }
  }
  function commit() {
    if (!pending.current) return;
    pending.current = false;
    if (value.trim()) persist(rememberSearch(recent, value));
  }
  useEffect(() => {
    if (!open) return;
    function outside(event: PointerEvent) {
      if (!container.current?.contains(event.target as Node)) {
        commit();
        setOpen(false);
      }
    }
    document.addEventListener("pointerdown", outside);
    return () => document.removeEventListener("pointerdown", outside);
  });
  return (
    <div
      className="search-box"
      ref={container}
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
          commit();
          setOpen(false);
        }
      }}
      onKeyDown={(event) => {
        if (event.key === "Escape" && open) {
          event.preventDefault();
          event.stopPropagation();
          inputRef.current?.focus();
          setOpen(false);
        }
      }}
    >
      <Search size={18} />
      <input
        ref={inputRef}
        aria-label="Search your library"
        maxLength={512}
        value={value}
        role="combobox"
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-controls={open ? "search-shortcuts" : undefined}
        aria-autocomplete="none"
        onFocus={() => setOpen(true)}
        onClick={() => setOpen(true)}
        onChange={(event) => {
          pending.current = true;
          onChange(event.target.value);
          setOpen(true);
        }}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            commit();
            setOpen(false);
          }
          if (event.key === "ArrowDown") {
            event.preventDefault();
            if (!open) setOpen(true);
            else
              container.current
                ?.querySelector<HTMLButtonElement>(
                  ".search-shortcuts button:not(:disabled)",
                )
                ?.focus();
          }
        }}
        placeholder="Search files or text inside documents"
        title="Search file names or indexed text inside supported documents"
      />
      {value ? (
        <button
          className="icon-button small"
          aria-label="Clear search"
          onClick={() => {
            pending.current = false;
            onChange("");
            inputRef.current?.focus();
          }}
        >
          <X size={15} />
        </button>
      ) : (
        <kbd>Ctrl F</kbd>
      )}
      {open && (
        <div
          className="search-shortcuts"
          id="search-shortcuts"
          role="dialog"
          aria-label="Search shortcuts"
        >
          <div className="search-shortcuts-heading">
            <span>Recent searches</span>
            <button
              aria-label="Clear recent searches"
              disabled={!recent.length}
              onClick={() => {
                pending.current = false;
                persist([]);
              }}
            >
              Clear
            </button>
          </div>
          {recent.length ? (
            <div className="recent-search-list">
              {recent.map((text) => (
                <button
                  key={text}
                  onClick={() => {
                    pending.current = false;
                    onChange(text);
                    persist(rememberSearch(recent, text));
                    inputRef.current?.focus();
                    setOpen(false);
                  }}
                >
                  <Clock3 size={14} />
                  <span>{text}</span>
                </button>
              ))}
            </div>
          ) : (
            <p className="search-shortcuts-empty">No recent searches yet</p>
          )}
          {error && (
            <p className="search-shortcuts-error" role="alert">
              {error}
            </p>
          )}
        </div>
      )}
    </div>
  );
}
