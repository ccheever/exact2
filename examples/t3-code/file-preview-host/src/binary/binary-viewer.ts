// Archives and SQLite databases (MIT reference, see LICENSE-T3: apps/web/src/components/files/BinaryViewer.tsx and
// BinaryFilePreview.tsx; T3 Code 7c3b3b3605ef): the binary worker lists a ZIP, TAR or gzip archive and reads its
// entries, or opens a database read-only with sql.js and pages a table's rows 50 at a time. An entry opened from the
// archive is drawn by the app (its source view, or its image with the zoom bar), under this page's bar; the page
// sends it as `entry` and says `entry-closed` when Back to archive leaves it.
import { ChevronLeft, ChevronRight, File, Folder } from "../icons";
import { h, icon, post, selectTrigger, surfaceAction, surfaceFailure, surfaceLoading, surfaceNotice, type IconNode } from "../chrome";
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

/** ArchiveFile, for the app to draw: an image's bytes, UTF-8 text, or why it cannot be opened here. */
export function archiveEntry(path: string, bytes: Uint8Array): Record<string, unknown> {
  const kind = filePreviewKind({ name: path });
  if (kind === "image") return { view: "image", data: base64(bytes) };
  if (["text", "markdown", "html", "mermaid"].includes(kind)) {
    try {
      const text = decodeFilePreviewText(bytes);
      return { view: "text", text: text.text, truncated: text.truncated };
    } catch { /* not UTF-8 */ }
  }
  return { view: "failure", message: "Only UTF-8 text and images can be opened inside an archive." };
}

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
    const nodes: Node[] = [];
    if (archive) {
      const back = surfaceAction({
        label: selectedPath ? "Back to archive" : "Parent folder", icon: ChevronLeft as IconNode, disabled: !selectedPath && !folder,
        onPress: () => {
          this.error = null;
          this.entry = null;
          if (this.selectedPath) this.selectedPath = null;
          else this.folder = folder.replace(/(?:[^/]+)?\/$/, "");
          this.render();
        },
      });
      nodes.push(h("div", { class: "archive-bar" }, back.element, h("span", { class: "archive-place" }, selectedPath ?? (folder || "Archive contents")),
        h("span", { class: "archive-count" }, `${archive.entries.length.toLocaleString()} entries${this.sizeBytes !== null ? ` · ${formatAttachmentSize(this.sizeBytes)}` : ""}`)));
    }
    const table = database?.tables.find((item) => item.name === this.selectedTable);
    if (database && table) {
      const select = selectTrigger({
        label: "Table", value: `${table.name} (${table.count.toLocaleString()} rows)`,
        rows: () => database.tables.map((item) => ({ id: item.name, label: `${item.name} (${item.count.toLocaleString()} rows)`, selected: item.name === this.selectedTable })),
        onPick: (name) => { if (!this.busy) this.selectPage(0, name); },
      });
      select.update({ busy });
      const previous = surfaceAction({ label: "Previous rows", icon: ChevronLeft as IconNode, disabled: busy || this.page === 0, onPress: () => this.selectPage(this.page - 1) });
      const next = surfaceAction({ label: "Next rows", icon: ChevronRight as IconNode, disabled: busy || (this.page + 1) * SQLITE_PAGE_SIZE >= table.count, onPress: () => this.selectPage(this.page + 1) });
      nodes.push(
        h("div", { class: "table-bar" }, h("div", { class: "table-select" }, select.element), previous.element,
          h("span", { class: "range", "aria-live": "polite" }, table.count === 0 ? "0 rows" : `${this.page * SQLITE_PAGE_SIZE + 1}–${Math.min(table.count, (this.page + 1) * SQLITE_PAGE_SIZE)} of ${table.count.toLocaleString()}`),
          next.element),
        h("div", { class: "status-line plain", role: "status" }, `${table.count.toLocaleString()} rows · ${rows?.columns.length ?? "…"} columns${this.sizeBytes !== null ? ` · ${formatAttachmentSize(this.sizeBytes)}` : ""}`),
        surfaceNotice("Read-only. Up to 50 columns and 1,024 characters per cell. Blob contents are omitted."),
      );
    }
    let entryShown = "";
    if (error) nodes.push(surfaceFailure(error, props.onRetry));
    else if (busy) nodes.push(surfaceLoading());
    else if (selectedPath) {
      if (entry) entryShown = entry.path;
    } else if (archive) nodes.push(this.entries(archive));
    else if (rows) nodes.push(this.table(rows));
    else nodes.push(h("p", { class: "empty-note" }, "This database has no readable tables."));
    this.element.replaceChildren(...nodes);
    // The opened entry, for the app to draw under the archive bar.
    if (entryShown !== this.sentEntry) {
      this.sentEntry = entryShown;
      if (entry && entryShown) post({ type: "entry", path: entry.path, ...archiveEntry(entry.path, entry.bytes) });
      else post({ type: "entry-closed" });
    }
  }

  private entries(archive: Archive): HTMLElement {
    const folder = this.folder;
    const children = new Map<string, { path: string; directory: boolean; size: number; modified: string | null }>();
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
    const body = h("tbody");
    for (const item of [...children.values()].sort((a, b) => Number(b.directory) - Number(a.directory) || a.path.localeCompare(b.path))) {
      const open = h("button", { type: "button", class: "entry-open" }, icon((item.directory ? Folder : File) as IconNode, 16), h("span", {}, item.path.slice(folder.length)));
      open.addEventListener("click", () => {
        if (item.directory) this.folder = `${item.path}/`;
        else {
          this.selectedPath = item.path;
          this.entry = null;
          this.send({ type: "entry", path: item.path });
        }
        this.render();
      });
      body.append(h("tr", {},
        h("td", { class: "name" }, open),
        h("td", { class: "size" }, formatAttachmentSize(item.size)),
        h("td", { class: "date" }, item.modified ? new Date(item.modified).toLocaleDateString() : "-")));
    }
    return h("div", { class: "scroll-both" },
      h("table", { "aria-label": "Archive entries", class: "entries" },
        h("thead", {}, h("tr", {}, h("th", { scope: "col" }, "Name"), h("th", { scope: "col", class: "end" }, "Size"), h("th", { scope: "col", class: "end" }, "Modified"))),
        body),
      children.size === 0 ? h("p", { class: "empty-note" }, "This folder is empty.") : null);
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
    if (this.timer) clearTimeout(this.timer);
    if (this.sentEntry) post({ type: "entry-closed" });
  }
}
