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
const DomEvent = domWindow.Event;

function mount(title: string) {
  network.respond = () => new Response(new Uint8Array([37, 80, 68, 70]), { headers: { 'content-length': '4' } });
  const viewer = new PdfViewer('http://127.0.0.1:1/planning.pdf', title);
  document.body.append(viewer.element);
  const region = viewer.element.querySelector<HTMLElement>('[role="region"]')!;
  let top = 0;
  sized(region, 500, 600);
  Object.defineProperty(region, 'scrollTop', { configurable: true, get: () => top, set: (value: number) => { top = value; } });
  region.scrollTo = ((options: ScrollToOptions) => {
    if (typeof options.top === 'number') top = Math.max(0, options.top);
    region.dispatchEvent(new DomEvent('scroll'));
  }) as typeof region.scrollTo;
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
