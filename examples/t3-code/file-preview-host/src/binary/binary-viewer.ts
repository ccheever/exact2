// Archives and SQLite databases (MIT reference, see LICENSE-T3: apps/web/src/components/files/BinaryViewer.tsx and
// BinaryFilePreview.tsx; T3 Code 7c3b3b3605ef): the binary worker lists a ZIP, TAR or gzip archive and reads its
// entries, or opens a database read-only with sql.js and pages a table's rows 50 at a time. An entry opened from the
// archive is drawn by the app (its source view, or its image with the zoom bar), under this page's bar; the page
// sends it as `entry` and says `entry-closed` when Back to archive leaves it.
import { ChevronLeft, ChevronRight, File, Folder } from "../icons";
import { h, icon, post, selectTrigger, surfaceAction, surfaceFailure, surfaceLoading, surfaceNotice, type IconNode, type SelectTrigger, type SurfaceAction } from "../chrome";
import { PictureView } from "./picture-view";
import { decodeFilePreviewText, filePreviewKind, formatAttachmentSize } from "../shared/file-preview";
import { readPreviewFile } from "../shared/read-bounded";
import { BINARY_PREVIEW_MAX_BYTES, SQLITE_PAGE_SIZE, type BinaryWorkerRequest, type BinaryWorkerResult } from "./binary-preview-protocol";

type Archive = Extract<BinaryWorkerResult, { type: "archive" }>;
type Database = Extract<BinaryWorkerResult, { type: "sqlite" }>;
type Rows = Extract<BinaryWorkerResult, { type: "rows" }>;
type Entry = Extract<BinaryWorkerResult, { type: "entry" }>;

const BASE64_CHUNK = 0x8000;
function base64(bytes: Uint8Array): string {
  let binary = "";
  for (let start = 0; start < bytes.length; start += BASE64_CHUNK) binary += String.fromCharCode(...bytes.subarray(start, start + BASE64_CHUNK));
  return btoa(binary);
}

/** The largest picture the app draws, as Copy contents is bounded: its base64 passes through the app's 64 MiB heap. */
export const APP_IMAGE_BYTES = 8 * 1024 * 1024;
/** Pictures upstream's `<img>` (Chromium) cannot decode, which say "Unable to load image." there. */
const UNDRAWN = /\.(?:heic|heif|tiff?)$/i;

/**
 * ArchiveFile, for the app to draw: an image's bytes, UTF-8 text, or why it cannot be opened here; `page` for a picture
 * this page draws itself (an SVG, which the app's image cannot draw, or one past APP_IMAGE_BYTES).
 */
export function archiveEntry(path: string, bytes: Uint8Array): Record<string, unknown> {
  const kind = filePreviewKind({ name: path });
  if (kind === "image") {
    if (UNDRAWN.test(path)) return { view: "failure", message: "Unable to load image." };
    if (pageDrawn(path, bytes)) return { view: "page" };
    return { view: "image", data: base64(bytes) };
  }
  if (["text", "markdown", "html", "mermaid"].includes(kind)) {
    try {
      const text = decodeFilePreviewText(bytes);
      return { view: "text", text: text.text, truncated: text.truncated };
    } catch { /* not UTF-8 */ }
  }
  return { view: "failure", message: "Only UTF-8 text and images can be opened inside an archive." };
}

/** A picture this page draws: an SVG, which the app's image cannot draw, or one past APP_IMAGE_BYTES. */
function pageDrawn(path: string, bytes: Uint8Array): boolean {
  return filePreviewKind({ name: path }) === "image" && !UNDRAWN.test(path) && (/\.svg$/i.test(path) || bytes.byteLength > APP_IMAGE_BYTES);
}

type Child = { path: string; directory: boolean; size: number; modified: string | null };

export class BinaryViewer {
  readonly element = h("div", { class: "binary" });
  private sizeBytes: number | null = null;
  private archive: Archive | null = null;
  private database: Database | null = null;
  private rows: Rows | null = null;
  private entry: Entry | null = null;
  private folder = "";
  private selectedPath: string | null = null;
  private selectedTable = "";
  private page = 0;
  private busy = true;
  private error: string | null = null;
  private worker: Worker | null = null;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private readonly controller = new AbortController();
  private sentEntry = "";
  /** The bars and the archive's scroll area, kept from render to render as React keeps them. */
  private archiveBar: { row: HTMLDivElement; back: SurfaceAction; place: HTMLSpanElement; count: HTMLSpanElement } | null = null;
  private tableBar: { nodes: HTMLElement[]; select: SelectTrigger; previous: SurfaceAction; next: SurfaceAction; range: HTMLSpanElement; status: HTMLDivElement } | null = null;
  private listing: { scroll: HTMLDivElement; body: HTMLTableSectionElement; empty: HTMLParagraphElement; rows: Map<string, HTMLTableRowElement> } | null = null;
  private readonly content = h("div", { style: "display:contents" });
  private picture: PictureView | null = null;

  constructor(private readonly props: { src: string; name: string; kind: "archive" | "sqlite"; onRetry?: () => void }) {
    const controller = this.controller;
    const worker = new Worker(new URL("./binary-preview.worker.js", import.meta.url), { type: "module" });
    this.worker = worker;
    const fail = (message: string) => {
      if (controller.signal.aborted) return;
      this.error = message;
      this.busy = false;
      controller.abort();
      worker.terminate();
      this.worker = null;
      if (this.timer) clearTimeout(this.timer);
      this.render();
    };
    this.timer = setTimeout(() => fail("This file took too long to preview. Close it and try again."), 30_000);
    worker.addEventListener("error", () => fail("Could not load the file viewer. Close it and try again."));
    worker.addEventListener("message", (event: MessageEvent<BinaryWorkerResult>) => {
      if (controller.signal.aborted) return;
      if (this.timer) clearTimeout(this.timer);
      const result = event.data;
      this.busy = false;
      if (result.type === "error") this.error = result.message;
      else if (result.type === "archive") this.archive = result;
      else if (result.type === "entry") this.entry = result;
      else if (result.type === "rows") this.rows = result;
      else {
        this.database = result;
        const table = result.tables[0];
        if (table) {
          this.selectedTable = table.name;
          this.send({ type: "rows", table: table.name, page: 0 });
        }
      }
      this.render();
    });
    void readPreviewFile(props.src, controller.signal, BINARY_PREVIEW_MAX_BYTES)
      .then((bytes) => {
        if (!controller.signal.aborted) {
          this.sizeBytes = bytes.byteLength;
          worker.postMessage({ type: "open", name: props.name, kind: props.kind, bytes }, [bytes]);
        }
      })
      .catch((cause: unknown) => fail(cause instanceof Error ? cause.message : "Could not read this file."));
    this.render();
  }

  private send(request: BinaryWorkerRequest) {
    this.busy = true;
    this.error = null;
    if (this.timer) clearTimeout(this.timer);
    this.timer = setTimeout(() => {
      this.error = "This file took too long to preview. Close it and try again.";
      this.busy = false;
      this.worker?.terminate();
      this.worker = null;
      this.render();
    }, 15_000);
    this.worker?.postMessage(request);
  }

  private selectPage(next: number, name = this.selectedTable) {
    this.selectedTable = name;
    this.page = next;
    this.rows = null;
    this.send({ type: "rows", table: name, page: next });
    this.render();
  }

  private render() {
    const { archive, database, rows, entry, folder, selectedPath, busy, error, props } = this;
    if (archive) this.renderArchiveBar(archive, folder, selectedPath);
    const table = database?.tables.find((item) => item.name === this.selectedTable);
    if (database && table) this.renderTableBar(table, rows, busy);
    let shown: Node | null = null;
    // The opened entry the app draws under the archive bar; a picture this page draws is not sent.
    let sent: Entry | null = null;
    if (error) shown = surfaceFailure(error, props.onRetry);
    else if (busy) shown = surfaceLoading();
    else if (selectedPath) {
      if (entry && pageDrawn(entry.path, entry.bytes)) {
        if (this.picture?.path !== entry.path) { this.picture?.dispose(); this.picture = new PictureView(entry.path, entry.bytes); }
        shown = this.picture.element;
      } else sent = entry;
    } else if (archive) shown = this.entries(archive);
    else if (rows) shown = this.table(rows);
    else shown = h("p", { class: "empty-note" }, "This database has no readable tables.");
    if (this.picture && shown !== this.picture.element) { this.picture.dispose(); this.picture = null; }
    if (!shown) this.content.replaceChildren();
    else if (this.content.childNodes.length !== 1 || this.content.firstChild !== shown) this.content.replaceChildren(shown);
    if (this.content.parentElement !== this.element) this.element.append(this.content);
    const sentPath = sent?.path ?? "";
    if (sentPath !== this.sentEntry) {
      this.sentEntry = sentPath;
      if (sent) post({ type: "entry", path: sent.path, ...archiveEntry(sent.path, sent.bytes) });
      else post({ type: "entry-closed" });
    }
  }

  private renderArchiveBar(archive: Archive, folder: string, selectedPath: string | null) {
    if (!this.archiveBar) {
      const back = surfaceAction({
        label: "Parent folder", icon: ChevronLeft as IconNode,
        onPress: () => {
          this.error = null;
          this.entry = null;
          if (this.selectedPath) this.selectedPath = null;
          else this.folder = this.folder.replace(/(?:[^/]+)?\/$/, "");
          this.render();
        },
      });
      const place = h("span", { class: "archive-place" });
      const count = h("span", { class: "archive-count" });
      this.archiveBar = { row: h("div", { class: "archive-bar" }, back.element, place, count), back, place, count };
      this.element.prepend(this.archiveBar.row);
    }
    const { back, place, count } = this.archiveBar;
    back.update({ label: selectedPath ? "Back to archive" : "Parent folder", disabled: !selectedPath && !folder });
    place.textContent = selectedPath ?? (folder || "Archive contents");
    count.textContent = `${archive.entries.length.toLocaleString()} entries${this.sizeBytes !== null ? ` · ${formatAttachmentSize(this.sizeBytes)}` : ""}`;
  }

  private renderTableBar(table: Database["tables"][number], rows: Rows | null, busy: boolean) {
    if (!this.tableBar) {
      const select = selectTrigger({
        label: "Table", value: "",
        rows: () => (this.database?.tables ?? []).map((item) => ({ id: item.name, label: `${item.name} (${item.count.toLocaleString()} rows)`, selected: item.name === this.selectedTable })),
        onPick: (name) => { if (!this.busy) this.selectPage(0, name); },
      });
      const previous = surfaceAction({ label: "Previous rows", icon: ChevronLeft as IconNode, onPress: () => this.selectPage(this.page - 1) });
      const next = surfaceAction({ label: "Next rows", icon: ChevronRight as IconNode, onPress: () => this.selectPage(this.page + 1) });
      const range = h("span", { class: "range", "aria-live": "polite" });
      const status = h("div", { class: "status-line plain", role: "status" });
      const nodes = [h("div", { class: "table-bar" }, h("div", { class: "table-select" }, select.element), previous.element, range, next.element), status,
        surfaceNotice("Read-only. Up to 50 columns and 1,024 characters per cell. Blob contents are omitted.")];
      this.tableBar = { nodes, select, previous, next, range, status };
      this.element.prepend(...nodes);
    }
    const { select, previous, next, range, status } = this.tableBar;
    select.update({ value: `${table.name} (${table.count.toLocaleString()} rows)`, busy });
    previous.update({ disabled: busy || this.page === 0 });
    next.update({ disabled: busy || (this.page + 1) * SQLITE_PAGE_SIZE >= table.count });
    range.textContent = table.count === 0 ? "0 rows" : `${this.page * SQLITE_PAGE_SIZE + 1}–${Math.min(table.count, (this.page + 1) * SQLITE_PAGE_SIZE)} of ${table.count.toLocaleString()}`;
    status.textContent = `${table.count.toLocaleString()} rows · ${rows?.columns.length ?? "…"} columns${this.sizeBytes !== null ? ` · ${formatAttachmentSize(this.sizeBytes)}` : ""}`;
  }

  /** The folder's entries in the archive's one scroll area (its position kept as a folder opens), rows keyed by path. */
  private entries(archive: Archive): HTMLElement {
    const folder = this.folder;
    const children = new Map<string, Child>();
    for (const item of archive.entries) {
      if (!item.path.startsWith(folder)) continue;
      const rest = item.path.slice(folder.length).replace(/\/$/, "");
      if (!rest) continue;
      const slash = rest.indexOf("/");
      const path = slash < 0 ? item.path.replace(/\/$/, "") : `${folder}${rest.slice(0, slash)}`;
      const directory = slash >= 0 || item.directory;
      const key = `${path}${directory ? "/" : ""}`;
      const previous = children.get(key);
      if (previous) previous.size += item.size;
      else children.set(key, { path, directory, size: item.size, modified: item.modified });
    }
    if (!this.listing) {
      const body = h("tbody");
      const empty = h("p", { class: "empty-note" }, "This folder is empty.");
      const scroll = h("div", { class: "scroll-both" },
        h("table", { "aria-label": "Archive entries", class: "entries" },
          h("thead", {}, h("tr", {}, h("th", { scope: "col" }, "Name"), h("th", { scope: "col", class: "end" }, "Size"), h("th", { scope: "col", class: "end" }, "Modified"))),
          body), empty);
      this.listing = { scroll, body, empty, rows: new Map() };
    }
    const { body, empty, rows } = this.listing;
    const kept = new Map(rows);
    rows.clear();
    let previous: Element | null = null;
    for (const item of [...children.values()].sort((a, b) => Number(b.directory) - Number(a.directory) || a.path.localeCompare(b.path))) {
      const key = `${item.path}${item.directory ? "/" : ""}`;
      let row = kept.get(key);
      kept.delete(key);
      if (!row) row = this.entryRow(item);
      const label = item.path.slice(folder.length);
      const name = row.querySelector(".entry-open span");
      if (name && name.textContent !== label) name.textContent = label;
      const size = row.querySelector(".size");
      if (size && size.textContent !== formatAttachmentSize(item.size)) size.textContent = formatAttachmentSize(item.size);
      rows.set(key, row);
      if (row.parentElement !== body || row.previousElementSibling !== previous) { if (previous) previous.after(row); else body.prepend(row); }
      previous = row;
    }
    for (const row of kept.values()) row.remove();
    empty.hidden = children.size !== 0;
    return this.listing.scroll;
  }

  private entryRow(item: Child): HTMLTableRowElement {
    const open = h("button", { type: "button", class: "entry-open" }, icon((item.directory ? Folder : File) as IconNode, 16), h("span", {}));
    open.addEventListener("click", () => {
      if (item.directory) this.folder = `${item.path}/`;
      else {
        this.selectedPath = item.path;
        this.entry = null;
        this.send({ type: "entry", path: item.path });
      }
      this.render();
    });
    return h("tr", {},
      h("td", { class: "name" }, open),
      h("td", { class: "size" }, ""),
      h("td", { class: "date" }, item.modified ? new Date(item.modified).toLocaleDateString() : "-"));
  }

  private table(rows: Rows): HTMLElement {
    return h("div", { class: "scroll-both" },
      h("table", { "aria-label": `${this.props.name}: ${rows.table}`, class: "db-rows" },
        h("thead", {}, h("tr", {}, ...rows.columns.map((name) => h("th", { scope: "col" }, name)))),
        h("tbody", {}, ...rows.rows.map((row) => h("tr", {}, ...row.map((cell) => h("td", {}, cell)))))),
      rows.rows.length === 0 ? h("p", { class: "empty-note" }, "This table is empty.") : null);
  }

  dispose() {
    this.controller.abort();
    this.worker?.terminate();
    this.worker = null;
    this.picture?.dispose();
    if (this.timer) clearTimeout(this.timer);
    if (this.sentEntry) post({ type: "entry-closed" });
  }
}
