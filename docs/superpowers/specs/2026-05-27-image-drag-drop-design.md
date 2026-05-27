# Image Drag & Drop Design Spec

## Goal

Allow users to drag and drop images onto the active terminal session. The dropped image's file path is written to the PTY as text, replicating the behavior of pasting an image into Claude Code's terminal.

## Constraints

- Drop target: active terminal only
- No auto-submit — path is pasted, user presses Enter
- Must handle both file drops (from Finder) and image data drops (from browsers/apps)
- Filter to image types only (png, jpg, jpeg, gif, webp, svg, bmp, tiff)

## Architecture

### Two-Layer Drop Detection

**Layer 1: Tauri `onDragDropEvent`** (primary — handles Finder/Desktop file drags)

- Use `getCurrentWebviewWindow().onDragDropEvent()` from `@tauri-apps/api/webviewWindow`
- Built into Tauri 2 core, no plugin required
- Provides full absolute file paths directly
- On `drop` event: filter paths for image extensions, write the first image path to the active session's PTY via existing `writeToSession` IPC

**Layer 2: HTML5 `drop` event** (fallback — handles image data from browsers/apps)

- Add `dragover` and `drop` handlers on the `TerminalArea` component
- On drop: check `dataTransfer.items` for image MIME types
- Read the image data as an `ArrayBuffer`
- Send to new Tauri command `save_dropped_image` which writes to a temp file
- Write the returned temp file path to the active session's PTY

### Visual Feedback

- On dragover: show a semi-transparent overlay on the terminal area with "Drop image here" text
- Style: dark overlay matching Tokyo Night theme, centered text, dashed border
- Remove overlay on dragleave or drop
- Track drag enter/leave with a counter (to handle child element events)

### New Tauri Command

```rust
#[tauri::command]
pub fn save_dropped_image(data: Vec<u8>, extension: String) -> Result<String, String>
```

- Validates extension is an allowed image type
- Writes bytes to a temp file: `{std::env::temp_dir()}/ao-dropped-{uuid}.{extension}`
- Returns the absolute path as a string
- Only used for the blob/data drop case; Finder file drops already have paths

### File Changes

1. **`src/components/TerminalArea/TerminalArea.tsx`** — Add drag-drop event handlers, drop overlay state, and path-writing logic
2. **`src/components/TerminalArea/DropOverlay.tsx`** (new) — Drop overlay UI component
3. **`src-tauri/src/commands.rs`** — Add `save_dropped_image` command
4. **`src-tauri/src/lib.rs`** — Register new command in invoke handler

### Data Flow

```
File dragged from Finder:
  Tauri onDragDropEvent(drop) → file paths[]
  → filter for image extensions
  → writeToSession(activeSessionId, path + " ")

Image data dragged from browser:
  HTML5 drop event → dataTransfer.items → blob
  → invoke("save_dropped_image", { data, extension })
  → temp file path returned
  → writeToSession(activeSessionId, path + " ")
```

### Edge Cases

- Multiple files dropped: only use the first image file
- Non-image files dropped: ignore silently (no error toast)
- No active session: ignore the drop
- Read-only / persisted sessions: ignore the drop
- Drag over sidebar or title bar: no overlay, no drop handling (events only on TerminalArea)
