import { describe, it, expect } from "vitest";
import { unwrapTerminalText } from "./unwrapTerminalText";

const GUTTER = "  ";

function wrapGreedily(text: string, width: number): string[] {
  const rows: string[] = [];
  let row = "";
  for (const word of text.split(" ")) {
    let remaining = word;
    while (remaining.length > width) {
      if (row !== "") {
        rows.push(row);
        row = "";
      }
      rows.push(remaining.slice(0, width));
      remaining = remaining.slice(width);
    }
    if (row === "") row = remaining;
    else if (row.length + 1 + remaining.length <= width) row += ` ${remaining}`;
    else {
      rows.push(row);
      row = remaining;
    }
  }
  if (row !== "") rows.push(row);
  return rows;
}

function renderAsTui(paragraphs: string[], cols: number): string {
  const width = cols - GUTTER.length;
  return paragraphs
    .map((p) => (p === "" ? [""] : wrapGreedily(p, width).map((r) => GUTTER + r)))
    .flat()
    .join("\n");
}

describe("unwrapTerminalText", () => {
  describe("rejoining wrapped prose", () => {
    it("rejoins a row whose first word could not have fitted on the row above", () => {
      const selection = ["  the quick brown fox jumped over", "  the exceptionally lazy dog"].join("\n");

      expect(unwrapTerminalText(selection, 36)).toBe(
        "the quick brown fox jumped over the exceptionally lazy dog",
      );
    });

    it("keeps a deliberate newline when the next word would have fitted above", () => {
      const selection = ["  short line", "  next thought entirely"].join("\n");

      expect(unwrapTerminalText(selection, 80)).toBe("short line\nnext thought entirely");
    });

    it("restores the original sentence from a genuine wrapped Slack draft", () => {
      const original =
        "One thing your review implied that I've split out into its own PR: `busy()` had the identical bare-substring bug one line below `uploadedCount()`. Captions get typed into the same textarea *before* each upload, so a caption containing the capitalised word \"Uploading\" pinned it true for the rest of the run - the completion loop never finished and the script aborted without posting after 90s per image. Not hypothetical for this skill either, whose own captions would plausibly read \"Uploading one at a time\".";

      expect(unwrapTerminalText(renderAsTui([original], 200), 200)).toBe(original);
    });

    it("round-trips a multi-paragraph draft through wrap and unwrap", () => {
      const paragraphs = [
        ":white_check_mark: Thanks @Daniel Wilson - all three actionable ones were fixed before you ticked it, and replies are on each thread.",
        "",
        "The completion loop never finished, so the script aborted without posting after ninety seconds per image, which is not hypothetical for this skill either.",
      ];

      expect(unwrapTerminalText(renderAsTui(paragraphs, 120), 120)).toBe(paragraphs.join("\n"));
    });
  });

  describe("preserving genuine structure", () => {
    it("keeps blank lines as paragraph breaks", () => {
      const selection = ["  first paragraph", "", "  second paragraph"].join("\n");

      expect(unwrapTerminalText(selection, 80)).toBe("first paragraph\n\nsecond paragraph");
    });

    it("never absorbs a bullet into the line above it", () => {
      const selection = [
        "  Here are the three things that went wrong during the run today",
        "  - the completion loop never finished",
        "  - the script aborted without posting",
      ].join("\n");

      expect(unwrapTerminalText(selection, 66)).toBe(
        "Here are the three things that went wrong during the run today\n- the completion loop never finished\n- the script aborted without posting",
      );
    });

    it("never absorbs a numbered item into the line above it", () => {
      const selection = ["  a line that very nearly fills the whole row", "  1. the first step"].join("\n");

      expect(unwrapTerminalText(selection, 47)).toBe(
        "a line that very nearly fills the whole row\n1. the first step",
      );
    });

    it("never absorbs a fence delimiter into the line above it", () => {
      const selection = ["  Slack reply for Dan, paste-ready and complete", "  ```"].join("\n");

      expect(unwrapTerminalText(selection, 49)).toBe("Slack reply for Dan, paste-ready and complete\n```");
    });
  });

  describe("split tokens", () => {
    it("rejoins a hard-split URL without inserting a space", () => {
      const url = "https://github.com/simplybusiness/power-rangers/pull/82";
      const selection = [`  ${url.slice(0, 38)}`, `  ${url.slice(38)} - one line`].join("\n");

      expect(unwrapTerminalText(selection, 40)).toBe(`${url} - one line`);
    });

    it("rejoins a URL split across three rows without inserting spaces", () => {
      const url = "https://github.com/simplybusiness/power-rangers/pull/82/files#r12345";
      const selection = [`  ${url.slice(0, 18)}`, `  ${url.slice(18, 36)}`, `  ${url.slice(36)}`].join("\n");

      expect(unwrapTerminalText(selection, 20)).toBe(url);
    });

    it("inserts a space when a full row ends in ordinary prose", () => {
      const selection = ["  alpha bravo charlie", "  delta echo foxtrot"].join("\n");

      expect(unwrapTerminalText(selection, 21)).toBe("alpha bravo charlie delta echo foxtrot");
    });
  });

  describe("gutter handling", () => {
    it("strips the common gutter shared by every row", () => {
      const selection = ["    indented one", "", "    indented two"].join("\n");

      expect(unwrapTerminalText(selection, 80)).toBe("indented one\n\nindented two");
    });

    it("leaves relative indentation intact below the common gutter", () => {
      const selection = ["  outer line", "      inner line"].join("\n");

      expect(unwrapTerminalText(selection, 80)).toBe("outer line\n    inner line");
    });

    it("still rejoins wrapped rows when the selection began part-way through the first row", () => {
      const selection = [
        ":white_check_mark: Thanks Dan",
        "  the quick brown fox jumped over",
        "  the exceptionally lazy dog",
      ].join("\n");

      expect(unwrapTerminalText(selection, 35)).toBe(
        ":white_check_mark: Thanks Dan\n  the quick brown fox jumped over the exceptionally lazy dog",
      );
    });
  });

  describe("refusing to guess", () => {
    it("leaves the next line alone when the row above already exceeds the grid width", () => {
      const alreadyJoined = "a logical line that xterm had already stitched back together itself";
      const selection = [`  ${alreadyJoined}`, "  a genuinely separate line"].join("\n");

      expect(unwrapTerminalText(selection, 40)).toBe(`${alreadyJoined}\na genuinely separate line`);
    });

    it("returns the selection untouched when the width is unknown", () => {
      const selection = ["  the quick brown fox jumped over", "  the exceptionally lazy dog"].join("\n");

      expect(unwrapTerminalText(selection, 0)).toBe(selection);
    });

    it("returns an empty selection untouched", () => {
      expect(unwrapTerminalText("", 120)).toBe("");
    });

    it("returns a single row with only its gutter stripped", () => {
      expect(unwrapTerminalText("  just the one line", 120)).toBe("just the one line");
    });
  });
});
