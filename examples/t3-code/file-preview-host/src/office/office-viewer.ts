// Office, OpenDocument, RTF, EPUB and notebook files (MIT reference, see LICENSE-T3: apps/web/src/components/files/
// OfficeFilePreview.tsx, OfficeViewer.tsx and OpenXmlOfficeViewer.tsx; T3 Code 7c3b3b3605ef). Word, Excel and
// PowerPoint files are prepared by the Office worker (office-archive.ts) and drawn by their engines; the other kinds
// become pages of sanitized HTML, a workbook of cell values, or a notebook's cells, which the app draws itself.
import { ChevronLeft, ChevronRight } from "../icons";
import { h, noPreview, post, sizeSuffix, surfaceAction, surfaceLoading, surfaceNotice, selectTrigger, type IconNode, type MenuRow } from "../chrome";
import { formatAttachmentSize, type OfficePreviewKind } from "../shared/file-preview";
import { readPreviewFile } from "../shared/read-bounded";
import { VirtualTable } from "../text/virtual-table";
import { DocumentPages } from "./document-pages";
import { OFFICE_PREVIEW_MAX_BYTES, type OfficeWorkerResult } from "./office-preview-protocol";

type Props = { readonly src: string; readonly name: string; readonly kind: string; readonly sizeBytes: number; readonly scheme: "light" | "dark" };
type Workbook = Extract<OfficeWorkerResult, { type: "ods" }>;
type Prepared = Extract<OfficeWorkerResult, { bytes: ArrayBuffer }>;
type Engine = { readonly element: HTMLElement; dispose(): void };

const officeWorker = () => new Worker(new URL("./office-preview.worker.js", import.meta.url), { type: "module" });

/** OfficeViewer: Word, Excel and PowerPoint open in their engines; the rest in the extended viewer. */
export function officeViewer(props: Props) {
  return props.kind === "docx" || props.kind === "xlsx" || props.kind === "pptx" ? new OpenXmlOfficeViewer(props) : new ExtendedOfficeViewer(props);
}

/** OpenXmlOfficeViewer: the file prepared in the Office worker, then its engine, with "<n> pages · Fit to width". */
class OpenXmlOfficeViewer {
  readonly element = h("div", { style: "display:flex;flex:1;min-height:0;flex-direction:column" });
  private readonly controller = new AbortController();
  private engine: Engine | null = null;
  private failed = false;
  private readonly details = h("span", { class: "office-details", "aria-live": "polite" });
  private readonly detailsRow = h("div", { class: "subheader", hidden: true }, this.details);

  constructor(private readonly props: Props) {
    this.element.append(surfaceLoading());
    if (props.sizeBytes > OFFICE_PREVIEW_MAX_BYTES) {
      // Metadata bounds are checked before fetching or starting an engine.
      this.fail(new Error("Preview limited to files under 50 MB."));
      return;
    }
    const controller = this.controller;
    const worker = officeWorker();
    const failure = (cause: unknown) => {
      if (!controller.signal.aborted) this.fail(cause);
      controller.abort();
      worker.terminate();
    };
    const timer = setTimeout(() => failure(new Error("This document took too long to preview. Open it in another app.")), 30000);
    controller.signal.addEventListener("abort", () => { worker.terminate(); clearTimeout(timer); });
    worker.addEventListener("error", () => failure(new Error("Could not load the Office viewer.")));
    worker.addEventListener("messageerror", () => failure(new Error("Could not read the Office viewer response.")));
    worker.addEventListener("message", (event: MessageEvent<OfficeWorkerResult>) => {
      if (controller.signal.aborted) return;
      clearTimeout(timer);
      if (event.data.type === "error") failure(new Error(event.data.message));
      else if ("bytes" in event.data) {
        void this.open(event.data);
        worker.terminate();
      }
    });
    void readPreviewFile(props.src, controller.signal, OFFICE_PREVIEW_MAX_BYTES)
      .then((bytes) => { if (!controller.signal.aborted) worker.postMessage({ type: "open", kind: props.kind, bytes }, [bytes]); })
      .catch(failure);
  }

  private fail(cause: unknown) {
    if (this.failed) return;
    this.failed = true;
    this.engine?.dispose();
    this.engine = null;
    this.element.replaceChildren(noPreview(cause instanceof Error ? cause.message : "Could not render this Office document."));
  }

  private readonly onDetails = (text: string) => {
    this.details.textContent = `${text}${this.props.kind !== "xlsx" ? " · Fit to width" : ""}`;
    this.detailsRow.hidden = !text;
  };

  private async open(prepared: Prepared) {
    const onError = (cause: unknown) => this.fail(cause);
    const loading = () => surfaceLoading();
    const engine: Engine = this.props.kind === "docx"
      ? new (await import("./word-document")).WordDocument(prepared.bytes, this.onDetails, onError, loading)
      : this.props.kind === "xlsx"
        ? new (await import("./spreadsheet")).Spreadsheet(prepared.bytes, this.onDetails, onError, loading)
        : new (await import("./presentation")).Presentation(prepared.bytes, this.onDetails, onError, loading);
    if (this.failed || this.controller.signal.aborted) { engine.dispose(); return; }
    this.engine = engine;
    this.element.replaceChildren(this.detailsRow,
      ...(prepared.truncated ? [surfaceNotice("Preview limited. Save it or open it in another app to read the full document.")] : []), engine.element);
  }

  dispose() {
    this.controller.abort();
    this.engine?.dispose();
  }
}

/** ExtendedOfficeViewer: OpenDocument, RTF and EPUB pages, an OpenDocument workbook, or a notebook. */
class ExtendedOfficeViewer {
  readonly element = h("div", { style: "display:flex;flex:1;min-height:0;flex-direction:column" });
  private readonly controller = new AbortController();
  private worker: Worker | null = null;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private error: string | null = null;
  private sheetPages = new Map<string, string[][]>();
  private readonly pendingPages = new Set<string>();
  private previewDocument: Extract<OfficeWorkerResult, { type: "document" }> | null = null;
  private notebook: Extract<OfficeWorkerResult, { type: "notebook" }> | null = null;
  private workbook: Workbook | null = null;
  private count = 0;
  private fileSize: number | undefined;
  private page = 0;
  /** DocumentPages' reading size, kept from page to page as the reference's one component keeps it. */
  private zoom = 1;
  private table: VirtualTable | null = null;
  private shownKey = "";

  constructor(private props: Props) {
    this.fileSize = props.sizeBytes > 0 ? props.sizeBytes : undefined;
    const controller = this.controller;
    if (props.sizeBytes > OFFICE_PREVIEW_MAX_BYTES) {
      // The file metadata can reject the preview before loading bytes or engines.
      this.error = "Preview limited to files under 50 MB.";
      this.render();
      return;
    }
    const worker = officeWorker();
    this.worker = worker;
    const fail = (message: string) => {
      if (!controller.signal.aborted) { this.error = message; this.render(); }
      controller.abort();
      worker.terminate();
      if (this.timer) clearTimeout(this.timer);
    };
    this.timer = setTimeout(() => fail("This document took too long to preview. Open it in another app."), 30_000);
    worker.addEventListener("error", () => fail("Could not load the Office viewer. Close the file and try again."));
    worker.addEventListener("message", (event: MessageEvent<OfficeWorkerResult>) => {
      if (controller.signal.aborted) return;
      const result = event.data;
      if (result.type === "error") fail(result.message);
      else if (result.type === "document") {
        if (this.timer) clearTimeout(this.timer);
        this.previewDocument = result;
        this.count = result.pages.length > 1 ? result.pages.length : 0;
        this.render();
      } else if (result.type === "notebook") {
        if (this.timer) clearTimeout(this.timer);
        this.notebook = result;
        this.render();
      } else if (result.type === "ods") {
        if (this.timer) clearTimeout(this.timer);
        this.workbook = result;
        this.count = result.sheets.length;
        this.render();
      } else if (result.type === "sheet-page") {
        const key = `${result.sheet}:${result.index}`;
        this.pendingPages.delete(key);
        const next = new Map(this.sheetPages);
        next.set(key, result.rows);
        if (next.size > 12) next.delete(next.keys().next().value!);
        this.sheetPages = next;
        this.render();
      }
    });
    void readPreviewFile(props.src, controller.signal, OFFICE_PREVIEW_MAX_BYTES)
      .then((bytes) => {
        if (!controller.signal.aborted) {
          this.fileSize = bytes.byteLength;
          worker.postMessage({ type: "open", kind: props.kind as OfficePreviewKind, bytes }, [bytes]);
        }
      })
      .catch((cause: unknown) => fail(cause instanceof Error ? cause.message : "Could not read this file."));
    this.render();
  }

  private selectPage(index: number) {
    this.page = index;
    this.render();
  }

  private render() {
    if (this.error) {
      this.show("error", () => [noPreview(this.error!)]);
      return;
    }
    if (this.notebook) {
      // The app draws the cells (src/panel/notebook-preview.contract); the page only parsed them.
      const notebook = this.notebook;
      this.show("notebook", () => {
        post({ type: "notebook", language: notebook.language, cells: notebook.cells, sizeBytes: this.fileSize ?? null });
        return [];
      });
      return;
    }
    const { workbook, previewDocument, page, count, props } = this;
    const sheet = workbook?.sheets[page];
    const documentPage = previewDocument?.pages[page];
    if (!documentPage && !sheet) {
      this.show("loading", () => [surfaceLoading()]);
      return;
    }
    const navigation = workbook ? "sheet" : (previewDocument?.navigation ?? "slide");
    const documentCount = previewDocument?.pages.length ?? 0;
    const documentDetails = navigation === "chapter" ? `Chapter ${page + 1} of ${documentCount}` : `${documentCount} ${navigation}${documentCount === 1 ? "" : "s"}`;
    const size = sizeSuffix(this.fileSize, formatAttachmentSize);
    this.show(`page:${page}:${this.fileSize}:${this.props.scheme}`, () => {
      const nodes: Node[] = [];
      if (count > 0) {
        const previous = surfaceAction({ label: `Previous ${navigation}`, icon: ChevronLeft as IconNode, disabled: page === 0, onPress: () => this.selectPage(page - 1) });
        const next = surfaceAction({ label: `Next ${navigation}`, icon: ChevronRight as IconNode, disabled: page + 1 === count, onPress: () => this.selectPage(page + 1) });
        let middle: HTMLElement;
        if (workbook || previewDocument?.navigation === "chapter") {
          const label = workbook ? "Sheet" : "Chapter";
          const titles = workbook ? workbook.sheets.map((item) => item.name) : previewDocument!.pages.map((item) => item.title);
          const rows = (): MenuRow[] => titles.map((title, index) => ({ id: String(index), label: title, selected: index === page }));
          middle = h("div", { class: "grow" }, selectTrigger({ label, value: titles[page] ?? "", rows, onPick: (id) => this.selectPage(Number(id)) }).element);
        } else middle = h("span", { class: "counter", "aria-live": "polite" }, `${navigation === "slide" ? "Slide" : "Page"} ${page + 1} of ${count}`);
        nodes.push(h("div", { class: "office-nav" }, previous.element, middle, next.element));
      }
      if (props.kind === "ods" && sheet) {
        nodes.push(h("div", { class: "status-line", role: "status" }, `Sheet ${page + 1} of ${count} · ${sheet.rowCount.toLocaleString()} rows · ${sheet.columns.toLocaleString()} columns${size}`));
      }
      if (documentPage) {
        if (props.kind !== "epub") nodes.push(surfaceNotice("Text and images. Layout and embedded objects may differ from the original."));
        nodes.push(new DocumentPages(documentPage.html, `${props.name}: ${documentPage.title}`, `${documentDetails}${size}`, props.scheme, this.zoom, (zoom) => { this.zoom = zoom; }).element);
      } else if (sheet) {
        nodes.push(surfaceNotice(`Cell values only. Charts, images, and formatting are not shown.${sheet.truncated ? (props.kind === "ods" ? " Limited to 500 rows and 50 columns." : " Preview limited to 250,000 rows, 1,024 columns, and bounded cell text.") : ""}`));
        if (sheet.rowCount === 0) nodes.push(h("p", { class: "empty-note" }, "This sheet is empty."));
      }
      return nodes;
    });
    // The sheet's table outlives the sheet pages its rows arrive in.
    if (sheet && sheet.rowCount > 0) {
      const tableProps = {
        name: `${props.name}: ${sheet.name}`, count: sheet.rowCount, columns: sheet.columns,
        getRows: (index: number) => (index === 0 ? sheet.rows : this.sheetPages.get(`${page}:${index}`)),
        loadPage: (index: number) => {
          const key = `${page}:${index}`;
          if (this.pendingPages.has(key)) return;
          this.pendingPages.add(key);
          this.worker?.postMessage({ type: "sheet-page", sheet: page, index });
        },
      };
      if (!this.table) {
        this.table = new VirtualTable(tableProps);
        this.element.append(this.table.element);
      } else this.table.update(tableProps);
    }
  }

  /** Replaces what the viewer shows when `key` changes; the sheet table is kept per sheet. */
  private show(key: string, make: () => Node[]) {
    if (key === this.shownKey) return;
    this.shownKey = key;
    this.table?.dispose();
    this.table = null;
    this.element.replaceChildren(...make());
  }

  setScheme(scheme: "light" | "dark") {
    // The document frame takes the palette when it is made.
    this.props = { ...this.props, scheme };
    this.render();
  }

  dispose() {
    this.controller.abort();
    this.worker?.terminate();
    this.worker = null;
    if (this.timer) clearTimeout(this.timer);
    this.table?.dispose();
  }
}
