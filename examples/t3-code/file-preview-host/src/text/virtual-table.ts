// The virtual table (MIT reference, see LICENSE-T3: apps/web/src/components/files/VirtualTablePreview.tsx; T3 Code
// 7c3b3b3605ef): rows a page of 32 at a time from a worker, row numbers, sticky headers measured from the first rows,
// virtual columns past the viewport, and a scroll track scaled for multi-million-line logs. A React render is
// `render`, and its layout effects run after it in their order, each with that render's values; one that sets state
// renders again before the frame paints, as React's do.
import { h, observeResize, tooltip } from "../chrome";
import { PREVIEW_PAGE_ROWS } from "./large-text-protocol";

export type VirtualTableProps = {
  readonly name: string;
  readonly count: number;
  readonly columns: number;
  readonly getRows: (page: number) => readonly (readonly string[])[] | undefined;
  readonly loadPage?: (page: number) => void;
  readonly header?: readonly string[] | undefined;
  readonly source?: boolean;
  readonly wrap?: boolean;
  readonly revealLine?: number | null;
  readonly revealRequestId?: number;
};

function columnLabel(index: number): string {
  let label = "";
  for (let value = index + 1; value > 0; value = Math.floor((value - 1) / 26)) label = String.fromCharCode(65 + ((value - 1) % 26)) + label;
  return label;
}

const ROW = 29;

export class VirtualTable {
  readonly element = h("div", { class: "vt" });
  private left = 0;
  private size = { width: 0, height: 600 };
  private widthSeen = 0;
  private measured: number[] | null = null;
  private measure: HTMLTableElement | null = null;
  private table: HTMLDivElement | null = null;
  private head: HTMLDivElement | null = null;
  private headKey = "";
  private rows: ScaledRows | null = null;
  private readonly unobserve: () => void;

  constructor(private props: VirtualTableProps) {
    this.element.addEventListener("scroll", () => {
      this.element.style.setProperty("--preview-scroll-left", `${this.element.scrollLeft}px`);
      this.left = this.element.scrollLeft;
      this.render();
    });
    const update = () => {
      const width = this.element.clientWidth, height = this.element.clientHeight;
      if (this.size.width !== width || this.size.height !== height) this.size = { width, height };
      if (this.widthSeen !== width) { this.widthSeen = width; this.measured = null; }
      this.render();
    };
    this.unobserve = observeResize(this.element, update);
    queueMicrotask(update);
  }

  update(props: VirtualTableProps) {
    this.props = props;
    this.render();
  }

  render() {
    const props = this.props, { width } = this.size;
    // The browser's own table lays the first rows out once, and its header widths size the columns.
    const measuring = !!props.header && props.columns <= 30 && !this.measured && width > 0;
    if (measuring && !this.measure) {
      this.measure = h("table", { class: "vt-measure", "aria-hidden": "true" },
        h("thead", {}, h("tr", {}, ...props.header!.map((cell) => h("th", {}, cell)))),
        h("tbody", {}, ...(props.getRows(0)?.slice(1, 20) ?? []).map((row) => h("tr", {}, ...row.map((cell) => h("td", {}, cell))))));
      this.measure.style.width = `${Math.max(width - 56, props.columns * 128)}px`;
      this.element.prepend(this.measure);
    } else if (!measuring && this.measure) { this.measure.remove(); this.measure = null; }
    if (this.measure && !this.measured) {
      const widths = [...this.measure.querySelectorAll("thead th")].map((cell) => cell.getBoundingClientRect().width);
      if (widths.length) { this.measured = widths; this.measure.remove(); this.measure = null; }
    }
    const columnWidth = props.source ? 0 : Math.max(128, Math.min(320, (width - 56) / Math.min(props.columns, 6)));
    const rowHeader = 56;
    const sizes = Array.from({ length: props.columns }, (_, column) => this.measured?.[column] ?? columnWidth);
    const offsets = [0];
    for (const size of sizes) offsets.push(offsets[offsets.length - 1]! + size);
    const left = this.left;
    const firstColumn = props.source ? 0 : Math.max(0, sizes.findIndex((_, column) => offsets[column + 1]! > left - rowHeader) - 1);
    const lastColumn = props.source ? 1 : Math.min(props.columns, offsets.findIndex((offset) => offset >= left + width - rowHeader) + 1 || props.columns);
    const columns = Array.from({ length: Math.max(0, lastColumn - firstColumn) }, (_, i) => firstColumn + i);
    const tableWidth = props.source ? "100%" : `${Math.max(width, offsets[props.columns]! + rowHeader)}px`;
    const shown = width > 0 && (!props.header || props.columns > 30 || !!this.measured);
    if (!shown) {
      this.table?.remove(); this.table = null; this.head = null; this.headKey = ""; this.rows?.dispose(); this.rows = null;
      return;
    }
    if (!this.table) {
      this.table = h("div", { class: "vt-table" });
      this.element.append(this.table);
    }
    const table = this.table;
    table.setAttribute("role", props.source ? "region" : "table");
    table.setAttribute("aria-label", props.name);
    table.setAttribute("aria-rowcount", String(props.count));
    if (props.source) table.removeAttribute("aria-colcount"); else table.setAttribute("aria-colcount", String(props.columns));
    table.style.width = tableWidth;
    if (!props.source) {
      const key = JSON.stringify([firstColumn, columns.map((column) => sizes[column]), offsets[firstColumn], props.header?.slice(firstColumn, lastColumn)]);
      if (key !== this.headKey) {
        this.headKey = key;
        const head = h("div", { class: "vt-head", role: "row" }, h("div", { class: "vt-corner" }));
        const spacer = h("div");
        spacer.style.cssText = `width:${offsets[firstColumn]}px;flex-shrink:0`;
        head.append(spacer);
        for (const column of columns) {
          const label = props.header?.[column] ?? columnLabel(column);
          const cell = h("div", { class: "vt-colhead", role: "columnheader", "aria-colindex": column + 1 }, label);
          cell.style.width = `${sizes[column]}px`;
          tooltip(cell, () => label);
          head.append(cell);
        }
        if (this.head) this.head.replaceWith(head); else table.prepend(head);
        this.head = head;
      }
    } else if (this.head) { this.head.remove(); this.head = null; this.headKey = ""; }
    const scaled = { ...props, columnsVisible: columns, sizes, rowHeader, firstColumn, viewportHeight: this.size.height - (props.source ? 0 : ROW) };
    if (!this.rows) {
      this.rows = new ScaledRows(scaled);
      table.append(this.rows.element);
    } else this.rows.update(scaled);
  }

  dispose() {
    this.unobserve();
    this.rows?.dispose();
  }
}

type RowsProps = VirtualTableProps & {
  readonly columnsVisible: readonly number[];
  readonly sizes: readonly number[];
  readonly rowHeader: number;
  readonly firstColumn: number;
  readonly viewportHeight: number;
};

/** RowPage: one page's rows, made again only when its rows, its limit or the visible columns change. */
type RowPage = { element: HTMLDivElement; key: string };

function rowPage(props: RowsProps, page: number, limit: number): RowPage {
  const rows = props.getRows(page);
  const start = page * PREVIEW_PAGE_ROWS;
  const count = Math.min(PREVIEW_PAGE_ROWS, props.count - start, limit);
  const reveal = props.revealLine != null ? Math.min(props.count, Math.max(1, props.revealLine)) : null;
  const element = h("div", { role: "rowgroup" });
  const firstWidth = props.sizes.slice(0, props.firstColumn).reduce((a, b) => a + b, 0);
  for (let local = 0; local < count; local++) {
    const row = rows?.[local];
    const index = start + local;
    if (props.header && index === 0) continue;
    const revealed = reveal === index + 1;
    const line = h("div", {
      "data-preview-row": index,
      role: props.source ? undefined : "row",
      "aria-rowindex": props.source ? undefined : index + 1,
      class: props.source ? `vt-row source${revealed ? " reveal" : ""}` : `vt-row table${(index - (props.header ? 1 : 0)) % 2 ? " odd" : ""}`,
      "data-preview-reveal": revealed ? "true" : undefined,
    });
    const head = h("div", { role: props.source ? undefined : "rowheader", class: "vt-rowhead" }, h("span", {}, String(index + (props.header ? 0 : 1))));
    head.style.width = `${props.rowHeader}px`;
    line.append(head);
    if (!props.source) {
      const spacer = h("div");
      spacer.style.cssText = `width:${firstWidth}px;flex-shrink:0`;
      line.append(spacer);
    }
    for (const column of props.columnsVisible) {
      const cell = h("div", {
        role: props.source ? undefined : "cell",
        "aria-colindex": props.source ? undefined : column + 1,
        class: props.source ? `vt-line${props.wrap ? " wrap" : ""}` : "vt-cell",
      }, row?.[column] ?? "");
      if (!props.source) cell.style.width = `${props.sizes[column]}px`;
      line.append(cell);
    }
    element.append(line);
  }
  return { element, key: "" };
}

/** ScaledRows: the rows' scroll track, scaled past 8 million pixels, with native scrolling over the final rows. */
class ScaledRows {
  readonly element = h("div", { class: "vt-rows", tabindex: 0 });
  private readonly track = h("div", { class: "vt-track" });
  private readonly content = h("div", { class: "vt-content" });
  private layout = { offset: 0, height: 0, rowHeight: ROW };
  private top = 0;
  private handledReveal: string | null = null;
  private pendingReveal: { row: number; attempts: number } | null = null;
  private pendingEnd = false;
  private scrollModel: { scale: number; maxFirst: number; heights: number[] } | null = null;
  private pages = new Map<number, RowPage>();
  private revealDeps = "";
  private layoutDeps = "";
  private stopLayout: (() => void) | null = null;
  private loaded = new Map<number, unknown>();
  private derived = { scale: 1, nativeTail: false, first: 0, headerRows: 0, atStart: true, maxFirst: 0 };
  private rendering = false;

  constructor(private props: RowsProps) {
    this.track.append(this.content);
    this.element.append(this.track);
    this.element.addEventListener("scroll", () => { this.top = this.element.scrollTop; this.render(); });
    this.element.addEventListener("keydown", (event) => this.keyDown(event));
    // A non-passive wheel listener, so the virtual track can take the browser's scroll.
    this.element.addEventListener("wheel", (event) => this.wheel(event), { passive: false });
    this.render();
  }

  update(props: RowsProps) {
    this.props = props;
    this.render();
  }

  private rowHeight() {
    return this.content.querySelector<HTMLElement>(`[data-preview-row="${this.derived.first}"]`)?.offsetHeight || ROW;
  }

  private wheel(event: WheelEvent) {
    const node = this.element;
    if (event.ctrlKey || Math.abs(event.deltaX) > Math.abs(event.deltaY)) return;
    const delta = event.deltaY * (event.deltaMode === 1 ? ROW : event.deltaMode === 2 ? node.clientHeight : 1);
    if (!delta) return;
    event.preventDefault();
    this.pendingEnd = false;
    node.scrollTop += delta * (this.derived.nativeTail ? 1 : (this.derived.scale * ROW) / this.rowHeight());
  }

  private keyDown(event: KeyboardEvent) {
    const node = this.element;
    const delta = event.key === "ArrowDown" ? ROW : event.key === "ArrowUp" ? -ROW : event.key === "PageDown" ? node.clientHeight : event.key === "PageUp" ? -node.clientHeight : null;
    if (delta !== null) {
      this.pendingEnd = false;
      event.preventDefault();
      node.scrollTop += delta * (this.derived.nativeTail ? 1 : (this.derived.scale * ROW) / this.rowHeight());
    } else if (event.key === "Home" || event.key === "End") {
      event.preventDefault();
      this.pendingEnd = event.key === "End";
      this.pendingReveal = null;
      node.scrollTop = event.key === "Home" ? 0 : node.scrollHeight;
      this.top = node.scrollTop;
      this.render();
    }
  }

  private render() {
    if (this.rendering) return;
    this.rendering = true;
    try {
      for (let pass = 0; pass < 12 && this.pass(); pass++);
    } finally {
      this.rendering = false;
    }
  }

  /** One render and its layout effects; true when an effect set state, so it renders again. */
  private pass(): boolean {
    const props = this.props;
    const position = { top: this.top, height: props.viewportHeight };
    const scrollHeight = Math.min(8_000_000, props.count * ROW);
    // Chromium caps a scroll surface at 33 million pixels. Scale the track for multi-million-line logs while retaining
    // real row heights in the viewport.
    const headerRows = props.header ? 1 : 0;
    const maxFirst = Math.max(0, props.count - headerRows - Math.floor(position.height / ROW));
    const scale = Math.max(1, scrollHeight - position.height) / Math.max(1, maxFirst * ROW);
    const first = Math.min(maxFirst, Math.floor(position.top / scale / ROW)) + headerRows;
    // The final mounted rows keep their measured height and scroll natively.
    const nativeTail = first - headerRows === maxFirst;
    const atStart = position.top === 0;
    this.derived = { scale, nativeTail, first, headerRows, atStart, maxFirst };
    const layout = this.layout;
    const page = Math.floor(first / PREVIEW_PAGE_ROWS);
    const visibleEnd = (first % PREVIEW_PAGE_ROWS) + Math.ceil(position.height / ROW) + 1;
    this.track.style.height = `${scrollHeight}px`;
    this.content.style.top = `${maxFirst === 0 ? 0 : nativeTail ? maxFirst * scale * ROW - layout.offset : position.top + (atStart ? 0 : -layout.offset - ((position.top / scale / ROW) % 1) * layout.rowHeight)}px`;
    // The mounted pages, each kept while its rows, limit and columns stay.
    const length = Math.max(0, Math.min(Math.ceil(visibleEnd / PREVIEW_PAGE_ROWS), Math.ceil(props.count / PREVIEW_PAGE_ROWS) - page));
    const wanted = new Map<number, number>();
    for (let offset = 0; offset < length; offset++) wanted.set(page + offset, visibleEnd - offset * PREVIEW_PAGE_ROWS);
    for (const [index, mounted] of this.pages) if (!wanted.has(index)) { mounted.element.remove(); this.pages.delete(index); }
    const columnsKey = JSON.stringify([props.columnsVisible, props.sizes, props.firstColumn, props.wrap, props.source, props.revealLine, props.count, !!props.header]);
    let previous: Element | null = null;
    for (const [index, limit] of wanted) {
      const rows = props.getRows(index);
      const key = `${columnsKey}|${limit}|${this.version(index, rows)}`;
      let mounted = this.pages.get(index);
      if (!mounted || mounted.key !== key) {
        const made = rowPage(props, index, limit);
        made.key = key;
        if (mounted) mounted.element.replaceWith(made.element);
        mounted = made;
        this.pages.set(index, mounted);
      }
      if (mounted.element.previousElementSibling !== previous || mounted.element.parentElement !== this.content) {
        if (previous) previous.after(mounted.element); else this.content.prepend(mounted.element);
      }
      previous = mounted.element;
      // A page stays mounted while the worker fills it, preserving the scroll position.
      const expected = Math.min(PREVIEW_PAGE_ROWS, props.count - index * PREVIEW_PAGE_ROWS);
      if (!rows || rows.length < expected) props.loadPage?.(index);
    }
    let again = false;
    const node = this.element, content = this.content;
    // Measure native tail rows and preserve the DOM scroll position when the virtual track scale changes.
    {
      const previousModel = this.scrollModel ?? { scale, maxFirst, heights: [] };
      const heights = nativeTail
        ? Array.from(content.querySelectorAll<HTMLElement>("[data-preview-row]")).filter((row) => Number(row.dataset.previewRow) >= first).map((row) => row.offsetHeight)
        : [];
      this.scrollModel = { scale, maxFirst, heights };
      if (previousModel.scale !== scale || previousModel.maxFirst !== maxFirst) {
        const previousEnd = previousModel.maxFirst * ROW * previousModel.scale;
        let logicalTop = Math.min(previousModel.maxFirst * ROW, node.scrollTop / previousModel.scale);
        let remaining = Math.max(0, node.scrollTop - previousEnd);
        // Native-tail pixels cross rows of different heights before returning to the scaled track.
        for (const height of previousModel.heights) {
          logicalTop += Math.min(1, remaining / height) * ROW;
          remaining = Math.max(0, remaining - height);
          if (!remaining) break;
        }
        let nativeOffset = 0;
        remaining = Math.max(0, logicalTop / ROW - maxFirst);
        for (const height of heights) {
          nativeOffset += Math.min(1, remaining) * height;
          remaining = Math.max(0, remaining - 1);
          if (!remaining) break;
        }
        node.scrollTop = Math.min(maxFirst * ROW, logicalTop) * scale + nativeOffset;
        if (node.scrollTop !== this.top) { this.top = node.scrollTop; again = true; }
      }
    }
    // Scroll the DOM viewport to a requested line after its virtual track has committed.
    {
      const deps = JSON.stringify([props.revealLine, props.revealRequestId, props.count, headerRows, scale]);
      if (deps !== this.revealDeps) {
        this.revealDeps = deps;
        if (props.revealLine == null) {
          this.handledReveal = null;
          this.pendingReveal = null;
        } else {
          const key = `${props.revealRequestId}:${props.revealLine}`;
          if (props.count && this.handledReveal !== key) {
            const row = Math.min(props.count, Math.max(1, props.revealLine)) - 1;
            node.scrollTop = Math.max(0, (row - headerRows) * ROW) * scale;
            if (node.scrollTop !== this.top) { this.top = node.scrollTop; again = true; }
            this.pendingEnd = false;
            this.pendingReveal = { row, attempts: 0 };
            this.handledReveal = key;
          }
        }
      }
    }
    // Measure mounted row offsets and observe content resizes to position the virtual rows against the scroll track.
    {
      const deps = `${first}|${atStart}`;
      if (deps !== this.layoutDeps) {
        this.layoutDeps = deps;
        this.stopLayout?.();
        this.stopLayout = null;
        if (!atStart) {
          const measure = () => {
            const row = content.querySelector<HTMLElement>(`[data-preview-row="${first}"]`);
            const offset = row ? row.getBoundingClientRect().top - content.getBoundingClientRect().top : 0;
            const height = content.clientHeight;
            const rowHeight = row?.offsetHeight || ROW;
            const old = this.layout;
            if (old.offset === offset && old.height === height && old.rowHeight === rowHeight) return false;
            this.layout = { offset, height, rowHeight };
            return true;
          };
          if (measure()) again = true;
          this.stopLayout = observeResize(content, () => { if (measure()) this.render(); });
        }
      }
    }
    const settled = () => {
      const firstRow = content.querySelector<HTMLElement>(`[data-preview-row="${first}"]`);
      return atStart || (this.layout.height === content.clientHeight && this.layout.offset === (firstRow ? firstRow.getBoundingClientRect().top - content.getBoundingClientRect().top : 0));
    };
    // Correct the DOM scroll position once a requested worker row has loaded and its actual bounds are measurable.
    {
      const pending = this.pendingReveal;
      const row = pending && content.querySelector<HTMLElement>(`[data-preview-row="${pending.row}"]`);
      if (pending && row && props.getRows(Math.floor(pending.row / PREVIEW_PAGE_ROWS))?.[pending.row % PREVIEW_PAGE_ROWS] && settled()) {
        const bounds = row.getBoundingClientRect();
        const viewport = node.getBoundingClientRect();
        const delta = bounds.top < viewport.top
          ? bounds.top - viewport.top
          : Math.min(bounds.bottom, bounds.top + viewport.height) > viewport.bottom ? Math.min(bounds.bottom, bounds.top + viewport.height) - viewport.bottom : 0;
        if (Math.abs(delta) < 1 || pending.attempts++ >= 8) this.pendingReveal = null;
        else {
          node.scrollTop += delta * (nativeTail ? 1 : (scale * ROW) / Math.max(ROW, bounds.height));
          if (node.scrollTop !== this.top) { this.top = node.scrollTop; again = true; }
        }
      }
    }
    // Complete End-key scrolling after the final worker row has loaded and its DOM layout has settled.
    if (this.pendingEnd && nativeTail && props.count) {
      const last = props.count - 1;
      const row = content.querySelector<HTMLElement>(`[data-preview-row="${last}"]`);
      if (props.getRows(Math.floor(last / PREVIEW_PAGE_ROWS))?.[last % PREVIEW_PAGE_ROWS] && row && settled()) {
        node.scrollTop = node.scrollHeight;
        if (node.scrollTop !== this.top) { this.top = node.scrollTop; again = true; }
        if (Math.abs(row.getBoundingClientRect().bottom - node.getBoundingClientRect().bottom) < 1) this.pendingEnd = false;
      }
    }
    return again;
  }

  /** A page's rows' identity: its array, numbered so a replaced page renders again. */
  private version(index: number, rows: unknown): number {
    const known = this.loaded.get(index);
    if (known !== rows) {
      this.loaded.set(index, rows);
      this.counter++;
      this.versions.set(index, this.counter);
    }
    return this.versions.get(index) ?? 0;
  }
  private counter = 0;
  private versions = new Map<number, number>();

  dispose() {
    this.stopLayout?.();
    this.element.remove();
  }
}
