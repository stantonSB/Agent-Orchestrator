import { describe, it, expect, vi } from "vitest";
import { FILE_PATH_REGEX } from "./filePathLinkProvider";
import { FilePathLinkProvider } from "./filePathLinkProvider";

// Mock openUrl
vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: vi.fn(() => Promise.resolve()),
}));

// Mock session store with a shared spy so tests can assert on toasts
const { addToastMock } = vi.hoisted(() => ({ addToastMock: vi.fn() }));
vi.mock("../../stores/sessionStore", () => ({
  useSessionStore: {
    getState: () => ({
      addToast: addToastMock,
    }),
  },
}));

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

    it("matches paths at the start of a line", () => {
      expect(matchAll("src/components/Foo.tsx was modified")).toEqual([
        "src/components/Foo.tsx",
      ]);
    });
  });

  describe("does not match non-paths", () => {
    it("ignores plain words", () => {
      expect(matchAll("hello world")).toEqual([]);
    });

    it("ignores URLs", () => {
      expect(matchAll("Visit https://example.com/page.html")).toEqual([]);
    });

    it("ignores bare directory names without file extension", () => {
      expect(matchAll("The src/components directory")).toEqual([]);
    });

    it("ignores single filenames without path separator", () => {
      expect(matchAll("See readme.md for details")).toEqual([]);
    });
  });
});

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
            "vscode://file/Users/test/project/src/file.ts",
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
            "vscode://file/project/src/file.ts:42:10",
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
            "vscode://file/Users/stanton/project/file.ts",
          );
          resolve();
        });
      });
    });

    it("keeps line-only suffix in the URL", async () => {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      vi.mocked(openUrl).mockClear();
      const terminal = createMockTerminal("Error in src/file.ts:42");
      const cwdRef = { current: "/project" };
      const provider = new FilePathLinkProvider(terminal, cwdRef);

      await new Promise<void>((resolve) => {
        provider.provideLinks(1, (links) => {
          const event = { metaKey: true } as MouseEvent;
          links![0].activate(event, links![0].text);
          expect(openUrl).toHaveBeenCalledWith(
            "vscode://file/project/src/file.ts:42",
          );
          resolve();
        });
      });
    });

    it("percent-encodes special characters from the cwd", async () => {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      vi.mocked(openUrl).mockClear();
      const terminal = createMockTerminal("See src/file.ts for details");
      const cwdRef = { current: "/Users/test/My Projects" };
      const provider = new FilePathLinkProvider(terminal, cwdRef);

      await new Promise<void>((resolve) => {
        provider.provideLinks(1, (links) => {
          const event = { metaKey: true } as MouseEvent;
          links![0].activate(event, links![0].text);
          expect(openUrl).toHaveBeenCalledWith(
            "vscode://file/Users/test/My%20Projects/src/file.ts",
          );
          resolve();
        });
      });
    });

    it("shows a toast with the underlying error when openUrl rejects", async () => {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      vi.mocked(openUrl).mockClear();
      vi.mocked(openUrl).mockRejectedValueOnce(
        "url not allowed on the configured scope",
      );
      addToastMock.mockClear();
      const terminal = createMockTerminal("See src/file.ts for details");
      const cwdRef = { current: "/project" };
      const provider = new FilePathLinkProvider(terminal, cwdRef);

      await new Promise<void>((resolve) => {
        provider.provideLinks(1, (links) => {
          const event = { metaKey: true } as MouseEvent;
          links![0].activate(event, links![0].text);
          resolve();
        });
      });

      await vi.waitFor(() => {
        expect(addToastMock).toHaveBeenCalledWith(
          "Could not open file: src/file.ts (url not allowed on the configured scope)",
          "error",
        );
      });
    });
  });

  describe("worktree-aware resolution", () => {
    it("resolves relative paths against the worktree cwd when one is set", async () => {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      vi.mocked(openUrl).mockClear();
      const terminal = createMockTerminal("Created src/newfile.ts");
      const cwdRef = { current: "/Users/test/project" };
      const worktreeCwdRef = {
        current: "/Users/test/project/.claude/worktrees/breezy-frog" as
          | string
          | null,
      };
      const provider = new FilePathLinkProvider(terminal, cwdRef, worktreeCwdRef);

      await new Promise<void>((resolve) => {
        provider.provideLinks(1, (links) => {
          links![0].activate({ metaKey: true } as MouseEvent, links![0].text);
          expect(openUrl).toHaveBeenCalledWith(
            "vscode://file/Users/test/project/.claude/worktrees/breezy-frog/src/newfile.ts",
          );
          resolve();
        });
      });
    });

    it("falls back to the base cwd when the worktree cwd is null", async () => {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      vi.mocked(openUrl).mockClear();
      const terminal = createMockTerminal("Created src/newfile.ts");
      const cwdRef = { current: "/Users/test/project" };
      const worktreeCwdRef = { current: null as string | null };
      const provider = new FilePathLinkProvider(terminal, cwdRef, worktreeCwdRef);

      await new Promise<void>((resolve) => {
        provider.provideLinks(1, (links) => {
          links![0].activate({ metaKey: true } as MouseEvent, links![0].text);
          expect(openUrl).toHaveBeenCalledWith(
            "vscode://file/Users/test/project/src/newfile.ts",
          );
          resolve();
        });
      });
    });

    it("leaves absolute paths untouched even when a worktree cwd is set", async () => {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      vi.mocked(openUrl).mockClear();
      const terminal = createMockTerminal("File at /Users/test/project/src/a.ts");
      const cwdRef = { current: "/Users/test/project" };
      const worktreeCwdRef = {
        current: "/Users/test/project/.claude/worktrees/breezy-frog" as
          | string
          | null,
      };
      const provider = new FilePathLinkProvider(terminal, cwdRef, worktreeCwdRef);

      await new Promise<void>((resolve) => {
        provider.provideLinks(1, (links) => {
          links![0].activate({ metaKey: true } as MouseEvent, links![0].text);
          expect(openUrl).toHaveBeenCalledWith(
            "vscode://file/Users/test/project/src/a.ts",
          );
          resolve();
        });
      });
    });
  });
});
