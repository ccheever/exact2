// The viewer page's text viewers and its frame under happy-dom (MIT reference, see LICENSE-T3: T3 Code 7c3b3b3605ef
// LargeTextViewer.tsx, VirtualTablePreview.tsx, fileSurfaceChrome.tsx's FileSurfaceAction tooltip): rows and cells kept
// across a redraw as React keeps keyed nodes, the status line, the tooltip inside a frame no taller than its bar, and
// the page's own requests narrowed to the file's origin.
import { describe, expect, test } from 'bun:test';
import { domWindow, FakeWorker, network, page, resizeAll, sized, tick, viewerDom } from './viewer-dom';

viewerDom();
const { LargeTextViewer } = await import(`${page}/text/large-text-viewer.ts`);
const { VirtualTable } = await import(`${page}/text/virtual-table.ts`);
const { surfaceAction } = await import(`${page}/chrome.ts`);
const { ChevronLeft } = await import(`${page}/icons.ts`);
const { Page, narrowConnections, fromApp } = await import(`${page}/entry.ts`);

const lines = (count: number, word = 'line') => Array.from({ length: count }, (_, index) => [`${word} ${index + 1}`]);
/** A large-text worker serving `pages` of one column. */
function serveText(count: number, pages: (index: number) => string[][], sizeBytes = 2048) {
  FakeWorker.answers['large-text.worker.js'] = (message, worker) => {
    if (message.type === 'open') { worker.reply({ type: 'meta', count, columns: 1, complete: true, sizeBytes }); worker.reply({ type: 'page', index: 0, rows: pages(0) }); }
    if (message.type === 'page') worker.reply({ type: 'page', index: message.index, rows: pages(message.index) });
  };
}
const DomEvent = domWindow.Event;
async function laidOut(root: HTMLElement) {
  for (const node of [root, ...root.querySelectorAll('.vt, .vt-rows')]) if (node.matches('.vt, .vt-rows')) sized(node, 800, 600);
  resizeAll(800, 600);
  await tick();
}

describe('LargeTextViewer and VirtualTablePreview', () => {
  test('a file streams in pages under its status line, and a page answered again keeps its rows and the selection in them', async () => {
    let word = 'line';
    serveText(40, index => lines(40, word).slice(index * 32, index * 32 + 32));
    const viewer = new LargeTextViewer({ name: 'server.log', src: 'http://127.0.0.1:1/api/assets/l', sizeBytes: 2048, delimiter: null, wrap: true });
    document.body.append(viewer.element);
    await tick(5);
    expect(viewer.element.querySelector('[role="status"]')?.textContent).toBe('40 lines · 2 KB');
    await laidOut(viewer.element);
    await tick(5);
    const rowOf = (n: number) => viewer.element.querySelector<HTMLElement>(`[data-preview-row="${n - 1}"]`)!;
    expect([rowOf(1).querySelector('.vt-rowhead')?.textContent, rowOf(1).querySelector('.vt-line')?.textContent, rowOf(1).className]).toEqual(['1', 'line 1', 'vt-row source']);
    const kept = rowOf(2), cell = kept.querySelector('.vt-line')!, text = cell.firstChild!;
    const selection = document.getSelection()!;
    selection.selectAllChildren(cell);
    // The worker answers the page again with one line changed: only that line's text changes.
    word = 'entry';
    const worker = FakeWorker.all.find(each => each.name === 'large-text.worker.js')!;
    worker.reply({ type: 'page', index: 0, rows: [['line 1'], ['line 2'], ['changed'], ...lines(40).slice(3, 32)] });
    await tick();
    expect([rowOf(2) === kept, kept.querySelector('.vt-line')!.firstChild === text, rowOf(3).querySelector('.vt-line')?.textContent]).toEqual([true, true, 'changed']);
    expect(selection.toString()).toBe('line 2');
    viewer.dispose();
  });

  test('a table keeps its row and cell nodes as its columns scroll; rows alternate and the header row is the first', async () => {
    const rows = [['name', 'qty', 'price'], ...Array.from({ length: 10 }, (_, index) => [`item ${index}`, `${index}`, `${index * 2}`])];
    const table = new VirtualTable({ name: 'orders.csv', count: rows.length, columns: 3, header: rows[0], getRows: (index: number) => (index === 0 ? rows : undefined) });
    document.body.append(table.element);
    await laidOut(table.element);
    // A table lays its header out in a hidden table first, then mounts its rows.
    await laidOut(table.element);
    const first = table.element.querySelector<HTMLElement>('[data-preview-row="1"]')!;
    expect([first.getAttribute('role'), first.getAttribute('aria-rowindex'), first.className, [...first.querySelectorAll('[role="cell"]')].map(cell => cell.textContent)])
      .toEqual(['row', '2', 'vt-row table', ['item 0', '0', '0']]);
    expect(table.element.querySelector('[data-preview-row="2"]')!.className).toBe('vt-row table odd');
    const cells = [...first.querySelectorAll('[role="cell"]')];
    table.update({ name: 'orders.csv', count: rows.length, columns: 3, header: rows[0], getRows: (index: number) => (index === 0 ? rows.map(row => [...row]) : undefined) });
    await tick();
    expect([table.element.querySelector('[data-preview-row="1"]') === first, [...first.querySelectorAll('[role="cell"]')].every((cell, at) => cell === cells[at])]).toEqual([true, true]);
    table.dispose();
  });

  test('Copy contents asks the worker for the whole text; past 8 MB it says why not', async () => {
    serveText(3, () => lines(3));
    FakeWorker.answers['large-text.worker.js'] = (message, worker) => {
      if (message.type === 'open') { worker.reply({ type: 'meta', count: 3, columns: 1, complete: true, sizeBytes: 20 }); worker.reply({ type: 'page', index: 0, rows: lines(3) }); }
      if (message.type === 'copy') worker.reply({ type: 'copy', text: 'line 1\nline 2\nline 3' });
    };
    const viewer = new LargeTextViewer({ name: 'a.log', text: 'line 1\nline 2\nline 3', delimiter: null, wrap: false });
    document.body.append(viewer.element);
    await tick(40);
    expect(await viewer.copy()).toBe('line 1\nline 2\nline 3');
    viewer.dispose();
    FakeWorker.answers['large-text.worker.js'] = (message, worker) => { if (message.type === 'open') worker.reply({ type: 'meta', count: 1, columns: 1, complete: true, sizeBytes: 9 * 1024 * 1024 }); };
    const large = new LargeTextViewer({ name: 'b.log', src: 'http://127.0.0.1:1/b', sizeBytes: 9 * 1024 * 1024, delimiter: null, wrap: false });
    await tick(5);
    await expect(large.copy()).rejects.toThrow('Copy contents supports files up to 8 MB. Save the file to copy it in full.');
    large.dispose();
  });
});

describe("the page's frame", () => {
  test('a tooltip the frame has no room for above or below its trigger goes beside it', async () => {
    (window as unknown as { happyDOM: { setViewport(size: { width: number; height: number }): void } }).happyDOM.setViewport({ width: 600, height: 40 });
    const back = surfaceAction({ label: 'Back to archive', icon: ChevronLeft, onPress: () => {} });
    document.body.append(back.element);
    back.element.getBoundingClientRect = () => ({ top: 6, bottom: 34, left: 12, right: 40, width: 28, height: 28, x: 12, y: 6, toJSON() {} }) as DOMRect;
    back.element.dispatchEvent(new DomEvent('pointerenter'));
    await tick(650);
    const tip = document.querySelector<HTMLElement>('.tooltip')!;
    sized(tip, 104, 26);
    back.element.dispatchEvent(new DomEvent('pointerleave'));
    back.element.dispatchEvent(new DomEvent('pointerenter'));
    await tick(650);
    expect([tip.textContent, tip.classList.contains('shown'), tip.style.top, tip.style.left]).toEqual(['Back to archive', true, '7px', '44px']);
    (window as unknown as { happyDOM: { setViewport(size: { width: number; height: number }): void } }).happyDOM.setViewport({ width: 1280, height: 800 });
    back.element.dispatchEvent(new DomEvent('pointerleave'));
    back.element.dispatchEvent(new DomEvent('pointerenter'));
    await tick(650);
    expect([tip.style.top, tip.style.left]).toEqual(['38px', '4px']);
  });

  test("the page's requests reach only its file's origin once one is known", () => {
    const at = document.implementation.createHTMLDocument('page');
    narrowConnections('blob:http://localhost/6f1c', at);
    expect(at.head.querySelectorAll('meta[data-connections]').length).toBe(0);
    narrowConnections('http://127.0.0.1:3773/api/assets/a?sig=1', at);
    narrowConnections('https://elsewhere.example/b', at);
    const metas = [...at.head.querySelectorAll<HTMLMetaElement>('meta[data-connections]')];
    expect(metas.map(meta => [meta.httpEquiv, meta.content])).toEqual([['Content-Security-Policy', "connect-src 'self' http://127.0.0.1:3773 blob: data:"]]);
    // Only the app's messages are taken: the parent's, from this page's origin.
    const view = { parent: {}, location: { origin: 'exact-app://app' } };
    expect([fromApp({ source: view.parent, origin: 'exact-app://app', data: '{}' } as MessageEvent, view, window), fromApp({ source: {}, origin: 'exact-app://app', data: '{}' } as MessageEvent, view, window)]).toEqual([true, false]);
  });

  test('a viewer that cannot load says so where the spinner was, with Try again, and tells the app', async () => {
    const posted: unknown[] = [];
    const parent = domWindow.parent as unknown as { postMessage: (data: string) => void };
    const original = parent.postMessage;
    parent.postMessage = (data: string) => { posted.push(JSON.parse(data)); };
    try {
      const root = document.createElement('div');
      document.body.append(root);
      new Page(root).receive({ type: 'config', view: 'unknown', key: 'k1', name: 'a.bin', src: 'http://127.0.0.1:1/a', text: null, sizeBytes: 1, kind: '', delimiter: null, wrap: true, revealLine: null, revealRequestId: 0, scheme: 'light', sheet: true, smoothing: true });
      await tick(5);
      expect([root.querySelector('.failure-message')?.textContent, root.querySelector('.failure-retry')?.textContent]).toEqual(['No viewer for unknown', 'Try again']);
      root.querySelector<HTMLButtonElement>('.failure-retry')!.click();
      expect(posted).toEqual([{ type: 'error', message: 'No viewer for unknown' }, { type: 'retry', key: 'k1' }]);
    } finally { parent.postMessage = original; }
    network.respond = () => new Response('');
  });
});
