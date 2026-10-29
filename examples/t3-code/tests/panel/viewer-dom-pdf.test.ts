// The viewer page's PDF viewer under happy-dom, after the reference's PdfViewer.browser.test.tsx (MIT reference, see
// LICENSE-T3: T3 Code 7c3b3b3605ef): its two-page document (a banner whose title is two text runs, then a letter page
// turned a quarter) through a stand-in for PDF.js, as happy-dom paints no canvas and runs no PDF.js worker. What the
// reference measures in a browser (the fit width, the rotated run's box) stays its own; this checks the pages, the
// find and its marks, the jump to a match, and the pages' order in the document.
import { describe, expect, mock, test } from 'bun:test';
import { domWindow, network, page, resizeAll, sized, tick, viewerDom } from './viewer-dom';

type Item = { str: string; hasEOL: boolean };
const documents = { pages: [] as { width: number; height: number; items: Item[] }[] };
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
  getDocument: () => {
    const pages = documents.pages;
    const pdf = { numPages: pages.length, getPage: async (n: number) => new FakePage(pages[n - 1]!), destroy: async () => {} };
    return { promise: Promise.resolve(pdf), destroy: async () => {} };
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
  const fill = async (value: string) => { find.value = value; find.dispatchEvent(new DomEvent('input')); await tick(300); };
  const label = () => viewer.element.querySelector('.pdf-pages-label')?.textContent;
  const status = () => viewer.element.querySelector('.toolbar [role="status"]')?.textContent;
  return { viewer, region, find, fill, label, status };
}

describe('PdfViewer', () => {
  test('finds split and rotated text and jumps to its page', async () => {
    documents.pages = [
      { width: 612, height: 140, items: [{ str: 'Quarter', hasEOL: false }, { str: 'ly outlook', hasEOL: false }] },
      { width: 792, height: 612, items: [{ str: 'Budget and risks', hasEOL: false }] },
    ];
    const { viewer, region, find, fill, label, status } = mount('Planning PDF');
    await tick(20);
    expect([label(), region.getAttribute('aria-label'), find.getAttribute('aria-label')]).toEqual(['1 / 2 pages', 'Planning PDF', 'Find in PDF']);
    await fill('quarterly outlook');
    expect(status()).toBe('1/1');
    await tick(5);
    expect([...region.querySelectorAll('.pdf-find-match')].map(span => span.textContent)).toEqual(['Quarter', 'ly outlook']);
    await fill('budget');
    await tick(5);
    expect([label(), status()]).toEqual(['2 / 2 pages', '1/1']);
    expect([...region.querySelectorAll('[data-pdf-text-layer] span')].find(span => span.textContent === 'Budget and risks')?.classList.contains('pdf-find-match')).toBe(true);
    await fill(' ');
    expect(status()).toBe('0/0');
    viewer.dispose();
  });

  test('pages mounted on the way back up come before the ones below them, so a selection runs down the pages', async () => {
    documents.pages = Array.from({ length: 30 }, (_, index) => ({ width: 612, height: 792, items: [{ str: `[p${index + 1}]`, hasEOL: false }] }));
    const { viewer, region, label } = mount('Long PDF');
    await tick(20);
    const step = (500 - 32) * (792 / 612) + 16;
    const order = () => [...region.querySelectorAll('canvas')].map(canvas => Number(canvas.getAttribute('aria-label')!.slice(5)));
    region.scrollTo({ top: 15 * step });
    await tick(5);
    region.scrollTo({ top: 14 * step });
    await tick(5);
    expect(label()).toBe('15 / 30 pages');
    const shown = order();
    expect([shown.length > 2, shown]).toEqual([true, [...shown].sort((a, b) => a - b)]);
    const text = (n: number) => [...region.querySelectorAll('[data-pdf-text-layer] span')].find(span => span.textContent === `[p${n}]`)!;
    expect(text(shown[0]!).compareDocumentPosition(text(shown[1]!)) & 4).toBe(4); // DOCUMENT_POSITION_FOLLOWING
    viewer.dispose();
  });
});
