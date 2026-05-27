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
