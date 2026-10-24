export interface TextSegment {
  code: boolean;
  text: string;
  href?: string;
}

const PUBLIC_URL_EXACT = /^https?:\/\/(?!(?:localhost|127\.0\.0\.1)\b)\S+$/;
const TOKEN_PATTERN = /(\[`[^`]+`\]\([^)\s]+\)|\[[^\]]+\]\([^)\s]+\)|`[^`]+`|https?:\/\/(?!(?:localhost|127\.0\.0\.1)\b)[^\s)）」]+)/g;

/**
 * Splits Markdown-style inline `code` spans, `[label](url)` links, and public URLs
 * into structured segments so displayed URLs are also clickable links.
 */
export function splitInlineCode(input: string): TextSegment[] {
  return input
    .split(TOKEN_PATTERN)
    .filter(Boolean)
    .map((part) => {
      const codeLinkMatch = part.match(/^\[`([^`]+)`\]\(([^)\s]+)\)$/);
      if (codeLinkMatch) {
        return { code: true, text: codeLinkMatch[1], href: codeLinkMatch[2] };
      }
      const linkMatch = part.match(/^\[([^\]]+)\]\(([^)\s]+)\)$/);
      if (linkMatch) {
        return { code: false, text: linkMatch[1], href: linkMatch[2] };
      }
      if (part.startsWith("`") && part.endsWith("`")) {
        const inner = part.slice(1, -1);
        return PUBLIC_URL_EXACT.test(inner)
          ? { code: true, text: inner, href: inner }
          : { code: true, text: inner };
      }
      if (PUBLIC_URL_EXACT.test(part)) {
        return { code: false, text: part, href: part };
      }
      return { code: false, text: part };
    });
}
