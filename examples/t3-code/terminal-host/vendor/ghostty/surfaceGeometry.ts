// Copied from T3 Code 1e2ecbd975 (MIT reference, see LICENSE-T3: apps/web/src/terminal/ghostty/surface.ts).
// This part holds the grid's layout constants and its cursor, origin, scrollbar and cell
// arithmetic. The reference keeps all of surface.ts in one file.
// Changes from the reference:
// - split out of surface.ts for the 1,500-line cap.
// - `CONTENT_PADDING`, `CURSOR_BLINK_INTERVAL_MS` and 74541bab17's `SYNCHRONIZED_OUTPUT_RENDER_TIMEOUT_MS`
//   are exported for the surface class.
import type { GhosttyScrollbar } from "./core";
import type { GhosttyCellMetrics } from "./renderer";

export const CONTENT_PADDING = 4;
const MIN_SCROLLBAR_THUMB_HEIGHT = 18;
/** Half a blink cycle: the visible and hidden phases are equally long. */
export const CURSOR_BLINK_INTERVAL_MS = 500;
/** End an unterminated synchronized update instead of freezing the last frame. */
export const SYNCHRONIZED_OUTPUT_RENDER_TIMEOUT_MS = 500;

/**
 * Whether the cursor should keep toggling. An unfocused surface draws a steady
 * hollow cursor instead of blinking, and a reduced-motion reader gets a steady
 * cursor too rather than a permanently animating element.
 */
export function shouldBlinkTerminalCursor(state: {
  readonly focused: boolean;
  readonly cursorBlinking: boolean;
  readonly cursorVisible: boolean;
  readonly reducedMotion: boolean;
}): boolean {
  return state.focused && state.cursorBlinking && state.cursorVisible && !state.reducedMotion;
}

/**
 * Vertical origin of the grid inside the mount. While content is shorter than
 * the viewport the grid sits at the top like a fresh terminal. Once scrollback
 * exists the prompt lives on the bottom row, so the grid anchors to the bottom
 * edge instead: the sub-row remainder moves above row 0 and resizing within a
 * row boundary keeps the prompt pinned instead of snapping up and down.
 */
export function terminalContentOriginY(
  mountHeight: number,
  padding: number,
  rows: number,
  cellHeight: number,
  anchorBottom: boolean,
): number {
  if (!anchorBottom) return padding;
  const slack = mountHeight - padding * 2 - rows * cellHeight;
  return padding + Math.max(0, slack);
}

export interface TerminalScrollbarGeometry {
  readonly thumbHeight: number;
  readonly thumbTop: number;
  readonly maxOffset: number;
}

export function terminalScrollbarGeometry(
  state: GhosttyScrollbar,
  trackHeight: number,
): TerminalScrollbarGeometry | null {
  const total = Math.max(0, state.total);
  const len = Math.max(0, Math.min(state.len, total));
  const maxOffset = Math.max(0, total - len);
  if (trackHeight <= 0 || len <= 0 || maxOffset === 0) return null;
  const thumbHeight = Math.min(
    trackHeight,
    Math.max(MIN_SCROLLBAR_THUMB_HEIGHT, (trackHeight * len) / total),
  );
  const travel = Math.max(0, trackHeight - thumbHeight);
  const offset = Math.max(0, Math.min(state.offset, maxOffset));
  return {
    thumbHeight,
    thumbTop: travel * (offset / maxOffset),
    maxOffset,
  };
}

export function terminalScrollbarOffsetAtPointer(
  state: GhosttyScrollbar,
  trackHeight: number,
  pointerY: number,
  pointerOffset: number,
): number {
  const geometry = terminalScrollbarGeometry(state, trackHeight);
  if (geometry === null) return 0;
  const travel = Math.max(0, trackHeight - geometry.thumbHeight);
  if (travel === 0) return 0;
  const thumbTop = Math.max(0, Math.min(pointerY - pointerOffset, travel));
  return Math.round((thumbTop / travel) * geometry.maxOffset);
}

export function terminalGridCellAt(options: {
  bounds: { left: number; top: number };
  clientX: number;
  clientY: number;
  cols: number;
  rows: number;
  metrics: Pick<GhosttyCellMetrics, "width" | "height">;
  padding: number;
  originY: number;
}): { x: number; y: number } | null {
  const { bounds, clientX, clientY, cols, rows, metrics, padding, originY } = options;
  const gridX = clientX - bounds.left - padding;
  const gridY = clientY - bounds.top - originY;
  if (gridX < 0 || gridY < 0 || gridX >= cols * metrics.width || gridY >= rows * metrics.height) {
    return null;
  }
  return {
    x: Math.floor(gridX / metrics.width),
    y: Math.floor(gridY / metrics.height),
  };
}
