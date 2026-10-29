// A Word document (MIT reference, see LICENSE-T3: apps/web/src/components/files/office/WordDocument.tsx; T3 Code
// 7c3b3b3605ef; adapted there from opencode, MIT, see LICENSE.opencode): the document worker lays the pages out and
// paints them; each page near the viewport carries a transparent text layer for selection and links.
import { h, post, tooltip } from "../chrome";
import { OfficePages, type PaintPage } from "./office-pages";
import { createWorkerClient } from "./worker-rpc";
import { layerFont, lineHeight, type WordLink, type WordMethods, type WordPageSize, type WordSpan } from "./word-protocol";

type Engine = ReturnType<typeof createWorkerClient<WordMethods>>;

export class WordDocument {
  readonly element = h("div", { style: "display:flex;flex:1;min-height:0;flex-direction:column" });
  private readonly controller = new AbortController();
  private readonly engine: Engine;
  private pages: OfficePages | null = null;
  private opened: readonly WordPageSize[] | null = null;
  /**
   * WordTextLayer per page near the viewport: the page it asked for, its spans, and the nodes drawn from them, kept
   * until they change; a page that leaves takes its layer with it, as the reference unmounts one.
   */
  private readonly layers = new Map<number, { page: WordPageSize; spans: readonly WordSpan[] | null; controller: AbortController; root: HTMLElement | null }>();

  constructor(bytes: ArrayBuffer, private readonly onDetails: (text: string) => void, private readonly onError: (cause: unknown) => void, loading: () => HTMLElement) {
    this.element.append(loading());
    const worker = new Worker(new URL("./document.worker.js", import.meta.url), { type: "module" });
    const engine = createWorkerClient<WordMethods>(worker);
    this.engine = engine;
    const controller = this.controller;
    const fail = (error: unknown) => { if (!controller.signal.aborted) onError(error); };
    worker.addEventListener("error", fail);
    worker.addEventListener("messageerror", fail);
    const buffer = bytes.slice(0);
    void engine.call("open", { bytes: buffer }, { transfer: [buffer], signal: controller.signal })
      .then(async (first) => {
        if (controller.signal.aborted) return;
        this.show(first.pages);
        onDetails(`${first.pages.length}${first.loading ? "+" : ""} pages`);
        if (!first.loading) return;
        await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
        while (!controller.signal.aborted) {
          const step = await engine.call("layout", undefined, { signal: controller.signal });
          if (!step.done) continue;
          if (step.pages.length > 1000) throw new Error("Preview limited to 1,000 pages.");
          this.show(step.pages);
          onDetails(`${step.pages.length} pages`);
          break;
        }
      })
      .catch(fail);
  }

  private readonly paint: PaintPage = async (index, canvas, scale, signal) => {
    const bitmap = await this.engine.call("paint", { index, scale, ratio: window.devicePixelRatio || 1 }, { key: `paint:${index}`, signal });
    void this.engine.call("build", { index: index + 1 }, { key: "build", signal }).catch(() => undefined);
    if (!bitmap) return;
    const context = canvas.getContext("bitmaprenderer");
    if (!context || signal.aborted) { bitmap.close(); return; }
    canvas.width = bitmap.width;
    canvas.height = bitmap.height;
    context.transferFromImageBitmap(bitmap);
  };

  private show(pages: readonly WordPageSize[]) {
    for (const [index, layer] of this.layers) if (index >= pages.length) { layer.controller.abort(); this.layers.delete(index); }
    this.opened = pages;
    const props = { pages, paint: this.paint, label: "Page", onError: this.onError, overlay: (index: number) => this.overlay(index), release: this.release };
    if (this.pages) this.pages.update(props);
    else {
      this.pages = new OfficePages(props);
      this.element.replaceChildren(this.pages.element);
    }
  }

  private follow(link: WordLink) {
    if (link.kind === "external") {
      post({ type: "link", url: link.url });
      return;
    }
    void this.engine.call("bookmark", { name: link.bookmark })
      .then((target) => { if (target) this.pages?.scrollTo(target.index, target.y); })
      .catch(this.onError);
  }

  private readonly release = (index: number) => {
    this.layers.get(index)?.controller.abort();
    this.layers.delete(index);
  };

  /**
   * WordTextLayer: the page's spans, asked of the worker when the page nears the viewport, and again for the full
   * layout's page (its effect on `page`); the spans shown stay until the new ones come.
   */
  private overlay(index: number): Node[] {
    const page = this.opened?.[index];
    if (!page) return [];
    let layer = this.layers.get(index);
    if (layer?.page !== page) {
      layer?.controller.abort();
      const controller = new AbortController();
      const asked = { page, spans: layer?.spans ?? null, controller, root: layer?.root ?? null };
      this.layers.set(index, asked);
      void this.engine.call("text", { index }, { signal: controller.signal })
        .then((spans) => {
          if (this.layers.get(index) !== asked) return;
          asked.spans = spans;
          asked.root = null;
          this.pages?.redraw();
        })
        .catch(() => undefined);
      layer = asked;
    }
    if (!layer.root) layer.root = this.textLayer(layer.spans ?? []);
    return [layer.root];
  }

  /** WordTextLayer's spans, transparent over the page for selection and links. */
  private textLayer(spans: readonly WordSpan[]): HTMLElement {
    const root = h("div", { class: "word-text" });
    root.style.fontFamily = layerFont;
    root.style.lineHeight = String(lineHeight);
    for (const span of spans) {
      const text = h("span", { dir: span.dir, role: span.link ? "link" : undefined, tabindex: span.link ? 0 : undefined, "aria-label": span.title }, span.text);
      Object.assign(text.style, { left: `${span.left * 100}%`, top: `${span.top * 100}%`, fontSize: `${span.size * 100}cqw`, transform: span.transform, cursor: span.link ? "pointer" : "text" });
      const link = span.link;
      if (link) {
        text.addEventListener("click", (event) => { if (event.button === 0 && (getSelection()?.isCollapsed ?? true)) this.follow(link); });
        text.addEventListener("keydown", (event) => { if (event.key === "Enter") { event.preventDefault(); this.follow(link); } });
      }
      if (span.title) { const title = span.title; tooltip(text, () => title); }
      root.append(text);
      if (span.after === "br") root.append(h("br"));
      else if (span.after === "tab") root.append("\t");
    }
    return root;
  }

  dispose() {
    this.controller.abort();
    for (const layer of this.layers.values()) layer.controller.abort();
    this.pages?.dispose();
    this.engine.close();
  }
}
