// A document as sanitized HTML (MIT reference, see LICENSE-T3: apps/web/src/components/files/DocumentPages.tsx; T3 Code
// 7c3b3b3605ef): OpenDocument, RTF and EPUB pages, and a notebook's HTML output, in a script-less frame of their own,
// in the surface's colours, with reading-size buttons for a page. An output measures its body, and the page tells the
// app its height (`height`), so the app's notebook sizes its box.
import DOMPurify from "dompurify";
import { Minus, Plus, RotateCcw } from "../icons";
import { h, observeResize, post, surfaceAction, type IconNode } from "../chrome";

export function sanitizeDocumentHtml(source: string): string {
  const fragment = DOMPurify.sanitize(source, {
    RETURN_DOM_FRAGMENT: true,
    ALLOWED_TAGS: ["p", "h1", "h2", "h3", "h4", "h5", "h6", "div", "span", "strong", "b", "em", "i", "u", "s", "ul", "ol", "li", "blockquote", "pre", "code",
      "table", "thead", "tbody", "tr", "th", "td", "caption", "figure", "figcaption", "br", "hr", "sub", "sup", "dl", "dt", "dd", "img"],
    ALLOWED_ATTR: ["src", "alt", "colspan", "rowspan", "data-odf-whitespace"],
  });
  for (const image of fragment.querySelectorAll("img")) {
    if (!/^data:image\/(png|jpeg|gif|webp);base64,[a-z\d+/]*={0,2}$/i.test(image.getAttribute("src") ?? "")) image.remove();
  }
  const container = document.createElement("div");
  container.append(fragment);
  return container.innerHTML;
}

/** The theme tokens the frame cannot inherit across its sandbox. */
function documentColors() {
  const style = getComputedStyle(document.documentElement);
  return ["background", "foreground", "card", "border"].map((token) => style.getPropertyValue(`--${token}`));
}

function srcDoc(html: string, output: boolean, zoom: number, scheme: string) {
  const colors = documentColors();
  return `<!doctype html><html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src data:; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'"><style>:root{color-scheme:${scheme}}html{background:${colors[0]};color:${colors[1]};font:${15 * zoom}px/1.65 system-ui,sans-serif}body{box-sizing:border-box;margin:${output ? "0" : "20px auto"};padding:${output ? "16px" : "32px"};width:${output ? "100%" : "calc(100% - 40px)"};max-width:${860 * zoom}px;min-height:${output ? "0" : "calc(100vh - 40px)"};display:flow-root;background:${colors[2]};overflow-wrap:anywhere}h1,h2,h3{line-height:1.25}img{max-width:100%;height:auto}table{border-collapse:collapse;min-width:100%;max-width:100%}td,th{border:1px solid ${colors[3]};padding:6px 12px;text-align:left}th{background:${colors[0]}}tr:nth-child(even){background:${colors[0]}}pre,span[data-odf-whitespace]{white-space:pre-wrap}*{scrollbar-width:thin}${output ? "" : "@media(max-width:420px){body{padding:20px;width:calc(100% - 24px);margin:12px auto}}"}</style></head><body>${html}</body></html>`;
}

/** DocumentPages for a page: its details, the reading size, and the document. */
export class DocumentPages {
  readonly element = h("div", { style: "display:contents" });
  private zoom = 1;
  private readonly frame: HTMLIFrameElement;
  private readonly html: string;
  private readonly zoomLabel = h("span", { class: "zoom" }, "100%");
  private readonly smaller; private readonly larger; private readonly reset;

  /** `zoom` is the reading size the viewer had, which `onZoom` hears change: a page keeps it as the next is shown. */
  constructor(html: string, name: string, details: string, private scheme: string, zoom = 1, private readonly onZoom: (zoom: number) => void = () => {}) {
    this.html = sanitizeDocumentHtml(html);
    this.smaller = surfaceAction({ label: "Decrease reading size", icon: Minus as IconNode, onPress: () => this.setZoom(Math.max(0.5, this.zoom - 0.25)) });
    this.larger = surfaceAction({ label: "Increase reading size", icon: Plus as IconNode, onPress: () => this.setZoom(Math.min(2, this.zoom + 0.25)) });
    this.reset = surfaceAction({ label: "Reset reading size", icon: RotateCcw as IconNode, onPress: () => this.setZoom(1) });
    this.frame = h("iframe", { title: name, sandbox: "", class: "document-frame" });
    this.element.append(
      h("div", { class: "reading" }, h("span", { class: "details", role: "status" }, details), this.smaller.element, this.zoomLabel, this.larger.element, this.reset.element),
      this.frame,
    );
    this.setZoom(zoom);
  }

  private setZoom(zoom: number) {
    if (zoom !== this.zoom) this.onZoom(zoom);
    this.zoom = zoom;
    this.zoomLabel.textContent = `${Math.round(zoom * 100)}%`;
    this.smaller.update({ disabled: zoom <= 0.5 });
    this.larger.update({ disabled: zoom >= 2 });
    this.reset.update({ disabled: zoom === 1 });
    this.frame.srcdoc = srcDoc(this.html, false, zoom, this.scheme);
  }

  setScheme(scheme: "light" | "dark") {
    this.scheme = scheme;
    this.setZoom(this.zoom);
  }
}

/** DocumentPages for a notebook's HTML output: the document alone, its height reported as it changes. */
export class OutputDocument {
  readonly element: HTMLIFrameElement;
  private stopObserving: (() => void) | null = null;
  private readonly html: string;

  constructor(html: string, name: string, private scheme: "light" | "dark") {
    this.html = sanitizeDocumentHtml(html);
    // Same-origin access lets the page measure saved output; scripts remain blocked.
    this.element = h("iframe", { title: name, sandbox: "allow-same-origin", class: "output-frame" });
    this.element.addEventListener("load", () => {
      this.stopObserving?.();
      const body = this.element.contentDocument?.body;
      if (!body) return;
      const measure = () => {
        const height = Math.ceil(body.getBoundingClientRect().height);
        this.element.style.height = `${height}px`;
        post({ type: "height", value: height });
      };
      this.stopObserving = observeResize(body, measure);
      measure();
    });
    this.element.srcdoc = srcDoc(this.html, true, 1, scheme);
  }

  setScheme(scheme: "light" | "dark") {
    if (scheme === this.scheme) return;
    this.scheme = scheme;
    this.element.srcdoc = srcDoc(this.html, true, 1, scheme);
  }

  dispose() {
    this.stopObserving?.();
  }
}
