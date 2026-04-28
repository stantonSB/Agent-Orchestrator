# File Path Link Opener — Design Spec

## Problem

Terminal output from Claude Code frequently contains file paths (e.g., `docs/knowledge-base-dashboard-epic.md`, `src/components/Foo.tsx:42`). Users expect to Cmd+click these paths to open them in their default editor, just like in VS Code's integrated terminal. Currently, only HTTP/HTTPS URLs are clickable via the WebLinksAddon.

## Solution

Add a custom xterm.js `ILinkProvider` that detects file paths in terminal output and opens them via Cmd+click using the system default application.

## Design Decisions

- **Approach A (chosen):** Register a custom `ILinkProvider` alongside the existing `WebLinksAddon`. Additive — no changes to URL handling.
- **Path types:** Both relative and absolute paths are supported.
- **Editor:** Uses system default app via Tauri's `openUrl` with `file://` URIs. No hardcoded editor.
- **Validation:** Paths are always clickable based on regex match. If the file doesn't exist when clicked, a toast notification is shown. No async validation during rendering.

## Architecture

### Data Flow

```
TerminalArea (has cwd from SessionInfo)
  └─ XTermInstance (receives cwd as prop)
       └─ useTerminal (receives cwd in options)
            └─ FilePathLinkProvider (reads cwd from ref, resolves paths)
                 └─ openUrl(`file://${resolvedPath}`) on Cmd+click
```

### Changes

#### 1. Thread `cwd` through the component hierarchy

**`TerminalArea.tsx`** — Add `cwd` to the `TerminalSession` interface:

```typescript
export interface TerminalSession {
  id: string;
  name: string;
  cwd: string;  // NEW
}
```

Pass `cwd` to `XTermInstance`:

```tsx
<XTermInstance
  key={session.id}
  sessionId={session.id}
  cwd={session.cwd}
  isActive={session.id === activeSessionId}
  ...
/>
```

**`XTermInstance.tsx`** — Accept `cwd` prop, pass to `useTerminal`:

```typescript
interface XTermInstanceProps {
  sessionId: string;
  cwd: string;  // NEW
  onData?: (data: string) => void;
  onResize?: (cols: number, rows: number) => void;
  mockMode?: boolean;
  isActive: boolean;
}
```

**`useTerminal.ts`** — Accept `cwd` in options, store in a ref:

```typescript
export interface UseTerminalOptions {
  onData?: (data: string) => void;
  onResize?: (cols: number, rows: number) => void;
  mockMode?: boolean;
  cwd?: string;  // NEW
}
```

The `cwd` is stored in a ref so the link provider always reads the current value without requiring terminal re-creation.

#### 2. New file: `filePathLinkProvider.ts`

**Location:** `src/components/XTermInstance/filePathLinkProvider.ts`

Implements xterm's `ILinkProvider` interface with:

**Path detection regex** — matches:
- Relative paths: `src/components/Foo.tsx`, `./docs/readme.md`, `../lib/util.ts`
- Absolute paths: `/Users/stanton/project/file.ts`
- With optional line/column: `src/file.ts:42`, `src/file.ts:42:10`

The regex targets strings that start with `/`, `./`, `../`, or `word/` and contain typical path characters, ending with a file extension. This avoids matching plain words or bare directory names.

**`provideLinks(bufferLineNumber)`:**
1. Read the line text from the terminal buffer
2. Run the regex to find all path matches with their column ranges
3. Return link objects with `activate` and `tooltip` callbacks

**`activate` (Cmd+click handler):**
1. If path is relative, join with `cwd` to produce absolute path
2. Call `openUrl(`file://${absolutePath}`)` from `@tauri-apps/plugin-opener`
3. On error, call `addToast("Could not open file: <path>", "error")` from the session store

**Hover tooltip:** `"Cmd+click to open in editor"`

#### 3. Register the provider in `useTerminal`

After loading the existing addons:

```typescript
const cwdRef = useRef(cwd);
cwdRef.current = cwd;

term.registerLinkProvider(
  new FilePathLinkProvider(cwdRef)
);
```

The provider receives the ref, not the value, so it always resolves against the current cwd.

### Error Handling

- `openUrl` rejection (file doesn't exist, permissions): caught, toast shown
- Regex false positives (matched text isn't a real path): handled gracefully — `openUrl` fails, toast shown
- No cwd provided: relative paths still attempted (may fail, toast shown)

### No Backend Changes

`openUrl` from `@tauri-apps/plugin-opener` already handles `file://` URIs. macOS resolves the default app for the file type. No new Tauri commands needed.

## Files Changed

| File | Change |
|------|--------|
| `src/components/XTermInstance/filePathLinkProvider.ts` | **New** — `ILinkProvider` implementation |
| `src/components/XTermInstance/useTerminal.ts` | Add `cwd` option, register link provider |
| `src/components/XTermInstance/XTermInstance.tsx` | Accept and forward `cwd` prop |
| `src/components/TerminalArea/TerminalArea.tsx` | Add `cwd` to `TerminalSession`, pass through |
| Parent component rendering `TerminalArea` | Map `SessionInfo.cwd` into `TerminalSession` |

## Testing

- Unit test for `FilePathLinkProvider`: verify regex matches expected path patterns and ignores non-paths
- Unit test for path resolution: relative + absolute paths, with and without line numbers
- Manual test: run app, verify Cmd+click on file paths opens VS Code
