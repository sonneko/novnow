export interface TextSegment {
  code: boolean;
  text: string;
}

/** Splits Markdown-style inline `code` spans into structured segments without altering data files. */
export function splitInlineCode(input: string): TextSegment[] {
  return input
    .split(/(`[^`]+`)/g)
    .filter(Boolean)
    .map((part) =>
      part.startsWith("`") && part.endsWith("`")
        ? { code: true, text: part.slice(1, -1) }
        : { code: false, text: part }
    );
}
