// Where Chromium's PDF viewer puts each page (BSD, The Chromium Authors: pdf/document_layout.cc and
// chrome/browser/resources/pdf/viewport.ts, as Electron 44.10 ships them). A page is laid out at 96 px to the inch
// inside its shadow's insets; pages stack with a separator between them, or sit in pairs either side of the middle in
// two page view. Every length here is at 100 %, in CSS pixels: the viewer multiplies by its zoom.

/** The shadow's insets around each page (viewport.ts PAGE_SHADOW). */
export const PAGE_SHADOW = { top: 3, bottom: 7, left: 5, right: 5 } as const;
/** Two page view's insets: the inner side of each page keeps 1 px (document_layout.cc kLeftInsets, kRightInsets). */
const LEFT_INSETS = { top: 3, bottom: 7, left: 5, right: 1 } as const;
const RIGHT_INSETS = { top: 3, bottom: 7, left: 1, right: 5 } as const;
/** Between one page's box and the next one's (document_layout.cc kBottomSeparator). */
export const PAGE_SEPARATOR = 4;
/** CSS pixels per PDF point: PDFium lays a page out at 96 dpi. */
export const PX_PER_POINT = 96 / 72;
/** The browser's preset zoom levels (third_party/blink/common/page/page_zoom.cc kPresetBrowserZoomFactors). */
export const ZOOM_FACTORS = [0.25, 1 / 3, 0.5, 2 / 3, 0.75, 0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2, 2.5, 3, 4, 5] as const;
export const MIN_ZOOM = ZOOM_FACTORS[0];
export const MAX_ZOOM = ZOOM_FACTORS[ZOOM_FACTORS.length - 1]!;

export type Size = { readonly width: number; readonly height: number };
export type Rect = { readonly x: number; readonly y: number; readonly width: number; readonly height: number };
export type Layout = { readonly pages: readonly Rect[]; readonly width: number; readonly height: number };

/** Each page's rectangle without its shadow, and the document's size, for pages of `sizes` (CSS px at 100 %). */
export function pdfLayout(sizes: readonly Size[], twoUp: boolean): Layout {
  if (!sizes.length) return { pages: [], width: 0, height: 0 };
  if (!twoUp) {
    const width = Math.max(...sizes.map((size) => size.width)) + PAGE_SHADOW.left + PAGE_SHADOW.right;
    let y = 0;
    const pages = sizes.map((size, index) => {
      if (index > 0) y += PAGE_SEPARATOR;
      const rect = { x: (width - size.width - PAGE_SHADOW.left - PAGE_SHADOW.right) / 2 + PAGE_SHADOW.left, y: y + PAGE_SHADOW.top, width: size.width, height: size.height };
      y += size.height + PAGE_SHADOW.top + PAGE_SHADOW.bottom;
      return rect;
    });
    return { pages, width, height: y };
  }
  const half = Math.max(...sizes.map((size, index) => size.width + (index % 2 ? RIGHT_INSETS.left + RIGHT_INSETS.right : LEFT_INSETS.left + LEFT_INSETS.right)));
  const width = half * 2;
  const pages: Rect[] = [];
  let y = 0;
  for (let index = 0; index < sizes.length; index += 2) {
    if (index > 0) y += PAGE_SEPARATOR;
    const left = sizes[index]!, right = sizes[index + 1];
    pages.push({ x: half - LEFT_INSETS.right - left.width, y: y + LEFT_INSETS.top, width: left.width, height: left.height });
    if (right) pages.push({ x: half + RIGHT_INSETS.left, y: y + RIGHT_INSETS.top, width: right.width, height: right.height });
    y += Math.max(left.height, right?.height ?? 0) + LEFT_INSETS.top + LEFT_INSETS.bottom;
  }
  return { pages, width, height: y };
}

/** The zoom that fits the document's width (and, for fit to page, the page's height) in the viewport. */
export function fittingZoom(layout: Layout, page: number, viewport: Size, fit: "width" | "page"): number {
  if (!layout.width) return 1;
  const width = viewport.width / layout.width;
  if (fit === "width") return clampZoom(width);
  const rect = layout.pages[page];
  const height = rect ? viewport.height / (rect.height + PAGE_SHADOW.top + PAGE_SHADOW.bottom) : width;
  return clampZoom(Math.min(width, height));
}

/** The zoom a document opens at: 100 %, or less to fit its width (viewport.ts fitToNone). */
export function initialZoom(layout: Layout, viewport: Size): number {
  return Math.min(1, fittingZoom(layout, 0, viewport, "width"));
}

export function clampZoom(zoom: number): number {
  return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, zoom));
}

/** The next preset above (`direction` 1) or below (-1) `zoom`, as viewport.ts zoomIn and zoomOut step. */
export function stepZoom(zoom: number, direction: 1 | -1): number {
  if (direction > 0) return ZOOM_FACTORS.find((factor) => factor > zoom + 1e-6) ?? MAX_ZOOM;
  return [...ZOOM_FACTORS].reverse().find((factor) => factor < zoom - 1e-6) ?? MIN_ZOOM;
}

/** The page most of the viewport shows, at `zoom`, scrolled to `top` (viewport.ts getMostVisiblePage). */
export function mostVisiblePage(layout: Layout, zoom: number, top: number, height: number): number {
  let best = 0, shown = -1;
  for (let index = 0; index < layout.pages.length; index++) {
    const rect = layout.pages[index]!;
    const start = (rect.y - PAGE_SHADOW.top) * zoom, end = (rect.y + rect.height + PAGE_SHADOW.bottom) * zoom;
    if (start > top + height) break;
    const visible = Math.min(end, top + height) - Math.max(start, top);
    // A page wholly in view, or the first one showing more than the rest.
    if (visible > shown + 0.5) { best = index; shown = visible; }
  }
  return best;
}

/** The pages with any part within one viewport of what shows. */
export function pagesNear(layout: Layout, zoom: number, top: number, height: number): number[] {
  const near: number[] = [];
  layout.pages.forEach((rect, index) => {
    if ((rect.y + rect.height) * zoom >= top - height && rect.y * zoom <= top + height * 2) near.push(index);
  });
  return near;
}

/** A page size as Document properties shows it: "8.50 × 11.00 in (portrait)" (pdf_viewer_utils FormatPageSize). */
export function pageSizeText(points: Size, metric: boolean): string {
  const orientation = points.width === points.height ? "square" : points.width > points.height ? "landscape" : "portrait";
  const [width, height] = metric
    ? [Math.round((points.width * 25.4) / 72), Math.round((points.height * 25.4) / 72)].map(String)
    : [(points.width / 72).toFixed(2), (points.height / 72).toFixed(2)];
  return `${width} × ${height} ${metric ? "mm" : "in"} (${orientation})`;
}

/** A file size in the units Chromium's properties dialog writes (ui::FormatBytes). */
export function fileSizeText(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB"];
  let value = bytes / 1024, unit = 0;
  while (value >= 1024 && unit < units.length - 1) { value /= 1024; unit++; }
  return `${value < 100 ? value.toFixed(1) : Math.round(value)} ${units[unit]}`;
}
