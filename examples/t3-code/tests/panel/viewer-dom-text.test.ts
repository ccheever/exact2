// The viewer page's text viewers and its frame under happy-dom (MIT reference, see LICENSE-T3: T3 Code 7c3b3b3605ef
// LargeTextViewer.tsx, VirtualTablePreview.tsx, fileSurfaceChrome.tsx's FileSurfaceAction tooltip): rows and cells kept
// across a redraw as React keeps keyed nodes, Copy contents asked before its viewer is ready, the tooltip inside a frame
// no taller than its bar, the page's requests narrowed for every file, and its workers started under its policy.
import { describe, expect, test } from 'bun:test';
import { domWindow, FakeWorker, network, page, resizeAll, sized, until, viewerDom } from './viewer-dom';

// Bun's own fetch reads a Blob URL; the tests' stands in for the network.
const blobText = globalThis.fetch.bind(globalThis);
viewerDom();
const { LargeTextViewer } = await import(`${page}/text/large-text-viewer.ts`);
const { VirtualTable } = await import(`${page}/text/virtual-table.ts`);
const { surfaceAction } = await import(`${page}/chrome.ts`);
const { ChevronLeft } = await import(`${page}/icons.ts`);
const { Page, narrowConnections, fromApp } = await import(`${page}/entry.ts`);
const { policyWorker } = await import(`${page}/shared/policy-workers.ts`);

const lines = (count: number, word = 'line') => Array.from({ length: count }, (_, index) => [`${word} ${index + 1}`]);
/** A large-text worker serving `pages` of one column. */
function serveText(count: number, pages: (index: number) => string[][], sizeBytes = 2048) {
  FakeWorker.answers['large-text.worker.js'] = (message, worker) => {
    if (message.type === 'open') { worker.reply({ type: 'meta', count, columns: 1, complete: true, sizeBytes }); worker.reply({ type: 'page', index: 0, rows: pages(0) }); }
    if (message.type === 'page') worker.reply({ type: 'page', index: message.index, rows: pages(message.index) });
    if (message.type === 'copy') worker.reply({ type: 'copy', text: lines(count).map(([line]) => line).join('\n') });
  };
}
const DomEvent = domWindow.Event;
function laidOut(root: HTMLElement) {
  for (const node of [root, ...root.querySelectorAll('.vt, .vt-rows')]) if (node.matches('.vt, .vt-rows')) sized(node, 800, 600);
  resizeAll(800, 600);
}
/** What the page posts to its app, parsed, while `run` goes. */
async function posts(run: (posted: Record<string, unknown>[]) => Promise<void>) {
  const posted: Record<string, unknown>[] = [];
  const parent = domWindow.parent as unknown as { postMessage: (data: string) => void };
  const original = parent.postMessage;
  parent.postMessage = (data: string) => { posted.push(JSON.parse(data)); };
  try { await run(posted); } finally { parent.postMessage = original; }
}
const config = (key: string, extra: Record<string, unknown> = {}) => ({ type: 'config', view: 'text', key, name: 'a.log', src: 'http://127.0.0.1:1/api/assets/a', text: null, sizeBytes: 2048, kind: '', delimiter: null, wrap: true, revealLine: null, revealRequestId: 0, scheme: 'light', sheet: true, smoothing: true, ...extra });

describe('LargeTextViewer and VirtualTablePreview', () => {
  test('a file streams in pages under its status line, and a page answered again keeps its rows and the selection in them', async () => {
    serveText(40, index => lines(40).slice(index * 32, index * 32 + 32));
    const viewer = new LargeTextViewer({ name: 'server.log', src: 'http://127.0.0.1:1/api/assets/l', sizeBytes: 2048, delimiter: null, wrap: true });
    document.body.append(viewer.element);
    await until(() => viewer.element.querySelector('.vt'), 'the table');
    expect(viewer.element.querySelector('[role="status"]')?.textContent).toBe('40 lines · 2 KB');
    laidOut(viewer.element);
    const rowOf = (n: number) => viewer.element.querySelector<HTMLElement>(`[data-preview-row="${n - 1}"]`)!;
    await until(() => rowOf(2), 'the rows');
    expect([rowOf(1).querySelector('.vt-rowhead')?.textContent, rowOf(1).querySelector('.vt-line')?.textContent, rowOf(1).className]).toEqual(['1', 'line 1', 'vt-row source']);
    const kept = rowOf(2), cell = kept.querySelector('.vt-line')!, text = cell.firstChild!;
    const selection = document.getSelection()!;
    selection.selectAllChildren(cell);
    // The worker answers the page again with one line changed: only that line's text changes.
    const worker = FakeWorker.all.find(each => each.name === 'large-text.worker.js')!;
    worker.reply({ type: 'page', index: 0, rows: [['line 1'], ['line 2'], ['changed'], ...lines(40).slice(3, 32)] });
    await until(() => rowOf(3).querySelector('.vt-line')?.textContent === 'changed', 'the changed line');
    expect([rowOf(2) === kept, kept.querySelector('.vt-line')!.firstChild === text, selection.toString()]).toEqual([true, true, 'line 2']);
    viewer.dispose();
  });

  test('a table keeps its row and cell nodes as its rows are answered again; rows alternate under the header row', async () => {
    const rows = [['name', 'qty', 'price'], ...Array.from({ length: 10 }, (_, index) => [`item ${index}`, `${index}`, `${index * 2}`])];
    const table = new VirtualTable({ name: 'orders.csv', count: rows.length, columns: 3, header: rows[0], getRows: (index: number) => (index === 0 ? rows : undefined) });
    document.body.append(table.element);
    // A table lays its header out in a hidden table first, then mounts its rows.
    laidOut(table.element);
    await until(() => table.element.querySelector('[data-preview-row="1"]'), 'the rows');
    const first = table.element.querySelector<HTMLElement>('[data-preview-row="1"]')!;
    expect([first.getAttribute('role'), first.getAttribute('aria-rowindex'), first.className, [...first.querySelectorAll('[role="cell"]')].map(cell => cell.textContent)])
      .toEqual(['row', '2', 'vt-row table', ['item 0', '0', '0']]);
    expect(table.element.querySelector('[data-preview-row="2"]')!.className).toBe('vt-row table odd');
    const cells = [...first.querySelectorAll('[role="cell"]')];
    table.update({ name: 'orders.csv', count: rows.length, columns: 3, header: rows[0], getRows: (index: number) => (index === 0 ? rows.map(row => [...row]) : undefined) });
    expect([table.element.querySelector('[data-preview-row="1"]') === first, [...first.querySelectorAll('[role="cell"]')].every((cell, at) => cell === cells[at])]).toEqual([true, true]);
    table.dispose();
  });

  test('Copy contents asked before the worker starts waits for it, then copies the whole text; past 8 MB it says why not', async () => {
    serveText(3, () => lines(3));
    // The first screen comes from a ranged read; the worker starts only after it.
    let answer: (response: Response) => void = () => {};
    network.respond = () => new Promise<Response>((resolve) => { answer = resolve; });
    const viewer = new LargeTextViewer({ name: 'a.log', src: 'http://127.0.0.1:1/a', sizeBytes: 20, delimiter: null, wrap: false });
    const copied = viewer.copy();
    answer(new Response('line 1\nline 2\nline 3', { status: 206, headers: { 'content-range': 'bytes 0-19/20', 'content-length': '20' } }));
    expect(await copied).toBe('line 1\nline 2\nline 3');
    viewer.dispose();
    FakeWorker.answers['large-text.worker.js'] = (message, worker) => { if (message.type === 'open') worker.reply({ type: 'meta', count: 1, columns: 1, complete: true, sizeBytes: 9 * 1024 * 1024 }); };
    network.respond = () => new Response('', { status: 200 });
    const large = new LargeTextViewer({ name: 'b.log', src: 'http://127.0.0.1:1/b', sizeBytes: 9 * 1024 * 1024, delimiter: null, wrap: false });
    await expect(large.copy()).rejects.toThrow('Copy contents supports files up to 8 MB. Save the file to copy it in full.');
    large.dispose();
  });
});

describe("the page's frame", () => {
  test('a tooltip the frame has no room for above or below its trigger goes beside it', async () => {
    const viewport = (width: number, height: number) => (domWindow as unknown as { happyDOM: { setViewport(size: { width: number; height: number }): void } }).happyDOM.setViewport({ width, height });
    viewport(600, 40);
    const back = surfaceAction({ label: 'Back to archive', icon: ChevronLeft, onPress: () => {} });
    document.body.append(back.element);
    back.element.getBoundingClientRect = () => ({ top: 6, bottom: 34, left: 12, right: 40, width: 28, height: 28, x: 12, y: 6, toJSON() {} }) as DOMRect;
    const hover = async () => {
      back.element.dispatchEvent(new DomEvent('pointerleave'));
      back.element.dispatchEvent(new DomEvent('pointerenter'));
      await until(() => document.querySelector('.tooltip.shown'), 'the tooltip');
    };
    await hover();
    const tip = document.querySelector<HTMLElement>('.tooltip')!;
    sized(tip, 104, 26);
    await hover();
    expect([tip.textContent, tip.style.top, tip.style.left]).toEqual(['Back to archive', '7px', '44px']);
    viewport(1280, 800);
    await hover();
    expect([tip.style.top, tip.style.left]).toEqual(['38px', '4px']);
    back.element.dispatchEvent(new DomEvent('pointerleave'));
  });

  test("the page's requests reach its file's origin, or only its own files, Blobs and data: for any other file", () => {
    const narrowed = (src: string) => {
      const at = document.implementation.createHTMLDocument('page');
      narrowConnections(src, at);
      narrowConnections('https://elsewhere.example/b', at);
      return [...at.head.querySelectorAll<HTMLMetaElement>('meta[data-connections]')].map(meta => [meta.httpEquiv, meta.content]);
    };
    expect(narrowed('http://127.0.0.1:3773/api/assets/a?sig=1')).toEqual([['Content-Security-Policy', "connect-src 'self' http://127.0.0.1:3773 blob: data:"]]);
    // A draft's Blob, a text the app sent, a notebook's output.
    for (const src of ['blob:http://localhost/6f1c', '']) expect(narrowed(src)).toEqual([['Content-Security-Policy', "connect-src 'self' blob: data:"]]);
    // Only the app's messages are taken: the parent's, from this page's origin.
    const view = { parent: {}, location: { origin: 'exact-app://app' } };
    expect([fromApp({ source: view.parent, origin: 'exact-app://app', data: '{}' } as MessageEvent, view, window), fromApp({ source: {}, origin: 'exact-app://app', data: '{}' } as MessageEvent, view, window)]).toEqual([true, false]);
  });

  test('a worker starts from a Blob module that imports its script, so it takes the page\'s policy', async () => {
    const started: { url: string; options?: WorkerOptions }[] = [];
    class Native { constructor(url: string | URL, options?: WorkerOptions) { started.push({ url: String(url), options }); } }
    const PolicyWorker = policyWorker(Native as unknown as typeof Worker, () => 'exact-app://app/assets/file-preview/js/entry.js');
    new PolicyWorker(new URL('exact-app://app/assets/file-preview/js/large-text.worker.js'), { type: 'module', name: 'text' });
    new PolicyWorker('./residentEngineWorker.mjs', { type: 'module' });
    new PolicyWorker('blob:exact-app://app/1234');
    const source = (url: string) => blobText(url).then(response => response.text());
    expect([started[0]!.url.startsWith('blob:'), started[0]!.options, await source(started[0]!.url)])
      .toEqual([true, { type: 'module', name: 'text' }, 'import "exact-app://app/assets/file-preview/js/large-text.worker.js";']);
    expect(await source(started[1]!.url)).toBe('import "exact-app://app/assets/file-preview/js/residentEngineWorker.mjs";');
    expect(started[2]!.url).toBe('blob:exact-app://app/1234');
  });

  test('a viewer that cannot load says so where the spinner was, with Try again, and tells the app', async () => {
    await posts(async (posted) => {
      const root = document.createElement('div');
      document.body.append(root);
      new Page(root).receive(config('k1', { view: 'unknown' }));
      await until(() => root.querySelector('.failure-message'), 'the failure');
      expect([root.querySelector('.failure-message')?.textContent, root.querySelector('.failure-retry')?.textContent]).toEqual(['No viewer for unknown', 'Try again']);
      root.querySelector<HTMLButtonElement>('.failure-retry')!.click();
      expect(posted).toEqual([{ type: 'error', message: 'No viewer for unknown' }, { type: 'retry', key: 'k1' }]);
    });
  });

  test('Copy contents asked with its config waits for the viewer it asks, and a Copy for another file is not answered', async () => {
    serveText(3, () => lines(3), 20);
    network.respond = () => new Response('line 1\nline 2\nline 3', { status: 206, headers: { 'content-range': 'bytes 0-19/20', 'content-length': '20' } });
    await posts(async (posted) => {
      const root = document.createElement('div');
      document.body.append(root);
      const page = new Page(root);
      page.receive(config('k2'));
      page.receive({ type: 'copy', key: 'k1' });
      page.receive({ type: 'copy', key: 'k2' });
      await until(() => posted.some(message => message.type === 'copy' || message.type === 'copy-error'), 'the answer');
      expect(posted).toEqual([{ type: 'copy', key: 'k2', text: 'line 1\nline 2\nline 3' }]);
    });
  });
});
