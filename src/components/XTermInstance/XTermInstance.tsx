import {
  forwardRef,
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
}

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface XTermInstanceProps {
  sessionId: string;
  /** Called when the user types in this terminal. */
  onData?: (data: string) => void;
  /** Called when the terminal grid is resized. */
  onResize?: (cols: number, rows: number) => void;
  /** Run in mock mode (no backend needed). */
  mockMode?: boolean;
  /** Whether this terminal is the visible / active one. */
  isActive: boolean;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export const XTermInstance = forwardRef<XTermInstanceHandle, XTermInstanceProps>(
  function XTermInstance({ sessionId: _sessionId, onData, onResize, mockMode, isActive }, ref) {
    const { containerRef, write, fit, getTerminal } = useTerminal({
      onData,
      onResize,
      mockMode,
    });

    // Expose handle to parent
    useImperativeHandle(ref, () => ({ write, fit }), [write, fit]);

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
);
