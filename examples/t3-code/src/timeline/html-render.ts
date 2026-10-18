// Adapted from T3 Code 9c161462c9 packages/shared/src/htmlRender.ts (MIT; LICENSE-T3).
// Document bootstrapping remains owned by the embedded server.
export const HTML_RENDER_TOOL_NAME = "html_render";
export const HTML_RENDER_MIN_HEIGHT = 80;
export const HTML_RENDER_MAX_HEIGHT = 2000;
export const HTML_RENDER_MAX_TITLE_LENGTH = 200;

export interface HtmlRenderReference {
  readonly attachmentId: string;
  readonly title: string;
  readonly height: number;
  readonly heights?: ReadonlyArray<readonly [width: number, height: number]>;
  readonly outline?: string;
}

export const HTML_RENDER_MEASURE_WIDTHS = [320, 375, 430, 520, 640, 728, 860, 1000, 1144] as const;

export const HTML_RENDER_COLUMN_WIDTH = 728;

const MAX_MEASURED_HEIGHTS = 24;

function readMeasuredHeights(value: unknown) {
  if (!Array.isArray(value) || value.length === 0 || value.length > MAX_MEASURED_HEIGHTS) {
    return undefined;
  }
  const heights = value.flatMap((entry) =>
    Array.isArray(entry) &&
    entry.length === 2 &&
    Number.isInteger(entry[0]) &&
    entry[0] >= 1 &&
    entry[0] <= 10_000 &&
    typeof entry[1] === "number" &&
    Number.isFinite(entry[1])
      ? [[entry[0] as number, clampHtmlRenderHeight(entry[1])] as const]
      : [],
  );
  return heights.length === value.length
    ? [...heights].sort((left, right) => left[0] - right[0])
    : undefined;
}

export function readHtmlRenderReference(value: unknown): HtmlRenderReference | undefined {
  if (typeof value !== "object" || value === null) return undefined;
  const { attachmentId, title, height, heights, outline } = value as Record<string, unknown>;
  if (
    typeof attachmentId !== "string" ||
    attachmentId.length === 0 ||
    attachmentId.length > 256 ||
    typeof title !== "string" ||
    typeof height !== "number" ||
    !Number.isFinite(height)
  ) {
    return undefined;
  }
  const measured = readMeasuredHeights(heights);
  return {
    attachmentId,
    title: title.trim().slice(0, HTML_RENDER_MAX_TITLE_LENGTH) || "HTML",
    height: clampHtmlRenderHeight(height),
    ...(measured === undefined ? {} : { heights: measured }),
    ...(measured !== undefined && typeof outline === "string" && OUTLINE_PATTERN.test(outline)
      ? { outline }
      : {}),
  };
}

export function htmlRenderReferencesEqual(left: HtmlRenderReference, right: HtmlRenderReference) {
  return (
    left.attachmentId === right.attachmentId &&
    left.title === right.title &&
    left.height === right.height &&
    left.outline === right.outline &&
    (left.heights ?? []).length === (right.heights ?? []).length &&
    (left.heights ?? []).every(
      ([width, height], index) =>
        right.heights?.[index]?.[0] === width && right.heights[index][1] === height,
    )
  );
}

// The taller of the heights measured at the nearest widths on each side. A
// breakpoint between two measured widths can make the page as tall as either.
function measuredHeight(heights: NonNullable<HtmlRenderReference["heights"]>, width: number) {
  const above = heights.findIndex(([measuredWidth]) => measuredWidth >= width);
  const high = above === -1 ? heights.length - 1 : above;
  const low = heights[high]![0] === width ? high : Math.max(0, high - 1);
  return Math.max(heights[low]![1], heights[high]![1]);
}

/**
 * A page even a few pixels taller than its frame scrolls inside it and takes
 * the reader's scroll, so the frame fits the page.
 */
export function htmlRenderFrameHeight(
  reference: HtmlRenderReference,
  width: number,
  contentHeight?: number,
) {
  const heights = reference.heights;
  if (heights === undefined || heights.length === 0) {
    return clampHtmlRenderHeight(Math.min(reference.height, contentHeight ?? reference.height));
  }
  const cap =
    measuredHeight(heights, HTML_RENDER_COLUMN_WIDTH) > reference.height
      ? reference.height
      : HTML_RENDER_MAX_HEIGHT;
  return clampHtmlRenderHeight(Math.min(cap, contentHeight ?? measuredHeight(heights, width)));
}

export interface HtmlRenderOutlineBox {
  readonly kind: "text" | "media" | "table";
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
  readonly lines: number;
}

const OUTLINE_KINDS = { t: "text", m: "media", g: "table" } as const;
const OUTLINE_LETTERS = { text: "t", media: "m", table: "g" } as const;
const OUTLINE_UNIT = 4;
export const HTML_RENDER_OUTLINE_MAX_BOXES = 48;
const OUTLINE_BOX = "[tmg]\\d{1,4},\\d{1,4},[1-9]\\d{0,3},[1-9]\\d{0,3},[1-9]\\d{0,2}";
const OUTLINE_PATTERN = new RegExp(
  `^${OUTLINE_BOX}(?:;${OUTLINE_BOX}){0,${HTML_RENDER_OUTLINE_MAX_BOXES - 1}}$`,
);

export function encodeHtmlRenderOutline(
  boxes: ReadonlyArray<HtmlRenderOutlineBox>,
): string | undefined {
  const unit = (value: number) => Math.min(9_999, Math.max(0, Math.round(value / OUTLINE_UNIT)));
  const encoded = boxes
    .slice(0, HTML_RENDER_OUTLINE_MAX_BOXES)
    .flatMap((box) =>
      unit(box.width) > 0 && unit(box.height) > 0
        ? [
            `${OUTLINE_LETTERS[box.kind]}${unit(box.x)},${unit(box.y)},${unit(box.width)},${unit(box.height)},${Math.min(999, Math.max(1, Math.round(box.lines)))}`,
          ]
        : [],
    )
    .join(";");
  return encoded === "" ? undefined : encoded;
}

export function htmlRenderOutlineBoxes(
  reference: HtmlRenderReference,
  width: number,
): ReadonlyArray<HtmlRenderOutlineBox> {
  const heights = reference.heights;
  if (reference.outline === undefined || heights === undefined || heights.length === 0) return [];
  const scaleX = width / HTML_RENDER_COLUMN_WIDTH;
  const scaleY = measuredHeight(heights, width) / measuredHeight(heights, HTML_RENDER_COLUMN_WIDTH);
  return reference.outline.split(";").map((entry) => {
    const [x = 0, y = 0, boxWidth = 0, boxHeight = 0, lines = 1] = entry
      .slice(1)
      .split(",")
      .map(Number);
    return {
      kind: OUTLINE_KINDS[entry[0] as keyof typeof OUTLINE_KINDS],
      x: x * OUTLINE_UNIT * scaleX,
      y: y * OUTLINE_UNIT * scaleY,
      width: boxWidth * OUTLINE_UNIT * scaleX,
      height: boxHeight * OUTLINE_UNIT * scaleY,
      lines,
    };
  });
}

export function htmlRenderFileName(title: string) {
  const name = title
    .replace(/[\\/:*?"<>|\p{Cc}]+/gu, " ")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, 120)
    .trim();
  return `${name || "Page"}.html`;
}

export function clampHtmlRenderHeight(height: number): number {
  return Math.min(HTML_RENDER_MAX_HEIGHT, Math.max(HTML_RENDER_MIN_HEIGHT, Math.round(height)));
}

export interface HtmlRenderFonts {
  readonly sans: string;
  readonly mono: string;
}

export const HTML_RENDER_DEFAULT_FONTS: HtmlRenderFonts = {
  sans: '-apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif',
  // Concrete names only: some engines alias `ui-monospace` to the proportional UI font.
  mono: '"SF Mono", "SFMono-Regular", Menlo, Consolas, "Liberation Mono", monospace',
};

/**
 * Arial-metric faces stand in for the system UI fonts readers see, so measured
 * heights and preview screenshots wrap text close to how a client will.
 */
export const HTML_RENDER_MEASURE_FONTS: HtmlRenderFonts = {
  sans: '"Liberation Sans", Arimo, Arial, Helvetica, sans-serif',
  mono: '"Liberation Mono", Cousine, Menlo, Consolas, monospace',
};

export interface HtmlRenderTheme {
  readonly appearance: 'light' | 'dark';
  readonly variables: Readonly<Record<string, string>>;
}

const FIXED_COLORS = {
  light: {
    success: "#10b981",
    successForeground: "#047857",
    info: "#3b82f6",
    infoForeground: "#1d4ed8",
    chart: ["#0d9488", "#d97706", "#9333ea", "#e11d48", "#65a30d"],
  },
  dark: {
    success: "#10b981",
    successForeground: "#34d399",
    info: "#3b82f6",
    infoForeground: "#60a5fa",
    chart: ["#2dd4bf", "#fbbf24", "#c084fc", "#fb7185", "#a3e635"],
  },
} as const;

/**
 * Names follow the app's own tokens, except `--accent`, which is the theme's
 * brand color here rather than the app's neutral hover surface.
 */
export function htmlRenderTheme(
  colors: Readonly<Record<string, string>>,
  appearance: 'light' | 'dark',
  fonts: HtmlRenderFonts = HTML_RENDER_DEFAULT_FONTS,
): HtmlRenderTheme {
  const fixed = FIXED_COLORS[appearance];
  return {
    appearance,
    variables: {
      "--background": colors.canvas,
      "--foreground": colors.text,
      "--muted": colors.muted,
      "--muted-foreground": colors.mutedForeground,
      "--card": colors.surface,
      "--card-foreground": colors.text,
      "--popover": colors.surfaceOverlay,
      "--popover-foreground": colors.text,
      "--secondary": colors.secondary,
      "--secondary-foreground": colors.secondaryForeground,
      "--border": colors.border,
      "--input": colors.input,
      "--ring": colors.focus,
      "--primary": colors.messageAction,
      "--primary-foreground": colors.messageActionForeground,
      "--accent": colors.accent,
      "--accent-foreground": colors.accentForeground,
      "--accent-surface": colors.accentSurface,
      "--accent-surface-foreground": colors.accentSurfaceForeground,
      "--destructive": colors.error,
      "--destructive-foreground": colors.errorForeground,
      "--destructive-surface": colors.errorSurface,
      "--warning": colors.warning,
      "--warning-foreground": colors.warningForeground,
      "--warning-surface": colors.warningSurface,
      "--success": fixed.success,
      "--success-foreground": fixed.successForeground,
      "--info": fixed.info,
      "--info-foreground": fixed.infoForeground,
      "--code-background": colors.codeBackground,
      "--code-foreground": colors.codeForeground,
      "--chart-1": colors.accent,
      ...Object.fromEntries(fixed.chart.map((color, index) => [`--chart-${index + 2}`, color])),
      "--radius": "0.625rem",
      "--font-sans": fonts.sans,
      "--font-mono": fonts.mono,
    },
  };
}

const HOST_CONTEXT_CHANGED_METHOD = "ui/notifications/host-context-changed";
const OPEN_LINK_METHOD = "ui/open-link";
const SIZE_CHANGED_METHOD = "ui/notifications/size-changed";
// Not part of MCP Apps: a focused frame keeps its keys from the app, so the
// client tells the page which chords are app shortcuts and the page passes
// those on.
const SHORTCUTS_METHOD = "t3/notifications/shortcuts";
const KEY_DOWN_METHOD = "t3/notifications/keydown";
// Not part of MCP Apps: a click on a page image opens it in the app's image viewer.
const OPEN_IMAGE_METHOD = "t3/notifications/open-image";

export function readHtmlRenderContentHeight(data: unknown): number | undefined {
  if (typeof data !== "object" || data === null) return undefined;
  const { jsonrpc, method, params } = data as Record<string, unknown>;
  if (jsonrpc !== "2.0" || method !== SIZE_CHANGED_METHOD) return undefined;
  const height =
    typeof params === "object" && params !== null
      ? (params as { height?: unknown }).height
      : undefined;
  return typeof height === "number" && Number.isFinite(height) && height > 0 ? height : undefined;
}

export function readHtmlRenderLinkRequest(data: unknown):
  | {
      readonly id: string | number;
      readonly url: string;
      readonly metaKey: boolean;
      readonly ctrlKey: boolean;
    }
  | undefined {
  if (typeof data !== "object" || data === null) return undefined;
  const { jsonrpc, id, method, params } = data as Record<string, unknown>;
  if (jsonrpc !== "2.0" || method !== OPEN_LINK_METHOD) return undefined;
  if (typeof id !== "string" && typeof id !== "number") return undefined;
  if (typeof params !== "object" || params === null) return undefined;
  const { url, metaKey, ctrlKey } = params as Record<string, unknown>;
  return typeof url === "string" && /^https?:\/\//i.test(url)
    ? { id, url, metaKey: metaKey === true, ctrlKey: ctrlKey === true }
    : undefined;
}

/**
 * The image a framed render's reader clicked, if `data` is an open-image
 * notification for an http(s) or inline image URL. Server-side publishing
 * inlines local images, so most arrive as `data:image/` URLs.
 */
export function readHtmlRenderImageRequest(
  data: unknown,
): { readonly src: string; readonly name: string } | undefined {
  if (typeof data !== "object" || data === null) return undefined;
  const { jsonrpc, method, params } = data as Record<string, unknown>;
  if (jsonrpc !== "2.0" || method !== OPEN_IMAGE_METHOD) return undefined;
  if (typeof params !== "object" || params === null) return undefined;
  const { src, name } = params as Record<string, unknown>;
  if (typeof src !== "string" || !/^(?:https?:\/\/|data:image\/)/i.test(src)) return undefined;
  const label = typeof name === "string" ? name.trim().slice(0, HTML_RENDER_MAX_TITLE_LENGTH) : "";
  return { src, name: label || "Image" };
}

/** The key a framed render passed on in a `t3/notifications/keydown` notification. */
export function readHtmlRenderKeyDown(data: unknown):
  | {
      readonly key: string;
      readonly code: string;
      readonly metaKey: boolean;
      readonly ctrlKey: boolean;
      readonly shiftKey: boolean;
      readonly altKey: boolean;
      readonly repeat: boolean;
    }
  | undefined {
  if (typeof data !== "object" || data === null) return undefined;
  const { jsonrpc, method, params } = data as Record<string, unknown>;
  if (jsonrpc !== "2.0" || method !== KEY_DOWN_METHOD) return undefined;
  if (typeof params !== "object" || params === null) return undefined;
  const { key, code, metaKey, ctrlKey, shiftKey, altKey, repeat } = params as Record<
    string,
    unknown
  >;
  if (typeof key !== "string" || typeof code !== "string") return undefined;
  return {
    key,
    code,
    metaKey: metaKey === true,
    ctrlKey: ctrlKey === true,
    shiftKey: shiftKey === true,
    altKey: altKey === true,
    repeat: repeat === true,
  };
}

export interface HtmlRenderShortcut {
  readonly key: string;
  readonly metaKey: boolean;
  readonly ctrlKey: boolean;
  readonly shiftKey: boolean;
  readonly altKey: boolean;
}

export function htmlRenderShortcutsMessage(
  shortcuts: ReadonlyArray<HtmlRenderShortcut>,
  blocked: ReadonlyArray<HtmlRenderShortcut>,
) {
  return { jsonrpc: "2.0", method: SHORTCUTS_METHOD, params: { shortcuts, blocked } } as const;
}

export function htmlRenderResult(id: string | number) {
  return { jsonrpc: "2.0", id, result: {} } as const;
}

const THEME_FRAGMENT_KEY = "t3-theme";

export function htmlRenderThemeFragment(theme: HtmlRenderTheme): string {
  return `#${THEME_FRAGMENT_KEY}=${encodeURIComponent(JSON.stringify(theme))}`;
}

export function htmlRenderThemeMessage(theme: HtmlRenderTheme) {
  return {
    jsonrpc: "2.0",
    method: HOST_CONTEXT_CHANGED_METHOD,
    params: { theme: theme.appearance, styles: { variables: theme.variables } },
  } as const;
}
