import {
  forwardRef,
  memo,
  useImperativeHandle,
  useEffect,
  useRef,
} from "react";
import { useTerminal } from "./useTerminal";
import styles from "./XTermInstance.module.css";

// ---------------------------------------------------------------------------
// Public handle exposed via ref
// ---------------------------------------------------------------------------

export interface XTermInstanceHandle {
  write: (data: string | Uint8Array) => void;
  fit: () => void;
  findNext: (query: string) => boolean;
  findPrevious: (query: string) => boolean;
  clearSearch: () => void;
  focus: () => void;
  getScrollbackText: (lines: number) => string;
}

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface XTermInstanceProps {
  sessionId: string;
  /** Current working directory for the session. */
  cwd: string;
  /** Called when the user types in this terminal. */
  onData?: (data: string) => void;
  /** Called when the terminal grid is resized. */
  onResize?: (cols: number, rows: number) => void;
  /** Run in mock mode (no backend needed). */
  mockMode?: boolean;
  /** Whether this terminal is the visible / active one. */
  isActive: boolean;
  /** If true, suppress user input (for persisted/read-only sessions). */
  readOnly?: boolean;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export const XTermInstance = memo(forwardRef<XTermInstanceHandle, XTermInstanceProps>(
  function XTermInstance({ sessionId: _sessionId, cwd, onData, onResize, mockMode, isActive, readOnly }, ref) {
    const { containerRef, write, fit, getTerminal, findNext, findPrevious, clearSearch } = useTerminal({
      onData: readOnly ? undefined : onData,
      onResize: readOnly ? undefined : onResize,
      mockMode,
      cwd,
      isActive,
    });

    // Expose handle to parent
    useImperativeHandle(ref, () => ({
      write,
      fit,
      findNext,
      findPrevious,
      clearSearch,
      focus: () => getTerminal()?.focus(),
      getScrollbackText: (lines: number) => {
        const term = getTerminal();
        if (!term) return "";
        const buffer = term.buffer.active;
        const totalLines = buffer.length;
        const startLine = Math.max(0, totalLines - lines);
        const result: string[] = [];
        for (let i = startLine; i < totalLines; i++) {
          const line = buffer.getLine(i);
          if (line) {
            result.push(line.translateToString(true));
          }
        }
        return result.join("\n");
      },
    }), [write, fit, findNext, findPrevious, clearSearch, getTerminal]);

    // Re-fit whenever this instance becomes active (the container may have
    // changed size while it was display:none).  Use double-rAF to ensure
    // the browser has fully laid out the now-visible container, plus a
    // delayed retry as a safety net.
    const prevActive = useRef(isActive);
    useEffect(() => {
      if (isActive && !prevActive.current) {
        requestAnimationFrame(() => {
          requestAnimationFrame(() => {
            fit();
            getTerminal()?.focus();
          });
        });
        // Safety-net retry: if the double-rAF fit ran during a transient
        // state and was skipped by the dimension guard, this catches it.
        const retryTimer = setTimeout(() => {
          fit();
        }, 100);
        return () => clearTimeout(retryTimer);
      }
      prevActive.current = isActive;
    }, [isActive, fit, getTerminal]);

    const className = [styles.container, isActive ? styles.active : "", !isActive ? styles.hidden : ""]
      .filter(Boolean)
      .join(" ");

    return <div className={className} ref={containerRef} onClick={() => getTerminal()?.focus()} />;
  },
));
