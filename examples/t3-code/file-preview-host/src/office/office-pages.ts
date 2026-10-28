// Word pages and slides as canvases (MIT reference, see LICENSE-T3: apps/web/src/components/files/office/OfficePages.tsx;
// T3 Code 7c3b3b3605ef; adapted there from opencode, MIT, see LICENSE.opencode): pages mount fifty at a time, each
// paints its bitmap while it is within a viewport of the scroll area, at the width the area settled on, and gives
// its canvas memory back once it leaves.
import { h, observeResize } from "../chrome";

export type PageSize = { readonly width: number; readonly height: number; readonly background?: string | undefined };
export type PaintPage = (index: number, canvas: HTMLCanvasElement, scale: number, signal: AbortSignal) => Promise<void>;

export type OfficePagesProps = {
  readonly pages: readonly PageSize[];
  readonly paint: PaintPage;
  readonly label: string;
  readonly overlay?: (index: number) => Node[];
  readonly footer?: (index: number) => Node | null;
  readonly onPageChange?: (index: number) => void;
  readonly onError: (error: unknown) => void;
};

/** OfficePage: one page's box, canvas and overlays. */
class OfficePage {
  readonly element = h("div", { class: "office-page-wrap" });
  readonly row: HTMLDivElement;
  private readonly canvas = h("canvas", { "aria-hidden": "true" });
  private readonly overlays = h("div", { style: "display:contents" });
  private near = false;
  private ratio = window.devicePixelRatio || 1;
  private paintKey = "";
  private controller: AbortController | null = null;
  private readonly observer: IntersectionObserver;
  private readonly query: MediaQueryList;
  private footerNode: Node | null = null;
  constructor(private props: OfficePagesProps, readonly index: number, root: HTMLElement, private scale: number, private rasterScale: number) {
    this.row = h("div", { role: "group", "aria-label": `${props.label} ${index + 1}`, class: "office-page" }, this.canvas, this.overlays);
    this.element.append(this.row);
    this.observer = new IntersectionObserver(([entry]) => { this.near = entry?.isIntersecting ?? false; this.refresh(); }, { root, rootMargin: "100% 0px" });
    this.observer.observe(this.row);
    this.query = matchMedia(`(resolution: ${this.ratio}dppx)`);
    this.query.addEventListener("change", this.densityChanged);
    this.refresh();
  }
  private readonly densityChanged = () => {
    this.query.removeEventListener("change", this.densityChanged);
    this.ratio = window.devicePixelRatio || 1;
    (this as { query: MediaQueryList }).query = matchMedia(`(resolution: ${this.ratio}dppx)`);
    this.query.addEventListener("change", this.densityChanged);
    this.refresh();
  };
  update(props: OfficePagesProps, scale: number, rasterScale: number) {
    this.props = props;
    this.scale = scale;
    this.rasterScale = rasterScale;
    this.refresh();
  }
  /** Re-runs what an overlay or a footer shows, after the viewer's state changed. */
  redraw() {
    this.paintOverlays();
    const footer = this.props.footer?.(this.index) ?? null;
    if (footer !== this.footerNode) {
      this.footerNode?.parentNode?.removeChild(this.footerNode);
      this.footerNode = footer;
      if (footer) this.element.append(footer);
    }
  }
  private paintOverlays() {
    this.overlays.replaceChildren(...(this.near ? (this.props.overlay?.(this.index) ?? []) : []));
  }
  private refresh() {
    const page = this.props.pages[this.index];
    if (!page) return;
    this.element.style.width = `${page.width * this.scale}px`;
    Object.assign(this.row.style, { width: `${page.width * this.scale}px`, height: `${page.height * this.scale}px`, background: page.background ?? "#fff" });
    this.redraw();
    const key = this.near && this.ratio > 0 && this.rasterScale > 0 ? `${this.rasterScale}|${this.ratio}|${page.width}x${page.height}` : "";
    if (key === this.paintKey) return;
    this.paintKey = key;
    this.controller?.abort();
    this.controller = null;
    if (!key) {
      // Release the canvas backing store when its page moves away from the viewport.
      if (!this.near) { this.canvas.width = 0; this.canvas.height = 0; }
      return;
    }
    const controller = new AbortController();
    this.controller = controller;
    void this.props.paint(this.index, this.canvas, this.rasterScale, controller.signal).catch((cause: unknown) => {
      if (!controller.signal.aborted) this.props.onError(cause);
    });
  }
  dispose() {
    this.controller?.abort();
    this.observer.disconnect();
    this.query.removeEventListener("change", this.densityChanged);
  }
}

export class OfficePages {
  readonly element = h("div", { class: "office-pages", "data-office-pages": "" });
  private readonly column = h("div", { class: "office-pages-column" });
  private width = 0;
  private settledWidth = 0;
  private mounted = 6;
  private readonly rows: OfficePage[] = [];
  private readonly unobserve: () => void;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private batch: ReturnType<typeof setTimeout> | undefined;

  constructor(private props: OfficePagesProps) {
    this.element.append(this.column);
    this.unobserve = observeResize(this.element, ([entry]) => {
      if (!entry) return;
      const next = entry.contentRect.width;
      this.width = next;
      this.render();
      clearTimeout(this.timer);
      this.timer = setTimeout(() => { this.settledWidth = next; this.render(); }, 150);
    });
    this.element.addEventListener("scroll", () => {
      const root = this.element;
      if (!this.props.onPageChange) return;
      if (root.scrollHeight > root.clientHeight && root.scrollTop + root.clientHeight >= root.scrollHeight - 1) {
        this.props.onPageChange(Math.min(this.mounted, this.props.pages.length) - 1);
        return;
      }
      for (const row of this.rows) {
        if (row.row.offsetTop + row.row.offsetHeight > root.scrollTop + 24) {
          this.props.onPageChange(row.index);
          break;
        }
      }
    });
    this.render();
  }

  update(props: OfficePagesProps) {
    this.props = props;
    this.render();
  }

  /** The overlays and footers, drawn again after their viewer's state changed. */
  redraw() {
    for (const row of this.rows) row.redraw();
  }

  /** scrollRef: to page `index`, `y` points down it. */
  scrollTo(index: number, y = 0) {
    const row = this.rows[index], page = this.props.pages[index];
    if (row && page) this.element.scrollTo({ top: row.row.offsetTop + y * Math.min(1, Math.max(0, this.width - 48) / page.width) - 24 });
  }

  private render() {
    const { pages } = this.props;
    // Mount the next batch after a yield, so a large document does not block the page.
    if (this.mounted < pages.length && this.batch === undefined) {
      this.batch = setTimeout(() => { this.batch = undefined; this.mounted = Math.min(pages.length, this.mounted + 50); this.render(); }, 0);
    }
    const count = Math.min(this.mounted, pages.length);
    while (this.rows.length > count) this.rows.pop()!.dispose();
    for (let index = 0; index < count; index++) {
      const page = pages[index]!;
      const scale = Math.min(1, Math.max(0, this.width - 48) / page.width);
      const rasterScale = Math.min(1, Math.max(0, (this.settledWidth || this.width) - 48) / page.width);
      const row = this.rows[index];
      if (row) row.update(this.props, scale, rasterScale);
      else {
        const made = new OfficePage(this.props, index, this.element, scale, rasterScale);
        this.rows.push(made);
        this.column.append(made.element);
      }
    }
  }

  dispose() {
    clearTimeout(this.timer);
    clearTimeout(this.batch);
    this.unobserve();
    for (const row of this.rows) row.dispose();
  }
}
