import type { ILinkProvider, ILink, IBufferRange, Terminal } from "@xterm/xterm";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useSessionStore } from "../../stores/sessionStore";

// Matches file paths that:
// - Start with /, ./, ../, or word-char followed by /
//   OR start with a dot-prefixed dir (e.g. .claude/)
// - Contain typical path characters (word chars, hyphens, dots, slashes)
// - End with a file extension (dot followed by 1-10 word chars)
// - Optionally followed by :line or :line:col
//
// Exported for testing.
export const FILE_PATH_REGEX =
  /(?<=^|\s)(?:\.\.?\/|(?<!\/)\/|(?:\.?[\w][\w.-]*\/))[\w.\-/]*\.[\w]{1,10}(?::[\d]+(?::[\d]+)?)?/;

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
          // Pointer cursor + underline decorations serve as the visual indicator.
        },
      });
    }

    callback(links.length > 0 ? links : undefined);
  }

  private openFilePath(pathWithLineCol: string): void {
    const lineColMatch = pathWithLineCol.match(/:(\d+)(?::(\d+))?$/);
    const filePath = pathWithLineCol.replace(/:[\d]+(?::[\d]+)?$/, "");

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

    // Percent-encode each segment so the URL stays valid when the path
    // contains spaces or other special characters (the session cwd is not
    // constrained by FILE_PATH_REGEX).
    const encodedPath = absolutePath
      .split("/")
      .map(encodeURIComponent)
      .join("/");

    let vscodeUrl = `vscode://file${encodedPath}`;
    if (lineColMatch) {
      vscodeUrl += `:${lineColMatch[1]}`;
      if (lineColMatch[2]) {
        vscodeUrl += `:${lineColMatch[2]}`;
      }
    }

    openUrl(vscodeUrl).catch((err) => {
      const reason = err instanceof Error ? err.message : String(err);
      useSessionStore
        .getState()
        .addToast(`Could not open file: ${filePath} (${reason})`, "error");
    });
  }
}
