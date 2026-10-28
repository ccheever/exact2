// A PowerPoint deck (MIT reference, see LICENSE-T3: apps/web/src/components/files/office/Presentation.tsx; T3 Code
// 7c3b3b3605ef; adapted there from opencode, MIT, see LICENSE.opencode): "Slide n of N" with previous and next, the
// slides painted by the presentation worker, SVG pictures it cannot decode rasterized here and handed back, hidden
// slides dimmed, and each slide's speaker notes under it.
import { ChevronLeft, ChevronRight } from "../icons";
import { h, surfaceAction, type IconNode } from "../chrome";
import { OfficePages, type PaintPage } from "./office-pages";
import { maxImageSide, type SlideMethods, type SlideSummary, type VectorRequest } from "./slide-protocol";
import { createWorkerClient } from "./worker-rpc";

async function rasterize(request: VectorRequest) {
  const url = URL.createObjectURL(request.blob);
  const image = new Image();
  image.src = url;
  try {
    await image.decode();
    const shrink = Math.min(1, maxImageSide / Math.max(image.naturalWidth, image.naturalHeight));
    const size = request.size ?? { width: Math.max(1, Math.round(image.naturalWidth * shrink)), height: Math.max(1, Math.round(image.naturalHeight * shrink)) };
    return { asset: request.asset, bitmap: await createImageBitmap(image, { resizeWidth: size.width, resizeHeight: size.height, resizeQuality: "high" }) };
  } catch {
    return { asset: request.asset, bitmap: undefined };
  } finally {
    URL.revokeObjectURL(url);
  }
}

export class Presentation {
  readonly element = h("div", { style: "display:flex;flex:1;min-height:0;flex-direction:column" });
  private readonly controller = new AbortController();
  private readonly engine: ReturnType<typeof createWorkerClient<SlideMethods>>;
  private slides: readonly SlideSummary[] = [];
  private page = 0;
  private readonly failed: Record<number, boolean> = {};
  /** Each slide's notes, made once, so a redraw keeps their selection. */
  private readonly footers = new Map<number, Node | null>();
  private pages: OfficePages | null = null;
  private counter: HTMLElement | null = null;
  private previous: ReturnType<typeof surfaceAction> | null = null;
  private next: ReturnType<typeof surfaceAction> | null = null;

  constructor(source: ArrayBuffer, onDetails: (text: string) => void, private readonly onError: (cause: unknown) => void, loading: () => HTMLElement) {
    this.element.append(loading());
    const controller = this.controller;
    const worker = new Worker(new URL("./presentation.worker.js", import.meta.url), { type: "module" });
    const engine = createWorkerClient<SlideMethods>(worker);
    this.engine = engine;
    const fail = (cause: unknown) => { if (!controller.signal.aborted) onError(cause); };
    worker.addEventListener("error", fail);
    worker.addEventListener("messageerror", fail);
    const bytes = source.slice(0);
    void engine.call("open", { bytes }, { transfer: [bytes], signal: controller.signal })
      .then((slides) => {
        if (slides.length === 0 || slides.length > 500) throw new Error("Preview limited to presentations with 1 to 500 slides.");
        this.slides = slides;
        this.mount();
        onDetails(`${slides.length} slides`);
      })
      .catch(fail);
  }

  private readonly paint = async (index: number, canvas: HTMLCanvasElement, scale: number, signal: AbortSignal, again = false): Promise<void> => {
    const result = await this.engine.call("paint", { index, density: (window.devicePixelRatio || 1) * scale }, { key: `slide:${index}`, signal });
    if (signal.aborted || result.kind === "cancelled") {
      if (result.kind === "painted") result.bitmap.close();
      return;
    }
    if (this.failed[index] !== (result.kind === "failed")) {
      this.failed[index] = result.kind === "failed";
      this.pages?.redraw();
    }
    if (result.kind === "failed") return;
    const context = canvas.getContext("bitmaprenderer");
    if (!context) {
      result.bitmap.close();
      throw new Error("This browser cannot draw slides.");
    }
    canvas.width = result.bitmap.width;
    canvas.height = result.bitmap.height;
    context.transferFromImageBitmap(result.bitmap);
    if (again || !result.vectors.length) return;
    const bitmaps = await Promise.all(result.vectors.map(rasterize));
    if (signal.aborted) {
      bitmaps.forEach((value) => value.bitmap?.close());
      return;
    }
    await this.engine.call("vectors", bitmaps, { transfer: bitmaps.flatMap((value) => (value.bitmap ? [value.bitmap] : [])), signal });
    if (!signal.aborted) await this.paint(index, canvas, scale, signal, true);
  };

  private select(index: number) {
    this.page = index;
    this.updateNav();
    this.pages?.scrollTo(index);
  }

  private updateNav() {
    if (this.counter) this.counter.textContent = `Slide ${this.page + 1} of ${this.slides.length}`;
    this.previous?.update({ disabled: this.page === 0 });
    this.next?.update({ disabled: this.page + 1 === this.slides.length });
  }

  private mount() {
    this.previous = surfaceAction({ label: "Previous slide", icon: ChevronLeft as IconNode, onPress: () => this.select(this.page - 1) });
    this.next = surfaceAction({ label: "Next slide", icon: ChevronRight as IconNode, onPress: () => this.select(this.page + 1) });
    this.counter = h("span", { class: "slide-counter", "aria-live": "polite" });
    this.pages = new OfficePages({
      pages: this.slides, paint: this.paint as PaintPage, label: "Slide", onError: this.onError,
      onPageChange: (index) => { if (index !== this.page) { this.page = index; this.updateNav(); } },
      overlay: (index) => [
        ...(this.slides[index]?.hidden ? [h("div", { class: "slide-hidden" })] : []),
        ...(this.failed[index] ? [h("p", { role: "status", class: "slide-failed" }, "This slide could not be drawn.")] : []),
      ],
      footer: (index) => {
        if (this.footers.has(index)) return this.footers.get(index)!;
        const slide = this.slides[index];
        const footer = !slide?.notes && !slide?.hidden ? null : h("div", { class: "slide-footer" },
          slide.hidden ? h("p", {}, "Hidden slide") : null,
          slide.notes ? h("p", { role: "note", "aria-label": "Speaker notes", class: "slide-notes" }, slide.notes) : null);
        this.footers.set(index, footer);
        return footer;
      },
    });
    this.element.replaceChildren(h("div", { class: "subheader" }, this.previous.element, this.counter, this.next.element), this.pages.element);
    this.updateNav();
  }

  dispose() {
    this.controller.abort();
    this.pages?.dispose();
    this.engine.close();
  }
}
