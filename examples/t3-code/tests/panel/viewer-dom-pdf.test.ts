// The viewer page's PDF viewer under happy-dom, after the reference's PdfViewer.browser.test.tsx (MIT reference, see
// LICENSE-T3: T3 Code 7c3b3b3605ef): its two-page document (a banner whose title is two text runs, then a letter page
// turned a quarter) through a stand-in for PDF.js, as happy-dom paints no canvas and runs no PDF.js worker. What the
// reference measures in a browser (the fit width, the rotated run's box) stays its own; this checks the pages, the
// find and its marks, the jump to a match, and the pages' order in the document.
import { describe, expect, mock, test } from 'bun:test';
import { domWindow, network, page, resizeAll, sized, until, viewerDom } from './viewer-dom';

type Item = { str: string; hasEOL: boolean };
const documents = { pages: [] as { width: number; height: number; items: Item[] }[], destroyed: 0 };
class FakePage {
  constructor(private readonly at: { width: number; height: number; items: Item[] }) {}
  getViewport({ scale }: { scale: number }) { return { width: this.at.width * scale, height: this.at.height * scale, scale }; }
  render() { return { promise: Promise.resolve(), cancel() {} }; }
  streamTextContent() { return this.at.items; }
  async getTextContent() { return { items: this.at.items }; }
  cleanup() {}
}
mock.module('pdfjs-dist', () => ({
  GlobalWorkerOptions: { workerSrc: '' },
  // As pdfjs-dist 6.4.299: the document has no `destroy`; its loading task's lets it go. Bytes that open with no PDF
  // header are refused, as PDF.js refuses them.
  getDocument: ({ data }: { data: Uint8Array }) => {
    if (String.fromCharCode(...data.subarray(0, 4)) !== '%PDF') return { promise: Promise.reject(new Error('Invalid PDF structure.')), destroy: async () => {} };
    const pages = documents.pages;
    const pdf = { numPages: pages.length, getPage: async (n: number) => new FakePage(pages[n - 1]!) };
    return { promise: Promise.resolve(pdf), destroy: async () => { documents.destroyed += 1; } };
  },
  TextLayer: class {
    constructor(private readonly options: { container: HTMLElement; textContentSource: Item[] }) {}
    async render() {
      for (const item of this.options.textContentSource) {
        const span = document.createElement('span');
        span.setAttribute('role', 'presentation');
        span.textContent = item.str;
        this.options.container.append(span);
        if (item.hasEOL) this.options.container.append(document.createElement('br'));
      }
    }
    cancel() {}
  },
}));

viewerDom();
const { PdfViewer } = await import(`${page}/pdf/pdf-viewer.ts`);
const { Page } = await import(`${page}/entry.ts`);
const DomEvent = domWindow.Event;

type View = { page: number; offset: number; zoom: number | null; left: number };
/** The scroll region at 500 by 600, its scroll offsets kept as happy-dom does not, a scroll event after each scrollTo. */
function scrolling(region: HTMLElement) {
  let top = 0, left = 0;
  sized(region, 500, 600);
  Object.defineProperty(region, 'scrollTop', { configurable: true, get: () => top, set: (value: number) => { top = value; } });
  Object.defineProperty(region, 'scrollLeft', { configurable: true, get: () => left, set: (value: number) => { left = value; } });
  region.scrollTo = ((options: ScrollToOptions) => {
    if (typeof options.top === 'number') top = Math.max(0, options.top);
    if (typeof options.left === 'number') left = Math.max(0, options.left);
    region.dispatchEvent(new DomEvent('scroll'));
  }) as typeof region.scrollTo;
  return region;
}
function mount(title: string, initialView?: View, onViewChange?: (view: View) => void) {
  network.respond = () => new Response(new Uint8Array([37, 80, 68, 70]), { headers: { 'content-length': '4' } });
  const viewer = new PdfViewer('http://127.0.0.1:1/planning.pdf', title, initialView, onViewChange);
  document.body.append(viewer.element);
  const region = scrolling(viewer.element.querySelector<HTMLElement>('[role="region"]')!);
  resizeAll(500, 600);
  const find = viewer.element.querySelector<HTMLInputElement>('input[type="search"], input')!;
  const label = () => viewer.element.querySelector('.pdf-pages-label')?.textContent;
  const status = () => viewer.element.querySelector('.toolbar [role="status"]')?.textContent;
  /** Types into Find in PDF and waits for its count (the search runs a quarter second after the last key). */
  const fill = async (value: string, count: string) => {
    find.value = value;
    find.dispatchEvent(new DomEvent('input'));
    await until(() => status() === count, `"${value}" to count ${count}`);
  };
  const order = () => [...region.querySelectorAll('canvas')].map(canvas => Number(canvas.getAttribute('aria-label')!.slice(5)));
  const spans = () => [...region.querySelectorAll('[data-pdf-text-layer] span')];
  return { viewer, region, find, fill, label, status, order, spans };
}

describe('PdfViewer', () => {
  test('finds split and rotated text and jumps to its page, the pages it mounts on the way kept in page order', async () => {
    // Page 14 is the banner whose title is two text runs; page 15 the letter page turned a quarter; the rest filler.
    documents.pages = Array.from({ length: 30 }, (_, index) => ({ width: 612, height: 792, items: [{ str: `[p${index + 1}]`, hasEOL: false }] }));
    documents.pages[13] = { width: 612, height: 140, items: [{ str: 'Quarter', hasEOL: false }, { str: 'ly outlook', hasEOL: false }] };
    documents.pages[14] = { width: 792, height: 612, items: [{ str: 'Budget and risks', hasEOL: false }] };
    const { viewer, region, find, fill, label, order, spans } = mount('Planning PDF');
    await until(() => label() === '1 / 30 pages', 'the document');
    expect([region.getAttribute('aria-label'), find.getAttribute('aria-label')]).toEqual(['Planning PDF', 'Find in PDF']);
    // From page 17, finding the banner mounts it above the pages already there.
    const step = (500 - 32) * (792 / 612) + 16;
    region.scrollTo({ top: 16 * step });
    await until(() => label() === '17 / 30 pages', 'page 17');
    await fill('quarterly outlook', '1/1');
    await until(() => spans().some(span => span.classList.contains('pdf-find-match')), 'the marks');
    expect([label(), spans().filter(span => span.classList.contains('pdf-find-match')).map(span => span.textContent)]).toEqual(['14 / 30 pages', ['Quarter', 'ly outlook']]);
    expect(order()).toEqual([...order()].sort((a, b) => a - b));
    await fill('budget', '1/1');
    await until(() => label() === '15 / 30 pages', 'the jump to page 15');
    expect(spans().find(span => span.textContent === 'Budget and risks')?.classList.contains('pdf-find-match')).toBe(true);
    await fill(' ', '0/0');
    viewer.dispose();
  });

  test('pages mounted on the way back up come before the ones below them, so a selection runs down the pages', async () => {
    documents.pages = Array.from({ length: 30 }, (_, index) => ({ width: 612, height: 792, items: [{ str: `[p${index + 1}]`, hasEOL: false }] }));
    const { viewer, region, label, order, spans } = mount('Long PDF');
    await until(() => label() === '1 / 30 pages', 'the document');
    const step = (500 - 32) * (792 / 612) + 16;
    region.scrollTo({ top: 15 * step });
    await until(() => label() === '16 / 30 pages', 'page 16');
    region.scrollTo({ top: 14 * step });
    await until(() => label() === '15 / 30 pages', 'page 15');
    const shown = order();
    expect([shown.length > 2, shown]).toEqual([true, [...shown].sort((a, b) => a - b)]);
    const text = (n: number) => spans().find(span => span.textContent === `[p${n}]`)!;
    await until(() => text(shown[0]!) && text(shown[1]!), 'their text');
    expect(text(shown[0]!).compareDocumentPosition(text(shown[1]!)) & 4).toBe(4); // DOCUMENT_POSITION_FOLLOWING
    viewer.dispose();
  });

  test('a new revision opens at the page, offset, zoom and horizontal scroll the reader left', async () => {
    documents.pages = Array.from({ length: 30 }, (_, index) => ({ width: 612, height: 792, items: [{ str: `[p${index + 1}]`, hasEOL: false }] }));
    let saved: View | undefined;
    const first = mount('Report', undefined, view => { saved = view; });
    await until(() => first.label() === '1 / 30 pages', 'the document');
    const zoomIn = first.viewer.element.querySelector('[aria-label="Zoom in"]') as HTMLElement;
    zoomIn.click();
    await until(() => saved?.zoom !== null && saved?.zoom !== undefined, 'the zoom');
    const zoom = saved!.zoom!, width = 612 * zoom;
    const top = 16 + 7 * (width * 792 / 612 + 16) + 40;
    first.region.scrollTo({ top: top - 16, left: 25 });
    await until(() => first.label() === '8 / 30 pages', 'page 8');
    expect({ ...saved, offset: Math.round(saved!.offset) }).toEqual({ page: 7, offset: 40, zoom, left: 25 });
    first.viewer.dispose();

    // The new revision's page 3 is short: the pages above the one it returns to are measured before it scrolls there.
    documents.pages[2] = { width: 612, height: 300, items: [{ str: '[p3]', hasEOL: false }] };
    const restored = top - 16 - (width * 792 / 612 - width * 300 / 612);
    let reported: View | undefined;
    const second = mount('Report', saved, view => { reported = view; });
    await until(() => second.label() === '8 / 30 pages', 'the same page');
    expect([Math.round(second.region.scrollTop), second.region.scrollLeft, second.viewer.element.querySelector('.pdf-zoom')?.textContent]).toEqual([Math.round(restored), 25, `${Math.round(zoom * 100)}%`]);
    expect({ ...reported, offset: Math.round(reported!.offset) }).toEqual({ page: 7, offset: 40, zoom, left: 25 });
    second.viewer.dispose();
  });

  test('the page opens a new key of the same document where the reader was, and another document at its start', async () => {
    documents.pages = Array.from({ length: 30 }, (_, index) => ({ width: 612, height: 792, items: [{ str: `[p${index + 1}]`, hasEOL: false }] }));
    const root = document.createElement('div');
    document.body.append(root);
    const shown = new Page(root);
    const label = () => root.querySelector('.pdf-pages-label')?.textContent;
    /** Shows `key` of `documentId`, its bytes held until its region is sized, and waits for its document. */
    const show = async (key: string, documentId: string, expected: string) => {
      let release = () => {};
      const held = new Promise<void>(resolve => { release = resolve; });
      network.respond = async () => { await held; return new Response(new Uint8Array([37, 80, 68, 70]), { headers: { 'content-length': '4' } }); };
      const before = root.querySelector('[role="region"]');
      shown.receive({ type: 'config', view: 'pdf', key, documentId, name: 'r.pdf', src: `http://127.0.0.1:1/r.pdf?file-revision=${key}`, text: null, sizeBytes: -1, kind: '',
        delimiter: null, wrap: true, scheme: 'light', sheet: false, smoothing: true, revealLine: null, revealRequestId: 0 });
      await until(() => root.querySelector('[role="region"]') && root.querySelector('[role="region"]') !== before, 'the viewer');
      const region = scrolling(root.querySelector<HTMLElement>('[role="region"]')!);
      resizeAll(500, 600);
      release();
      await until(() => label() === expected, `${key} at ${expected}`);
      return region;
    };
    const first = await show('r1', 'doc:a', '1 / 30 pages');
    first.scrollTo({ top: 7 * ((500 - 32) * (792 / 612) + 16) + 30 });
    await until(() => label() === '8 / 30 pages', 'page 8');
    const second = await show('r2', 'doc:a', '8 / 30 pages');
    expect(Math.round(second.scrollTop)).toBe(Math.round(first.scrollTop));
    await show('r3', 'doc:b', '1 / 30 pages');
  });

  test('a loaded PDF lets its document go through its loading task, so the page can show the next file', async () => {
    documents.pages = [{ width: 612, height: 792, items: [{ str: 'Only page', hasEOL: false }] }];
    documents.destroyed = 0;
    const { viewer, label } = mount('Short PDF');
    await until(() => label() === '1 / 1 pages', 'the document');
    viewer.dispose();
    expect(documents.destroyed).toBe(1);
  });

  test('a URL that answers a page draws nothing and runs nothing: PDF.js refuses its bytes, and the viewer says so', async () => {
    // A server tab's PDF from another origin (server-pdf-viewer.ts) whose server answers HTML once it has answered the
    // server's own browser with a PDF.
    const page = '<h1>Injected</h1><script>globalThis.injectedRan = true</script><img src="x" onerror="globalThis.injectedRan = true">';
    network.respond = () => new Response(page, { headers: { 'content-type': 'text/html' } });
    const viewer = new PdfViewer('https://other.example/api/assets/token/guide.pdf', 'guide.pdf');
    document.body.append(viewer.element);
    await until(() => viewer.element.textContent?.includes('Invalid PDF structure.'), 'the failure');
    expect([document.body.innerHTML.includes('Injected'), document.querySelectorAll('script, img, iframe, embed, object').length, (globalThis as { injectedRan?: boolean }).injectedRan])
      .toEqual([false, 0, undefined]);
    viewer.element.remove();
  });
});
