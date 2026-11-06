// A server Browser tab's PDF as Chromium's PDF viewer shows it (BSD, The Chromium Authors:
// chrome/browser/resources/pdf, as Electron 44.10 ships it), where the reference embeds the URL (ServerBrowserSurface
// `<embed type="application/pdf">`). Its toolbar (sidebar, title, page field, zoom, fit, rotate, download, more
// actions), its sidebar (thumbnails, and the document outline when there is one), two page view, annotations shown or
// hidden, Present and Document properties are drawn here over PDF.js, which reads the URL's bytes: an answer that is no
// PDF draws nothing and runs nothing. Print prints the PDF's own pages through the platform's print panel. What
// differs: drawing on the PDF (Chromium's Ink annotation tools), and Present, which fills the viewer rather than the
// screen.
import { AnnotationMode, TextLayer, type PDFDocumentProxy, type PDFPageProxy } from "pdfjs-dist";
import { h, observeResize, post } from "../chrome";
import { readBoundedDocument } from "../shared/read-bounded";
import { openPdf } from "./pdf-document";
import {
  PAGE_SHADOW, PX_PER_POINT, clampZoom, fileSizeText, fittingZoom, initialZoom, mostVisiblePage, pageSizeText, pagesNear,
  pdfLayout, stepZoom, type Layout, type Size,
} from "./browser-pdf-layout";

const MAX_PAGES = 2000;
const MAX_BYTES = 100 * 1024 * 1024;
const BOOKMARK_INDENT = 20;
const SIDENAV_KEY = "sidenavCollapsed";

/** The viewer's Material Symbols (pdf/elements/icons.html; cr's file-download and chevron-right). */
const ICONS: Record<string, string> = {
  menu: "M144-264v-72h672v72H144Zm0-180v-72h672v72H144Zm0-180v-72h672v72H144Z",
  remove: "M232-444v-72h496v72H232Z",
  add: "M444-444H240v-72h204v-204h72v204h204v72H516v204h-72v-204Z",
  "fit-to-width": "M168-192q-29.7 0-50.85-21.16Q96-234.32 96-264.04v-432.24Q96-726 117.15-747T168-768h624q29.7 0 50.85 21.16Q864-725.68 864-695.96v432.24Q864-234 842.85-213T792-192H168Zm624-504H168v432h624v-432Zm-624 0v432-432Zm192 336v-240L240-480l120 120Zm360-120L600-600v240l120-120Z",
  "fit-to-height": "M263.72-96Q234-96 213-117.15T192-168v-624q0-29.7 21.16-50.85Q234.32-864 264.04-864h432.24Q726-864 747-842.85T768-792v624q0 29.7-21.16 50.85Q725.68-96 695.96-96H263.72ZM696-168v-624H264v624h432Zm0-624H264h432ZM360-600h240L480-720 360-600Zm120 360 120-120H360l120 120Z",
  "rotate-left": "M528-84q-52 0-98.5-14.5T343-140l52-52q29 17 63 26.5t70 9.5q109 0 186.5-77.5T792-420q0-109-77.5-187.5T522-684l57 57-51 51-144-144 144-144 51 51-57 57q144-3 243 96.5T864-420q0 70-26.5 131t-72 106.5Q720-137 659-110.5T528-84Zm-240-96L48-420l240-240 240 240-240 240Zm0-102 138-138-138-138-138 138 138 138Zm0-138Z",
  more: "M479.79-192Q450-192 429-213.21t-21-51Q408-294 429.21-315t51-21Q510-336 531-314.79t21 51Q552-234 530.79-213t-51 21Zm0-216Q450-408 429-429.21t-21-51Q408-510 429.21-531t51-21Q510-552 531-530.79t21 51Q552-450 530.79-429t-51 21Zm0-216Q450-624 429-645.21t-21-51Q408-726 429.21-747t51-21Q510-768 531-746.79t21 51Q552-666 530.79-645t-51 21Z",
  check: "M389-267 195-460l51-52 143 143 325-324 51 51-376 375Z",
  thumbnails: "M216-144q-29.7 0-50.85-21.5Q144-187 144-216v-528q0-29 21.15-50.5T216-816h528q29.7 0 50.85 21.5Q816-773 816-744v528q0 29-21.15 50.5T744-144H216Zm0-72h528v-528H216v528Zm48-72h432L552-480 444-336l-72-96-108 144Zm-48 72v-528 528Z",
  "doc-outline": "M323.79-288q15.21 0 25.71-10.29t10.5-25.5q0-15.21-10.29-25.71t-25.5-10.5q-15.21 0-25.71 10.29t-10.5 25.5q0 15.21 10.29 25.71t25.5 10.5Zm0-156q15.21 0 25.71-10.29t10.5-25.5q0-15.21-10.29-25.71t-25.5-10.5q-15.21 0-25.71 10.29t-10.5 25.5q0 15.21 10.29 25.71t25.5 10.5Zm0-156q15.21 0 25.71-10.29t10.5-25.5q0-15.21-10.29-25.71t-25.5-10.5q-15.21 0-25.71 10.29t-10.5 25.5q0 15.21 10.29 25.71t25.5 10.5ZM432-288h240v-72H432v72Zm0-156h240v-72H432v72Zm0-156h240v-72H432v72ZM216-144q-29.7 0-50.85-21.15Q144-186.3 144-216v-528q0-29.7 21.15-50.85Q186.3-816 216-816h528q29.7 0 50.85 21.15Q816-773.7 816-744v528q0 29.7-21.15 50.85Q773.7-144 744-144H216Zm0-72h528v-528H216v528Zm0-528v528-528Z",
  print: "M648-624v-120H312v120h-72v-192h480v192h-72Zm-480 72h625-625Zm539.79 96q15.21 0 25.71-10.29t10.5-25.5q0-15.21-10.29-25.71t-25.5-10.5q-15.21 0-25.71 10.29t-10.5 25.5q0 15.21 10.29 25.71t25.5 10.5ZM648-216v-144H312v144h336Zm72 72H240v-144H96v-240q0-40 28-68t68-28h576q40 0 68 28t28 68v240H720v144Zm73-216v-153.67Q793-530 781-541t-28-11H206q-16.15 0-27.07 11.04Q168-529.92 168-513.6V-360h72v-72h480v72h73Z",
  download: "M480-336 288-528l51-51 105 105v-342h72v342l105-105 51 51-192 192ZM263.72-192Q234-192 213-213.15T192-264v-72h72v72h432v-72h72v72q0 29.7-21.16 50.85Q725.68-192 695.96-192H263.72Z",
};
const CHEVRON = "M10 6L8.59 7.41 13.17 12l-4.58 4.59L10 18l6-6z";

function svg(path: string, size: number, viewBox = "0 -960 960 960"): SVGSVGElement {
  const ns = "http://www.w3.org/2000/svg";
  const element = document.createElementNS(ns, "svg");
  for (const [name, value] of Object.entries({ viewBox, width: String(size), height: String(size), "aria-hidden": "true", focusable: "false" }))
    element.setAttribute(name, value);
  const shape = document.createElementNS(ns, "path");
  shape.setAttribute("d", path);
  element.append(shape);
  return element;
}

/** cr-icon-button: a 36 px round button with a 20 px icon, its label as its title. */
function iconButton(id: string, icon: string, label: string, onPress: () => void): HTMLButtonElement {
  const button = h("button", { type: "button", class: "cr-icon-button", id, title: label, "aria-label": label }, svg(ICONS[icon]!, 20));
  button.addEventListener("click", () => { if (!button.disabled) onPress(); });
  return button;
}
function setIcon(button: HTMLButtonElement, icon: string) {
  button.querySelector("path")?.setAttribute("d", ICONS[icon]!);
}

type OutlineItem = { title: string; dest: string | unknown[] | null; items: OutlineItem[] };
type Fit = "none" | "width" | "page";

/** One page's canvas and text layer, painted again when its size, rotation or annotation setting changes. */
class BrowserPdfPage {
  readonly element = h("div", { class: "page" });
  private readonly canvas = h("canvas");
  private readonly text = h("div", { "data-pdf-text-layer": "" });
  private painted = "";
  private stop: (() => void) | null = null;
  constructor(private readonly pdf: PDFDocumentProxy, readonly index: number, private readonly onError: (message: string) => void) {
    this.element.append(this.canvas, this.text);
  }
  layout(x: number, y: number, width: number, height: number, rotation: number, annotations: boolean) {
    Object.assign(this.element.style, { left: `${x}px`, top: `${y}px`, width: `${width}px`, height: `${height}px` });
    const key = `${Math.round(width)}|${rotation}|${annotations}`;
    if (key !== this.painted) this.paint(width, rotation, annotations, key);
  }
  private paint(width: number, rotation: number, annotations: boolean, key: string) {
    this.stop?.();
    this.painted = key;
    let cancelled = false;
    let task: ReturnType<PDFPageProxy["render"]> | undefined;
    let layer: TextLayer | undefined;
    let loaded: PDFPageProxy | undefined;
    const { canvas, text } = this;
    void (async () => {
      const page = await this.pdf.getPage(this.index + 1);
      loaded = page;
      if (cancelled) return;
      const turn = (page.rotate + rotation) % 360;
      const natural = page.getViewport({ scale: 1, rotation: turn });
      const viewport = page.getViewport({ scale: width / natural.width, rotation: turn });
      const ratio = Math.min(2, window.devicePixelRatio || 1, Math.sqrt((8 * 1024 * 1024) / (viewport.width * viewport.height)));
      canvas.width = Math.ceil(viewport.width * ratio);
      canvas.height = Math.ceil(viewport.height * ratio);
      task = page.render({
        canvas, viewport, transform: [ratio, 0, 0, ratio, 0, 0], background: "white",
        annotationMode: annotations ? AnnotationMode.ENABLE : AnnotationMode.DISABLE,
      });
      text.replaceChildren();
      text.style.setProperty("--total-scale-factor", String(viewport.scale));
      layer = new TextLayer({ container: text, viewport, textContentSource: page.streamTextContent() });
      await Promise.all([task.promise, layer.render()]);
    })().catch((cause: unknown) => {
      if (!cancelled && !(cause instanceof Error && cause.name === "RenderingCancelledException"))
        this.onError(cause instanceof Error ? cause.message : "Failed to load PDF document.");
    });
    this.stop = () => {
      cancelled = true;
      task?.cancel();
      layer?.cancel();
      void Promise.allSettled([task?.promise]).then(() => loaded?.cleanup());
    };
  }
  dispose() {
    this.stop?.();
    this.element.remove();
  }
}

export class BrowserPdfViewer {
  readonly element = h("div", { class: "browser-pdf" });
  private pdf: PDFDocumentProxy | null = null;
  private bytes = 0;
  private failed = false;
  private loading = true;
  /** Each page's size at 100 % without rotation (PDF points × 96 / 72), measured as pages load. */
  private sizes: Size[] = [];
  private rotation = 0;
  private twoUp = false;
  private annotations = true;
  private zoom = 1;
  private fit: Fit = "none";
  /** What the fit button does next (viewer-toolbar fittingType_): fit to page, then fit to width, in turn. */
  private nextFit: "page" | "width" = "page";
  private sidenavCollapsed: boolean;
  private tab: "thumbnails" | "outline" = "thumbnails";
  private outline: OutlineItem[] = [];
  private page = 0;
  private viewport: Size = { width: 600, height: 800 };
  private readonly pages = new Map<number, BrowserPdfPage>();
  private readonly thumbs: HTMLElement[] = [];
  private thumbObserver: IntersectionObserver | null = null;
  private closeLoad: (() => void) | null = null;
  private readonly unobserve: () => void;

  private readonly toolbar = h("div", { id: "toolbar" });
  private readonly title = h("span", { id: "title" });
  private readonly pageInput = h("input", { type: "text", id: "pageSelector", "aria-label": "Page number", value: "1" });
  private readonly pageLength = h("span", { id: "pagelength" }, "0");
  private readonly zoomInput = h("input", { type: "text", id: "zoom-input", "aria-label": "Zoom level", value: "100%" });
  private readonly zoomOut: HTMLButtonElement;
  private readonly zoomIn: HTMLButtonElement;
  private readonly fitButton: HTMLButtonElement;
  private readonly sidenavToggle: HTMLButtonElement;
  private readonly more: HTMLButtonElement;
  private readonly progress = h("div", { class: "progress", "aria-hidden": "true" });
  private readonly sidenav = h("div", { id: "sidenav-container" });
  private readonly tabs = h("div", { id: "icons", role: "tablist" });
  private readonly thumbList = h("div", { id: "thumbnails", role: "tablist", tabindex: 0 });
  private readonly outlineList = h("div", { id: "outline" });
  private readonly scroller = h("div", { id: "scroller", tabindex: 0, role: "region" });
  private readonly sizer = h("div", { id: "sizer" });
  private readonly menu = h("div", { id: "menu", role: "menu", "aria-label": "More actions", hidden: true });
  private readonly twoUpItem: HTMLButtonElement;
  private readonly annotationsItem: HTMLButtonElement;
  private dialog: HTMLElement | null = null;
  private presenting = false;
  private printing = false;
  private beforePresent: { zoom: number; fit: Fit } = { zoom: 1, fit: "none" };

  constructor(private readonly src: string, private readonly name: string, private readonly scheme: () => "light" | "dark") {
    this.sidenavCollapsed = readCollapsed();
    this.sidenavToggle = iconButton("sidenavToggle", "menu", "Toggle sidebar", () => this.toggleSidenav());
    this.zoomOut = iconButton("zoom-out", "remove", "Zoom out", () => this.setZoom(stepZoom(this.zoom, -1)));
    this.zoomIn = iconButton("zoom-in", "add", "Zoom in", () => this.setZoom(stepZoom(this.zoom, 1)));
    this.fitButton = iconButton("fit", "fit-to-height", "Fit to page", () => this.toggleFit());
    const rotate = iconButton("rotate", "rotate-left", "Rotate counterclockwise", () => this.rotate(-1));
    const download = iconButton("save", "download", "Download", () => post({ type: "download" }));
    const print = iconButton("print", "print", "Print", () => this.print());
    this.more = iconButton("more", "more", "More actions", () => this.toggleMenu());
    this.more.setAttribute("aria-haspopup", "menu");
    this.title.textContent = name;

    const separator = () => h("span", { class: "vertical-separator" });
    const selector = h("div", { id: "page-selector" }, this.pageInput, h("span", { id: "divider" }, "/"), this.pageLength);
    this.toolbar.append(
      h("div", { id: "start" }, this.sidenavToggle, this.title),
      h("div", { id: "center" }, selector, separator(),
        h("span", { id: "zoom-controls" }, this.zoomOut, this.zoomInput, this.zoomIn), separator(), this.fitButton, rotate),
      h("div", { id: "end" }, download, print, this.more),
      this.progress,
    );
    this.twoUpItem = this.menuItem("Two page view", true, () => { this.twoUp = !this.twoUp; this.relayout(true); });
    this.annotationsItem = this.menuItem("Annotations", true, () => { this.annotations = !this.annotations; this.update(); });
    this.menu.append(this.twoUpItem, this.annotationsItem, h("hr"), this.menuItem("Present", false, () => this.present(true)),
      this.menuItem("Document properties", false, () => this.showProperties()));

    this.sidenav.append(h("div", { id: "sidenav" }, this.tabs, h("div", { id: "sidenav-content" }, this.thumbList, this.outlineList)));
    this.scroller.append(this.sizer);
    this.element.append(h("div", { id: "toolbar-host" }, this.toolbar, this.menu), h("div", { id: "container" }, this.sidenav, h("div", { id: "main" }, this.scroller)));
    this.wire();
    const unobserveScroller = observeResize(this.scroller, () => this.measure());
    const unobserveRoot = observeResize(this.element, () => this.breakpoints());
    this.unobserve = () => { unobserveScroller(); unobserveRoot(); };
    this.load();
    this.update();
    queueMicrotask(() => this.measure());
  }

  private menuItem(label: string, checkbox: boolean, onPick: () => void): HTMLButtonElement {
    const check = h("span", { class: "check-container", "aria-hidden": "true" });
    if (checkbox) check.append(svg(ICONS.check!, 16));
    const item = h("button", { type: "button", class: "dropdown-item", role: checkbox ? "menuitemcheckbox" : "menuitem", tabindex: -1 }, check, label);
    item.addEventListener("click", () => { if (item.disabled) return; this.closeMenu(); onPick(); });
    return item;
  }

  private wire() {
    this.scroller.addEventListener("scroll", () => this.update());
    this.scroller.addEventListener("wheel", (event) => {
      if (!this.presenting || !this.pdf || Math.abs(event.deltaY) < 4) return;
      event.preventDefault();
      this.goToPage(Math.min(this.pdf.numPages - 1, Math.max(0, this.page + Math.sign(event.deltaY))));
    }, { passive: false });
    this.pageInput.addEventListener("pointerup", () => this.pageInput.select());
    this.pageInput.addEventListener("input", () => { this.pageInput.value = this.pageInput.value.replace(/\D/g, ""); });
    this.pageInput.addEventListener("change", () => {
      const page = Number.parseInt(this.pageInput.value, 10);
      if (this.pdf && page >= 1 && page <= this.pdf.numPages) this.goToPage(page - 1);
      else this.pageInput.value = String(this.page + 1);
    });
    this.zoomInput.addEventListener("pointerup", () => this.zoomInput.select());
    const zoomChange = () => {
      const value = Number.parseInt(this.zoomInput.value, 10);
      if (!Number.isNaN(value) && Math.abs(this.zoom * 100 - value) >= 0.5) this.setZoom(clampZoom(value / 100));
      else this.zoomInput.value = `${Math.round(this.zoom * 100)}%`;
    };
    this.zoomInput.addEventListener("change", zoomChange);
    this.zoomInput.addEventListener("blur", zoomChange);
    this.zoomInput.addEventListener("keydown", (event) => { if (event.key === "Enter") this.zoomInput.blur(); });
    this.thumbList.addEventListener("keydown", (event) => {
      const step = event.key === "ArrowDown" || event.key === "ArrowRight" ? 1 : event.key === "ArrowUp" || event.key === "ArrowLeft" ? -1 : 0;
      if (!step || !this.pdf) return;
      event.preventDefault();
      const next = Math.min(this.pdf.numPages - 1, Math.max(0, this.page + step));
      this.goToPage(next);
      this.thumbs[next]?.focus();
    });
    this.menu.addEventListener("keydown", (event) => {
      const items = [...this.menu.querySelectorAll<HTMLButtonElement>(".dropdown-item:not([disabled])")];
      const at = items.indexOf(document.activeElement as HTMLButtonElement);
      if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        event.preventDefault();
        items[(at + (event.key === "ArrowDown" ? 1 : items.length - 1)) % items.length]?.focus();
      } else if (event.key === "Escape" || event.key === "Tab") {
        event.preventDefault();
        this.closeMenu();
        this.more.focus();
      }
    });
    document.addEventListener("pointerdown", (event) => {
      if (!this.menu.hidden && !this.menu.contains(event.target as Node) && !this.more.contains(event.target as Node)) this.closeMenu();
    }, true);
    this.element.addEventListener("keydown", (event) => this.shortcut(event));
  }

  /** The viewer's own keys (pdf_viewer.ts handleKeyEvent): ⌘[ and ⌘] rotate, ⌘\ toggles the fit, ⌘= and ⌘- zoom. */
  private shortcut(event: KeyboardEvent) {
    const target = event.target as HTMLElement;
    if (target.tagName === "INPUT") return;
    if (this.presentKey(event)) { event.preventDefault(); return; }
    const command = event.metaKey || event.ctrlKey;
    if (command && event.key === "[") this.rotate(-1);
    else if (command && event.key === "]") this.rotate(1);
    else if (command && event.key === "\\") this.toggleFit();
    else if (command && (event.key === "=" || event.key === "+")) this.setZoom(stepZoom(this.zoom, 1));
    else if (command && event.key === "-") this.setZoom(stepZoom(this.zoom, -1));
    else if (command && event.key === "0") this.setZoom(initialZoom(this.layout(), this.viewport));
    else if (!command && !event.altKey && (event.key === "ArrowLeft" || event.key === "ArrowRight") && this.scroller.scrollWidth <= this.scroller.clientWidth && this.pdf) {
      // Left and Right turn the page while nothing scrolls sideways.
      this.goToPage(Math.min(this.pdf.numPages - 1, Math.max(0, this.page + (event.key === "ArrowRight" ? 1 : -1))));
    } else return;
    event.preventDefault();
  }

  private load() {
    const controller = new AbortController();
    let task: ReturnType<typeof openPdf> | undefined;
    void readBoundedDocument(this.src, controller.signal, MAX_BYTES)
      .then(async (bytes) => {
        this.bytes = bytes.byteLength;
        task = openPdf(bytes);
        const pdf = await task.promise;
        if (controller.signal.aborted) return;
        if (pdf.numPages > MAX_PAGES) throw new Error("Too many pages.");
        const first = (await pdf.getPage(1)).getViewport({ scale: 1, rotation: 0 });
        this.sizes = Array.from({ length: pdf.numPages }, () => ({ width: first.width * PX_PER_POINT, height: first.height * PX_PER_POINT }));
        this.pdf = pdf;
        const [outline, metadata] = await Promise.all([pdf.getOutline().catch(() => null), pdf.getMetadata().catch(() => null)]);
        if (controller.signal.aborted) return;
        this.outline = (outline ?? []) as OutlineItem[];
        const docTitle = (metadata?.info as { Title?: string } | undefined)?.Title?.trim();
        if (docTitle) this.title.textContent = docTitle;
        this.loading = false;
        this.viewport = { width: this.scroller.clientWidth || this.viewport.width, height: this.scroller.clientHeight || this.viewport.height };
        this.zoom = initialZoom(this.layout(), this.viewport);
        this.buildSidenav();
        this.update();
        // Every page's own size, a page at a time, so a document of mixed sizes lays out as it is.
        for (let index = 1; index < pdf.numPages && !controller.signal.aborted; index++) {
          const size = (await pdf.getPage(index + 1)).getViewport({ scale: 1, rotation: 0 });
          const next = { width: size.width * PX_PER_POINT, height: size.height * PX_PER_POINT };
          if (next.width !== this.sizes[index]!.width || next.height !== this.sizes[index]!.height) { this.sizes[index] = next; this.relayout(false); }
        }
      })
      .catch((cause: unknown) => {
        if (controller.signal.aborted) return;
        post({ type: "error", message: cause instanceof Error ? cause.message : "Failed to load PDF document." });
        this.failed = true;
        this.loading = false;
        this.update();
      });
    this.closeLoad = () => { controller.abort(); void task?.destroy(); };
  }

  /** The pages' sizes as rotated: a quarter turn swaps width and height. */
  private layout(): Layout {
    const quarter = this.rotation % 180 !== 0;
    return pdfLayout(quarter ? this.sizes.map(({ width, height }) => ({ width: height, height: width })) : this.sizes, this.twoUp);
  }

  /** viewer_toolbar.css's and pdf_viewer.css's breakpoints, by the viewer's own size. */
  private breakpoints() {
    const width = this.element.clientWidth, height = this.element.clientHeight;
    if (!width || !height) return;
    const narrow = [600, 500, 420, 200].filter((limit) => width <= limit).join(" ");
    if (narrow) this.element.setAttribute("data-narrow", narrow); else this.element.removeAttribute("data-narrow");
    this.element.toggleAttribute("data-short", height <= 250);
    this.measure();
  }

  private measure() {
    const next = { width: this.scroller.clientWidth, height: this.scroller.clientHeight };
    if (next.width === this.viewport.width && next.height === this.viewport.height) return;
    this.viewport = next;
    if (this.fit !== "none" && this.pdf) this.zoom = fittingZoom(this.layout(), this.page, this.viewport, this.fit);
    this.update();
  }

  /** A new zoom keeps the point at the viewport's top where it was in the document. */
  private setZoom(zoom: number, fit: Fit = "none") {
    const old = this.zoom;
    this.fit = fit;
    this.zoom = clampZoom(zoom);
    if (old === this.zoom) { this.update(); return; }
    const top = this.scroller.scrollTop / old, left = (this.scroller.scrollLeft + this.viewport.width / 2) / old;
    this.update();
    this.scroller.scrollTo({ top: top * this.zoom, left: left * this.zoom - this.viewport.width / 2 });
    this.update();
  }

  private toggleFit() {
    if (!this.pdf) return;
    const fit = this.nextFit;
    this.nextFit = fit === "page" ? "width" : "page";
    setIcon(this.fitButton, this.nextFit === "page" ? "fit-to-height" : "fit-to-width");
    const label = this.nextFit === "page" ? "Fit to page" : "Fit to width";
    this.fitButton.title = label;
    this.fitButton.setAttribute("aria-label", label);
    const page = this.page;
    this.setZoom(fittingZoom(this.layout(), page, this.viewport, fit), fit);
    if (fit === "page") this.goToPage(page);
  }

  private rotate(direction: 1 | -1) {
    this.rotation = (this.rotation + (direction > 0 ? 90 : 270)) % 360;
    this.relayout(true);
  }

  /** The layout changed (rotation, two page view, a page's size): the current page stays in view. */
  private relayout(keepPage: boolean) {
    const page = this.page;
    // A layout wider than the viewport (two page view, a turned page) shrinks to its width.
    this.zoom = this.fit !== "none" ? fittingZoom(this.layout(), page, this.viewport, this.fit) : Math.min(this.zoom, fittingZoom(this.layout(), page, this.viewport, "width"));
    this.twoUpItem.setAttribute("aria-checked", String(this.twoUp));
    this.update();
    if (keepPage) this.goToPage(page);
  }

  /** Present: the pages alone, one at a time, fitted to the viewer; the keys and the wheel step a page; Escape ends it. */
  private present(on: boolean) {
    if (!this.pdf || this.presenting === on) return;
    const page = this.page;
    this.presenting = on;
    this.element.toggleAttribute("data-present", on);
    if (on) { this.beforePresent = { zoom: this.zoom, fit: this.fit }; this.scroller.focus(); }
    queueMicrotask(() => {
      this.viewport = { width: this.scroller.clientWidth || this.viewport.width, height: this.scroller.clientHeight || this.viewport.height };
      if (on) this.setZoom(fittingZoom(this.layout(), page, this.viewport, "page"), "page");
      else this.setZoom(this.beforePresent.zoom, this.beforePresent.fit);
      this.goToPage(page);
    });
  }
  private presentKey(event: KeyboardEvent): boolean {
    if (!this.presenting || !this.pdf) return false;
    const forward = ["ArrowDown", "ArrowRight", "PageDown", " "].includes(event.key), back = ["ArrowUp", "ArrowLeft", "PageUp"].includes(event.key);
    if (event.key === "Escape") this.present(false);
    else if (forward || back) this.goToPage(Math.min(this.pdf.numPages - 1, Math.max(0, this.page + (forward ? 1 : -1))));
    else return false;
    return true;
  }

  private goToPage(index: number, offset = 0) {
    const rect = this.layout().pages[index];
    if (!rect) return;
    this.scroller.scrollTo({ top: (rect.y - PAGE_SHADOW.top + offset) * this.zoom });
    this.update();
  }

  private toggleSidenav() {
    this.sidenavCollapsed = !this.sidenavCollapsed;
    try { localStorage.setItem(SIDENAV_KEY, this.sidenavCollapsed ? "1" : "0"); } catch { /* storage is off */ }
    this.update();
  }

  private toggleMenu() {
    if (!this.menu.hidden) { this.closeMenu(); return; }
    this.menu.hidden = false;
    this.more.classList.add("open");
    this.more.setAttribute("aria-expanded", "true");
    const box = this.more.getBoundingClientRect();
    this.menu.style.top = `${box.bottom}px`;
    this.menu.style.right = `${Math.max(0, window.innerWidth - box.right)}px`;
    this.menu.querySelector<HTMLButtonElement>(".dropdown-item:not([disabled])")?.focus();
  }
  private closeMenu() {
    this.menu.hidden = true;
    this.more.classList.remove("open");
    this.more.setAttribute("aria-expanded", "false");
  }

  private buildSidenav() {
    const pdf = this.pdf;
    if (!pdf) return;
    this.thumbObserver?.disconnect();
    this.thumbList.replaceChildren();
    this.thumbs.length = 0;
    this.thumbObserver = new IntersectionObserver((entries) => {
      for (const entry of entries) if (entry.isIntersecting) this.paintThumb(Number((entry.target as HTMLElement).dataset.index));
    }, { root: this.thumbList, rootMargin: "200px 0px" });
    for (let index = 0; index < pdf.numPages; index++) {
      const box = h("div", { class: "thumbnail-box", "data-index": index }, h("canvas"));
      const thumb = h("div", { class: "thumbnail", role: "tab", tabindex: 0, "aria-label": `Thumbnail for page ${index + 1}`, "aria-selected": "false", "data-index": index },
        box, h("div", { class: "page-number" }, String(index + 1)));
      box.addEventListener("click", () => this.goToPage(index));
      thumb.addEventListener("keydown", (event) => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); this.goToPage(index); } });
      this.thumbs.push(thumb);
      this.thumbList.append(thumb);
      this.thumbObserver.observe(box);
    }
    const tabs: { id: "thumbnails" | "outline"; icon: string; title: string }[] = [{ id: "thumbnails", icon: "thumbnails", title: "Thumbnails" }];
    if (this.outline.length) tabs.push({ id: "outline", icon: "doc-outline", title: "Document outline" });
    this.tabs.replaceChildren(...tabs.map((tab) => {
      const button = iconButton(`tab-${tab.id}`, tab.icon, tab.title, () => { this.tab = tab.id; this.update(); });
      button.setAttribute("role", "tab");
      return h("div", { class: "button-wrapper", "data-tab": tab.id }, button);
    }));
    this.tabs.hidden = tabs.length === 1;
    this.outlineList.replaceChildren(...this.outline.map((item) => this.bookmark(item, 0)));
  }

  /** viewer-bookmark: a row with its title, a chevron when it has children (collapsed at first), then its children. */
  private bookmark(item: OutlineItem, depth: number): HTMLElement {
    const node = h("div", { class: "bookmark" });
    const expand = h("button", { type: "button", class: "expand cr-icon-button", "aria-label": "Expand/collapse", "aria-expanded": "false" }, svg(CHEVRON, 16, "0 0 24 24"));
    expand.hidden = !item.items.length;
    const title = h("span", { class: "bookmark-title", tabindex: 0 }, item.title);
    const row = h("div", { class: "bookmark-item", style: `padding-inline-start: ${depth * BOOKMARK_INDENT}px` }, h("div", { class: "expand-container" }, expand), title);
    const children = h("div", { class: "bookmark-children", hidden: true });
    node.append(row, children);
    expand.addEventListener("click", (event) => {
      event.stopPropagation();
      const shown = children.hasAttribute("hidden");
      if (shown && !children.childElementCount) children.append(...item.items.map((child) => this.bookmark(child, depth + 1)));
      children.hidden = !shown;
      expand.setAttribute("aria-expanded", String(shown));
      node.toggleAttribute("data-shown", shown);
    });
    row.addEventListener("click", () => void this.follow(item.dest));
    title.addEventListener("keydown", (event) => { if (event.key === "Enter") void this.follow(item.dest); });
    return node;
  }

  /** An outline entry's destination: its page, and how far down it (a point from the page's top, at 100 %). */
  private async follow(dest: string | unknown[] | null) {
    const pdf = this.pdf;
    if (!pdf || !dest) return;
    const explicit = typeof dest === "string" ? await pdf.getDestination(dest).catch(() => null) : dest;
    if (!Array.isArray(explicit) || !explicit.length) return;
    const ref = explicit[0];
    const index = typeof ref === "number" ? ref : await pdf.getPageIndex(ref as Parameters<PDFDocumentProxy["getPageIndex"]>[0]).catch(() => -1);
    if (index < 0) return;
    const name = (explicit[1] as { name?: string } | undefined)?.name;
    const y = name === "XYZ" || name === "FitH" || name === "FitBH" ? Number(explicit[name === "XYZ" ? 3 : 2]) : NaN;
    let offset = 0;
    if (Number.isFinite(y) && this.rotation === 0) offset = Math.max(0, this.sizes[index]!.height - y * PX_PER_POINT);
    this.goToPage(index, offset);
  }

  private paintThumb(index: number) {
    const box = this.thumbs[index]?.firstElementChild as HTMLElement | undefined, pdf = this.pdf;
    const canvas = box?.querySelector("canvas");
    if (!box || !canvas || !pdf || box.dataset.painted === String(this.rotation)) return;
    box.dataset.painted = String(this.rotation);
    void pdf.getPage(index + 1).then(async (page) => {
      const turn = (page.rotate + this.rotation) % 360;
      const natural = page.getViewport({ scale: 1, rotation: turn });
      // viewer-thumbnail: 108 px wide upright, 140 px wide on its side.
      const width = natural.width > natural.height ? 140 : 108;
      const viewport = page.getViewport({ scale: width / natural.width, rotation: turn });
      const ratio = Math.min(2, window.devicePixelRatio || 1);
      canvas.width = Math.round(viewport.width * ratio);
      canvas.height = Math.round(viewport.height * ratio);
      canvas.style.width = `${viewport.width}px`;
      canvas.style.height = `${viewport.height}px`;
      await page.render({ canvas, viewport, transform: [ratio, 0, 0, ratio, 0, 0], background: "white" }).promise;
    }).catch(() => { delete box.dataset.painted; });
  }

  /**
   * Print (viewer-toolbar `pdf:print`) prints the document itself, as Chromium's plugin does, not the viewer: its bytes
   * in a hidden frame, whose `print()` the host answers with the platform's print panel for that frame's PDF (LLP
   * 1020.002 D5). The page waits in `print()` until the panel ends; the job takes the file's name.
   */
  private print() {
    const pdf = this.pdf;
    if (!pdf || this.printing) return;
    this.printing = true;
    // The host names the job after the page's title as `print()` runs (LLP 1020.002 D5, "Printing").
    const title = document.title;
    document.title = this.name;
    const done = () => { document.title = title; this.printing = false; };
    void pdf.getData().then((data) => {
      const url = URL.createObjectURL(new Blob([new Uint8Array(data)], { type: "application/pdf" }));
      const frame = h("iframe", { class: "print-frame", src: url, tabindex: -1, "aria-hidden": "true" });
      frame.addEventListener("load", () => {
        try { frame.contentWindow?.print(); } finally {
          frame.remove();
          URL.revokeObjectURL(url);
          done();
        }
      }, { once: true });
      document.body.append(frame);
    }, done);
  }

  private showProperties() {
    const pdf = this.pdf;
    if (!pdf || this.dialog) return;
    const opener = document.activeElement as HTMLElement | null;
    const row = (name: string, value: string, id: string, rule = false) =>
      h("tr", rule ? { class: "break" } : {}, h("td", { class: "name" }, name), h("td", { class: "value", id }, value || "-"));
    const close = h("button", { type: "button", class: "action-button" }, "Close");
    const table = h("table");
    const dialog = h("div", { class: "dialog-backdrop", "data-scheme": this.scheme() },
      h("div", { class: "dialog", role: "dialog", "aria-modal": "true", "aria-labelledby": "properties-title" },
        h("div", { class: "dialog-title", id: "properties-title" }, "Document properties"),
        h("div", { class: "dialog-body" }, table),
        h("div", { class: "dialog-buttons" }, close)));
    const dismiss = () => { dialog.remove(); this.dialog = null; opener?.focus(); };
    close.addEventListener("click", dismiss);
    dialog.addEventListener("keydown", (event) => { if (event.key === "Escape") { event.preventDefault(); dismiss(); } else if (event.key === "Tab") { event.preventDefault(); close.focus(); } });
    this.dialog = dialog;
    this.element.append(dialog);
    close.focus();
    void Promise.all([pdf.getMetadata().catch(() => null), pdf.getPage(1)]).then(([metadata, first]) => {
      const info = (metadata?.info ?? {}) as Record<string, unknown>;
      const text = (key: string) => (typeof info[key] === "string" ? (info[key] as string) : "");
      const date = (key: string) => formatPdfDate(text(key));
      const size = first.getViewport({ scale: 1, rotation: 0 });
      const metric = !/^en-(US|LR|MM)$/i.test(navigator.language);
      table.append(
        row("File name:", this.name, "file-name"),
        row("File size:", fileSizeText(this.bytes), "file-size", true),
        row("Title:", text("Title"), "title"),
        row("Author:", text("Author"), "author"),
        row("Subject:", text("Subject"), "subject"),
        row("Keywords:", text("Keywords"), "keywords"),
        row("Created:", date("CreationDate"), "created"),
        row("Modified:", date("ModDate"), "modified", true),
        row("Application:", text("Creator"), "application"),
        row("PDF producer:", text("Producer"), "pdf-producer"),
        row("PDF version:", text("PDFFormatVersion"), "pdf-version"),
        row("Page count:", String(pdf.numPages), "page-count"),
        row("Page size:", pageSizeText({ width: size.width, height: size.height }, metric), "page-size", true),
        row("Fast web view:", info.IsLinearized === true ? "Yes" : "No", "fast-web-view"),
      );
    });
  }

  private update() {
    const pdf = this.pdf, layout = this.layout(), zoom = this.zoom;
    this.element.toggleAttribute("data-loading", this.loading);
    this.toolbar.toggleAttribute("data-loading", this.loading);
    this.sidenav.toggleAttribute("data-closed", this.sidenavCollapsed);
    this.sidenavToggle.setAttribute("aria-expanded", String(!this.sidenavCollapsed));
    this.annotationsItem.setAttribute("aria-checked", String(this.annotations));
    this.twoUpItem.setAttribute("aria-checked", String(this.twoUp));
    this.twoUpItem.querySelector("svg")?.toggleAttribute("hidden", !this.twoUp);
    this.annotationsItem.querySelector("svg")?.toggleAttribute("hidden", !this.annotations);
    this.zoomOut.disabled = zoom <= 0.25 + 1e-6;
    this.zoomIn.disabled = zoom >= 5 - 1e-6;
    if (document.activeElement !== this.zoomInput) this.zoomInput.value = `${Math.round(zoom * 100)}%`;
    if (this.failed) {
      this.sizer.replaceChildren(h("div", { class: "load-failed", role: "status" }, "Failed to load PDF document."));
      return;
    }
    if (!pdf) return;
    const count = pdf.numPages;
    this.scroller.setAttribute("aria-label", count === 1 ? "PDF document containing 1 page" : `PDF document containing ${count} pages`);
    this.pageLength.textContent = String(count);
    this.pageInput.parentElement?.style.setProperty("--page-length-digits", String(String(count).length));
    const width = layout.width * zoom, height = layout.height * zoom;
    const left = Math.max(0, (this.viewport.width - width) / 2);
    Object.assign(this.sizer.style, { width: `${Math.max(width, this.viewport.width)}px`, height: `${height}px` });
    this.page = mostVisiblePage(layout, zoom, this.scroller.scrollTop, this.viewport.height);
    if (document.activeElement !== this.pageInput) this.pageInput.value = String(this.page + 1);
    const near = new Set(pagesNear(layout, zoom, this.scroller.scrollTop, this.viewport.height));
    for (const [index, page] of this.pages) if (!near.has(index)) { page.dispose(); this.pages.delete(index); }
    for (const index of near) {
      let page = this.pages.get(index);
      if (!page) {
        page = new BrowserPdfPage(pdf, index, () => { this.failed = true; this.update(); });
        this.pages.set(index, page);
        this.sizer.append(page.element);
      }
      const rect = layout.pages[index]!;
      page.layout(left + rect.x * zoom, rect.y * zoom, rect.width * zoom, rect.height * zoom, this.rotation, this.annotations);
    }
    this.thumbs.forEach((thumb, index) => {
      const active = index === this.page;
      if (thumb.hasAttribute("data-active") !== active) {
        thumb.toggleAttribute("data-active", active);
        thumb.setAttribute("aria-selected", String(active));
        if (active && !this.sidenavCollapsed) thumb.scrollIntoView?.({ block: "nearest" });
      }
      const box = thumb.firstElementChild as HTMLElement;
      if (box.dataset.painted !== undefined && box.dataset.painted !== String(this.rotation)) this.paintThumb(index);
    });
    for (const wrapper of this.tabs.children) {
      const selected = (wrapper as HTMLElement).dataset.tab === this.tab;
      wrapper.classList.toggle("selected", selected);
      const button = wrapper.firstElementChild as HTMLElement;
      button.setAttribute("aria-selected", String(selected));
      button.tabIndex = selected ? 0 : -1;
    }
    this.thumbList.hidden = this.tab !== "thumbnails";
    this.outlineList.hidden = this.tab !== "outline";
  }

  dispose() {
    this.closeLoad?.();
    this.unobserve();
    this.thumbObserver?.disconnect();
    for (const page of this.pages.values()) page.dispose();
    this.pages.clear();
    this.pdf = null;
  }
}

function readCollapsed(): boolean {
  try { return localStorage.getItem(SIDENAV_KEY) === "1"; } catch { return false; }
}

/** A PDF date ("D:20261102155500+09'00'") as the properties dialog writes one: the locale's short date and time. */
export function formatPdfDate(raw: string): string {
  const match = /^D?:?(\d{4})(\d{2})?(\d{2})?(\d{2})?(\d{2})?(\d{2})?([Zz+-])?(\d{2})?'?(\d{2})?/.exec(raw.trim());
  if (!match) return raw;
  const [, year, month = "01", day = "01", hour = "00", minute = "00", second = "00", sign, offsetHour = "00", offsetMinute = "00"] = match;
  let time = Date.UTC(Number(year), Number(month) - 1, Number(day), Number(hour), Number(minute), Number(second));
  if (sign === "+" || sign === "-") time -= (sign === "+" ? 1 : -1) * (Number(offsetHour) * 60 + Number(offsetMinute)) * 60_000;
  else if (!sign) time += new Date(time).getTimezoneOffset() * 60_000;
  return new Date(time).toLocaleString(undefined, { dateStyle: "short", timeStyle: "medium" });
}
