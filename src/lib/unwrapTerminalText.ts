const BLOCK_START = /^(?:[-*+•]\s|\d+[.)]\s|#{1,6}\s|```|>\s|\|)/;
const SPLIT_TOKEN = /:\/\/|^www\.|^\//;

function commonGutterWidth(rows: string[]): number {
  let gutter = Number.POSITIVE_INFINITY;
  for (const row of rows) {
    if (row.trim() === "") continue;
    gutter = Math.min(gutter, row.length - row.trimStart().length);
  }
  return Number.isFinite(gutter) ? gutter : 0;
}

function lastTokenOf(line: string): string {
  const trimmed = line.trimEnd();
  const boundary = trimmed.lastIndexOf(" ");
  return boundary === -1 ? trimmed : trimmed.slice(boundary + 1);
}

export function unwrapTerminalText(text: string, cols: number): string {
  if (text === "" || cols <= 0) return text;

  const rows = text.split("\n");
  const gutter = commonGutterWidth(rows);
  const lines: string[] = [];
  let current: string | null = null;

  const flush = () => {
    if (current !== null) lines.push(current);
    current = null;
  };

  for (let index = 0; index < rows.length; index++) {
    const row = rows[index];

    if (row.trim() === "") {
      flush();
      lines.push("");
      continue;
    }

    const content = row.slice(gutter);
    if (current === null) {
      current = content;
      continue;
    }

    const above = rows[index - 1];
    const body = content.trimStart();
    const firstWord = body.split(/\s/)[0];
    const aboveFilledTheRow = above.length <= cols;
    const couldHaveFitted = above.length + 1 + firstWord.length <= cols;

    if (!aboveFilledTheRow || couldHaveFitted || BLOCK_START.test(body)) {
      flush();
      current = content;
      continue;
    }

    const splitMidToken = above.length >= cols && SPLIT_TOKEN.test(lastTokenOf(current));
    current += (splitMidToken ? "" : " ") + body;
  }

  flush();
  return lines.join("\n");
}
