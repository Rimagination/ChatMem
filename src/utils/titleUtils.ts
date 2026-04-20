const SIDEBAR_TITLE_MAX_WIDTH = 44;
const WORKSPACE_TITLE_MAX_WIDTH = 97;
const ELLIPSIS = "...";

function isFullWidthCodePoint(codePoint: number) {
  if (codePoint >= 0x1100 &&
      (codePoint <= 0x115f ||
        codePoint === 0x2329 ||
        codePoint === 0x232a ||
        (codePoint >= 0x2e80 && codePoint <= 0x3247 && codePoint !== 0x303f) ||
        (codePoint >= 0x3250 && codePoint <= 0x4dbf) ||
        (codePoint >= 0x4e00 && codePoint <= 0xa4c6) ||
        (codePoint >= 0xa960 && codePoint <= 0xa97c) ||
        (codePoint >= 0xac00 && codePoint <= 0xd7a3) ||
        (codePoint >= 0xf900 && codePoint <= 0xfaff) ||
        (codePoint >= 0xfe10 && codePoint <= 0xfe19) ||
        (codePoint >= 0xfe30 && codePoint <= 0xfe6b) ||
        (codePoint >= 0xff01 && codePoint <= 0xff60) ||
        (codePoint >= 0xffe0 && codePoint <= 0xffe6) ||
        (codePoint >= 0x1aff0 && codePoint <= 0x1aff3) ||
        (codePoint >= 0x1aff5 && codePoint <= 0x1affb) ||
        (codePoint >= 0x1affd && codePoint <= 0x1affe) ||
        (codePoint >= 0x1b000 && codePoint <= 0x1b122) ||
        (codePoint >= 0x1b132 && codePoint <= 0x1b150) ||
        codePoint === 0x1b155 ||
        (codePoint >= 0x1b164 && codePoint <= 0x1b167) ||
        (codePoint >= 0x1b170 && codePoint <= 0x1b2fb) ||
        (codePoint >= 0x1f200 && codePoint <= 0x1f202) ||
        (codePoint >= 0x1f210 && codePoint <= 0x1f23b) ||
        (codePoint >= 0x1f240 && codePoint <= 0x1f248) ||
        (codePoint >= 0x1f250 && codePoint <= 0x1f251) ||
        (codePoint >= 0x20000 && codePoint <= 0x3fffd))) {
    return true;
  }

  return false;
}

function displayWidthOf(char: string) {
  const codePoint = char.codePointAt(0);
  if (!codePoint) {
    return 0;
  }

  return isFullWidthCodePoint(codePoint) ? 2 : 1;
}

export function normalizeConversationTitle(text: string | null | undefined) {
  return (text ?? "").replace(/\s+/g, " ").trim();
}

export function measureDisplayWidth(text: string) {
  let width = 0;
  for (const char of text) {
    width += displayWidthOf(char);
  }
  return width;
}

export function truncateTitleByWidth(text: string | null | undefined, maxWidth: number) {
  const normalized = normalizeConversationTitle(text);
  if (!normalized) {
    return normalized;
  }

  const ellipsisWidth = measureDisplayWidth(ELLIPSIS);
  if (measureDisplayWidth(normalized) <= maxWidth) {
    return normalized;
  }

  if (maxWidth <= ellipsisWidth) {
    return ELLIPSIS;
  }

  let currentWidth = 0;
  let output = "";

  for (const char of normalized) {
    const nextWidth = displayWidthOf(char);
    if (currentWidth + nextWidth > maxWidth - ellipsisWidth) {
      break;
    }
    output += char;
    currentWidth += nextWidth;
  }

  return `${output.trimEnd()}${ELLIPSIS}`;
}

export function truncateSidebarTitle(text: string | null | undefined) {
  return truncateTitleByWidth(text, SIDEBAR_TITLE_MAX_WIDTH);
}

export function truncateWorkspaceTitle(text: string | null | undefined) {
  return truncateTitleByWidth(text, WORKSPACE_TITLE_MAX_WIDTH);
}
