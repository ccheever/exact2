// The file preview page (LLP 1020.001: a bundled page under assets/file-preview/, framed by the attachment body,
// src/panel/viewer-frame.contract). It runs T3 Code's document viewers (MIT reference, see LICENSE-T3:
// apps/web/src/components/files/; T3 Code 7c3b3b3605ef) without React: PdfViewer (PDF.js), LargeTextViewer for text
// over 1 MB and for CSV and TSV tables, OfficeViewer (Word, Excel, PowerPoint, OpenDocument, RTF, EPUB; a notebook is
// parsed here and drawn by the app), BinaryViewer (archives and SQLite; an opened entry is drawn by the app) and a
// notebook's HTML output. The app's subheader keeps the name, Copy and Save; this page draws a viewer's own rows.
// It talks to the app by `postMessage` only, in JSON strings:
// - app → page: config (what to show, the theme, word wrap), bytes (a draft's file, in chunks), pick (a menu's row),
//   copy (Copy contents of a streamed text);
// - page → app: boot, menu (a select's rows at its trigger), copy (the text) or copy-error, retry, link, entry,
//   entry-closed, notebook, height, error.
import "./shared/packed";
import { h, hideTooltip, menuPicked, post, surfaceFailure, surfaceLoading } from "./chrome";

export type Config = {
  readonly type: "config";
  /** pdf, text (LargeTextViewer), office, binary or output (a notebook's HTML output). */
  readonly view: string;
  /** A viewer's identity: another one mounts a new viewer, as the reference's keys do. */
  readonly key: string;
  readonly name: string;
  /** The file's signed URL; "" for a draft, whose bytes follow in `bytes` messages. */
  readonly src: string;
  /** A delimited table's text (DelimitedTablePreview), or an output's HTML. */
  readonly text: string | null;
  /** The size the app knows, or -1 for none (a Files read, whose viewer learns it from the reply). */
  readonly sizeBytes: number;
  readonly kind: string;
  readonly delimiter: "," | "\t" | null;
  readonly wrap: boolean;
  readonly scheme: "light" | "dark";
  readonly sheet: boolean;
  readonly smoothing: boolean;
  readonly revealLine: number | null;
  readonly revealRequestId: number;
};

/** What a mounted viewer offers the page. */
export type Viewer = {
  readonly element: HTMLElement;
  dispose(): void;
  setWrap?(wrap: boolean): void;
  reveal?(line: number | null, id: number): void;
  copy?(): Promise<string>;
  setScheme?(scheme: "light" | "dark"): void;
};

type Message = { readonly type: string; readonly [key: string]: unknown };

/** A message the page takes: from its app (on Apple the host, through the page's own window). */
export function fromApp(event: Pick<MessageEvent, "source" | "origin" | "data">, view: { parent: unknown; location: { origin: string } }, self: unknown): boolean {
  return typeof event.data === "string" && event.source === view.parent && (view.parent === self || event.origin === view.location.origin);
}

/** A draft's bytes, base64 in order, until `done`. */
class Incoming {
  private parts: Uint8Array<ArrayBuffer>[] = [];
  constructor(readonly key: string) {}
  add(data: string) {
    const binary = atob(data);
    const bytes = new Uint8Array(binary.length);
    for (let index = 0; index < binary.length; index++) bytes[index] = binary.charCodeAt(index);
    this.parts.push(bytes);
  }
  url() {
    return URL.createObjectURL(new Blob(this.parts));
  }
}

/**
 * The page's own requests reach only the origin its file comes from, once one is known (a second policy narrows the
 * first, CSP3 §4.1). The static policy allows any http(s) origin because an environment's is known only at run time.
 * A worker started from a URL takes no policy from its document, so this bounds the page, not its workers.
 */
export function narrowConnections(src: string, at: Document = document): void {
  if (at.head.querySelector("meta[data-connections]")) return;
  let origin: string;
  try {
    const url = new URL(src, at.location.href);
    if (url.protocol !== "http:" && url.protocol !== "https:") return;
    origin = url.origin;
  } catch {
    return;
  }
  const meta = at.createElement("meta");
  meta.httpEquiv = "Content-Security-Policy";
  meta.content = `connect-src 'self' ${origin} blob: data:`;
  meta.dataset.connections = origin;
  at.head.append(meta);
}

async function mount(config: Config, src: string, retry: () => void): Promise<Viewer> {
  if (config.view === "pdf") {
    const { PdfViewer } = await import("./pdf/pdf-viewer");
    return new PdfViewer(src, config.name);
  }
  if (config.view === "text") {
    const { LargeTextViewer } = await import("./text/large-text-viewer");
    return new LargeTextViewer({
      name: config.name, ...(config.text !== null ? { text: config.text } : { src }), ...(config.sizeBytes >= 0 ? { sizeBytes: config.sizeBytes } : {}),
      delimiter: config.delimiter, wrap: config.wrap, revealLine: config.revealLine, revealRequestId: config.revealRequestId, onRetry: retry,
    });
  }
  if (config.view === "office") {
    const { officeViewer } = await import("./office/office-viewer");
    return officeViewer({ src, name: config.name, kind: config.kind, sizeBytes: config.sizeBytes, scheme: config.scheme });
  }
  if (config.view === "binary") {
    const { BinaryViewer } = await import("./binary/binary-viewer");
    return new BinaryViewer({ src, name: config.name, kind: config.kind === "sqlite" ? "sqlite" : "archive", onRetry: retry });
  }
  if (config.view === "output") {
    const { OutputDocument } = await import("./office/document-pages");
    return new OutputDocument(config.text ?? "", config.name, config.scheme);
  }
  throw new Error(`No viewer for ${config.view}`);
}

export class Page {
  private config: Config | null = null;
  private viewer: Viewer | null = null;
  private incoming: Incoming | null = null;
  private objectUrl: string | null = null;
  private serial = 0;

  constructor(private readonly root: HTMLElement) {}

  receive(message: Message) {
    if (message.type === "config") this.configure(message as unknown as Config);
    else if (message.type === "bytes") this.bytes(message);
    else if (message.type === "pick") menuPicked(Number(message.serial), String(message.id ?? ""));
    else if (message.type === "copy") this.copy();
  }

  private configure(config: Config) {
    const previous = this.config;
    this.config = config;
    const root = document.documentElement;
    root.style.colorScheme = config.scheme;
    root.dataset.scheme = config.scheme;
    root.dataset.smoothing = config.smoothing ? "on" : "off";
    if (config.sheet) delete root.dataset.previewPanelMode; else root.dataset.previewPanelMode = "inline";
    if (previous?.key === config.key) {
      if (previous.wrap !== config.wrap) this.viewer?.setWrap?.(config.wrap);
      if (previous.revealLine !== config.revealLine || previous.revealRequestId !== config.revealRequestId) this.viewer?.reveal?.(config.revealLine, config.revealRequestId);
      if (previous.scheme !== config.scheme) this.viewer?.setScheme?.(config.scheme);
      return;
    }
    this.unmount();
    if (!config.view) return;
    if (!config.src && config.text === null) {
      // A draft: its bytes come next.
      this.incoming = new Incoming(config.key);
      this.show(surfaceLoading());
      return;
    }
    this.start(config, config.src);
  }

  private bytes(message: Message) {
    const incoming = this.incoming, config = this.config;
    if (!incoming || !config || message.key !== incoming.key) return;
    if (typeof message.data === "string" && message.data) incoming.add(message.data);
    if (message.done === true) {
      this.incoming = null;
      this.objectUrl = incoming.url();
      this.start(config, this.objectUrl);
    }
  }

  private start(config: Config, src: string) {
    const serial = ++this.serial;
    const retry = () => post({ type: "retry", key: config.key });
    narrowConnections(src);
    this.show(surfaceLoading());
    void mount(config, src, retry)
      .then((viewer) => {
        if (serial !== this.serial) { viewer.dispose(); return; }
        this.viewer = viewer;
        this.show(viewer.element);
      })
      .catch((cause: unknown) => {
        // A viewer that cannot load (its chunk, its engine) says so where the spinner was, with Try again.
        const message = cause instanceof Error && cause.message ? cause.message : "The preview could not be loaded.";
        if (serial === this.serial) this.show(surfaceFailure(message, retry));
        post({ type: "error", message });
      });
  }

  private show(element: HTMLElement) {
    hideTooltip();
    this.root.replaceChildren(element);
  }

  private unmount() {
    this.serial++;
    this.viewer?.dispose();
    this.viewer = null;
    this.incoming = null;
    if (this.objectUrl) URL.revokeObjectURL(this.objectUrl);
    this.objectUrl = null;
    this.show(h("div"));
  }

  private copy() {
    const viewer = this.viewer, key = this.config?.key;
    if (!viewer?.copy) {
      post({ type: "copy-error", key, message: "Wait for the file to finish loading before copying." });
      return;
    }
    void viewer.copy().then(
      (text) => post({ type: "copy", key, text }),
      (cause: unknown) => post({ type: "copy-error", key, message: cause instanceof Error ? cause.message : "Try again." }),
    );
  }
}

function boot(): void {
  const root = document.getElementById("viewer") ?? document.body.appendChild(h("div", { id: "viewer" }));
  const page = new Page(root);
  window.addEventListener("message", (event) => {
    if (!fromApp(event, window, window)) return;
    let message: Message;
    try { message = JSON.parse(event.data) as Message; } catch { return; }
    if (message && typeof message.type === "string") page.receive(message);
  });
  window.addEventListener("error", (event) => post({ type: "error", message: event.message }));
  window.addEventListener("unhandledrejection", (event) =>
    post({ type: "error", message: event.reason instanceof Error ? event.reason.message : String(event.reason) }),
  );
  post({ type: "boot" });
}

if (typeof document !== "undefined" && typeof window !== "undefined" && !("Bun" in globalThis)) boot();
