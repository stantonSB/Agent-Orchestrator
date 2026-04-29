import { useState, useRef, useEffect, useCallback } from "react";
import styles from "./SearchBar.module.css";

interface SearchBarProps {
  onFindNext: (query: string) => boolean;
  onFindPrevious: (query: string) => boolean;
  onClose: () => void;
}

export function SearchBar({ onFindNext, onFindPrevious, onClose }: SearchBarProps) {
  const [query, setQuery] = useState("");
  const [matchInfo, setMatchInfo] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Auto-focus on mount
  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, []);

  const doSearch = useCallback(
    (q: string) => {
      if (!q) {
        setMatchInfo("");
        return;
      }
      const found = onFindNext(q);
      setMatchInfo(found ? "" : "No results");
    },
    [onFindNext],
  );

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = e.target.value;
    setQuery(val);
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => doSearch(val), 150);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (e.shiftKey) {
        onFindPrevious(query);
      } else {
        onFindNext(query);
      }
    }
  };

  return (
    <div className={styles.searchBar}>
      <input
        ref={inputRef}
        className={styles.searchInput}
        type="text"
        value={query}
        onChange={handleChange}
        onKeyDown={handleKeyDown}
        placeholder="Search..."
        spellCheck={false}
      />
      {matchInfo && <span className={styles.matchCount}>{matchInfo}</span>}
      <button
        className={styles.btn}
        onClick={() => query && onFindPrevious(query)}
        title="Previous match (Shift+Enter)"
        aria-label="Previous match"
      >
        &#x25B2;
      </button>
      <button
        className={styles.btn}
        onClick={() => query && onFindNext(query)}
        title="Next match (Enter)"
        aria-label="Next match"
      >
        &#x25BC;
      </button>
      <button
        className={styles.btn}
        onClick={onClose}
        title="Close (Escape)"
        aria-label="Close search"
      >
        &#x2715;
      </button>
    </div>
  );
}
