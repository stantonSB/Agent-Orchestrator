# File Path Link Opener Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enable Cmd+click on file paths in terminal output to open them in the system default editor.

**Architecture:** A custom xterm.js `ILinkProvider` registered alongside the existing `WebLinksAddon`. The provider uses regex to detect file paths in terminal lines, resolves relative paths against the session's `cwd` (threaded through the component hierarchy via props/refs), and opens files using Tauri's `openUrl` with `file://` URIs.

**Tech Stack:** TypeScript, React, xterm.js (`@xterm/xterm` v6), Tauri (`@tauri-apps/plugin-opener`), Vitest

**Spec:** `docs/superpowers/specs/2026-04-28-file-path-link-opener-design.md`

---

## Chunk 1: FilePathLinkProvider (core logic + tests)

### Task 1: Create FilePathLinkProvider with tests

**Files:**
- Create: `src/components/XTermInstance/filePathLinkProvider.ts`
- Create: `src/components/XTermInstance/filePathLinkProvider.test.ts`

#### Path detection regex

The regex must match:
- Relative: `src/components/Foo.tsx`, `./docs/readme.md`, `../lib/util.ts`
- Absolute: `/Users/stanton/project/file.ts`
- With line/col: `src/file.ts:42`, `src/file.ts:42:10`
- Must NOT match: plain words, URLs (handled by WebLinksAddon), bare dirs without extensions

- [ ] **Step 1: Write tests for path regex matching**

Create `src/components/XTermInstance/filePathLinkProvider.test.ts`:

```typescript
import { describe, it, expect } from "vitest";
import { FILE_PATH_REGEX } from "./filePathLinkProvider";

describe("FILE_PATH_REGEX", () => {
  const matchAll = (text: string) =>
    [...text.matchAll(new RegExp(FILE_PATH_REGEX, "g"))].map((m) => m[0]);

  describe("matches valid file paths", () => {
    it("matches relative paths", () => {
      expect(matchAll("Written to src/components/Foo.tsx")).toEqual([
        "src/components/Foo.tsx",
      ]);
    });

    it("matches dot-relative paths", () => {
      expect(matchAll("Editing ./docs/readme.md now")).toEqual([
        "./docs/readme.md",
      ]);
    });

    it("matches parent-relative paths", () => {
      expect(matchAll("See ../lib/util.ts")).toEqual(["../lib/util.ts"]);
    });

    it("matches absolute paths", () => {
      expect(matchAll("File at /Users/stanton/project/file.ts")).toEqual([
        "/Users/stanton/project/file.ts",
      ]);
    });

    it("matches paths with line numbers", () => {
      expect(matchAll("Error in src/file.ts:42")).toEqual(["src/file.ts:42"]);
    });

    it("matches paths with line and column", () => {
      expect(matchAll("Error at src/file.ts:42:10")).toEqual([
        "src/file.ts:42:10",
      ]);
    });

    it("matches multiple paths on one line", () => {
      expect(matchAll("Changed src/a.ts and src/b.tsx")).toEqual([
        "src/a.ts",
        "src/b.tsx",
      ]);
    });

    it("matches paths with hyphens and underscores", () => {
      expect(matchAll("See my-component/foo_bar.module.css")).toEqual([
        "my-component/foo_bar.module.css",
      ]);
    });

    it("matches paths with dots in directory names", () => {
      expect(matchAll("In .claude/settings.json")).toEqual([
        ".claude/settings.json",
      ]);
    });
  });

  describe("does not match non-paths", () => {
    it("ignores plain words", () => {
      expect(matchAll("hello world")).toEqual([]);
    });

    it("ignores URLs", () => {
      // URLs may partially match — WebLinksAddon handles these.
      // The key is we don't match the protocol portion.
      const urls = matchAll("Visit https://example.com/page.html");
      // Should not match the full URL; partial path match is acceptable
      // since WebLinksAddon takes priority for URL clicks
      expect(urls).not.toContain("https://example.com/page.html");
    });

    it("ignores bare directory names without file extension", () => {
      expect(matchAll("The src/components directory")).toEqual([]);
    });

    it("ignores single filenames without path separator", () => {
      expect(matchAll("See readme.md for details")).toEqual([]);
    });
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npx vitest run src/components/XTermInstance/filePathLinkProvider.test.ts`
Expected: FAIL — `FILE_PATH_REGEX` not found

- [ ] **Step 3: Implement FilePathLinkProvider**

Create `src/components/XTermInstance/filePathLinkProvider.ts`:

```typescript
import type { ILinkProvider, ILink, IBufferRange, Terminal } from "@xterm/xterm";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useSessionStore } from "../../stores/sessionStore";

// ---------------------------------------------------------------------------
// Path detection regex
// ---------------------------------------------------------------------------

// Matches file paths that:
// - Start with /, ./, ../, or word-char followed by /
//   OR start with a dot-prefixed dir (e.g. .claude/)
// - Contain typical path characters (word chars, hyphens, dots, slashes)
// - End with a file extension (dot followed by 1-10 word chars)
// - Optionally followed by :line or :line:col
//
// Exported for testing.
export const FILE_PATH_REGEX =
  /(?:\.\.?\/|\/|(?:\.?[\w][\w.-]*\/))[\w.\-/]*\.[\w]{1,10}(?::[\d]+(?::[\d]+)?)?/;

// ---------------------------------------------------------------------------
// FilePathLinkProvider
// ---------------------------------------------------------------------------

export class FilePathLinkProvider implements ILinkProvider {
  constructor(
    private readonly terminal: Terminal,
    private readonly cwdRef: { current: string | undefined },
  ) {}

  provideLinks(
    bufferLineNumber: number,
    callback: (links: ILink[] | undefined) => void,
  ): void {
    const line = this.terminal.buffer.active.getLine(bufferLineNumber - 1);
    if (!line) {
      callback(undefined);
      return;
    }

    const text = line.translateToString(true);
    const links: ILink[] = [];
    const regex = new RegExp(FILE_PATH_REGEX, "g");
    let match: RegExpExecArray | null;

    while ((match = regex.exec(text)) !== null) {
      const startX = match.index + 1; // IBufferRange is 1-based
      const endX = match.index + match[0].length;
      const range: IBufferRange = {
        start: { x: startX, y: bufferLineNumber },
        end: { x: endX, y: bufferLineNumber },
      };

      const matchedText = match[0];

      links.push({
        range,
        text: matchedText,
        decorations: { pointerCursor: true, underline: true },
        activate: (event: MouseEvent, linkText: string) => {
          if (!event.metaKey) return;
          this.openFilePath(linkText);
        },
        hover: () => {
          // xterm.js shows the tooltip text via the link's decorations.
          // The pointer cursor + underline serve as the visual indicator.
          // For an explicit tooltip, we'd need DOM manipulation within
          // Terminal.element — the decorations already communicate clickability.
        },
      });
    }

    callback(links.length > 0 ? links : undefined);
  }

  private openFilePath(pathWithLineCol: string): void {
    // Strip :line:col suffix to get the raw file path
    const filePath = pathWithLineCol.replace(/:[\d]+(?::[\d]+)?$/, "");

    // Resolve relative paths against cwd
    let absolutePath: string;
    if (filePath.startsWith("/")) {
      absolutePath = filePath;
    } else {
      const cwd = this.cwdRef.current;
      if (cwd) {
        absolutePath = `${cwd.replace(/\/$/, "")}/${filePath}`;
      } else {
        absolutePath = filePath;
      }
    }

    openUrl(`file://${absolutePath}`).catch(() => {
      useSessionStore
        .getState()
        .addToast(`Could not open file: ${filePath}`, "error");
    });
  }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `npx vitest run src/components/XTermInstance/filePathLinkProvider.test.ts`
Expected: All tests PASS

- [ ] **Step 5: Add tests for provideLinks and openFilePath behavior**

Append to `src/components/XTermInstance/filePathLinkProvider.test.ts`:

```typescript
import { vi } from "vitest";
import { FilePathLinkProvider } from "./filePathLinkProvider";

// Mock openUrl
vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: vi.fn(() => Promise.resolve()),
}));

// Mock session store
vi.mock("../../stores/sessionStore", () => ({
  useSessionStore: {
    getState: () => ({
      addToast: vi.fn(),
    }),
  },
}));

describe("FilePathLinkProvider", () => {
  const createMockTerminal = (lineText: string) =>
    ({
      buffer: {
        active: {
          getLine: (y: number) =>
            y === 0
              ? { translateToString: () => lineText }
              : undefined,
        },
      },
    }) as unknown as import("@xterm/xterm").Terminal;

  describe("provideLinks", () => {
    it("returns links for detected file paths", () => {
      const terminal = createMockTerminal(
        "Written to src/components/Foo.tsx. Done.",
      );
      const cwdRef = { current: "/Users/test/project" };
      const provider = new FilePathLinkProvider(terminal, cwdRef);

      return new Promise<void>((resolve) => {
        provider.provideLinks(1, (links) => {
          expect(links).toHaveLength(1);
          expect(links![0].text).toBe("src/components/Foo.tsx");
          expect(links![0].range).toEqual({
            start: { x: 12, y: 1 },
            end: { x: 33, y: 1 },
          });
          resolve();
        });
      });
    });

    it("returns undefined for lines with no paths", () => {
      const terminal = createMockTerminal("Hello world, no paths here");
      const cwdRef = { current: "/test" };
      const provider = new FilePathLinkProvider(terminal, cwdRef);

      return new Promise<void>((resolve) => {
        provider.provideLinks(1, (links) => {
          expect(links).toBeUndefined();
          resolve();
        });
      });
    });

    it("returns undefined for missing buffer lines", () => {
      const terminal = createMockTerminal("anything");
      const cwdRef = { current: "/test" };
      const provider = new FilePathLinkProvider(terminal, cwdRef);

      return new Promise<void>((resolve) => {
        // Line 999 doesn't exist — getLine returns undefined for y !== 0
        provider.provideLinks(999, (links) => {
          expect(links).toBeUndefined();
          resolve();
        });
      });
    });
  });

  describe("activate", () => {
    it("calls openUrl with resolved absolute path on Cmd+click", async () => {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      const terminal = createMockTerminal("See src/file.ts for details");
      const cwdRef = { current: "/Users/test/project" };
      const provider = new FilePathLinkProvider(terminal, cwdRef);

      await new Promise<void>((resolve) => {
        provider.provideLinks(1, (links) => {
          const event = { metaKey: true } as MouseEvent;
          links![0].activate(event, links![0].text);
          expect(openUrl).toHaveBeenCalledWith(
            "file:///Users/test/project/src/file.ts",
          );
          resolve();
        });
      });
    });

    it("does not call openUrl without metaKey", async () => {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      vi.mocked(openUrl).mockClear();
      const terminal = createMockTerminal("See src/file.ts for details");
      const cwdRef = { current: "/Users/test/project" };
      const provider = new FilePathLinkProvider(terminal, cwdRef);

      await new Promise<void>((resolve) => {
        provider.provideLinks(1, (links) => {
          const event = { metaKey: false } as MouseEvent;
          links![0].activate(event, links![0].text);
          expect(openUrl).not.toHaveBeenCalled();
          resolve();
        });
      });
    });

    it("strips line:col before resolving path", async () => {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      vi.mocked(openUrl).mockClear();
      const terminal = createMockTerminal("Error at src/file.ts:42:10");
      const cwdRef = { current: "/project" };
      const provider = new FilePathLinkProvider(terminal, cwdRef);

      await new Promise<void>((resolve) => {
        provider.provideLinks(1, (links) => {
          const event = { metaKey: true } as MouseEvent;
          links![0].activate(event, links![0].text);
          expect(openUrl).toHaveBeenCalledWith(
            "file:///project/src/file.ts",
          );
          resolve();
        });
      });
    });

    it("uses absolute path directly when path starts with /", async () => {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      vi.mocked(openUrl).mockClear();
      const terminal = createMockTerminal(
        "File at /Users/stanton/project/file.ts",
      );
      const cwdRef = { current: "/other/dir" };
      const provider = new FilePathLinkProvider(terminal, cwdRef);

      await new Promise<void>((resolve) => {
        provider.provideLinks(1, (links) => {
          const event = { metaKey: true } as MouseEvent;
          links![0].activate(event, links![0].text);
          expect(openUrl).toHaveBeenCalledWith(
            "file:///Users/stanton/project/file.ts",
          );
          resolve();
        });
      });
    });
  });
});
```

- [ ] **Step 6: Run all tests to verify they pass**

Run: `npx vitest run src/components/XTermInstance/filePathLinkProvider.test.ts`
Expected: All tests PASS

- [ ] **Step 7: Commit**

```bash
git add src/components/XTermInstance/filePathLinkProvider.ts src/components/XTermInstance/filePathLinkProvider.test.ts
git commit -m "feat: add FilePathLinkProvider for Cmd+click file opening"
```

---

## Chunk 2: Wire cwd through component hierarchy and register provider

### Task 2: Thread cwd through TerminalArea → XTermInstance → useTerminal

**Files:**
- Modify: `src/components/TerminalArea/TerminalArea.tsx:19-22` (add `cwd` to `TerminalSession`)
- Modify: `src/components/TerminalArea/TerminalArea.tsx:195-205` (pass `cwd` prop)
- Modify: `src/components/XTermInstance/XTermInstance.tsx:23-33` (accept `cwd` prop)
- Modify: `src/components/XTermInstance/XTermInstance.tsx:40` (forward to `useTerminal`)
- Modify: `src/components/XTermInstance/useTerminal.ts:41-48` (accept `cwd` option)

- [ ] **Step 1: Add `cwd` to `TerminalSession` interface**

In `src/components/TerminalArea/TerminalArea.tsx`, change:

```typescript
export interface TerminalSession {
  id: string;
  name: string;
}
```

to:

```typescript
export interface TerminalSession {
  id: string;
  name: string;
  cwd: string;
}
```

- [ ] **Step 2: Pass `cwd` to XTermInstance**

In `src/components/TerminalArea/TerminalArea.tsx`, change the render in the `sessions.map`:

```tsx
<XTermInstance
  key={session.id}
  ref={setRef(session.id)}
  sessionId={session.id}
  isActive={session.id === activeSessionId}
  mockMode={mockMode}
  onData={(data) => handleSessionData(session.id, data)}
  onResize={(cols, rows) =>
    handleSessionResize(session.id, cols, rows)
  }
/>
```

to:

```tsx
<XTermInstance
  key={session.id}
  ref={setRef(session.id)}
  sessionId={session.id}
  cwd={session.cwd}
  isActive={session.id === activeSessionId}
  mockMode={mockMode}
  onData={(data) => handleSessionData(session.id, data)}
  onResize={(cols, rows) =>
    handleSessionResize(session.id, cols, rows)
  }
/>
```

- [ ] **Step 3: Accept `cwd` prop in XTermInstance**

In `src/components/XTermInstance/XTermInstance.tsx`, change:

```typescript
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
```

to:

```typescript
interface XTermInstanceProps {
  sessionId: string;
  /** Working directory for resolving relative file paths in link clicks. */
  cwd: string;
  /** Called when the user types in this terminal. */
  onData?: (data: string) => void;
  /** Called when the terminal grid is resized. */
  onResize?: (cols: number, rows: number) => void;
  /** Run in mock mode (no backend needed). */
  mockMode?: boolean;
  /** Whether this terminal is the visible / active one. */
  isActive: boolean;
}
```

- [ ] **Step 4: Forward `cwd` to useTerminal**

In `src/components/XTermInstance/XTermInstance.tsx`, change:

```typescript
function XTermInstance({ sessionId: _sessionId, onData, onResize, mockMode, isActive }, ref) {
    const { containerRef, write, fit, getTerminal } = useTerminal({
      onData,
      onResize,
      mockMode,
    });
```

to:

```typescript
function XTermInstance({ sessionId: _sessionId, cwd, onData, onResize, mockMode, isActive }, ref) {
    const { containerRef, write, fit, getTerminal } = useTerminal({
      onData,
      onResize,
      mockMode,
      cwd,
    });
```

- [ ] **Step 5: Accept `cwd` in useTerminal and register FilePathLinkProvider**

In `src/components/XTermInstance/useTerminal.ts`, add the import at the top:

```typescript
import { FilePathLinkProvider } from "./filePathLinkProvider";
```

Change the `UseTerminalOptions` interface:

```typescript
export interface UseTerminalOptions {
  /** Callback when the user types in the terminal. */
  onData?: (data: string) => void;
  /** Callback when the terminal is resized (cols, rows). */
  onResize?: (cols: number, rows: number) => void;
  /** Run in mock mode — echo input and emit fake output. */
  mockMode?: boolean;
}
```

to:

```typescript
export interface UseTerminalOptions {
  /** Callback when the user types in the terminal. */
  onData?: (data: string) => void;
  /** Callback when the terminal is resized (cols, rows). */
  onResize?: (cols: number, rows: number) => void;
  /** Run in mock mode — echo input and emit fake output. */
  mockMode?: boolean;
  /** Working directory for resolving relative file paths in link clicks. */
  cwd?: string;
}
```

Change the destructuring at the top of `useTerminal`:

```typescript
const { onData, onResize, mockMode = false } = options;
```

to:

```typescript
const { onData, onResize, mockMode = false, cwd } = options;
```

Add a `cwdRef` right after the existing `onResizeRef` lines (around line 78):

```typescript
const cwdRef = useRef(cwd);
cwdRef.current = cwd;
```

After the WebLinksAddon registration (after line 105), add:

```typescript
term.registerLinkProvider(new FilePathLinkProvider(term, cwdRef));
```

**Important:** Do NOT add `cwd` to the `useEffect` dependency array — it stays `[mockMode]`. The `cwdRef` pattern ensures the provider always reads the latest value.

- [ ] **Step 6: Run full test suite to verify nothing is broken**

Run: `npx vitest run`
Expected: All tests PASS (existing + new)

- [ ] **Step 7: Commit**

```bash
git add src/components/TerminalArea/TerminalArea.tsx src/components/XTermInstance/XTermInstance.tsx src/components/XTermInstance/useTerminal.ts
git commit -m "feat: wire cwd through component hierarchy and register FilePathLinkProvider"
```

---

## Chunk 3: Verify TypeScript compilation and manual test

### Task 3: Final verification

**Files:** None (verification only)

- [ ] **Step 1: TypeScript type check**

Run: `npx tsc --noEmit`
Expected: No type errors

- [ ] **Step 2: Run full test suite**

Run: `npx vitest run`
Expected: All tests PASS

- [ ] **Step 3: Manual testing instructions**

1. Run `npm run tauri dev`
2. Create a new session in a directory with known files
3. Type or have Claude output a file path like `src/components/XTermInstance/useTerminal.ts`
4. Hold Cmd and hover the path — should show underline + pointer cursor
5. Cmd+click — should open the file in the system default application (VS Code)
6. Try a non-existent path — should show a toast error
7. Try an absolute path — should open directly
8. Try a path with `:line:col` suffix — should strip suffix and open
