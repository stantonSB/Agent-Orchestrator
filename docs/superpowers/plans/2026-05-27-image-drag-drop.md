# Image Drag & Drop Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow users to drag and drop images onto the active terminal, writing the image file path as text into the PTY.

**Architecture:** Two-layer drop detection — Tauri's built-in `onDragDropEvent` for Finder file drags (provides full paths), HTML5 `drop` event fallback for browser image data (saves to temp file via new Tauri command). A `DropOverlay` component provides visual feedback during drag. All image paths are written to the active session's PTY via the existing `writeToSession` IPC.

**Tech Stack:** Tauri 2 core events (`@tauri-apps/api/webviewWindow`), React, Rust

**Spec:** `docs/superpowers/specs/2026-05-27-image-drag-drop-design.md`

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `src-tauri/src/commands.rs` | Modify | Add `save_dropped_image` command |
| `src-tauri/src/lib.rs:129` | Modify | Register `save_dropped_image` in invoke handler |
| `src/lib/tauri-ipc.ts` | Modify | Add `saveDroppedImage` wrapper |
| `src/components/TerminalArea/DropOverlay.tsx` | Create | Drop overlay UI component |
| `src/components/TerminalArea/DropOverlay.module.css` | Create | Drop overlay styles |
| `src/components/TerminalArea/useImageDrop.ts` | Create | Custom hook encapsulating all drag-drop logic |
| `src/components/TerminalArea/TerminalArea.tsx` | Modify | Wire up `useImageDrop` hook |
| `src/components/TerminalArea/TerminalArea.module.css` | Modify | Add `position: relative` to terminalArea if needed |

---

## Chunk 1: Rust Backend — `save_dropped_image` Command

### Task 1: Add `save_dropped_image` Tauri command

**Files:**
- Modify: `src-tauri/src/commands.rs` (append after line 295)
- Modify: `src-tauri/src/lib.rs:129` (add to invoke_handler)

- [ ] **Step 1: Write the Rust command**

Add to `src-tauri/src/commands.rs` at the end of the file:

```rust
#[tauri::command]
pub fn save_dropped_image(data: Vec<u8>, extension: String) -> Result<String, String> {
    const ALLOWED: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "tiff"];
    let ext = extension.to_lowercase();
    if !ALLOWED.contains(&ext.as_str()) {
        return Err(format!("Unsupported image extension: {extension}"));
    }
    let filename = format!("ao-dropped-{}.{}", uuid::Uuid::new_v4(), ext);
    let path = std::env::temp_dir().join(&filename);
    std::fs::write(&path, &data)
        .map_err(|e| format!("Failed to write temp image: {e}"))?;
    Ok(path.to_string_lossy().to_string())
}
```

- [ ] **Step 2: Register in invoke handler**

In `src-tauri/src/lib.rs`, add `commands::save_dropped_image` to the `invoke_handler` macro call at line 129, after `commands::delete_persisted_session`:

```rust
            commands::delete_persisted_session,
            commands::save_dropped_image,
```

- [ ] **Step 3: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: compiles without errors

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add save_dropped_image Tauri command"
```

---

## Chunk 2: Frontend IPC Wrapper

### Task 2: Add `saveDroppedImage` IPC wrapper

**Files:**
- Modify: `src/lib/tauri-ipc.ts` (append after line 98)

- [ ] **Step 1: Add the interface and function**

Add to the end of `src/lib/tauri-ipc.ts`:

```typescript
export interface SaveDroppedImageArgs {
  data: number[];
  extension: string;
}

export async function saveDroppedImage(args: SaveDroppedImageArgs): Promise<string> {
  return invoke<string>("save_dropped_image", { ...args });
}
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/tauri-ipc.ts
git commit -m "feat: add saveDroppedImage IPC wrapper"
```

---

## Chunk 3: Drop Overlay Component

### Task 3: Create `DropOverlay` component

**Files:**
- Create: `src/components/TerminalArea/DropOverlay.module.css`
- Create: `src/components/TerminalArea/DropOverlay.tsx`

- [ ] **Step 1: Create the CSS module**

Create `src/components/TerminalArea/DropOverlay.module.css`:

```css
.overlay {
  position: absolute;
  inset: 0;
  z-index: 10;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(26, 27, 38, 0.85);
  border: 2px dashed var(--accent-primary, #7aa2f7);
  border-radius: 8px;
  pointer-events: none;
}

.label {
  font-size: 16px;
  font-weight: 500;
  color: var(--accent-primary, #7aa2f7);
  user-select: none;
}
```

- [ ] **Step 2: Create the component**

Create `src/components/TerminalArea/DropOverlay.tsx`:

```tsx
import styles from "./DropOverlay.module.css";

export function DropOverlay() {
  return (
    <div className={styles.overlay}>
      <span className={styles.label}>Drop image here</span>
    </div>
  );
}
```

- [ ] **Step 3: Commit**

```bash
git add src/components/TerminalArea/DropOverlay.module.css src/components/TerminalArea/DropOverlay.tsx
git commit -m "feat: add DropOverlay component"
```

---

## Chunk 4: `useImageDrop` Hook — Core Logic

### Task 4: Create the `useImageDrop` custom hook

This hook encapsulates all drag-drop detection, image filtering, and path writing. It returns `isDragging` state for the overlay.

**Files:**
- Create: `src/components/TerminalArea/useImageDrop.ts`

**Reference docs:**
- Tauri 2 `onDragDropEvent`: `getCurrentWebviewWindow().onDragDropEvent(callback)` from `@tauri-apps/api/webviewWindow`
- The callback receives events with `type: "enter" | "over" | "drop" | "leave"` and `paths: string[]` for drop events

- [ ] **Step 1: Create the hook file**

Create `src/components/TerminalArea/useImageDrop.ts`:

```typescript
import { useState, useEffect, useCallback, useRef } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { writeToSession, saveDroppedImage } from "../../lib/tauri-ipc";

const IMAGE_EXTENSIONS = new Set([
  "png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "tiff",
]);

function isImagePath(filePath: string): boolean {
  const ext = filePath.split(".").pop()?.toLowerCase() ?? "";
  return IMAGE_EXTENSIONS.has(ext);
}

function getImageExtensionFromMime(mime: string): string | null {
  const map: Record<string, string> = {
    "image/png": "png",
    "image/jpeg": "jpg",
    "image/gif": "gif",
    "image/webp": "webp",
    "image/svg+xml": "svg",
    "image/bmp": "bmp",
    "image/tiff": "tiff",
  };
  return map[mime] ?? null;
}

interface UseImageDropOptions {
  activeSessionId: string | null;
  isActiveSessionReadOnly: boolean;
  mockMode: boolean;
}

export function useImageDrop({
  activeSessionId,
  isActiveSessionReadOnly,
  mockMode,
}: UseImageDropOptions) {
  const [isDragging, setIsDragging] = useState(false);
  const dragCounter = useRef(0);

  const writePathToSession = useCallback(
    (path: string) => {
      if (!activeSessionId || isActiveSessionReadOnly || mockMode) return;
      const encoder = new TextEncoder();
      const bytes = Array.from(encoder.encode(path + " "));
      writeToSession({ id: activeSessionId, data: bytes }).catch((err) => {
        console.error("Failed to write dropped image path:", err);
      });
    },
    [activeSessionId, isActiveSessionReadOnly, mockMode],
  );

  // Layer 1: Tauri onDragDropEvent (Finder file drags)
  useEffect(() => {
    if (mockMode) return;

    let unlisten: (() => void) | undefined;

    getCurrentWebviewWindow()
      .onDragDropEvent((event) => {
        if (event.payload.type === "enter" || event.payload.type === "over") {
          setIsDragging(true);
        } else if (event.payload.type === "leave") {
          setIsDragging(false);
        } else if (event.payload.type === "drop") {
          setIsDragging(false);
          const paths = event.payload.paths ?? [];
          const imagePath = paths.find(isImagePath);
          if (imagePath) {
            writePathToSession(imagePath);
          }
        }
      })
      .then((fn) => {
        unlisten = fn;
      });

    return () => {
      unlisten?.();
    };
  }, [mockMode, writePathToSession]);

  // Layer 2: HTML5 drop event (browser image data fallback)
  const onDragEnter = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    dragCounter.current++;
    if (dragCounter.current === 1) {
      setIsDragging(true);
    }
  }, []);

  const onDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
  }, []);

  const onDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    dragCounter.current--;
    if (dragCounter.current === 0) {
      setIsDragging(false);
    }
  }, []);

  const onDrop = useCallback(
    async (e: React.DragEvent) => {
      e.preventDefault();
      dragCounter.current = 0;
      setIsDragging(false);

      if (!activeSessionId || isActiveSessionReadOnly || mockMode) return;

      // Check for file paths first (in case HTML5 also fires for file drags)
      const files = e.dataTransfer.files;
      if (files.length > 0) {
        // Tauri's Layer 1 handles file drags — skip here to avoid double-paste
        return;
      }

      // Handle image data items (e.g., dragged from browser)
      const items = e.dataTransfer.items;
      for (let i = 0; i < items.length; i++) {
        const item = items[i];
        if (item.kind !== "file") continue;
        const ext = getImageExtensionFromMime(item.type);
        if (!ext) continue;

        const blob = item.getAsFile();
        if (!blob) continue;

        const buffer = await blob.arrayBuffer();
        const data = Array.from(new Uint8Array(buffer));
        try {
          const path = await saveDroppedImage({ data, extension: ext });
          writePathToSession(path);
        } catch (err) {
          console.error("Failed to save dropped image:", err);
        }
        return; // Only handle the first image
      }
    },
    [activeSessionId, isActiveSessionReadOnly, mockMode, writePathToSession],
  );

  return {
    isDragging,
    dropHandlers: {
      onDragEnter,
      onDragOver,
      onDragLeave,
      onDrop,
    },
  };
}
```

- [ ] **Step 2: Verify TypeScript compiles**

Run: `npx tsc --noEmit`
Expected: no errors (or only pre-existing ones)

- [ ] **Step 3: Commit**

```bash
git add src/components/TerminalArea/useImageDrop.ts
git commit -m "feat: add useImageDrop hook with two-layer drop detection"
```

---

## Chunk 5: Wire Everything Together in TerminalArea

### Task 5: Integrate `useImageDrop` and `DropOverlay` into `TerminalArea`

**Files:**
- Modify: `src/components/TerminalArea/TerminalArea.tsx`
- Modify: `src/components/TerminalArea/TerminalArea.module.css` (if `terminalArea` needs `position: relative`)

- [ ] **Step 1: Add imports to TerminalArea.tsx**

At the top of `src/components/TerminalArea/TerminalArea.tsx`, add after the existing imports:

```typescript
import { DropOverlay } from "./DropOverlay";
import { useImageDrop } from "./useImageDrop";
```

- [ ] **Step 2: Use the hook inside the component**

Inside the `TerminalArea` component function, after the `mockMode` destructuring (around line 44), add:

```typescript
  const activeSession = sessions.find((s) => s.id === activeSessionId);
  const { isDragging, dropHandlers } = useImageDrop({
    activeSessionId,
    isActiveSessionReadOnly: activeSession?.persisted ?? false,
    mockMode,
  });
```

- [ ] **Step 3: Attach drop handlers and overlay to the JSX**

Replace the return block (starting at line 292) that has sessions:

```tsx
  return (
    <div className={styles.terminalArea} {...dropHandlers}>
      <div className={styles.terminalContainer}>
        {isDragging && <DropOverlay />}
        {isSearchOpen && (
          <SearchBar
            onFindNext={handleFindNext}
            onFindPrevious={handleFindPrevious}
            onClose={closeSearch}
          />
        )}
        {sessions.map((session) => (
          <XTermInstance
            key={session.id}
            ref={setRef(session.id)}
            sessionId={session.id}
            cwd={session.cwd}
            isActive={session.id === activeSessionId}
            mockMode={mockMode}
            readOnly={session.persisted}
            onData={(data) => handleSessionData(session.id, data)}
            onResize={(cols, rows) =>
              handleSessionResize(session.id, cols, rows)
            }
          />
        ))}
      </div>
    </div>
  );
```

The only changes from the original: `{...dropHandlers}` on the outer div, and `{isDragging && <DropOverlay />}` inside `terminalContainer`.

- [ ] **Step 4: Ensure `terminalContainer` has `position: relative`**

Check `src/components/TerminalArea/TerminalArea.module.css`. The `.terminalContainer` class already has `position: relative` (line 15). The `DropOverlay` uses `position: absolute; inset: 0` so it will fill the terminal container correctly. No CSS change needed.

- [ ] **Step 5: Verify TypeScript compiles**

Run: `npx tsc --noEmit`
Expected: no errors

- [ ] **Step 6: Commit**

```bash
git add src/components/TerminalArea/TerminalArea.tsx
git commit -m "feat: wire up image drag-and-drop in TerminalArea"
```

---

## Chunk 6: Manual Testing & Final Commit

### Task 6: Manual integration test

- [ ] **Step 1: Start dev mode**

Run: `npm run tauri dev`

- [ ] **Step 2: Test Finder file drag**

1. Open a Claude Code session in the app
2. Drag a `.png` file from Finder onto the terminal
3. Verify: the overlay appears during drag, disappears on drop, and the absolute file path appears in the terminal as text

- [ ] **Step 3: Test non-image file drag**

1. Drag a `.txt` file onto the terminal
2. Verify: overlay appears during drag, but no path is pasted on drop

- [ ] **Step 4: Test edge cases**

1. Drag an image when no session is active → nothing happens
2. Drag an image onto a persisted/read-only session → nothing happens
3. Drag multiple files where only one is an image → only the image path is pasted

- [ ] **Step 5: Test browser image drag (if applicable)**

1. Drag an image from a web browser onto the terminal
2. Verify: image is saved to temp file and path appears in terminal

- [ ] **Step 6: Final commit if any fixes were needed**

```bash
git add -A
git commit -m "fix: address issues found during manual testing"
```
