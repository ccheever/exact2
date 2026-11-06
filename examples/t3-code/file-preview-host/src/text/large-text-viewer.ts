// Text past 1 MB, and CSV and TSV tables (MIT reference, see LICENSE-T3: apps/web/src/components/files/LargeTextViewer.tsx
// and DelimitedTablePreview.tsx; T3 Code 7c3b3b3605ef): the first screen parses here from a 16 KB range, then a worker
// (large-text.worker.ts) indexes the whole file, up to 256 MB and 5 million rows, and serves pages of 32 rows to the
// virtual table. Copy contents asks the worker for the whole text.
import { h, sizeSuffix, surfaceFailure, surfaceLoading, surfaceNotice } from "../chrome";
import { parseDelimitedPreview } from "../shared/delimited-preview";
import { formatAttachmentSize } from "../shared/file-preview";
import { PREVIEW_PAGE_ROWS, type TextPreviewResult } from "./large-text-protocol";
import { VirtualTable, type VirtualTableProps } from "./virtual-table";

const FIRST_SCREEN_BYTES = 16 * 1024;
/**
 * The most Copy contents carries. The text goes to the clipboard through the app's TypeScript, whose engine has a
 * 64 MiB heap (docs/reference.md, globals on Hermes) and holds the message and its text at once; the reference copies
 * up to its 256 MB preview limit.
 */
export const COPY_LIMIT_BYTES = 8 * 1024 * 1024;

export function firstScreen(text: string, delimiter: "," | "\t" | null, complete: boolean) {
  if (!text || text.includes("\0")) return [];
  const rows = delimiter
    ? parseDelimitedPreview(text, delimiter, { maxRows: PREVIEW_PAGE_ROWS * 2 + 1, maxColumns: 1024, maxCellLength: FIRST_SCREEN_BYTES }).rows
    : text.replace(/\r?\n$|\r$/, "").split(/\r\n|\n|\r/).slice(0, PREVIEW_PAGE_ROWS * 2 + 1).map((line) => [line]);
  // A byte range can end inside a quoted record or a UTF-8 character.
  if (!complete) rows.pop();
  return rows.slice(0, PREVIEW_PAGE_ROWS * 2);
}

export type LargeTextProps = {
  readonly name: string;
  readonly src?: string | undefined;
  readonly text?: string | undefined;
  readonly sizeBytes?: number | undefined;
  readonly onRetry?: (() => void) | undefined;
  readonly delimiter: "," | "\t" | null;
  readonly revealLine?: number | null;
  readonly revealRequestId?: number;
  readonly wrap: boolean;
};

type Meta = Extract<TextPreviewResult, { type: "meta" }>;

export class LargeTextViewer {
  readonly element = h("div", { class: "large-text-host" });
  private meta: Meta | null = null;
  private pages = new Map<number, string[][]>();
  private error: string | null = null;
  private worker: Worker | null = null;
  private copyPending: { resolve: (text: string) => void; reject: (cause: Error) => void } | null = null;
  private readonly pending = new Set<number>();
  private readonly controller = new AbortController();
  private table: VirtualTable | null = null;
  private body: HTMLElement | null = null;
  private status: HTMLElement | null = null;
  private loadingRest: HTMLElement | null = null;
  private columnsNotice: HTMLElement | null = null;
  /** The worker has measured the file (its first `meta`). */
  private measured = false;
  /** Settles once the worker has measured the file, or the viewer has failed or gone. */
  private settled!: () => void;
  private readonly measuredOrGone = new Promise<void>((resolve) => { this.settled = resolve; });

  constructor(private props: LargeTextProps) {
    this.element.style.cssText = "display:flex;flex:1;min-height:0;flex-direction:column";
    this.start();
    this.render();
  }

  /** The effect that owns the prefix fetch and the indexing worker. */
  private start() {
    const props = this.props, controller = this.controller;
    let indexed = false;
    let prefetched = false;
    const paintFirstScreen = (text: string, complete: boolean) => {
      if (indexed || controller.signal.aborted) return;
      const rows = firstScreen(text, props.delimiter, complete);
      if (!rows.length) return;
      prefetched = true;
      this.pages = new Map([0, PREVIEW_PAGE_ROWS].filter((start) => start < rows.length).map((start) => [start / PREVIEW_PAGE_ROWS, rows.slice(start, start + PREVIEW_PAGE_ROWS)]));
      this.meta = { type: "meta", count: rows.length, columns: rows.reduce((max, row) => Math.max(max, row.length), 1), complete: false };
      this.render();
    };
    const startWorker = () => {
      if (controller.signal.aborted) return;
      const engine = new Worker(new URL("./large-text.worker.js", import.meta.url), { type: "module" });
      this.worker = engine;
      engine.addEventListener("message", (event: MessageEvent<TextPreviewResult>) => {
        const result = event.data;
        if (result.type === "copy") {
          this.copyPending?.resolve(result.text);
          this.copyPending = null;
        } else if (result.type === "error") {
          this.copyPending?.reject(new Error(result.message));
          this.copyPending = null;
          if (result.operation !== "copy") { this.error = result.message; this.settled(); this.render(); }
        } else if (result.type === "meta") {
          indexed = true;
          this.measured = true;
          this.settled();
          if (prefetched) {
            this.pages = new Map();
            this.pending.clear();
            prefetched = false;
          }
          this.meta = result;
          this.render();
        } else if (result.type === "page") {
          this.pending.delete(result.index);
          const previous = this.pages.get(result.index);
          if (previous?.length === result.rows.length && previous.every((row, index) => row.length === result.rows[index]!.length && row.every((cell, column) => cell === result.rows[index]![column]))) return;
          const next = new Map(this.pages);
          next.delete(result.index);
          next.set(result.index, result.rows);
          if (next.size > 12) next.delete([...next.keys()].find((key) => key !== 0)!);
          this.pages = next;
          this.render();
        }
      });
      engine.addEventListener("error", () => {
        this.copyPending?.reject(new Error("The text viewer stopped."));
        this.copyPending = null;
        this.error = "Could not load the text viewer. Close the file and try again.";
        this.settled();
        this.render();
      });
      engine.postMessage({ type: "open", src: props.src, text: props.text, sizeBytes: props.sizeBytes, delimiter: props.delimiter }, []);
    };
    const scheduleWorker = () => {
      if (prefetched) requestAnimationFrame(() => requestAnimationFrame(startWorker));
      else startWorker();
    };
    // Only this bounded prefix is parsed on the main thread. Indexing the full file and fetching later pages remain in
    // the worker.
    if (props.sizeBytes === 0) startWorker();
    else if (props.text !== undefined) {
      paintFirstScreen(props.text.slice(0, FIRST_SCREEN_BYTES), props.text.length <= FIRST_SCREEN_BYTES);
      scheduleWorker();
    } else if (props.src) {
      void fetch(props.src, { headers: { Range: `bytes=0-${FIRST_SCREEN_BYTES - 1}` }, cache: "no-store", signal: controller.signal })
        .then(async (response) => {
          if (!response.ok || Number(response.headers.get("content-length")) > FIRST_SCREEN_BYTES || response.status !== 206) {
            await response.body?.cancel();
            return;
          }
          const total = Number(response.headers.get("content-range")?.split("/")[1]);
          paintFirstScreen(await response.text(), total <= FIRST_SCREEN_BYTES);
        })
        .catch(() => {
          // The worker reports authorization, network and format failures.
        })
        .finally(scheduleWorker);
    } else startWorker();
  }

  /**
   * copyContents: the whole text, once the worker has read it. Asked before the worker has measured the file (with the
   * page's config, after a Try again), it waits for that, where the reference's handle would say the file is
   * unavailable: only then is the 8 MB bound known.
   */
  copy(): Promise<string> {
    if (!this.measured && !this.error && !this.controller.signal.aborted) return this.measuredOrGone.then(() => this.copy());
    return new Promise<string>((resolve, reject) => {
      if (!this.worker) { reject(new Error("The file is unavailable.")); return; }
      if (this.copyPending) { reject(new Error("Another copy is in progress.")); return; }
      if ((this.meta?.sizeBytes ?? 0) > COPY_LIMIT_BYTES) { reject(new Error("Copy contents supports files up to 8 MB. Save the file to copy it in full.")); return; }
      this.copyPending = { resolve, reject };
      this.worker.postMessage({ type: "copy" }, []);
    });
  }

  setWrap(wrap: boolean) {
    if (wrap === this.props.wrap) return;
    this.props = { ...this.props, wrap };
    this.render();
  }

  reveal(revealLine: number | null, revealRequestId: number) {
    if (revealLine === this.props.revealLine && revealRequestId === this.props.revealRequestId) return;
    this.props = { ...this.props, revealLine, revealRequestId };
    this.render();
  }

  private loadPage = (page: number) => {
    if (this.pending.has(page)) return;
    this.pending.add(page);
    this.worker?.postMessage({ type: "page", index: page });
  };

  private show(body: HTMLElement) {
    if (this.body === body) return;
    this.body?.remove();
    this.body = body;
    this.element.append(body);
  }

  private render() {
    const { props, meta } = this;
    if (this.error) {
      this.table?.dispose(); this.table = null;
      this.show(surfaceFailure(this.error, props.onRetry));
      return;
    }
    if (!meta || (meta.count > 0 && !this.pages.has(0))) {
      this.table?.dispose(); this.table = null;
      if (this.body?.dataset.state !== "loading") { const loading = surfaceLoading(); loading.dataset.state = "loading"; this.show(loading); }
      return;
    }
    let root = this.body;
    if (root?.dataset.state !== "ready") {
      this.status = h("div", { class: "status-line", role: "status" });
      root = h("div", { class: "large-text" }, this.status);
      root.dataset.state = "ready";
      this.loadingRest = null;
      this.columnsNotice = null;
      this.show(root);
    }
    this.status!.textContent = `${Math.max(0, meta.count - (props.delimiter && meta.count ? 1 : 0)).toLocaleString()} ${props.delimiter ? "rows" : "lines"}`
      + (props.delimiter ? ` · ${meta.count ? meta.columns.toLocaleString() : 0} columns` : "")
      + (meta.complete && meta.sizeBytes !== undefined ? sizeSuffix(meta.sizeBytes, formatAttachmentSize) : " · Indexing…");
    if (!meta.complete && !this.loadingRest) {
      this.loadingRest = h("div", { class: "large-text-pending" }, surfaceNotice("Loading the rest of the file…"));
      root.append(this.loadingRest);
    } else if (meta.complete && this.loadingRest) { this.loadingRest.remove(); this.loadingRest = null; }
    if (meta.columns === 1024 && !this.columnsNotice) {
      this.columnsNotice = surfaceNotice("Showing the first 1,024 columns. Source and Save retain every column.");
      this.status!.after(this.columnsNotice);
    } else if (meta.columns !== 1024 && this.columnsNotice) { this.columnsNotice.remove(); this.columnsNotice = null; }
    if (meta.count > 0) {
      root.querySelector(".large-text-empty")?.remove();
      const tableProps: VirtualTableProps = {
        name: props.name, count: meta.count, columns: meta.columns, header: props.delimiter ? this.pages.get(0)?.[0] : undefined,
        source: !props.delimiter, wrap: props.wrap, revealLine: meta.complete ? (props.revealLine ?? null) : null,
        revealRequestId: props.revealRequestId ?? 0, getRows: (page) => this.pages.get(page), loadPage: this.loadPage,
      };
      if (!this.table) {
        this.table = new VirtualTable(tableProps);
        root.append(this.table.element);
      } else this.table.update(tableProps);
    } else {
      this.table?.dispose(); this.table = null;
      if (!root.querySelector(".large-text-empty")) root.append(h("div", { class: "large-text-empty" }, "This file is empty."));
    }
  }

  dispose() {
    this.controller.abort();
    this.settled();
    this.copyPending?.reject(new Error("Preview closed."));
    this.copyPending = null;
    this.worker?.terminate();
    this.worker = null;
    this.pending.clear();
    this.table?.dispose();
  }
}
