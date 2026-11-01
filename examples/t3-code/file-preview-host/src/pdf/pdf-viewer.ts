// The PDF viewer (MIT reference, see LICENSE-T3: apps/web/src/components/files/PdfViewer.tsx; T3 Code 2359643cff),
// which the Electron app always takes (BrowserDocumentFrame.tsx PdfDocument). PDF.js renders each page near the
// viewport to a canvas with its text layer; the toolbar pages, zooms 25–300 % from fit width, and finds a phrase
// across line ends (pdf-layout.ts). It opens at the page, offset, zoom and horizontal scroll it is given, and reports
// them as they change, so a new revision of the same document opens where the reader was. Its React state is this
// class's fields; `update` is its render.
import { TextLayer, type PDFDocumentProxy } from "pdfjs-dist";
import { ChevronDown, ChevronUp, Minus, Plus, Scan } from "../icons";
import { h, observeResize, searchField, surfaceAction, surfaceFailure, surfaceLoading, type IconNode } from "../chrome";
import { readBoundedDocument } from "../shared/read-bounded";
import { openPdf } from "./pdf-document";
import {
  currentPdfPage, pdfFindIncludes, pdfFindMatchRuns, pdfPageOffsets, pdfPageScrollTop, pdfPageText, pdfPageWidth,
  PDF_MAX_ZOOM, PDF_MIN_ZOOM, stepPdfZoom, visiblePdfPages,
} from "./pdf-layout";

const MAX_PAGES = 2000;
const MAX_FIND_TEXT = 4 * 1024 * 1024;

/** Where the reader is: the current page, how far into it, the zoom (null: fit width) and the horizontal scroll. */
export type PdfViewState = { page: number; offset: number; zoom: number | null; left: number };

/** PdfPage: one page's canvas and text layer, painted again when its width changes. */
class PdfPage {
  readonly element = h("div", { class: "pdf-page" });
  private readonly canvas: HTMLCanvasElement;
  private readonly text = h("div", { "data-pdf-text-layer": "" });
  private width = 0;
  private query: string | null = null;
  private stop: (() => void) | null = null;
  private observer: MutationObserver | null = null;
  constructor(
    private readonly pdf: PDFDocumentProxy,
    readonly index: number,
    private readonly onSize: (index: number, ratio: number) => void,
    private readonly onError: (error: string) => void,
  ) {
    this.canvas = h("canvas", { "aria-label": `Page ${index + 1}` });
    this.element.append(this.canvas, this.text);
  }
  layout(top: number, left: number, width: number, height: number, query: string) {
    Object.assign(this.element.style, { top: `${top}px`, left: `${left}px`, width: `${width}px`, height: `${height}px` });
    if (width !== this.width) this.paint(width);
    if (query !== this.query) this.highlight(query);
  }
  private paint(width: number) {
    this.stop?.();
    this.width = width;
    let cancelled = false;
    let paint: ReturnType<Awaited<ReturnType<PDFDocumentProxy["getPage"]>>["render"]> | undefined;
    let layer: TextLayer | undefined;
    let loadedPage: Awaited<ReturnType<PDFDocumentProxy["getPage"]>> | undefined;
    const { pdf, index, canvas, text } = this;
    void (async () => {
      const page = await pdf.getPage(index + 1);
      loadedPage = page;
      if (cancelled) { page.cleanup(); return; }
      const natural = page.getViewport({ scale: 1 });
      this.onSize(index, natural.height / natural.width);
      const viewport = page.getViewport({ scale: width / natural.width });
      const ratio = Math.min(2, window.devicePixelRatio || 1, Math.sqrt((8 * 1024 * 1024) / (viewport.width * viewport.height)));
      canvas.width = Math.ceil(viewport.width * ratio);
      canvas.height = Math.ceil(viewport.height * ratio);
      paint = page.render({ canvas, viewport, transform: [ratio, 0, 0, ratio, 0, 0], background: "white" });
      text.replaceChildren();
      text.style.setProperty("--total-scale-factor", String(viewport.scale));
      layer = new TextLayer({ container: text, viewport, textContentSource: page.streamTextContent() });
      await Promise.all([paint.promise, layer.render()]);
    })().catch((cause: unknown) => {
      if (!cancelled) this.onError(cause instanceof Error ? cause.message : "This PDF page could not render.");
    });
    this.stop = () => {
      cancelled = true;
      paint?.cancel();
      layer?.cancel();
      // Release operator lists when virtual pages leave the viewport.
      void Promise.allSettled([paint?.promise]).then(() => loadedPage?.cleanup());
    };
  }
  private highlight(query: string) {
    this.query = query;
    this.observer?.disconnect();
    const node = this.text;
    const mark = () => {
      const elements = Array.from(node.querySelectorAll('span[role="presentation"], br'));
      const hits = pdfFindMatchRuns(elements.map((element) => (element.tagName === "BR" ? "\n" : (element.textContent ?? ""))), query);
      elements.forEach((element, index) => element.classList.toggle("pdf-find-match", hits[index]));
    };
    mark();
    this.observer = new MutationObserver(mark);
    this.observer.observe(node, { childList: true, subtree: true });
  }
  dispose() {
    this.stop?.();
    this.observer?.disconnect();
    this.element.remove();
  }
}

export class PdfViewer {
  readonly element = h("div", { class: "pdf" });
  private pdf: PDFDocumentProxy | null = null;
  private error: string | null = null;
  private revision = 0;
  private ratios: number[] = [];
  private zoom: number | null = null;
  private pageWidth = 612;
  private viewport = { width: 600, height: 800 };
  private scrollTop = 0;
  private query = "";
  private matches: number[] = [];
  private finding = false;
  private findError: string | null = null;
  private match = 0;
  private zoomAnchor: { page: number; offset: number; width: number } | null = null;
  private pages = new Map<number, PdfPage>();
  private closeLoad: (() => void) | null = null;
  private closeFind: (() => void) | null = null;
  private readonly unobserve: () => void;
  private readonly label = h("span", { class: "pdf-pages-label", "aria-live": "polite" }, "PDF");
  private readonly zoomLabel = h("span", { class: "pdf-zoom" }, "Fit width");
  private readonly status = h("span", { role: "status" });
  private readonly scroll: HTMLDivElement;
  private readonly find: HTMLInputElement;
  private readonly previous; private readonly next; private readonly zoomOut; private readonly zoomIn; private readonly nextMatch;
  private body: HTMLElement | null = null;
  private track: HTMLDivElement | null = null;
  private readonly initialView: PdfViewState;
  /** The loaded document whose place was restored; until then a scroll reports nothing. */
  private restored: PDFDocumentProxy | null = null;

  constructor(private readonly src: string, title: string, initialView?: PdfViewState, private readonly onViewChange?: (view: PdfViewState) => void) {
    this.initialView = initialView ?? { page: 0, offset: 0, zoom: null, left: 0 };
    this.zoom = this.initialView.zoom;
    this.previous = surfaceAction({ label: "Previous page", icon: ChevronUp as IconNode, disabled: true, onPress: () => this.showPage(this.current() - 1) });
    this.next = surfaceAction({ label: "Next page", icon: ChevronDown as IconNode, disabled: true, onPress: () => this.showPage(this.current() + 1) });
    this.zoomOut = surfaceAction({ label: "Zoom out", icon: Minus as IconNode, onPress: () => this.changeZoom(stepPdfZoom(this.sizes().scale, -1)) });
    this.zoomIn = surfaceAction({ label: "Zoom in", icon: Plus as IconNode, onPress: () => this.changeZoom(stepPdfZoom(this.sizes().scale, 1)) });
    const fit = surfaceAction({ label: "Fit width", icon: Scan as IconNode, onPress: () => { this.changeZoom(null); this.scroll.scrollTo({ left: 0 }); } });
    this.nextMatch = surfaceAction({
      label: "Next matching page", icon: ChevronDown as IconNode, disabled: true,
      onPress: () => {
        const next = (this.match + 1) % this.matches.length;
        this.match = next;
        this.showPage(this.matches[next]!);
        this.update();
      },
    });
    this.find = searchField("Find in PDF", (value) => {
      this.query = value;
      this.matches = [];
      this.findError = null;
      this.search();
      this.update();
    });
    this.scroll = h("div", { class: "pdf-scroll", role: "region", "aria-label": title, tabindex: 0 });
    this.scroll.addEventListener("scroll", () => { this.scrollTop = this.scroll.scrollTop; this.update(); this.saveView(); });
    this.scroll.addEventListener("keydown", (event) => {
      if ((event.ctrlKey || event.metaKey) && event.key === "f") {
        event.preventDefault();
        event.stopPropagation();
        this.find.focus();
      }
    });
    this.element.append(
      h("div", { class: "toolbar" }, this.label, this.previous.element, this.next.element, this.zoomOut.element, this.zoomLabel,
        this.zoomIn.element, fit.element, h("div", { class: "pdf-find" }, this.find), this.status, this.nextMatch.element),
      this.scroll,
    );
    const measure = () => { this.viewport = { width: this.scroll.clientWidth, height: this.scroll.clientHeight }; this.update(); };
    this.unobserve = observeResize(this.scroll, measure);
    this.load();
    this.update();
    queueMicrotask(measure);
  }

  private load() {
    const controller = new AbortController();
    let disposed = false;
    let task: ReturnType<typeof openPdf> | undefined;
    const timeout = setTimeout(() => {
      this.error = "This PDF took too long to load. Try again.";
      controller.abort();
      void task?.destroy();
      this.update();
    }, 30_000);
    void readBoundedDocument(this.src, controller.signal, 50 * 1024 * 1024, this.revision > 0 ? "reload" : "default")
      .then(async (bytes) => {
        controller.signal.throwIfAborted();
        task = openPdf(bytes);
        const document = await task.promise;
        if (document.numPages > MAX_PAGES) throw new Error("Preview limited to 2,000 pages. Save this PDF to read it in full.");
        const first = (await document.getPage(1)).getViewport({ scale: 1 });
        // The pages up to the one to restore are measured, so it is laid out where it is.
        const sizes = await Promise.all(Array.from({ length: Math.min(this.initialView.page + 1, document.numPages) }, async (_, index) => {
          const size = (await document.getPage(index + 1)).getViewport({ scale: 1 });
          return size.height / size.width;
        }));
        if (controller.signal.aborted) return;
        this.pageWidth = first.width;
        this.ratios = Array.from({ length: document.numPages }, (_, index) => sizes[index] ?? first.height / first.width);
        this.pdf = document;
        this.search();
        this.update();
      })
      .catch((cause: unknown) => {
        if (!disposed && !controller.signal.aborted) {
          this.error = cause instanceof Error ? cause.message : "Unable to open PDF.";
          this.update();
        }
      })
      .finally(() => clearTimeout(timeout));
    this.closeLoad = () => {
      disposed = true;
      controller.abort();
      clearTimeout(timeout);
      void task?.destroy();
    };
  }

  private sizes() {
    const { scale, width } = pdfPageWidth(this.viewport.width, this.pageWidth, this.zoom);
    return { scale, width, offsets: pdfPageOffsets(this.ratios, width) };
  }
  private current() {
    return currentPdfPage(this.sizes().offsets, this.ratios.length, this.scrollTop);
  }
  private showPage(index: number) {
    this.scroll.scrollTo({ top: pdfPageScrollTop(this.sizes().offsets, index) });
  }
  /** PdfViewer's saveView: the reader's place, reported once the document's own place is restored. */
  private saveView() {
    if (!this.pdf || this.restored !== this.pdf) return;
    const { offsets } = this.sizes(), page = this.current();
    this.onViewChange?.({ page, offset: Math.max(0, this.scroll.scrollTop - (offsets[page]! - 16)), zoom: this.zoom, left: this.scroll.scrollLeft });
  }
  private changeZoom(value: number | null) {
    const { offsets, width } = this.sizes(), current = this.current();
    this.zoomAnchor = { page: current, offset: this.scroll.scrollTop - (offsets[current]! - 16), width };
    this.zoom = value;
    this.update();
  }
  private readonly onSize = (index: number, ratio: number) => {
    if (this.ratios[index] === ratio) return;
    this.ratios = this.ratios.map((value, i) => (i === index ? ratio : value));
    this.update();
  };
  private readonly onError = (error: string) => {
    this.error = error;
    this.update();
  };

  /** The find effect: a quarter second after the query or the document changes, every page's text is scanned. */
  private search() {
    this.closeFind?.();
    this.closeFind = null;
    const pdf = this.pdf, query = this.query;
    if (!pdf || !query.trim()) { this.finding = false; return; }
    let cancelled = false;
    const timer = setTimeout(() => {
      this.finding = true;
      this.findError = null;
      this.matches = [];
      this.match = 0;
      this.update();
      void (async () => {
        const found: number[] = [];
        const sizes: number[] = [];
        let size = 0;
        for (let index = 0; index < pdf.numPages; index++) {
          if (cancelled) return;
          const page = await pdf.getPage(index + 1);
          const pageSize = page.getViewport({ scale: 1 });
          sizes.push(pageSize.height / pageSize.width);
          const content = await page.getTextContent();
          const text = pdfPageText(content.items);
          size += text.length;
          if (size > MAX_FIND_TEXT) throw new Error("This PDF has too much text to search. Save it to search in another app.");
          if (pdfFindIncludes(text, query)) found.push(index);
        }
        if (cancelled) return;
        this.ratios = sizes;
        this.matches = found;
        this.update();
        // The first result, once its measured page offsets are laid out.
        if (found.length) this.showPage(found[0]!);
      })()
        .catch((cause: unknown) => {
          if (!cancelled) this.findError = cause instanceof Error ? cause.message : "Could not search this PDF.";
        })
        .finally(() => {
          if (!cancelled) { this.finding = false; this.update(); }
        });
    }, 250);
    this.closeFind = () => { cancelled = true; clearTimeout(timer); };
  }

  private update() {
    const { scale, width, offsets } = this.sizes();
    const pdf = this.pdf, current = this.current();
    this.label.textContent = pdf ? `${current + 1} / ${pdf.numPages} pages` : "PDF";
    this.previous.update({ disabled: !pdf || current === 0 });
    this.next.update({ disabled: !pdf || current + 1 >= pdf.numPages });
    this.zoomOut.update({ disabled: scale <= PDF_MIN_ZOOM });
    this.zoomIn.update({ disabled: scale >= PDF_MAX_ZOOM });
    this.zoomLabel.textContent = this.zoom === null ? "Fit width" : `${Math.round(scale * 100)}%`;
    this.status.hidden = this.nextMatch.element.hidden = !this.query;
    this.status.textContent = this.finding ? "Searching…" : (this.findError ?? `${this.matches.length ? this.match + 1 : 0}/${this.matches.length}`);
    this.nextMatch.update({ disabled: !this.matches.length });
    const state = this.error ? `error:${this.error}` : pdf ? "pages" : "loading";
    if (this.body?.dataset.state !== state) {
      for (const page of this.pages.values()) page.dispose();
      this.pages.clear();
      this.body?.remove();
      this.track = null;
      if (this.error) {
        this.body = surfaceFailure(this.error, () => {
          this.error = null;
          this.pdf = null;
          this.closeLoad?.();
          this.revision++;
          this.load();
          this.update();
        });
      } else if (!pdf) this.body = surfaceLoading();
      else this.body = this.track = h("div", { class: "pdf-track" });
      this.body.dataset.state = state;
      this.scroll.append(this.body);
    }
    const track = this.track;
    if (!track || !pdf) return;
    track.style.height = `max(${offsets.at(-1) ?? 0}px, calc(100% + ${(offsets.at(-2) ?? 16) - 16}px))`;
    track.style.minWidth = `${width + 32}px`;
    const visible = visiblePdfPages(this.ratios, offsets, this.scrollTop, this.viewport.height);
    const shown = new Set(visible.map(({ index }) => index));
    for (const [index, page] of this.pages) if (!shown.has(index)) { page.dispose(); this.pages.delete(index); }
    // In page order, as React orders keyed pages: a selection dragged from one page into the next runs in that order.
    let previous: Element | null = null;
    for (const { index, ratio } of visible) {
      let page = this.pages.get(index);
      if (!page) {
        page = new PdfPage(pdf, index, this.onSize, this.onError);
        this.pages.set(index, page);
      }
      if (page.element.parentElement !== track || page.element.previousElementSibling !== previous) {
        if (previous) previous.after(page.element); else track.prepend(page.element);
      }
      previous = page.element;
      page.layout(offsets[index]!, Math.max(16, (this.viewport.width - width) / 2), width, width * ratio, this.query);
    }
    // The place it was given, once the loaded document is laid out (PdfViewer's restoreView).
    if (this.restored !== pdf) {
      this.restored = pdf;
      const page = Math.min(this.initialView.page, pdf.numPages - 1);
      this.scroll.scrollTo({ top: offsets[page]! - 16 + Math.min(this.initialView.offset, Math.max(0, width * this.ratios[page]! - 1)), left: this.initialView.left });
      this.scrollTop = this.scroll.scrollTop;
      this.saveView();
    }
    // The zoom's scroll anchor, restored once the new width is laid out.
    const anchor = this.zoomAnchor;
    this.zoomAnchor = null;
    if (!anchor) return;
    if (anchor.width !== width) this.scroll.scrollTo({ top: offsets[anchor.page]! - 16 + (anchor.offset * width) / anchor.width });
    this.saveView();
  }

  // The loading task's `destroy` (closeLoad) lets the document go, as the reference's effect does: PDF.js 6's document
  // has no `destroy` of its own.
  dispose() {
    this.closeLoad?.();
    this.closeFind?.();
    this.unobserve();
    for (const page of this.pages.values()) page.dispose();
    this.pages.clear();
    this.pdf = null;
  }
}
