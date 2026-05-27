import { useRef, useEffect, useCallback } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon } from "@xterm/addon-search";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { openUrl } from "@tauri-apps/plugin-opener";
import { FilePathLinkProvider } from "./filePathLinkProvider";
import "@xterm/xterm/css/xterm.css";

// ---------------------------------------------------------------------------
// Theme — Tokyo Night-inspired dark palette
// ---------------------------------------------------------------------------

const THEME = {
  background: "#1a1a2e",
  foreground: "#a9b1d6",
  cursor: "#c0caf5",
  cursorAccent: "#1a1a2e",
  selectionBackground: "#33467c",
  selectionForeground: "#c0caf5",
  black: "#15161e",
  red: "#f7768e",
  green: "#9ece6a",
  yellow: "#e0af68",
  blue: "#7aa2f7",
  magenta: "#bb9af7",
  cyan: "#7dcfff",
  white: "#a9b1d6",
  brightBlack: "#414868",
  brightRed: "#f7768e",
  brightGreen: "#9ece6a",
  brightYellow: "#e0af68",
  brightBlue: "#7aa2f7",
  brightMagenta: "#bb9af7",
  brightCyan: "#7dcfff",
  brightWhite: "#c0caf5",
};

// ---------------------------------------------------------------------------
// Hook options & return type
// ---------------------------------------------------------------------------

export interface UseTerminalOptions {
  /** Callback when the user types in the terminal. */
  onData?: (data: string) => void;
  /** Callback when the terminal is resized (cols, rows). */
  onResize?: (cols: number, rows: number) => void;
  /** Run in mock mode — echo input and emit fake output. */
  mockMode?: boolean;
  /** Current working directory for the session. */
  cwd?: string;
}

export interface UseTerminalReturn {
  /** Ref to attach to the container div. */
  containerRef: React.RefObject<HTMLDivElement | null>;
  /** Write raw data (string or Uint8Array) to the terminal. */
  write: (data: string | Uint8Array) => void;
  /** Trigger a re-fit to the container size. */
  fit: () => void;
  /** Access the underlying Terminal instance (may be null before mount). */
  getTerminal: () => Terminal | null;
  /** Search forward for the given query. Returns true if a match was found. */
  findNext: (query: string) => boolean;
  /** Search backward for the given query. Returns true if a match was found. */
  findPrevious: (query: string) => boolean;
  /** Clear search highlights. */
  clearSearch: () => void;
}

// ---------------------------------------------------------------------------
// useTerminal hook
// ---------------------------------------------------------------------------

export function useTerminal(options: UseTerminalOptions = {}): UseTerminalReturn {
  const { onData, onResize, mockMode = false, cwd } = options;

  const containerRef = useRef<HTMLDivElement | null>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const searchAddonRef = useRef<SearchAddon | null>(null);
  const observerRef = useRef<ResizeObserver | null>(null);
  const mockIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // Stable callbacks stored in refs so we don't need them as deps
  const onDataRef = useRef(onData);
  onDataRef.current = onData;
  const onResizeRef = useRef(onResize);
  onResizeRef.current = onResize;
  const cwdRef = useRef(cwd);
  cwdRef.current = cwd;

  // -----------------------------------------------------------------------
  // Lifecycle
  // -----------------------------------------------------------------------

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    // Create terminal
    const term = new Terminal({
      theme: THEME,
      fontFamily: "'JetBrains Mono', 'Fira Code', 'Cascadia Code', Menlo, monospace",
      fontSize: 13,
      lineHeight: 1.3,
      cursorBlink: true,
      cursorStyle: "bar",
      scrollback: 10_000,
      allowProposedApi: true,
    });

    // Addons
    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    const searchAddon = new SearchAddon();
    term.loadAddon(searchAddon);
    term.loadAddon(new WebLinksAddon((_event, uri) => {
      openUrl(uri);
    }));
    term.registerLinkProvider(new FilePathLinkProvider(term, cwdRef));

    // Intercept Cmd+F so it doesn't get sent to the PTY
    term.attachCustomKeyEventHandler((event) => {
      if (event.metaKey && event.key === "f") return false;
      return true;
    });

    fitAddonRef.current = fitAddon;
    searchAddonRef.current = searchAddon;
    termRef.current = term;

    // Mount
    term.open(container);

    // Shift+Enter: intercept at the DOM level in the capture phase so
    // the event is caught *before* xterm.js sees it.  We send \n to the
    // PTY (Claude Code in legacy mode treats \n as "insert newline" and
    // \r as "submit").  preventDefault + stopImmediatePropagation ensures
    // xterm.js never generates its own \r for this keypress.
    const shiftEnterHandler = (event: KeyboardEvent) => {
      if (event.shiftKey && event.key === "Enter") {
        event.preventDefault();
        event.stopImmediatePropagation();
        onDataRef.current?.("\n");
      }
    };
    container.addEventListener("keydown", shiftEnterHandler, { capture: true });

    // Initial fit — use double-rAF to ensure the browser has completed
    // layout after xterm inserts its DOM elements.  A single rAF can fire
    // before the renderer has measured cell dimensions, producing a
    // bogus column count that permanently breaks the PTY size.
    const scheduleInitialFit = () => {
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          if (container.offsetParent === null || container.clientWidth < 50) {
            // Container not yet visible — retry shortly
            setTimeout(scheduleInitialFit, 50);
            return;
          }
          try {
            fitAddon.fit();
          } catch {
            // Container may not have dimensions yet — safe to ignore
          }
          if (container.offsetParent !== null) {
            term.focus();
          }
        });
      });
    };
    scheduleInitialFit();

    // Forward user input
    const dataDisposable = term.onData((data) => {
      onDataRef.current?.(data);
    });

    // Forward resize events
    const resizeDisposable = term.onResize(({ cols, rows }) => {
      onResizeRef.current?.(cols, rows);
    });

    // ResizeObserver for auto-fit — skip entries with tiny dimensions
    // to avoid fitting during transient layout states (e.g. display:none
    // transitions) which can produce bogus column counts.
    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry || entry.contentRect.width < 50 || entry.contentRect.height < 50) {
        return;
      }
      requestAnimationFrame(() => {
        try {
          fitAddon.fit();
        } catch {
          // ignore
        }
      });
    });
    observer.observe(container);
    observerRef.current = observer;

    // ------ Mock mode ------
    if (mockMode) {
      term.writeln("\x1b[1;34m--- Mock Terminal ---\x1b[0m");
      term.writeln("Type anything and press Enter to see it echoed.\r\n");

      // Echo input
      let lineBuffer = "";
      const mockDataDisposable = term.onData((data) => {
        if (data === "\r") {
          term.writeln("");
          if (lineBuffer.trim()) {
            term.writeln(`\x1b[32m> ${lineBuffer}\x1b[0m`);
          }
          lineBuffer = "";
        } else if (data === "\x7f") {
          // Backspace
          if (lineBuffer.length > 0) {
            lineBuffer = lineBuffer.slice(0, -1);
            term.write("\b \b");
          }
        } else {
          lineBuffer += data;
          term.write(data);
        }
      });

      // Periodic fake output
      const messages = [
        "\x1b[33m[mock]\x1b[0m Thinking...",
        "\x1b[36m[mock]\x1b[0m Processing request...",
        "\x1b[35m[mock]\x1b[0m Analyzing codebase...",
        "\x1b[32m[mock]\x1b[0m Ready for input.",
      ];
      let msgIdx = 0;
      mockIntervalRef.current = setInterval(() => {
        term.writeln(messages[msgIdx % messages.length]);
        msgIdx++;
      }, 4000);

      // Cleanup mock-specific resources
      return () => {
        if (mockIntervalRef.current) clearInterval(mockIntervalRef.current);
        container.removeEventListener("keydown", shiftEnterHandler, { capture: true });
        mockDataDisposable.dispose();
        dataDisposable.dispose();
        resizeDisposable.dispose();
        observer.disconnect();
        term.dispose();
        termRef.current = null;
        fitAddonRef.current = null;
        searchAddonRef.current = null;
        observerRef.current = null;
      };
    }

    // Cleanup (non-mock)
    return () => {
      container.removeEventListener("keydown", shiftEnterHandler, { capture: true });
      dataDisposable.dispose();
      resizeDisposable.dispose();
      observer.disconnect();
      term.dispose();
      termRef.current = null;
      fitAddonRef.current = null;
      searchAddonRef.current = null;
      observerRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mockMode]);

  // -----------------------------------------------------------------------
  // Public API
  // -----------------------------------------------------------------------

  const write = useCallback((data: string | Uint8Array) => {
    termRef.current?.write(data);
  }, []);

  const fit = useCallback(() => {
    const container = containerRef.current;
    // Don't fit if container is hidden (display:none) or detached from DOM
    if (!container || container.offsetParent === null) return;
    // Don't fit if container has near-zero dimensions (transient layout state)
    if (container.clientWidth < 50 || container.clientHeight < 50) return;
    try {
      fitAddonRef.current?.fit();
    } catch {
      // ignore
    }
  }, []);

  const getTerminal = useCallback(() => termRef.current, []);

  const findNext = useCallback((query: string) => {
    return searchAddonRef.current?.findNext(query, { regex: false, caseSensitive: false, incremental: true }) ?? false;
  }, []);

  const findPrevious = useCallback((query: string) => {
    return searchAddonRef.current?.findPrevious(query, { regex: false, caseSensitive: false }) ?? false;
  }, []);

  const clearSearch = useCallback(() => {
    searchAddonRef.current?.clearDecorations();
  }, []);

  return { containerRef, write, fit, getTerminal, findNext, findPrevious, clearSearch };
}
