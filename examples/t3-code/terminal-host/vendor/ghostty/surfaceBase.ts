// Copied from T3 Code 1e2ecbd975 (MIT reference, see LICENSE-T3: apps/web/src/terminal/ghostty/surface.ts),
// with 74541bab17's synchronized output render gate and its key and paste state (surface.ts).
// This part holds the surface's options and state, and the methods that select, track the hovered
// link, scroll, render and encode the mouse. The reference keeps all of surface.ts in one file.
// Changes from the reference:
// - split out of surface.ts for the 1,500-line cap: these members are `GhosttyTerminalSurfaceBase`,
//   which `GhosttyTerminalSurface` (surface.ts) extends, in the reference's order.
// - the class's `private` members are `protected`.
// - its constructor holds the field assignments of the reference's constructor up to
//   `resizeObserver`; the rest of that constructor is surface.ts's.
import type { GhosttyScrollbar, GhosttySnapshot, GhosttyTerminalCore, GhosttyTheme } from "./core";
import { renderGhosttySnapshot, type GhosttyCellMetrics } from "./renderer";
import { terminalFontSize, type GhosttyTerminalFont } from "./surfaceFont";
import {
  CONTENT_PADDING,
  CURSOR_BLINK_INTERVAL_MS,
  SYNCHRONIZED_OUTPUT_RENDER_TIMEOUT_MS,
  shouldBlinkTerminalCursor,
  terminalContentOriginY,
  terminalGridCellAt,
  terminalScrollbarGeometry,
  terminalScrollbarOffsetAtPointer,
} from "./surfaceGeometry";
import { terminalLinkAtPositionWithRange, type TerminalLinkWithRange } from "./surfaceLinks";
import {
  advanceTerminalSelectionClickSequence,
  resolveTerminalMouseData,
  resolveTerminalMouseTrackingState,
  type TerminalMouseAction,
  type TerminalSelectionClickSequence,
} from "./surfaceInput";

export interface GhosttyTerminalSurfaceOptions {
  readonly theme: GhosttyTheme;
  readonly font?: GhosttyTerminalFont;
  /** Read after font and WASM loading. Hosts can supply a getter for the latest value. */
  readonly visible?: boolean;
  readonly onData: (data: string) => void;
  readonly onResize: (cols: number, rows: number) => void;
  readonly onSelectionChange: () => void;
  readonly beforeKey: (event: KeyboardEvent) => boolean;
  readonly onLinkActivate: (text: string, event: MouseEvent) => void;
  /**
   * A right-click the running application did not claim through mouse
   * reporting. The host owns the menu, so it also owns preventing the browser
   * default — whose Paste entry can never reach a canvas terminal.
   */
  readonly onContextMenu?: (event: MouseEvent) => void;
}

export class GhosttyTerminalSurfaceBase {
  readonly canvas: HTMLCanvasElement;
  readonly input: HTMLTextAreaElement;
  readonly scrollbar: HTMLDivElement;
  cols = 1;
  rows = 1;

  protected readonly mount: HTMLElement;
  protected readonly context: CanvasRenderingContext2D;
  protected readonly core: GhosttyTerminalCore;
  protected readonly options: GhosttyTerminalSurfaceOptions;
  protected visible: boolean;
  protected hasSize = false;
  protected metrics: GhosttyCellMetrics;
  protected fontFamily: string;
  protected requestedFontFamily: string | undefined;
  protected fontSize: number;
  protected fontEpoch = 0;
  protected pendingFontEpoch: number | null = null;
  protected readonly scrollbarThumb: HTMLDivElement;
  protected snapshot: GhosttySnapshot | null = null;
  protected frame = 0;
  protected synchronizedRenderTimer: number | null = null;
  protected cursorTimer: number | null = null;
  protected compositionInputToSuppress: string | null = null;
  protected compositionSuppressionTimer: number | null = null;
  protected cursorOn = true;
  protected renderedCursorY: number | null = null;
  protected forceFullRender = true;
  protected scrollbarDirty = true;
  protected scrollbarState: GhosttyScrollbar | null = null;
  protected scrollbarPointerId: number | null = null;
  protected scrollbarPointerOffset = 0;
  protected disposed = false;
  protected resizeNotifyTimer: number | null = null;
  protected originY = CONTENT_PADDING;
  protected mountHeight = 0;
  protected selectionEnd: { x: number; y: number } | null = null;
  protected selectionAnchorScreen: { x: number; y: number } | null = null;
  protected selectionEndScreen: { x: number; y: number } | null = null;
  protected selectionMode: "cell" | "word" | "line" = "cell";
  // Word/line selection base in screen coordinates so streaming output cannot
  // shift the origin of a drag selection.
  protected selectionBase: {
    start: { x: number; y: number };
    end: { x: number; y: number };
  } | null = null;
  protected selectionScrollTimer: number | null = null;
  protected selectionScrollDelta = 0;
  protected selectionPointer: { x: number; y: number } | null = null;
  protected mouseReportingPointerId: number | null = null;
  protected mouseReportingButton: number | null = null;
  protected linkActivationPointerId: number | null = null;
  protected linkActivationLink: TerminalLinkWithRange | null = null;
  protected linkActivationOrigin: {
    x: number;
    y: number;
    clickCount: number;
  } | null = null;
  protected hoveredLink: TerminalLinkWithRange | null = null;
  protected hoverPointer: { x: number; y: number } | null = null;
  protected selectionClickSequence: TerminalSelectionClickSequence | null = null;
  protected selectionMoved = false;
  protected composing = false;
  protected focused = false;
  protected resizeNotified = false;
  protected canvasConfigured = false;
  protected canvasDevicePixelRatio = 0;
  protected pendingCanvasConfiguration: {
    readonly width: number;
    readonly height: number;
    readonly ratio: number;
  } | null = null;
  protected theme: GhosttyTheme;
  // App capture can hide keydowns, so only encoded presses qualify for Kitty releases.
  protected readonly encodedKeyCodes = new Set<string>();
  protected pasteShortcutToken = 0;
  // Chromium can deliver native paste after the clipboard read resolves.
  // Suppress its duplicate only for the current shortcut gesture.
  protected pasteShortcutGesture: { code: string; readText: string } | null = null;
  protected copyShortcutToken = 0;
  protected clearSelectionAfterCopy = false;
  protected primedCopySelection = "";
  protected wheelRemainder = 0;
  protected lastMouseMotionData = "";
  protected mouseAnyEventTracking = false;
  protected dprMedia: MediaQueryList | null = null;
  // Read live on every blink decision, and watched so that dropping the
  // preference restarts a blink cycle that has no timer left to notice it.
  protected readonly reducedMotionMedia = window.matchMedia?.("(prefers-reduced-motion: reduce)");
  protected inputLeft = -1;
  protected inputTop = -1;

  protected constructor(
    mount: HTMLElement,
    canvas: HTMLCanvasElement,
    input: HTMLTextAreaElement,
    scrollbar: HTMLDivElement,
    scrollbarThumb: HTMLDivElement,
    context: CanvasRenderingContext2D,
    core: GhosttyTerminalCore,
    metrics: GhosttyCellMetrics,
    fontFamily: string,
    options: GhosttyTerminalSurfaceOptions,
  ) {
    this.mount = mount;
    this.canvas = canvas;
    this.input = input;
    this.scrollbar = scrollbar;
    this.scrollbarThumb = scrollbarThumb;
    this.context = context;
    this.core = core;
    this.mouseAnyEventTracking = core.isMouseAnyEventTracking();
    this.metrics = metrics;
    this.options = options;
    this.visible = options.visible ?? true;
    this.theme = options.theme;
    this.fontFamily = fontFamily;
    this.requestedFontFamily = options.font?.family;
    this.fontSize = terminalFontSize(options.font?.size);
  }

  protected recordSelectionClick(
    event: Pick<PointerEvent, "clientX" | "clientY" | "timeStamp">,
  ): number {
    this.selectionClickSequence = advanceTerminalSelectionClickSequence(
      this.selectionClickSequence,
      event,
    );
    return this.selectionClickSequence.count;
  }

  protected beginSelection(event: { clientX: number; clientY: number }, clickCount: number): void {
    const cell = this.cellAt(event.clientX, event.clientY);
    this.selectionMoved = false;
    this.selectionMode = clickCount >= 3 ? "line" : clickCount === 2 ? "word" : "cell";
    const range =
      this.selectionMode === "line"
        ? this.core.selectLine(cell.x, cell.y)
        : this.selectionMode === "word"
          ? this.core.selectWord(cell.x, cell.y)
          : null;
    if (range) {
      this.selectionBase = range.screen;
      this.selectionEnd = range.viewport.end;
      this.selectionAnchorScreen = range.screen.start;
      this.selectionEndScreen = range.screen.end;
      this.options.onSelectionChange();
    } else {
      this.selectionMode = "cell";
      this.selectionBase = null;
      this.selectionEnd = cell;
      const screen = this.core.viewportPointToScreen(cell.x, cell.y);
      this.selectionAnchorScreen = screen;
      this.selectionEndScreen = screen;
      if (screen) {
        this.core.setSelection({ ...screen, tag: 2 }, { ...screen, tag: 2 });
      } else {
        this.core.setSelection(cell, cell);
      }
    }
    this.forceFullRender = true;
    this.requestRender();
  }

  protected extendSelectionTo(clientX: number, clientY: number): void {
    const anchorScreen = this.selectionAnchorScreen;
    if (anchorScreen === null) return;
    const cell = this.cellAt(clientX, clientY);
    this.selectionMoved = true;
    this.selectionEnd = cell;
    const range =
      this.selectionMode === "line"
        ? this.core.selectLine(cell.x, cell.y)
        : this.selectionMode === "word"
          ? this.core.selectWord(cell.x, cell.y)
          : null;
    const cellScreen = this.core.viewportPointToScreen(cell.x, cell.y);
    if (cellScreen === null) return;
    const base = this.selectionBase;
    const beforeBase =
      base !== null &&
      (cellScreen.y < base.start.y ||
        (cellScreen.y === base.start.y && cellScreen.x < base.start.x));
    const anchor = base === null ? anchorScreen : beforeBase ? base.end : base.start;
    const end = range === null ? cellScreen : beforeBase ? range.screen.start : range.screen.end;
    this.selectionAnchorScreen = anchor;
    this.selectionEndScreen = end;
    this.core.setSelection({ ...anchor, tag: 2 }, { ...end, tag: 2 });
    this.options.onSelectionChange();
    this.forceFullRender = true;
    this.requestRender();
  }

  protected setSelectionAutoscroll(delta: number): void {
    this.selectionScrollDelta = delta;
    if (delta === 0) {
      if (this.selectionScrollTimer !== null) {
        window.clearInterval(this.selectionScrollTimer);
        this.selectionScrollTimer = null;
      }
      return;
    }
    if (this.selectionScrollTimer !== null) return;
    // Dragging past the edge scrolls the viewport and keeps extending the
    // selection into the newly revealed rows, like xterm's drag scroller.
    this.selectionScrollTimer = window.setInterval(() => {
      if (this.disposed || this.selectionScrollDelta === 0) return;
      this.scrollViewport(this.selectionScrollDelta);
      const pointer = this.selectionPointer;
      if (pointer) this.extendSelectionTo(pointer.x, pointer.y);
    }, 80);
  }

  protected updateHoverCursor(event: PointerEvent): void {
    this.hoverPointer = { x: event.clientX, y: event.clientY };
    this.refreshHoveredLink();
  }

  protected clearHoveredLink(cursor = ""): void {
    this.hoverPointer = null;
    this.setHoveredLink(null);
    this.canvas.style.cursor = cursor;
  }

  protected refreshHoveredLink(): void {
    const pointer = this.hoverPointer;
    const link = pointer ? this.linkAt(pointer.x, pointer.y) : null;
    this.setHoveredLink(link);
  }

  protected setHoveredLink(link: TerminalLinkWithRange | null): void {
    const previous = this.hoveredLink;
    const unchanged =
      previous?.text === link?.text &&
      previous?.range.start.x === link?.range.start.x &&
      previous?.range.start.y === link?.range.start.y &&
      previous?.range.end.x === link?.range.end.x &&
      previous?.range.end.y === link?.range.end.y;
    this.canvas.style.cursor = link ? "pointer" : "";
    if (unchanged) return;
    this.hoveredLink = link;
    this.forceFullRender = true;
    this.requestRender();
  }

  protected scrollViewport(deltaRows: number): void {
    let delta = Math.trunc(deltaRows);
    const state = this.readScrollbarState();
    if (state !== null) {
      const maxOffset = Math.max(0, state.total - state.len);
      const offset = Math.max(0, Math.min(state.offset + delta, maxOffset));
      delta = offset - state.offset;
      this.scrollbarState = { ...state, offset };
    }
    if (delta === 0) return;
    this.core.scroll(delta);
    this.forceFullRender = true;
    this.scrollbarDirty = true;
    this.requestRender();
  }

  protected scrollbarToPointer(clientY: number, bounds: DOMRect): void {
    const state = this.scrollbarState;
    if (state === null) return;
    const offset = terminalScrollbarOffsetAtPointer(
      state,
      bounds.height,
      clientY - bounds.top,
      this.scrollbarPointerOffset,
    );
    this.scrollViewport(offset - state.offset);
  }

  protected updateScrollbar(): void {
    const state = this.readScrollbarState();
    const geometry =
      state === null
        ? null
        : terminalScrollbarGeometry(
            state,
            Math.max(0, this.mount.clientHeight - CONTENT_PADDING * 2),
          );
    this.scrollbar.hidden = geometry === null;
    if (state === null || geometry === null) return;
    this.scrollbar.setAttribute("aria-valuemin", "0");
    this.scrollbar.setAttribute("aria-valuemax", String(geometry.maxOffset));
    this.scrollbar.setAttribute(
      "aria-valuenow",
      String(Math.max(0, Math.min(state.offset, geometry.maxOffset))),
    );
    this.scrollbarThumb.style.height = `${geometry.thumbHeight}px`;
    this.scrollbarThumb.style.transform = `translateY(${geometry.thumbTop}px)`;
  }

  protected readScrollbarState(): GhosttyScrollbar | null {
    const state = this.core.scrollbarState();
    this.scrollbarState = state;
    return state;
  }

  protected requestRender(): void {
    if (this.disposed || !this.visible || !this.hasSize) return;
    if (this.deferSynchronizedRender() || this.frame !== 0) return;
    this.frame = window.requestAnimationFrame(() => {
      this.frame = 0;
      this.renderFrame();
    });
  }

  protected cancelRender(): void {
    this.cancelSynchronizedRenderTimer();
    if (this.frame !== 0) {
      window.cancelAnimationFrame(this.frame);
      this.frame = 0;
    }
    if (this.cursorTimer !== null) {
      window.clearTimeout(this.cursorTimer);
      this.cursorTimer = null;
    }
  }

  protected cancelSynchronizedRenderTimer(): void {
    if (this.synchronizedRenderTimer === null) return;
    window.clearTimeout(this.synchronizedRenderTimer);
    this.synchronizedRenderTimer = null;
  }

  protected deferSynchronizedRender(): boolean {
    if (!this.core.isSynchronizedOutput()) {
      this.cancelSynchronizedRenderTimer();
      return false;
    }
    // A frame queued before the opening marker would now consume partial rows.
    if (this.frame !== 0) {
      window.cancelAnimationFrame(this.frame);
      this.frame = 0;
    }
    if (this.cursorTimer !== null) {
      window.clearTimeout(this.cursorTimer);
      this.cursorTimer = null;
    }
    if (this.synchronizedRenderTimer === null) {
      this.synchronizedRenderTimer = window.setTimeout(() => {
        this.synchronizedRenderTimer = null;
        this.core.endSynchronizedOutput();
        this.renderFrame();
      }, SYNCHRONIZED_OUTPUT_RENDER_TIMEOUT_MS);
    }
    return true;
  }

  protected renderFrame(): void {
    if (this.disposed || !this.visible) return;
    if (this.frame !== 0) {
      window.cancelAnimationFrame(this.frame);
      this.frame = 0;
    }
    // Hidden thread drawers stay mounted so switching back is instant, but a
    // display:none canvas has nothing to show. Ghostty keeps parsing; the
    // ResizeObserver refits and repaints in full once the mount has a size.
    if (this.mount.clientWidth === 0 || this.mount.clientHeight === 0) {
      this.hasSize = false;
      this.forceFullRender = true;
      this.cancelRender();
      return;
    }
    if (this.deferSynchronizedRender()) return;
    const canvasConfiguration = this.pendingCanvasConfiguration;
    if (canvasConfiguration !== null) {
      this.pendingCanvasConfiguration = null;
      this.canvas.width = canvasConfiguration.width;
      this.canvas.height = canvasConfiguration.height;
      this.context.setTransform(canvasConfiguration.ratio, 0, 0, canvasConfiguration.ratio, 0, 0);
      this.canvasConfigured = true;
      this.canvasDevicePixelRatio = canvasConfiguration.ratio;
    }
    this.snapshot = this.core.snapshot();
    // A cursor that is not blinking right now must be drawn, never caught in an
    // off phase left behind by a blink that has since been turned off.
    if (!this.blinkEnabled()) this.cursorOn = true;
    // The origin only moves together with a forced full repaint: partial
    // dirty-row redraws must never composite rows at a shifted origin over
    // rows painted at the previous one. Bottom anchoring starts once
    // scrollback exists, i.e. when the prompt actually lives at the bottom.
    const scrollState = this.readScrollbarState();
    const anchorBottom = scrollState !== null && scrollState.total > scrollState.len;
    const nextOriginY = terminalContentOriginY(
      this.mountHeight,
      CONTENT_PADDING,
      this.rows,
      this.metrics.height,
      anchorBottom,
    );
    if (nextOriginY !== this.originY) {
      this.originY = nextOriginY;
      this.forceFullRender = true;
    }
    this.refreshHoveredLink();
    renderGhosttySnapshot({
      context: this.context,
      snapshot: this.snapshot,
      metrics: this.metrics,
      fontSize: this.fontSize,
      fontFamily: this.fontFamily,
      padding: CONTENT_PADDING,
      originY: this.originY,
      forceFull: this.forceFullRender,
      cursorOn: this.cursorOn,
      previousCursorY: this.renderedCursorY,
      focused: this.focused,
      hoveredLinkRange: this.hoveredLink?.range ?? null,
      ...(this.theme.selectionBackground !== undefined
        ? { selectionBackground: this.theme.selectionBackground }
        : {}),
    });
    this.positionInput();
    this.renderedCursorY =
      this.cursorOn && this.snapshot.cursorVisible && this.snapshot.cursorY >= 0
        ? this.snapshot.cursorY
        : null;
    if (this.scrollbarDirty) {
      this.scrollbarDirty = false;
      this.updateScrollbar();
    }
    this.forceFullRender = false;
    this.scheduleCursorBlink();
  }

  protected scheduleCursorBlink(): void {
    if (this.cursorTimer !== null) window.clearTimeout(this.cursorTimer);
    this.cursorTimer = null;
    if (!this.blinkEnabled()) return;
    this.cursorTimer = window.setTimeout(() => {
      this.cursorTimer = null;
      this.cursorOn = !this.cursorOn;
      this.requestRender();
    }, CURSOR_BLINK_INTERVAL_MS);
  }

  protected blinkEnabled(): boolean {
    const snapshot = this.snapshot;
    if (!snapshot || !this.visible || !this.hasSize) return false;
    return shouldBlinkTerminalCursor({
      focused: this.focused,
      cursorBlinking: snapshot.cursorBlinking,
      cursorVisible: snapshot.cursorVisible,
      reducedMotion: this.reducedMotionMedia?.matches ?? false,
    });
  }

  protected positionInput(): void {
    const snapshot = this.snapshot;
    if (!snapshot || !snapshot.cursorVisible || snapshot.cursorX < 0 || snapshot.cursorY < 0) {
      return;
    }
    // The IME candidate window anchors to the textarea, so it must follow the
    // terminal cursor for composition to appear where the user is typing.
    const left = CONTENT_PADDING + snapshot.cursorX * this.metrics.width;
    const top = this.originY + snapshot.cursorY * this.metrics.height;
    if (left === this.inputLeft && top === this.inputTop) return;
    this.inputLeft = left;
    this.inputTop = top;
    this.input.style.left = `${left}px`;
    this.input.style.top = `${top}px`;
    this.input.style.height = `${this.metrics.height}px`;
  }

  protected cellAt(clientX: number, clientY: number): { x: number; y: number } {
    const bounds = this.canvas.getBoundingClientRect();
    return {
      x: Math.max(
        0,
        Math.min(
          this.cols - 1,
          Math.floor((clientX - bounds.left - CONTENT_PADDING) / this.metrics.width),
        ),
      ),
      y: Math.max(
        0,
        Math.min(
          this.rows - 1,
          Math.floor((clientY - bounds.top - this.originY) / this.metrics.height),
        ),
      ),
    };
  }

  protected linkAt(clientX: number, clientY: number): TerminalLinkWithRange | null {
    if (!this.snapshot) return null;
    const cell = terminalGridCellAt({
      bounds: this.canvas.getBoundingClientRect(),
      clientX,
      clientY,
      cols: this.cols,
      rows: this.rows,
      metrics: this.metrics,
      padding: CONTENT_PADDING,
      originY: this.originY,
    });
    if (!cell) return null;
    const explicitHyperlink = this.core.hyperlinkAt(cell.x, cell.y);
    if (explicitHyperlink) {
      const start = { ...cell };
      const end = { ...cell };
      while (true) {
        const previous =
          start.x > 0
            ? { x: start.x - 1, y: start.y }
            : start.y > 0 && this.snapshot.rowData[start.y]?.isWrapContinuation
              ? { x: this.cols - 1, y: start.y - 1 }
              : null;
        if (!previous || this.core.hyperlinkAt(previous.x, previous.y) !== explicitHyperlink) break;
        start.x = previous.x;
        start.y = previous.y;
      }
      while (true) {
        const next =
          end.x + 1 < this.cols
            ? { x: end.x + 1, y: end.y }
            : end.y + 1 < this.rows && this.snapshot.rowData[end.y]?.wrapsToNext
              ? { x: 0, y: end.y + 1 }
              : null;
        if (!next || this.core.hyperlinkAt(next.x, next.y) !== explicitHyperlink) break;
        end.x = next.x;
        end.y = next.y;
      }
      return {
        text: explicitHyperlink,
        range: { start, end },
      };
    }
    return terminalLinkAtPositionWithRange(this.snapshot.rowData, cell.y, cell.x);
  }

  protected sendMouse(action: TerminalMouseAction, button: number | null, event: MouseEvent): void {
    const bounds = this.canvas.getBoundingClientRect();
    const data = this.core.encodeMouse({
      action,
      button,
      mods:
        (event.shiftKey ? 1 : 0) |
        (event.ctrlKey ? 1 << 1 : 0) |
        (event.altKey ? 1 << 2 : 0) |
        (event.metaKey ? 1 << 3 : 0),
      x: Math.max(0, event.clientX - bounds.left),
      y: Math.max(0, event.clientY - bounds.top),
      screenWidth: bounds.width,
      screenHeight: bounds.height,
      cellWidth: this.metrics.width,
      cellHeight: this.metrics.height,
      paddingLeft: CONTENT_PADDING,
      paddingRight: CONTENT_PADDING,
      paddingTop: this.originY,
      paddingBottom: Math.max(0, bounds.height - this.originY - this.rows * this.metrics.height),
      anyButtonPressed: event.buttons !== 0,
    });
    const resolution = resolveTerminalMouseData(action, data, this.lastMouseMotionData);
    this.lastMouseMotionData = resolution.nextMotionData;
    if (resolution.send) this.options.onData(data);
  }

  protected synchronizeMouseTrackingState(): boolean {
    // Output writes can toggle DEC 1003 without moving the pointer. Keep the
    // previous mode so the next same-cell motion starts a fresh tracking session.
    const tracking = this.core.isMouseAnyEventTracking();
    const state = resolveTerminalMouseTrackingState(
      this.mouseAnyEventTracking,
      tracking,
      this.lastMouseMotionData,
    );
    this.mouseAnyEventTracking = state.tracking;
    this.lastMouseMotionData = state.motionData;
    return tracking;
  }

  protected buttonFromButtons(buttons: number): number | null {
    if ((buttons & 1) !== 0) return 1;
    if ((buttons & 4) !== 0) return 3;
    if ((buttons & 2) !== 0) return 2;
    if ((buttons & 8) !== 0) return 4;
    if ((buttons & 16) !== 0) return 5;
    return null;
  }
}
