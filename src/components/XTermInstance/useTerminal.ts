import { useRef, useEffect, useCallback } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import "@xterm/xterm/css/xterm.css";

// ---------------------------------------------------------------------------
// Theme — Tokyo Night-inspired dark palette
// ---------------------------------------------------------------------------

const THEME = {
  background: "#1a1b26",
  foreground: "#a9b1d6",
  cursor: "#c0caf5",
  cursorAccent: "#1a1b26",
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
}

// ---------------------------------------------------------------------------
// useTerminal hook
// ---------------------------------------------------------------------------

export function useTerminal(options: UseTerminalOptions = {}): UseTerminalReturn {
  const { onData, onResize, mockMode = false } = options;

  const containerRef = useRef<HTMLDivElement | null>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const observerRef = useRef<ResizeObserver | null>(null);
  const mockIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // Stable callbacks stored in refs so we don't need them as deps
  const onDataRef = useRef(onData);
  onDataRef.current = onData;
  const onResizeRef = useRef(onResize);
  onResizeRef.current = onResize;

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
    term.loadAddon(new WebLinksAddon());

    fitAddonRef.current = fitAddon;
    termRef.current = term;

    // Mount
    term.open(container);

    // Initial fit (defer one frame so the container has layout dimensions)
    requestAnimationFrame(() => {
      try {
        fitAddon.fit();
      } catch {
        // Container may not have dimensions yet — safe to ignore
      }
    });

    // Forward user input
    const dataDisposable = term.onData((data) => {
      onDataRef.current?.(data);
    });

    // Forward resize events
    const resizeDisposable = term.onResize(({ cols, rows }) => {
      onResizeRef.current?.(cols, rows);
    });

    // ResizeObserver for auto-fit
    const observer = new ResizeObserver(() => {
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
        mockDataDisposable.dispose();
        dataDisposable.dispose();
        resizeDisposable.dispose();
        observer.disconnect();
        term.dispose();
        termRef.current = null;
        fitAddonRef.current = null;
        observerRef.current = null;
      };
    }

    // Cleanup (non-mock)
    return () => {
      dataDisposable.dispose();
      resizeDisposable.dispose();
      observer.disconnect();
      term.dispose();
      termRef.current = null;
      fitAddonRef.current = null;
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
    try {
      fitAddonRef.current?.fit();
    } catch {
      // ignore
    }
  }, []);

  const getTerminal = useCallback(() => termRef.current, []);

  return { containerRef, write, fit, getTerminal };
}
