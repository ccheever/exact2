// An Excel workbook (MIT reference, see LICENSE-T3: apps/web/src/components/files/office/Spreadsheet.tsx; T3 Code
// 7c3b3b3605ef; adapted there from opencode, MIT, see LICENSE.opencode): the spreadsheet worker paints the visible
// cells into one canvas under sticky headers; selection, keys, links and copy follow Excel, and the sheet tabs switch
// sheets. The reference's grid effect is already imperative, so it is kept as one function.
import {
  cellAtPoint, extendTo, hyperlinkAtCell, normalizeRange, parseHyperlinkLocation, safeExternalHyperlink, selectionAt, selectionKeyReducer,
  type CellAddr, type Selection,
} from "@betteroffice/xlsx/headless";
import { h, observeResize, post, surfaceNotice } from "../chrome";
import { externalUrl } from "./links";
import { columnName, contains, coverMerges, lastCell, settleMerges, skipHidden } from "./sheet-cells";
import { headerHeight, paintCells, paintHeaders, paintSelection, shiftGrid, type Painted } from "./sheet-paint";
import type { SheetFrameRequest, SheetLayout, SheetMethods, SheetTab } from "./sheet-protocol";
import { createWorkerClient, SupersededError } from "./worker-rpc";

type Client = ReturnType<typeof createWorkerClient<SheetMethods>>;

export class Spreadsheet {
  readonly element = h("div", { class: "sheet" });
  private readonly controller = new AbortController();
  private readonly engine: Client;
  private tabs: readonly SheetTab[] = [];
  private layout: SheetLayout | null = null;
  private picking: AbortController | null = null;
  private notice: HTMLElement | null = null;
  private grid: { element: HTMLElement; dispose: () => void } | null = null;
  private tablist: HTMLDivElement | null = null;

  constructor(source: ArrayBuffer, onDetails: (text: string) => void, private readonly onError: (cause: unknown) => void, loading: () => HTMLElement) {
    this.element.append(loading());
    const controller = this.controller;
    const worker = new Worker(new URL("./spreadsheet.worker.js", import.meta.url), { type: "module" });
    const engine = createWorkerClient<SheetMethods>(worker);
    this.engine = engine;
    const fail = (cause: unknown) => { if (!controller.signal.aborted) onError(cause); };
    worker.addEventListener("error", fail);
    worker.addEventListener("messageerror", fail);
    const bytes = source.slice(0);
    void engine.call("open", { bytes }, { transfer: [bytes], signal: controller.signal })
      .then((value) => {
        if (!value) throw new Error("This workbook has no visible sheets.");
        this.tabs = value.tabs;
        this.mount(value.layout, undefined);
        onDetails(`${value.tabs.length} sheets`);
      })
      .catch(fail);
  }

  private show(sheet: number, target?: CellAddr) {
    if (!this.layout) return;
    this.picking?.abort();
    if (sheet === this.layout.info.activeSheet) return;
    const controller = new AbortController();
    this.picking = controller;
    void this.engine.call("show", { sheet }, { signal: controller.signal })
      .then((layout) => { this.setNotice(""); this.mount(layout, target); })
      .catch(() => { if (!controller.signal.aborted) this.setNotice("This sheet could not be opened."); });
  }

  private setNotice(text: string) {
    this.notice?.remove();
    this.notice = text ? surfaceNotice(text) : null;
    if (this.notice) this.element.prepend(this.notice);
  }

  private mount(layout: SheetLayout, target: CellAddr | undefined) {
    this.layout = layout;
    this.grid?.dispose();
    this.grid = grid(this.engine, layout, target, (name, cell) => {
      const tab = this.tabs.find((tab) => tab.name.toLowerCase() === name.toLowerCase());
      if (tab) this.show(tab.index, cell);
    });
    if (!this.tablist) {
      this.tablist = h("div", { role: "tablist", "aria-label": "Sheets", class: "sheet-tabs" });
      this.tablist.addEventListener("keydown", (event) => {
        const active = this.layout?.info.activeSheet;
        const current = this.tabs.findIndex((tab) => tab.index === active);
        const next = event.key === "ArrowRight" ? (current + 1) % this.tabs.length : event.key === "ArrowLeft" ? (current + this.tabs.length - 1) % this.tabs.length
          : event.key === "Home" ? 0 : event.key === "End" ? this.tabs.length - 1 : -1;
        if (next < 0) return;
        event.preventDefault();
        const tab = this.tabs[next];
        if (tab) this.show(tab.index);
        this.tablist?.querySelectorAll<HTMLButtonElement>('[role="tab"]')[next]?.focus();
      });
      this.element.replaceChildren(this.grid.element, this.tablist);
    } else this.tablist.before(this.grid.element);
    if (this.notice) this.element.prepend(this.notice);
    this.tablist.replaceChildren(...this.tabs.map((tab) => {
      const selected = tab.index === layout.info.activeSheet;
      const button = h("button", { type: "button", role: "tab", class: "sheet-tab", "aria-selected": String(selected), tabindex: selected ? 0 : -1 }, tab.name);
      button.addEventListener("click", () => this.show(tab.index));
      return button;
    }));
  }

  dispose() {
    this.controller.abort();
    this.picking?.abort();
    this.grid?.dispose();
    this.engine.close();
  }
}

/** SpreadsheetGrid: the canvas, its scroll spacer and the screen reader's cells around the focus. */
function grid(engine: Client, layout: SheetLayout, initialCell: CellAddr | undefined, navigate: (sheet: string, cell: CellAddr) => void) {
  const canvas = h("canvas");
  const space = h("div", { "aria-hidden": "true", class: "sheet-spacer" });
  const reader = h("div", { class: "sr-only" });
  const root = h("div", {
    dir: "ltr", tabindex: 0, role: "grid", "aria-label": layout.info.sheetNames[layout.info.activeSheet], "aria-readonly": "true", "aria-multiselectable": "true",
    "aria-rowcount": lastCell.row + 1, "aria-colcount": lastCell.col + 1, "aria-activedescendant": "office-cell-0-0", class: "sheet-grid",
  }, space, h("div", { "aria-hidden": "true", class: "sheet-sticky" }, canvas), reader);
  const noticeBox = h("div", { role: "status", class: "sheet-notice", hidden: true });
  const element = h("div", { class: "sheet-area" }, root, noticeBox);
  const setNotice = (text: string) => { noticeBox.textContent = text; noticeBox.hidden = !text; };
  const context = canvas.getContext("2d");
  const controller = new AbortController();
  const setReader = (cells: { row: number; col: number; text: string }[]) => {
    reader.replaceChildren(...[...new Set(cells.map((cell) => cell.row))].map((row) =>
      h("div", { role: "row", "aria-rowindex": row + 1 }, ...cells.filter((cell) => cell.row === row).map((cell) =>
        h("span", { role: "gridcell", id: `office-cell-${cell.row}-${cell.col}`, "aria-colindex": cell.col + 1, "aria-label": `${columnName(cell.col)}${cell.row + 1}` }, cell.text)))));
  };
  if (!context) return { element, dispose: () => controller.abort() };
  const info = layout.info;
  const sheet = info.activeSheet;
  const frozen = { rows: info.frozenRows, cols: info.frozenCols };
  const headers = { width: 44, height: headerHeight };
  let selection: Selection = selectionAt(initialCell ?? { row: 0, col: 0 });
  let painted: Painted | undefined;
  let shifted: ReturnType<typeof shiftGrid> | undefined;
  let raf = 0;
  let requested = "";
  let sent = 0;
  let received = 0;
  let drag = false;
  let header: "row" | "column" | "all" | undefined;
  let pressed: { row: number; col: number } | undefined;
  let end: { x: number; y: number } | undefined;
  let left = info.initialScrollX;
  let top = info.initialScrollY;
  let reachX = 0;
  let reachY = 0;
  let readController: AbortController | undefined;
  const range = () => coverMerges(normalizeRange(selection), layout.merges);
  const at = (cell: { row: number; col: number }) => {
    const merge = layout.merges.find((item) => contains(item, cell));
    return selectionAt(merge ? { row: merge.top, col: merge.left } : cell);
  };
  const schedule = () => {
    if (raf || controller.signal.aborted) return;
    raf = requestAnimationFrame(() => { raf = 0; paint(); });
  };
  const resizeArea = () => {
    const width = Math.min((end?.x ?? Infinity) + (painted?.frame.frozen.width ?? 0), Math.max(info.contentWidth, layout.chartWidth, reachX, left + root.clientWidth) + root.clientWidth);
    const height = Math.min((end?.y ?? Infinity) + (painted?.frame.frozen.height ?? 0), Math.max(info.contentHeight, layout.chartHeight, reachY, top + root.clientHeight) + root.clientHeight);
    space.style.width = `${headers.width + width}px`;
    space.style.height = `${headers.height + height}px`;
  };
  const paint = () => {
    const ratio = window.devicePixelRatio || 1;
    const width = root.clientWidth;
    const height = root.clientHeight;
    const size = { width: width - headers.width, height: height - headers.height };
    if (size.width <= 0 || size.height <= 0) return;
    left = root.scrollLeft;
    top = root.scrollTop;
    resizeArea();
    const input: SheetFrameRequest = { sheet, x: left, y: top, ...size, ratio, offset: { x: 0, y: 0 }, wider: undefined };
    const key = JSON.stringify(input);
    if (key !== requested) {
      requested = key;
      sent += 1;
      const order = sent;
      void engine.call("frame", input, { key: "frame", signal: controller.signal })
        .then((reply) => {
          if (order < received || controller.signal.aborted) { reply?.bitmap.close(); return; }
          received = order;
          if (!reply) { setNotice("These cells could not be drawn. Try scrolling to another area."); return; }
          setNotice("");
          painted?.frame.bitmap.close();
          painted = { request: input, frame: reply };
          end = reply.end;
          schedule();
        })
        .catch((cause: unknown) => {
          if (!(cause instanceof SupersededError) && !controller.signal.aborted) setNotice("These cells could not be drawn.");
        });
    }
    if (canvas.width !== Math.round(width * ratio) || canvas.height !== Math.round(height * ratio)) {
      canvas.width = Math.round(width * ratio);
      canvas.height = Math.round(height * ratio);
      canvas.style.width = `${width}px`;
      canvas.style.height = `${height}px`;
    }
    if (!painted) {
      context.fillStyle = "#fff";
      context.fillRect(0, 0, canvas.width, canvas.height);
      return;
    }
    paintCells(context, painted, { ratio, headers, x: left, y: top, ...size });
    shifted = painted.frame.grid && shiftGrid(painted.frame.grid, frozen, painted.frame.frozen, { x: painted.frame.viewport.x - left, y: painted.frame.viewport.y - top });
    if (!shifted) return;
    const anchor = selection.anchor;
    paintSelection(context, { grid: shifted, ratio, headers, ...size, range: range(), active: coverMerges({ top: anchor.row, left: anchor.col, bottom: anchor.row, right: anchor.col }, layout.merges) });
    paintHeaders(context, { grid: shifted, ratio, headers, width, height, selected: range() });
  };
  const read = () => {
    const focus = selection.focus;
    root.setAttribute("aria-activedescendant", `office-cell-${focus.row}-${focus.col}`);
    readController?.abort();
    readController = new AbortController();
    const top = Math.max(0, focus.row - 5);
    const left = Math.max(0, focus.col - 3);
    const cells = { top, left, bottom: Math.min(lastCell.row, top + 10), right: Math.min(lastCell.col, left + 6) };
    void engine.call("cells", { sheet, range: cells }, { key: "reader", signal: readController.signal })
      .then((rows) => { if (!controller.signal.aborted) setReader(rows.flatMap((row, r) => row.map((text, c) => ({ row: top + r, col: left + c, text })))); })
      .catch(() => undefined);
  };
  const select = (next: Selection) => { selection = next; read(); schedule(); };
  const reveal = () => {
    const cell = selection.focus;
    void engine.call("position", { sheet, cell }, { key: "position", signal: controller.signal })
      .then((position) => {
        if (!position || controller.signal.aborted) return;
        const panes = painted?.frame.frozen ?? { width: 0, height: 0 };
        const place = (start: number, next: number, current: number, visible: number) => (start < current ? start : next > current + visible ? next - visible : current);
        const x = cell.col < info.frozenCols ? left : place(position.start.x, position.next.x, left, root.clientWidth - headers.width - panes.width);
        const y = cell.row < info.frozenRows ? top : place(position.start.y, position.next.y, top, root.clientHeight - headers.height - panes.height);
        reachX = Math.max(reachX, x + root.clientWidth);
        reachY = Math.max(reachY, y + root.clientHeight);
        resizeArea();
        root.scrollTo({ left: Math.max(0, x), top: Math.max(0, y) });
        schedule();
      })
      .catch(() => undefined);
  };
  const hit = (event: PointerEvent) => {
    if (!shifted) return;
    const rect = canvas.getBoundingClientRect();
    const x = event.clientX - rect.left - headers.width;
    const y = event.clientY - rect.top - headers.height;
    const frame = painted?.frame;
    if (frame && ((x >= frame.frozen.width && x < frame.frozen.width + frame.viewport.x - left) || (y >= frame.frozen.height && y < frame.frozen.height + frame.viewport.y - top))) return;
    const cell = cellAtPoint(shifted, Math.max(0, Math.min(x, root.clientWidth - headers.width - 1)), Math.max(0, Math.min(y, root.clientHeight - headers.height - 1)));
    return cell ? { cell, x, y } : undefined;
  };
  const pointerDown = (event: PointerEvent) => {
    if (event.button !== 0) return;
    const found = hit(event);
    if (!found) return;
    root.focus({ preventScroll: true });
    root.setPointerCapture(event.pointerId);
    drag = true;
    header = found.x < 0 ? (found.y < 0 ? "all" : "row") : found.y < 0 ? "column" : undefined;
    pressed = header ? undefined : found.cell;
    if (found.x < 0 && found.y < 0) select({ anchor: { row: lastCell.row, col: lastCell.col }, focus: { row: 0, col: 0 } });
    else if (found.x < 0) select({ anchor: { row: found.cell.row, col: lastCell.col }, focus: { row: found.cell.row, col: 0 } });
    else if (found.y < 0) select({ anchor: { row: lastCell.row, col: found.cell.col }, focus: { row: 0, col: found.cell.col } });
    else select(event.shiftKey ? extendTo(selection, found.cell) : at(found.cell));
  };
  const pointerMove = (event: PointerEvent) => {
    if (!drag) return;
    const found = hit(event);
    if (found && header !== "all") select(extendTo(selection, { row: header === "column" ? 0 : found.cell.row, col: header === "row" ? 0 : found.cell.col }));
  };
  const pointerUp = (event: PointerEvent) => {
    drag = false;
    const found = hit(event);
    if (!found || !pressed || found.cell.row !== pressed.row || found.cell.col !== pressed.col) return;
    const cell = at(found.cell).anchor;
    const list = painted?.frame;
    if (!list) return;
    const link = hyperlinkAtCell({ width: 0, height: 0, commands: [], hyperlinks: [...list.hyperlinks] }, cell.row, cell.col);
    if (!link) return;
    const url = safeExternalHyperlink(link);
    const safe = url && externalUrl(url);
    if (safe) {
      post({ type: "link", url: safe });
      return;
    }
    const internal = link.location && parseHyperlinkLocation(link.location, info.sheetNames[sheet] ?? "");
    if (!internal) return;
    if (internal.sheetName.toLowerCase() !== (info.sheetNames[sheet] ?? "").toLowerCase()) navigate(internal.sheetName, { row: internal.row, col: internal.col });
    else {
      select(at({ row: internal.row, col: internal.col }));
      reveal();
    }
  };
  const copy = () => {
    const used = layout.used;
    const selected = range();
    const clipped = { ...selected, bottom: Math.min(selected.bottom, Math.max(selected.top, used?.bottom ?? 0)), right: Math.min(selected.right, Math.max(selected.left, used?.right ?? 0)) };
    if ((clipped.bottom - clipped.top + 1) * (clipped.right - clipped.left + 1) > 200_000) {
      setNotice("Select fewer than 200,000 cells to copy.");
      return;
    }
    void engine.call("copy", { sheet, range: clipped }, { signal: controller.signal })
      .then((text) => {
        if (text === undefined) throw new Error("No cells");
        return navigator.clipboard.writeText(text);
      })
      .catch(() => { if (!controller.signal.aborted) setNotice("The selected cells could not be copied."); });
  };
  const key = (event: KeyboardEvent) => {
    const command = event.ctrlKey || event.metaKey;
    if (command && event.key.toLowerCase() === "c") {
      event.preventDefault();
      copy();
      return;
    }
    if (event.key === "Tab" || event.altKey || event.isComposing) return;
    const action = selectionKeyReducer(selection, event, {
      rows: Math.max((layout.used?.bottom ?? 0) + 1, selection.focus.row + 1),
      cols: Math.max((layout.used?.right ?? 0) + 1, selection.focus.col + 1),
      rowsPerPage: Math.max(1, Math.floor((root.clientHeight - headerHeight) / 20)),
    });
    if (action.type !== "move") return;
    event.preventDefault();
    select(skipHidden(selection.focus, settleMerges(selection, action.selection, layout.merges), layout.hidden));
    reveal();
  };
  const cancel = () => { drag = false; };
  root.addEventListener("scroll", schedule);
  root.addEventListener("keydown", key);
  root.addEventListener("pointerdown", pointerDown);
  root.addEventListener("pointermove", pointerMove);
  root.addEventListener("pointerup", pointerUp);
  root.addEventListener("pointercancel", cancel);
  const unobserve = observeResize(root, schedule);
  window.addEventListener("resize", schedule);
  // The grid is laid out once it is in the page; the reference's effect runs after its commit.
  queueMicrotask(() => {
    resizeArea();
    root.scrollTo({ left, top });
    if (initialCell) {
      selection = at(initialCell);
      reveal();
    }
    read();
    schedule();
  });
  return {
    element,
    dispose() {
      controller.abort();
      readController?.abort();
      cancelAnimationFrame(raf);
      unobserve();
      window.removeEventListener("resize", schedule);
      painted?.frame.bitmap.close();
      element.remove();
    },
  };
}
