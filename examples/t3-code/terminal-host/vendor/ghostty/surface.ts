// Copied from T3 Code 1e2ecbd975 (MIT reference, see LICENSE-T3: apps/web/src/terminal/ghostty/surface.ts),
// with 9c161462c9's history keys (Shift+PageUp/PageDown, Cmd+Shift+Home/End), `selectAll`, the PTY
// resize on a pointer release and the selection autoscroll at the window edge, and 74541bab17's
// synchronized output, Kitty releases for encoded presses only and the single paste per shortcut.
// 9c161462c9's touch scrolling, link gating (`canActivateLink`, `resendSize`) and shared resize observer
// are not here.
// This part holds `GhosttyTerminalSurface`: its creation, public API and DOM event handlers.
// Changes from the reference:
// - `isMacPlatform` comes from `src/shims/utils.ts` (lib/utils.ts imports contracts, effect and tailwind-merge).
// - split for the 1,500-line cap; the reference keeps all of it in this file. The helpers are
//   surfaceFont.ts, surfaceGeometry.ts, surfaceLinks.ts and surfaceInput.ts (each lists its own
//   changes), and the class's state, rendering, scrolling, selection and mouse encoding are
//   `GhosttyTerminalSurfaceBase` (surfaceBase.ts), which this class extends. This file re-exports
//   what the reference's surface.ts exports.
// - the constructor passes the fields' arguments to `super` and keeps the rest of the reference's
//   body, so the event handlers it installs are initialized first; `resizeObserver` is declared here.
import { isMacPlatform } from "../../src/shims/utils";
import { GhosttyTerminalCore, type GhosttyTheme } from "./core";
import { measureGhosttyCell, terminalGridSize, type GhosttyCellMetrics } from "./renderer";
import {
  ensureTerminalSymbolsFont,
  loadTerminalFontFamily,
  terminalFontFamily,
  terminalFontSize,
  type GhosttyTerminalFont,
} from "./surfaceFont";
import { CONTENT_PADDING, terminalScrollbarGeometry } from "./surfaceGeometry";
import { isSameTerminalLink } from "./surfaceLinks";
import {
  applyTerminalCopyEvent,
  clearPrimedTerminalCopyInput,
  ghosttyMouseButton,
  isMiddleClickPastePlatform,
  isTerminalAltGraphText,
  isTerminalCompositionCommitInput,
  isTerminalCompositionKey,
  isTerminalCopyShortcut,
  isTerminalPasteShortcut,
  primeTerminalCopyInput,
  shouldReportTerminalMouse,
  terminalWheelArrowData,
  terminalWheelDeltaRows,
  TERMINAL_LINK_DRAG_THRESHOLD_PX,
} from "./surfaceInput";
import { GhosttyTerminalSurfaceBase, type GhosttyTerminalSurfaceOptions } from "./surfaceBase";

export {
  DEFAULT_TERMINAL_FONT_FAMILY,
  DEFAULT_TERMINAL_FONT_SIZE,
  loadTerminalFontFamily,
  terminalFontFamily,
  terminalFontSize,
  type GhosttyTerminalFont,
} from "./surfaceFont";
export {
  shouldBlinkTerminalCursor,
  terminalContentOriginY,
  terminalGridCellAt,
  terminalScrollbarGeometry,
  terminalScrollbarOffsetAtPointer,
  type TerminalScrollbarGeometry,
} from "./surfaceGeometry";
export { terminalLinkAtPositionWithRange, type TerminalLinkWithRange } from "./surfaceLinks";
export {
  advanceTerminalSelectionClickSequence,
  applyTerminalCopyEvent,
  clearPrimedTerminalCopyInput,
  ghosttyMouseButton,
  isTerminalAltGraphText,
  isTerminalCompositionCommitInput,
  isTerminalCompositionKey,
  isTerminalCopyShortcut,
  isTerminalPasteShortcut,
  primeTerminalCopyInput,
  resolveTerminalMouseData,
  resolveTerminalMouseTrackingState,
  shouldReportTerminalMouse,
  terminalWheelArrowData,
  terminalWheelDeltaRows,
  type TerminalSelectionClickSequence,
} from "./surfaceInput";
export type { GhosttyTerminalSurfaceOptions } from "./surfaceBase";

export interface GhosttySelectionPosition {
  readonly start: { readonly x: number; readonly y: number };
  readonly end: { readonly x: number; readonly y: number };
}

export class GhosttyTerminalSurface extends GhosttyTerminalSurfaceBase {
  private readonly resizeObserver: ResizeObserver;

  private constructor(
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
    super(mount, canvas, input, scrollbar, scrollbarThumb, context, core, metrics, fontFamily, options);
    this.resizeObserver = new ResizeObserver(() => this.fit());
    this.installEvents();
    this.watchDevicePixelRatio();
    this.reducedMotionMedia?.addEventListener("change", this.onReducedMotionChange);
    document.fonts.addEventListener("loadingdone", this.onFontsLoaded);
    this.resizeObserver.observe(mount);
  }

  static async create(
    mount: HTMLElement,
    options: GhosttyTerminalSurfaceOptions,
  ): Promise<GhosttyTerminalSurface> {
    const canvas = document.createElement("canvas");
    canvas.className = "block size-full cursor-text";
    canvas.setAttribute("aria-hidden", "true");

    const input = document.createElement("textarea");
    input.className = "t3-ghostty-input";
    input.setAttribute("aria-label", "Terminal input");
    input.autocapitalize = "off";
    input.autocomplete = "off";
    input.spellcheck = false;
    input.style.cssText =
      "position:absolute;left:4px;top:4px;width:1px;height:1px;opacity:0;padding:0;border:0;resize:none;pointer-events:none;";

    const scrollbar = document.createElement("div");
    scrollbar.className =
      "group absolute top-1 right-px bottom-1 z-1 w-[var(--app-scrollbar-width)] cursor-default touch-none";
    scrollbar.setAttribute("role", "scrollbar");
    scrollbar.setAttribute("aria-label", "Terminal scrollback");
    scrollbar.setAttribute("aria-orientation", "vertical");
    scrollbar.tabIndex = 0;
    scrollbar.hidden = true;
    const scrollbarThumb = document.createElement("div");
    scrollbarThumb.className =
      "absolute inset-x-px top-0 rounded-[3px] bg-[var(--app-scrollbar-thumb)] transition-[background-color] duration-[120ms] ease-[ease-out] group-hover:bg-[var(--app-scrollbar-thumb-hover)] group-focus-visible:bg-[var(--app-scrollbar-thumb-hover)]";
    scrollbar.append(scrollbarThumb);
    mount.replaceChildren(canvas, input, scrollbar);

    const context = canvas.getContext("2d", { alpha: false });
    if (!context) throw new Error("Canvas 2D is unavailable");
    // An opaque canvas backing store initializes to solid black, and the font
    // and WASM loads below leave it on screen for the whole setup window; paint
    // the theme background first so the mount never flashes a black box.
    context.fillStyle = `rgb(${options.theme.background.r}, ${options.theme.background.g}, ${options.theme.background.b})`;
    context.fillRect(0, 0, canvas.width, canvas.height);
    const fontSize = terminalFontSize(options.font?.size);
    try {
      // Cell metrics must come from the faces that will render; measuring before
      // the bundled webfonts load would size the grid from a fallback font.
      await ensureTerminalSymbolsFont();
    } catch {
      // Metrics fall back to whichever faces are already available.
    }
    const fontFamily = await loadTerminalFontFamily(options.font?.family, fontSize);
    const metrics = measureGhosttyCell(context, fontSize, fontFamily);
    const grid = terminalGridSize(mount.clientWidth, mount.clientHeight, metrics, CONTENT_PADDING);
    const core = await GhosttyTerminalCore.create(
      grid.cols,
      grid.rows,
      metrics.width,
      metrics.height,
      options.theme,
      options.onData,
    );
    const surface = new GhosttyTerminalSurface(
      mount,
      canvas,
      input,
      scrollbar,
      scrollbarThumb,
      context,
      core,
      metrics,
      fontFamily,
      options,
    );
    surface.fit();
    return surface;
  }

  /** Pause canvas work without interrupting output parsing or terminal replies. */
  setVisible(visible: boolean): void {
    if (this.disposed || this.visible === visible) return;
    this.visible = visible;
    this.cursorOn = true;
    this.forceFullRender = true;
    this.scrollbarDirty = true;
    if (!visible) {
      this.cancelRender();
      this.setSelectionAutoscroll(0);
      return;
    }
    this.fit();
  }

  write(data: string): void {
    if (this.disposed) return;
    this.core.write(
      data,
      this.synchronizedRenderTimer === null
        ? undefined
        : () => this.cancelSynchronizedRenderTimer(),
    );
    this.synchronizeMouseTrackingState();
    // Restart the blink cycle from the visible phase so the cursor never sits
    // invisible through a stream of output or a burst of typing echo.
    this.cursorOn = true;
    this.scrollbarDirty = true;
    this.requestRender();
  }

  resetAndWrite(data: string): void {
    if (this.disposed) return;
    this.cancelSynchronizedRenderTimer();
    this.lastMouseMotionData = "";
    this.core.resetAndWrite(data);
    this.synchronizeMouseTrackingState();
    // A replayed session starts from the visible phase like any other write:
    // reattaching mid-blink must not open on an invisible cursor.
    this.cursorOn = true;
    this.forceFullRender = true;
    this.scrollbarDirty = true;
    this.requestRender();
  }

  setTheme(theme: GhosttyTheme): void {
    if (this.disposed) return;
    this.theme = theme;
    this.core.setTheme(theme);
    this.forceFullRender = true;
    this.requestRender();
  }

  async setFont(font: GhosttyTerminalFont): Promise<void> {
    if (this.disposed) return;
    const fontSize = terminalFontSize(font.size);
    // The fields only change together with their metrics after the load, and
    // the epoch lets the newest overlapping call win regardless of load order.
    const epoch = ++this.fontEpoch;
    this.pendingFontEpoch = epoch;
    const fontFamily = await loadTerminalFontFamily(font.family, fontSize);
    if (this.disposed || epoch !== this.fontEpoch) return;
    this.pendingFontEpoch = null;
    this.fontFamily = fontFamily;
    this.requestedFontFamily = font.family;
    this.fontSize = fontSize;
    this.applyFontMetrics();
  }

  private applyFontMetrics(): void {
    this.metrics = measureGhosttyCell(this.context, this.fontSize, this.fontFamily);
    this.core.resize(this.cols, this.rows, this.metrics.width, this.metrics.height);
    // Cached IME textarea coordinates are stale in the new cell geometry.
    this.inputLeft = -1;
    this.inputTop = -1;
    this.forceFullRender = true;
    this.scrollbarDirty = true;
    this.fit();
    this.requestRender();
  }

  private readonly onReducedMotionChange = () => {
    if (this.disposed) return;
    // Nothing else wakes an idle steady cursor: the blink timer only reschedules
    // from a render, and reduced motion is exactly the state that stopped it.
    this.cursorOn = true;
    this.requestRender();
  };

  private readonly onFontsLoaded = () => {
    if (this.disposed) return;
    // The explicit load validates every style and applies the newest request.
    // Its own loading events must not revalidate the previously applied face.
    if (this.pendingFontEpoch !== null) return;
    // A face may become available after an earlier fallback measurement. Run
    // the fixed-width guard again before using its newly loaded metrics.
    const fontFamily = terminalFontFamily(this.requestedFontFamily);
    if (fontFamily !== this.fontFamily) {
      this.fontFamily = fontFamily;
      this.applyFontMetrics();
      return;
    }
    // A face that finished loading after the initial measurement changes glyph
    // advances; re-measure and refit so the grid matches what actually renders.
    const metrics = measureGhosttyCell(this.context, this.fontSize, this.fontFamily);
    if (
      metrics.width === this.metrics.width &&
      metrics.height === this.metrics.height &&
      metrics.baseline === this.metrics.baseline
    ) {
      return;
    }
    this.applyFontMetrics();
  };

  fit(): boolean {
    if (this.disposed || !this.visible) return false;
    const width = this.mount.clientWidth;
    const height = this.mount.clientHeight;
    if (width <= 0 || height <= 0) {
      this.hasSize = false;
      this.forceFullRender = true;
      this.cancelRender();
      return false;
    }
    this.hasSize = true;
    const ratio = window.devicePixelRatio || 1;
    const pixelWidth = Math.max(1, Math.round(width * ratio));
    const pixelHeight = Math.max(1, Math.round(height * ratio));
    let shouldRender = false;
    // The DPR transform must be installed even when the target size happens to
    // equal the canvas default 300x150 backing store, so the first fit always
    // schedules a canvas configuration.
    if (
      this.canvas.width !== pixelWidth ||
      this.canvas.height !== pixelHeight ||
      !this.canvasConfigured ||
      this.canvasDevicePixelRatio !== ratio
    ) {
      // A backing-size change clears the canvas. Apply it with the gated paint
      // so a DPR change cannot erase a frame while synchronized output is open.
      this.pendingCanvasConfiguration = { width: pixelWidth, height: pixelHeight, ratio };
      this.forceFullRender = true;
      this.scrollbarDirty = true;
      shouldRender = true;
    } else {
      // A resize can return to the painted size before its queued frame runs.
      this.pendingCanvasConfiguration = null;
    }
    const grid = terminalGridSize(width, height, this.metrics, CONTENT_PADDING);
    this.mountHeight = height;
    // onResize is the only PTY resize channel, so the first successful fit must
    // notify even when the measured grid equals the 1x1 construction sentinel.
    if (grid.cols !== this.cols || grid.rows !== this.rows || !this.resizeNotified) {
      this.cols = grid.cols;
      this.rows = grid.rows;
      this.core.resize(grid.cols, grid.rows, this.metrics.width, this.metrics.height);
      this.notifyResize();
      this.forceFullRender = true;
      this.scrollbarDirty = true;
      shouldRender = true;
    }
    // Rendering synchronously keeps the repaint inside the same frame as the
    // layout change: ResizeObserver fires before paint, so the browser never
    // composites the old backing store stretched into the new element box.
    if (shouldRender || this.forceFullRender) this.renderFrame();
    return true;
  }

  /**
   * The local grid reflows immediately, but the PTY only hears about settled
   * dimensions: notifying on every drag step makes the shell reprint its
   * prompt mid-drag, which reads as jitter.
   */
  private notifyResize(): void {
    this.resizeNotified = true;
    if (this.resizeNotifyTimer !== null) window.clearTimeout(this.resizeNotifyTimer);
    this.resizeNotifyTimer = window.setTimeout(() => {
      this.resizeNotifyTimer = null;
      if (!this.disposed) this.options.onResize(this.cols, this.rows);
    }, 150);
  }

  /**
   * Releasing a panel, drawer, or split divider settles the size, so the PTY
   * hears it now instead of after the debounce. Otherwise typing right after
   * the release reaches a shell that still lays out the old width.
   */
  private readonly onWindowPointerRelease = () => {
    if (this.disposed || this.resizeNotifyTimer === null) return;
    // The drag hook writes the release position before this bubbles to window,
    // so fit now rather than wait a frame for the observer.
    this.fit();
    if (this.resizeNotifyTimer === null) return;
    window.clearTimeout(this.resizeNotifyTimer);
    this.resizeNotifyTimer = null;
    this.options.onResize(this.cols, this.rows);
  };

  focus(): void {
    if (this.disposed || !this.visible) return;
    this.input.focus({ preventScroll: true });
  }

  /**
   * Pastes clipboard text read by the host (context menu) with the same
   * bracketed-paste encoding as a native paste event. The read joins the same
   * race the paste shortcut uses — the token is claimed before it starts — so
   * a shortcut or native paste arriving during the read supersedes this one
   * instead of both reaching the shell.
   */
  async pasteFromClipboard(
    readText: () => Promise<string>,
    isCurrent: () => boolean = () => true,
  ): Promise<void> {
    const token = ++this.pasteShortcutToken;
    const text = await readText();
    if (this.disposed || this.pasteShortcutToken !== token || !isCurrent()) return;
    // As in every paste path, delivering bumps the token so a clipboard read
    // still in flight cannot land after this text reaches the shell.
    this.pasteShortcutToken += 1;
    if (text.length === 0) return;
    const encoded = this.core.encodePaste(text);
    if (encoded.length > 0) this.options.onData(encoded);
  }

  /**
   * Middle-click pastes the terminal's own selection, which is the only
   * primary-selection-like buffer a browser can read. It goes through
   * pasteFromClipboard so it joins the same paste race as every other path.
   * With nothing selected here there is no buffer to paste, and CLIPBOARD is
   * deliberately not substituted: middle-click must never emit text the user
   * only ever copied.
   */
  private pasteTerminalSelection(): void {
    const selection = this.getSelection();
    if (selection.length === 0) return;
    void this.pasteFromClipboard(() => Promise.resolve(selection));
  }

  hasSelection(): boolean {
    return this.core.selectionText().length > 0;
  }

  getSelection(): string {
    return this.core.selectionText();
  }

  /** Select the active screen's entire history, including rows outside the viewport. */
  selectAll(): void {
    this.clearPrimedCopy();
    const range = this.core.selectAll();
    this.selectionAnchorScreen = range?.start ?? null;
    this.selectionEndScreen = range?.end ?? null;
    this.selectionEnd = range ? this.core.screenPointToViewport(range.end.x, range.end.y) : null;
    this.selectionMode = "cell";
    this.selectionBase = null;
    this.setSelectionAutoscroll(0);
    this.options.onSelectionChange();
    this.forceFullRender = true;
    this.requestRender();
  }

  getSelectionPosition(): GhosttySelectionPosition | null {
    if (!this.selectionAnchorScreen || !this.selectionEndScreen || !this.hasSelection())
      return null;
    const before =
      this.selectionAnchorScreen.y < this.selectionEndScreen.y ||
      (this.selectionAnchorScreen.y === this.selectionEndScreen.y &&
        this.selectionAnchorScreen.x <= this.selectionEndScreen.x);
    return before
      ? { start: this.selectionAnchorScreen, end: this.selectionEndScreen }
      : { start: this.selectionEndScreen, end: this.selectionAnchorScreen };
  }

  getSelectionEndClientRect(): { readonly right: number; readonly bottom: number } | null {
    const position = this.getSelectionPosition();
    if (!position) return null;
    const viewportEnd = this.core.screenPointToViewport(position.end.x, position.end.y);
    if (
      !viewportEnd ||
      viewportEnd.x < 0 ||
      viewportEnd.x >= this.cols ||
      viewportEnd.y < 0 ||
      viewportEnd.y >= this.rows
    ) {
      return null;
    }
    const bounds = this.canvas.getBoundingClientRect();
    return {
      right: bounds.left + CONTENT_PADDING + (viewportEnd.x + 1) * this.metrics.width,
      bottom: bounds.top + this.originY + (viewportEnd.y + 1) * this.metrics.height,
    };
  }

  clearSelection(): void {
    this.clearPrimedCopy();
    this.core.clearSelection();
    this.selectionEnd = null;
    this.selectionAnchorScreen = null;
    this.selectionEndScreen = null;
    this.selectionMode = "cell";
    this.selectionBase = null;
    this.setSelectionAutoscroll(0);
    this.options.onSelectionChange();
    // Selection highlights span rows Ghostty may not mark dirty for this change.
    this.forceFullRender = true;
    this.requestRender();
  }

  scrollToBottom(): void {
    this.core.scrollToBottom();
    this.forceFullRender = true;
    this.scrollbarDirty = true;
    this.requestRender();
  }

  isAtBottom(): boolean {
    return this.core.isViewportActive();
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.resizeObserver.disconnect();
    document.fonts.removeEventListener("loadingdone", this.onFontsLoaded);
    this.dprMedia?.removeEventListener("change", this.onDevicePixelRatioChange);
    this.dprMedia = null;
    this.reducedMotionMedia?.removeEventListener("change", this.onReducedMotionChange);
    if (this.selectionScrollTimer !== null) window.clearInterval(this.selectionScrollTimer);
    if (this.resizeNotifyTimer !== null) {
      window.clearTimeout(this.resizeNotifyTimer);
      this.resizeNotifyTimer = null;
      // Flush the settled dimensions so the PTY keeps the final size even when
      // the surface unmounts inside the debounce window.
      this.options.onResize(this.cols, this.rows);
    }
    this.cancelRender();
    if (this.compositionSuppressionTimer !== null) {
      window.clearTimeout(this.compositionSuppressionTimer);
    }
    this.removeEvents();
    this.core.dispose();
    if (
      this.canvas.parentElement === this.mount ||
      this.input.parentElement === this.mount ||
      this.scrollbar.parentElement === this.mount
    ) {
      this.canvas.remove();
      this.input.remove();
      this.scrollbar.remove();
    }
  }

  private readonly onKeyDown = (event: KeyboardEvent) => {
    // A consumed repeat still owes the original encoded press a release.
    if (!event.repeat) this.encodedKeyCodes.delete(event.code);
    if (isTerminalAltGraphText(event) || !this.options.beforeKey(event)) return;
    // IME candidates belong to the textarea, even if the key also resembles
    // a local shortcut. Safari signals the initial composition with code 229.
    if (isTerminalCompositionKey(event, this.composing)) {
      return;
    }
    const mac = isMacPlatform(navigator.platform);
    const primaryModifier = mac ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
    const selectAllShortcut =
      event.key.toLowerCase() === "a" &&
      primaryModifier &&
      !event.altKey &&
      (mac ? !event.shiftKey : event.shiftKey);
    if (selectAllShortcut) {
      event.preventDefault();
      event.stopPropagation();
      this.selectAll();
      return;
    }
    // Shift+PageUp/Down pages history; Ctrl+Shift+Home/End (Cmd on macOS)
    // jumps to its edges. Full-screen applications retain these keys.
    const pageHistory =
      event.shiftKey &&
      !event.altKey &&
      !event.ctrlKey &&
      !event.metaKey &&
      (event.key === "PageUp" || event.key === "PageDown");
    const jumpHistory =
      event.shiftKey &&
      !event.altKey &&
      primaryModifier &&
      (event.key === "Home" || event.key === "End");
    if ((pageHistory || jumpHistory) && !this.core.isAlternateScreen()) {
      const state = this.readScrollbarState();
      if (state !== null) {
        event.preventDefault();
        event.stopPropagation();
        const delta =
          event.key === "Home"
            ? -state.offset
            : event.key === "End"
              ? state.total - state.len - state.offset
              : Math.max(1, state.len) * (event.key === "PageUp" ? -1 : 1);
        this.scrollViewport(delta);
        return;
      }
    }
    if (isTerminalCopyShortcut(event) && this.hasSelection()) {
      // A plain Ctrl+C/Cmd+C fires the browser's native copy event, caught in
      // onCopyEvent; not preventing the default keeps that path alive. WebKit
      // omits the keyboard copy event without a DOM selection, so race the
      // clipboard write against it the same way paste races its read. Ctrl+Shift+C
      // and Ctrl+Insert have no native copy event (Chrome binds the former to
      // inspect), so synthesize one with execCommand("copy").
      const selection = this.getSelection();
      this.primeCopy(selection);
      if (event.shiftKey || event.key.toLowerCase() === "insert") {
        event.preventDefault();
        document.execCommand("copy");
      } else {
        // A plain Ctrl+C is also SIGINT on non-mac: clear the selection once
        // it copies so the next Ctrl+C reaches the shell. The Shift chord and
        // Cmd+C are copy-only, so they keep the selection; resetting the flag
        // up front also drops any clear owed by an earlier gesture that never
        // completed.
        this.clearSelectionAfterCopy = !event.shiftKey && !isMacPlatform(navigator.platform);
        const clipboard = navigator.clipboard;
        if (typeof clipboard?.writeText === "function") {
          // Defer the write past the default action: the native copy event
          // (dispatched synchronously with the default action) claims the
          // token first when it actually writes, and the write covers browsers
          // whose shortcut produces no copy event. The primed textarea is what
          // Electron's edit-menu Copy reads if it runs after this handler.
          const token = ++this.copyShortcutToken;
          void Promise.resolve().then(() => {
            if (this.disposed || this.copyShortcutToken !== token) return;
            void clipboard.writeText(selection).then(
              () => {
                // The write may have been superseded while in flight; only
                // touch the selection if this gesture still owns the token.
                if (this.disposed || this.copyShortcutToken !== token) return;
                if (this.clearSelectionAfterCopy) {
                  this.clearSelectionAfterCopy = false;
                  this.clearSelection();
                }
              },
              () => {
                // The write failed and the native event has already had its
                // chance, so nothing copied and no clear is owed by this
                // gesture; a newer one may have just set the flag, so only
                // drop it if this gesture still owns the token.
                if (this.copyShortcutToken === token) {
                  this.clearSelectionAfterCopy = false;
                }
              },
            );
          });
        }
      }
      return;
    }
    if (isTerminalPasteShortcut(event)) {
      const gesture = { code: event.code, readText: "" };
      this.pasteShortcutGesture = gesture;
      const clipboard = navigator.clipboard;
      if (typeof clipboard?.readText === "function") {
        // Keep native paste available if the read is denied. The token drops
        // a late read; the gesture drops a native duplicate when the read wins.
        const token = ++this.pasteShortcutToken;
        void clipboard.readText().then(
          (text) => {
            if (this.disposed || this.pasteShortcutToken !== token) return;
            this.pasteShortcutToken += 1;
            if (text.length === 0) return;
            if (this.pasteShortcutGesture === gesture) gesture.readText = text;
            this.options.onData(this.core.encodePaste(text));
          },
          () => {
            // Clipboard read denied; the native paste event remains the path.
          },
        );
      }
      return;
    }
    this.clearPrimedCopy();
    const data = this.core.encodeKey(event);
    if (data.length === 0) return;
    this.encodedKeyCodes.add(event.code);
    event.preventDefault();
    event.stopPropagation();
    this.options.onData(data);
  };

  private readonly onKeyUp = (event: KeyboardEvent) => {
    // macOS can omit V keyup while Command is held.
    if (
      event.code === this.pasteShortcutGesture?.code ||
      event.key === "Meta" ||
      event.key === "Control" ||
      event.key === "Shift"
    ) {
      this.pasteShortcutGesture = null;
    }
    if (!this.encodedKeyCodes.delete(event.code)) return;
    if (isTerminalCompositionKey(event, this.composing)) {
      return;
    }
    // Ghostty's encoder only emits release codes when the terminal enabled the
    // Kitty report-event-types flag, so legacy sessions send nothing here.
    const data = this.core.encodeKey(event, "release");
    if (data.length === 0) return;
    event.preventDefault();
    event.stopPropagation();
    this.options.onData(data);
  };

  // A release elsewhere must not leave a press for a later app shortcut to inherit.
  private readonly onWindowKeyUp = (event: KeyboardEvent) => {
    if (event.composedPath().includes(this.input)) return;
    this.encodedKeyCodes.delete(event.code);
  };

  // A fresh press proves a release was missed; repeats can span a focus round trip.
  private readonly onWindowKeyDown = (event: KeyboardEvent) => {
    if (!event.repeat) this.encodedKeyCodes.delete(event.code);
  };

  private readonly onFocus = () => {
    this.focused = true;
    this.cursorOn = true;
    this.requestRender();
  };

  private readonly onBlur = () => {
    this.focused = false;
    this.pasteShortcutGesture = null;
    this.refreshHoveredLink();
    // The steady unfocused hollow cursor must not inherit an off blink phase.
    this.cursorOn = true;
    this.requestRender();
  };

  private readonly onDevicePixelRatioChange = () => {
    this.watchDevicePixelRatio();
    this.fit();
  };

  private watchDevicePixelRatio(): void {
    this.dprMedia?.removeEventListener("change", this.onDevicePixelRatioChange);
    // A resolution media query only fires once for the ratio it was created at,
    // so re-arm it after every change (monitor moves, browser zoom).
    this.dprMedia = window.matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);
    this.dprMedia.addEventListener("change", this.onDevicePixelRatioChange);
  }

  private primeCopy(selection: string): void {
    this.primedCopySelection = selection;
    primeTerminalCopyInput(this.input, selection);
  }

  private clearPrimedCopy(): void {
    clearPrimedTerminalCopyInput(this.input, this.primedCopySelection);
    this.primedCopySelection = "";
  }

  private readonly onCopyEvent = (event: ClipboardEvent) => {
    const selection = this.hasSelection() ? this.getSelection() : this.input.value;
    // Menu-role Copy never hits the keydown primer. The native action reads
    // this.input, so park the current selection first — including when
    // clipboardData is missing and we must not preventDefault.
    this.primeCopy(selection);
    const result = applyTerminalCopyEvent(selection, event.clipboardData);
    if (result.preventDefault) event.preventDefault();
    if (result.claimWriteFallback) {
      // The native event actually wrote the selection; drop the in-flight
      // writeText so a late resolution cannot clobber a later user copy.
      this.copyShortcutToken += 1;
      if (this.clearSelectionAfterCopy) {
        this.clearSelectionAfterCopy = false;
        this.clearSelection();
      }
    }
  };

  private readonly onPaste = (event: ClipboardEvent) => {
    // Always suppress the browser's default insertion: content the textarea
    // would receive (for example an html-only clipboard converted to text)
    // leaks through onInput without bracketed-paste encoding.
    event.preventDefault();
    const data = event.clipboardData?.getData("text/plain") ?? "";
    if (data.length === 0) return;
    if (data === this.pasteShortcutGesture?.readText) {
      // The shortcut's clipboard read already sent this paste.
      this.pasteShortcutGesture = null;
      return;
    }
    // The native paste won the race with actual text; a pending clipboard read
    // must not double. An empty native paste leaves the read as the only path.
    this.pasteShortcutToken += 1;
    this.options.onData(this.core.encodePaste(data));
  };

  private readonly onCompositionStart = () => {
    this.clearPrimedCopy();
    this.clearCompositionInputSuppression();
    this.composing = true;
  };

  private readonly onCompositionEnd = (event: CompositionEvent) => {
    this.composing = false;
    const data = this.input.value || event.data;
    if (data.length > 0) this.options.onData(data);
    this.input.value = "";
    this.compositionInputToSuppress = data;
    this.compositionSuppressionTimer = window.setTimeout(() => {
      this.compositionInputToSuppress = null;
      this.compositionSuppressionTimer = null;
    }, 100);
  };

  private readonly onInput = (event: Event) => {
    const inputEvent = event as InputEvent;
    if (this.composing || inputEvent.isComposing) return;
    const data = this.input.value || inputEvent.data || "";
    if (data === this.compositionInputToSuppress && isTerminalCompositionCommitInput(inputEvent)) {
      this.clearCompositionInputSuppression();
      this.input.value = "";
      return;
    }
    this.clearCompositionInputSuppression();
    if (data.length > 0) this.options.onData(data);
    this.input.value = "";
  };

  private clearCompositionInputSuppression(): void {
    if (this.compositionSuppressionTimer !== null) {
      window.clearTimeout(this.compositionSuppressionTimer);
      this.compositionSuppressionTimer = null;
    }
    this.compositionInputToSuppress = null;
  }

  private readonly onPointerDown = (event: PointerEvent) => {
    this.focus();
    if (shouldReportTerminalMouse(this.core.isMouseTracking(), event)) {
      const button = ghosttyMouseButton(event.button);
      if (button === null) return;
      event.preventDefault();
      event.stopPropagation();
      this.clearHoveredLink("default");
      this.mouseReportingPointerId = event.pointerId;
      this.mouseReportingButton = button;
      this.sendMouse("press", button, event);
      this.canvas.setPointerCapture(event.pointerId);
      return;
    }
    if (event.button === 1 && isMiddleClickPastePlatform()) {
      // Left uncancelled on purpose: cancelling pointerdown drops the
      // compatibility mousedown, which is what activates a split pane.
      this.pasteTerminalSelection();
      return;
    }
    if (event.button !== 0) return;
    const clickCount = this.recordSelectionClick(event);
    const link = this.linkAt(event.clientX, event.clientY);
    if (link && !event.shiftKey && clickCount === 1) {
      event.preventDefault();
      event.stopPropagation();
      this.linkActivationPointerId = event.pointerId;
      this.linkActivationLink = link;
      this.linkActivationOrigin = {
        x: event.clientX,
        y: event.clientY,
        clickCount,
      };
      this.canvas.setPointerCapture(event.pointerId);
      return;
    }
    this.clearHoveredLink();
    this.beginSelection(event, clickCount);
    this.canvas.setPointerCapture(event.pointerId);
  };

  private readonly onPointerMove = (event: PointerEvent) => {
    if (this.linkActivationPointerId === event.pointerId) {
      const origin = this.linkActivationOrigin;
      if (
        origin === null ||
        Math.hypot(event.clientX - origin.x, event.clientY - origin.y) <=
          TERMINAL_LINK_DRAG_THRESHOLD_PX
      ) {
        return;
      }
      this.linkActivationPointerId = null;
      this.linkActivationLink = null;
      this.linkActivationOrigin = null;
      this.clearHoveredLink();
      this.beginSelection(
        {
          clientX: origin.x,
          clientY: origin.y,
        },
        origin.clickCount,
      );
    }
    // Hover motion is only reportable in any-event tracking (DEC 1003); normal and
    // button-event tracking never report motion without a captured pressed button.
    const anyEventTracking = this.synchronizeMouseTrackingState();
    if (
      this.mouseReportingPointerId === event.pointerId ||
      shouldReportTerminalMouse(anyEventTracking, event)
    ) {
      event.preventDefault();
      this.hoverPointer = { x: event.clientX, y: event.clientY };
      // A drag whose press was already sent to the terminal application cannot
      // turn into link activation midway through, so link feedback would lie.
      this.setHoveredLink(null);
      this.canvas.style.cursor = "default";
      this.sendMouse("motion", this.buttonFromButtons(event.buttons), event);
      return;
    }
    this.lastMouseMotionData = "";
    if (!this.selectionAnchorScreen || !this.canvas.hasPointerCapture(event.pointerId)) {
      this.updateHoverCursor(event);
      return;
    }
    this.clearHoveredLink();
    this.selectionPointer = { x: event.clientX, y: event.clientY };
    const bounds = this.canvas.getBoundingClientRect();
    const top = Math.max(bounds.top, 0) + CONTENT_PADDING;
    const bottom = Math.min(bounds.bottom, window.innerHeight) - CONTENT_PADDING;
    this.setSelectionAutoscroll(event.clientY < top ? -1 : event.clientY > bottom ? 1 : 0);
    const cell = this.cellAt(event.clientX, event.clientY);
    if (cell.x === this.selectionEnd?.x && cell.y === this.selectionEnd.y) return;
    this.extendSelectionTo(event.clientX, event.clientY);
  };

  private readonly onPointerLeave = () => {
    this.lastMouseMotionData = "";
    this.clearHoveredLink();
  };

  private readonly onPointerUp = (event: PointerEvent) => {
    this.setSelectionAutoscroll(0);
    if (this.linkActivationPointerId === event.pointerId) {
      event.preventDefault();
      event.stopPropagation();
      const link = this.linkActivationLink;
      this.linkActivationPointerId = null;
      this.linkActivationLink = null;
      this.linkActivationOrigin = null;
      if (this.canvas.hasPointerCapture(event.pointerId)) {
        this.canvas.releasePointerCapture(event.pointerId);
      }
      if (event.type !== "pointercancel") {
        if (link && isSameTerminalLink(link, this.linkAt(event.clientX, event.clientY))) {
          this.options.onLinkActivate(link.text, event);
        }
      }
      return;
    }
    if (this.mouseReportingPointerId === event.pointerId) {
      event.preventDefault();
      event.stopPropagation();
      this.sendMouse("release", this.mouseReportingButton, event);
      this.mouseReportingPointerId = null;
      this.mouseReportingButton = null;
      if (this.canvas.hasPointerCapture(event.pointerId)) {
        this.canvas.releasePointerCapture(event.pointerId);
      }
      if (event.type === "pointercancel") {
        this.clearHoveredLink();
      } else {
        this.hoverPointer = { x: event.clientX, y: event.clientY };
        this.refreshHoveredLink();
      }
      return;
    }
    if (this.canvas.hasPointerCapture(event.pointerId)) {
      this.canvas.releasePointerCapture(event.pointerId);
    }
    if (event.button === 1 && isMiddleClickPastePlatform()) {
      event.preventDefault();
      return;
    }
    if (event.button !== 0) return;
    if (!this.selectionMoved && this.selectionMode === "cell") {
      this.clearSelection();
    }
    this.options.onSelectionChange();
  };

  private readonly onWheel = (event: WheelEvent) => {
    if (event.deltaY === 0) return;
    event.preventDefault();
    const delta = terminalWheelDeltaRows(
      event,
      this.metrics.height,
      this.rows,
      this.wheelRemainder,
    );
    this.wheelRemainder = delta.remainder;
    if (delta.rows === 0) return;
    const magnitude = Math.abs(delta.rows);
    if (shouldReportTerminalMouse(this.core.isMouseTracking(), event)) {
      const button = delta.rows < 0 ? 4 : 5;
      for (let index = 0; index < magnitude; index += 1) {
        this.sendMouse("press", button, event);
      }
      return;
    }
    if (this.core.isAlternateScreen()) {
      // The alternate screen has no scrollback: translate wheel motion into
      // arrow keys so full-screen apps like vim and less scroll, matching xterm.
      this.options.onData(terminalWheelArrowData(delta.rows, this.core.isApplicationCursorKeys()));
      return;
    }
    this.scrollViewport(delta.rows);
  };

  private readonly onMouseDown = (event: MouseEvent) => {
    // Cancelling the middle button here stops autoscroll while still letting
    // the event bubble to the drawer handler that activates a split pane.
    if (event.button === 0 || (event.button === 1 && isMiddleClickPastePlatform())) {
      event.preventDefault();
    }
    this.focus();
  };

  /**
   * Chromium pastes PRIMARY into the focused editable on a middle mouseup, and
   * the hidden textarea is focused, so leaving the default alive would deliver
   * a second paste through onPaste on top of the one onPointerDown sent.
   */
  private readonly onMouseUp = (event: MouseEvent) => {
    if (event.button === 1 && isMiddleClickPastePlatform()) event.preventDefault();
  };

  private readonly onContextMenu = (event: MouseEvent) => {
    if (shouldReportTerminalMouse(this.core.isMouseTracking(), event)) {
      event.preventDefault();
      return;
    }
    this.options.onContextMenu?.(event);
  };

  private readonly onScrollbarPointerDown = (event: PointerEvent) => {
    if (event.button !== 0) return;
    const state = this.readScrollbarState();
    if (state === null) return;
    const bounds = this.scrollbar.getBoundingClientRect();
    const geometry = terminalScrollbarGeometry(state, bounds.height);
    if (geometry === null) return;
    event.preventDefault();
    event.stopPropagation();
    this.scrollbarPointerId = event.pointerId;
    this.scrollbarPointerOffset =
      event.target === this.scrollbarThumb
        ? event.clientY - bounds.top - geometry.thumbTop
        : geometry.thumbHeight / 2;
    this.scrollbar.setPointerCapture(event.pointerId);
    this.scrollbarToPointer(event.clientY, bounds);
  };

  private readonly onScrollbarPointerMove = (event: PointerEvent) => {
    if (event.pointerId !== this.scrollbarPointerId || this.scrollbarState === null) return;
    event.preventDefault();
    this.scrollbarToPointer(event.clientY, this.scrollbar.getBoundingClientRect());
  };

  private readonly onScrollbarPointerUp = (event: PointerEvent) => {
    if (event.pointerId !== this.scrollbarPointerId) return;
    event.preventDefault();
    this.scrollbarPointerId = null;
    if (this.scrollbar.hasPointerCapture(event.pointerId)) {
      this.scrollbar.releasePointerCapture(event.pointerId);
    }
  };

  private readonly onScrollbarKeyDown = (event: KeyboardEvent) => {
    const state = this.readScrollbarState();
    if (state === null) return;
    let delta = 0;
    switch (event.key) {
      case "ArrowUp":
        delta = -1;
        break;
      case "ArrowDown":
        delta = 1;
        break;
      case "PageUp":
        delta = -Math.max(1, state.len);
        break;
      case "PageDown":
        delta = Math.max(1, state.len);
        break;
      case "Home":
        delta = -state.offset;
        break;
      case "End":
        delta = state.total - state.len - state.offset;
        break;
      default:
        return;
    }
    event.preventDefault();
    event.stopPropagation();
    this.scrollViewport(delta);
  };

  private installEvents(): void {
    this.input.addEventListener("keydown", this.onKeyDown);
    this.input.addEventListener("keyup", this.onKeyUp);
    // App shortcuts stop propagation at window, so observe the same capture target.
    window.addEventListener("keyup", this.onWindowKeyUp, { capture: true });
    window.addEventListener("keydown", this.onWindowKeyDown, { capture: true });
    this.input.addEventListener("focus", this.onFocus);
    this.input.addEventListener("blur", this.onBlur);
    this.input.addEventListener("input", this.onInput);
    this.input.addEventListener("paste", this.onPaste);
    this.input.addEventListener("copy", this.onCopyEvent);
    this.input.addEventListener("compositionstart", this.onCompositionStart);
    this.input.addEventListener("compositionend", this.onCompositionEnd);
    this.canvas.addEventListener("pointerdown", this.onPointerDown);
    this.canvas.addEventListener("pointermove", this.onPointerMove);
    this.canvas.addEventListener("pointerleave", this.onPointerLeave);
    this.canvas.addEventListener("pointerup", this.onPointerUp);
    this.canvas.addEventListener("pointercancel", this.onPointerUp);
    this.canvas.addEventListener("wheel", this.onWheel, { passive: false });
    this.canvas.addEventListener("mousedown", this.onMouseDown);
    this.canvas.addEventListener("mouseup", this.onMouseUp);
    this.canvas.addEventListener("contextmenu", this.onContextMenu);
    this.scrollbar.addEventListener("pointerdown", this.onScrollbarPointerDown);
    this.scrollbar.addEventListener("pointermove", this.onScrollbarPointerMove);
    this.scrollbar.addEventListener("pointerup", this.onScrollbarPointerUp);
    this.scrollbar.addEventListener("pointercancel", this.onScrollbarPointerUp);
    this.scrollbar.addEventListener("keydown", this.onScrollbarKeyDown);
    window.addEventListener("pointerup", this.onWindowPointerRelease);
    window.addEventListener("pointercancel", this.onWindowPointerRelease);
  }

  private removeEvents(): void {
    this.input.removeEventListener("keydown", this.onKeyDown);
    this.input.removeEventListener("keyup", this.onKeyUp);
    window.removeEventListener("keyup", this.onWindowKeyUp, { capture: true });
    window.removeEventListener("keydown", this.onWindowKeyDown, { capture: true });
    this.input.removeEventListener("focus", this.onFocus);
    this.input.removeEventListener("blur", this.onBlur);
    this.input.removeEventListener("input", this.onInput);
    this.input.removeEventListener("paste", this.onPaste);
    this.input.removeEventListener("copy", this.onCopyEvent);
    this.input.removeEventListener("compositionstart", this.onCompositionStart);
    this.input.removeEventListener("compositionend", this.onCompositionEnd);
    this.canvas.removeEventListener("pointerdown", this.onPointerDown);
    this.canvas.removeEventListener("pointermove", this.onPointerMove);
    this.canvas.removeEventListener("pointerleave", this.onPointerLeave);
    this.canvas.removeEventListener("pointerup", this.onPointerUp);
    this.canvas.removeEventListener("pointercancel", this.onPointerUp);
    this.canvas.removeEventListener("wheel", this.onWheel);
    this.canvas.removeEventListener("mousedown", this.onMouseDown);
    this.canvas.removeEventListener("mouseup", this.onMouseUp);
    this.canvas.removeEventListener("contextmenu", this.onContextMenu);
    this.scrollbar.removeEventListener("pointerdown", this.onScrollbarPointerDown);
    this.scrollbar.removeEventListener("pointermove", this.onScrollbarPointerMove);
    this.scrollbar.removeEventListener("pointerup", this.onScrollbarPointerUp);
    this.scrollbar.removeEventListener("pointercancel", this.onScrollbarPointerUp);
    this.scrollbar.removeEventListener("keydown", this.onScrollbarKeyDown);
    window.removeEventListener("pointerup", this.onWindowPointerRelease);
    window.removeEventListener("pointercancel", this.onWindowPointerRelease);
  }
}
