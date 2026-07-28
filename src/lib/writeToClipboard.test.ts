import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { writeToClipboard } from "./writeToClipboard";

describe("writeToClipboard", () => {
  beforeEach(() => {
    document.execCommand = vi.fn(() => true);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("uses the asynchronous clipboard API when the webview provides one", async () => {
    const writeText = vi.fn(async () => {});
    vi.stubGlobal("navigator", { clipboard: { writeText } });

    await expect(writeToClipboard("paste me")).resolves.toBe(true);

    expect(writeText).toHaveBeenCalledWith("paste me");
    expect(document.execCommand).not.toHaveBeenCalled();
  });

  it("falls back to a selection copy when the webview has no clipboard API", async () => {
    vi.stubGlobal("navigator", {});

    await expect(writeToClipboard("paste me")).resolves.toBe(true);

    expect(document.execCommand).toHaveBeenCalledWith("copy");
  });

  it("falls back to a selection copy when the asynchronous API rejects", async () => {
    vi.stubGlobal("navigator", {
      clipboard: {
        writeText: vi.fn(async () => {
          throw new Error("NotAllowedError");
        }),
      },
    });

    await expect(writeToClipboard("paste me")).resolves.toBe(true);

    expect(document.execCommand).toHaveBeenCalledWith("copy");
  });

  it("reports failure when neither route can reach the clipboard", async () => {
    vi.stubGlobal("navigator", {});
    document.execCommand = vi.fn(() => false);

    await expect(writeToClipboard("paste me")).resolves.toBe(false);
  });

  it("leaves no scratch textarea behind after falling back", async () => {
    vi.stubGlobal("navigator", {});

    await writeToClipboard("paste me");

    expect(document.querySelectorAll("textarea")).toHaveLength(0);
  });
});
