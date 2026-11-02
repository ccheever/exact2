// A server Browser tab's PDF in the viewer page, drawn as Chromium's PDF viewer (file-preview-host/src/pdf/
// browser-pdf-viewer.ts), under happy-dom through a stand-in for PDF.js. What a browser measures (the canvases, the
// text layer's boxes) stays its own; this checks the toolbar's names and states, paging, zoom steps, the fit button's
// turns, rotation and two page view in the layout, the sidebar's thumbnails and outline, Download, Print and Document
// properties.
import { describe, expect, mock, test } from 'bun:test';
import { domWindow, intersectAll, network, page, resizeAll, sized, until, viewerDom } from './viewer-dom';

type Item = { str: string; hasEOL: boolean };
const documents = { pages: [] as { width: number; height: number }[], outline: [] as unknown[], info: {} as Record<string, unknown> };
class FakePage {
  rotate = 0;
  constructor(private readonly at: { width: number; height: number }) {}
  getViewport({ scale, rotation = 0 }: { scale: number; rotation?: number }) {
    const quarter = rotation % 180 !== 0;
    return { width: (quarter ? this.at.height : this.at.width) * scale, height: (quarter ? this.at.width : this.at.height) * scale, scale };
  }
  render() { return { promise: Promise.resolve(), cancel() {} }; }
  streamTextContent() { return [] as Item[]; }
  cleanup() {}
}
mock.module('pdfjs-dist', () => ({
  GlobalWorkerOptions: { workerSrc: '' },
  AnnotationMode: { DISABLE: 0, ENABLE: 1 },
  getDocument: ({ data }: { data: Uint8Array }) => {
    if (String.fromCharCode(...data.subarray(0, 4)) !== '%PDF') return { promise: Promise.reject(new Error('Invalid PDF structure.')), destroy: async () => {} };
    const pages = documents.pages;
    const pdf = {
      numPages: pages.length,
      getPage: async (n: number) => new FakePage(pages[n - 1]!),
      getOutline: async () => documents.outline,
      getMetadata: async () => ({ info: documents.info }),
      getDestination: async () => null,
      getPageIndex: async (ref: { num: number }) => ref.num,
      getData: async () => new Uint8Array(data),
    };
    return { promise: Promise.resolve(pdf), destroy: async () => {} };
  },
  TextLayer: class { async render() {} cancel() {} },
}));

viewerDom();
const { BrowserPdfViewer } = await import(`${page}/pdf/browser-pdf-viewer.ts`);
const DomEvent = domWindow.Event;

/** The viewer at 900 by 700, its pages area 599 by 644 (the sidebar's 300 and its border, the toolbar's 56). */
function mount() {
  network.respond = () => new Response(new Uint8Array([37, 80, 68, 70]), { headers: { 'content-length': '4' } });
  const posted: string[] = [];
  const parent = { postMessage: (message: string) => posted.push(message) };
  Object.defineProperty(domWindow, 'parent', { configurable: true, get: () => parent });
  const viewer = new BrowserPdfViewer('http://127.0.0.1:1/guide.pdf', 'guide.pdf', () => 'dark');
  document.body.append(viewer.element);
  const scroller = viewer.element.querySelector<HTMLElement>('#scroller')!;
  let top = 0;
  sized(scroller, 599, 644);
  Object.defineProperty(scroller, 'scrollTop', { configurable: true, get: () => top, set: (value: number) => { top = value; } });
  scroller.scrollTo = ((options: ScrollToOptions) => {
    if (typeof options.top === 'number') top = Math.max(0, options.top);
    scroller.dispatchEvent(new DomEvent('scroll'));
  }) as typeof scroller.scrollTo;
  resizeAll(599, 644);
  const $ = <T extends Element = HTMLElement>(selector: string) => viewer.element.querySelector<T & Element>(selector)!;
  const pageField = () => $<HTMLInputElement>('#pageSelector').value;
  const zoom = () => $<HTMLInputElement>('#zoom-input').value;
  const pages = () => [...viewer.element.querySelectorAll<HTMLElement>('.page')].map(node => ({ left: parseFloat(node.style.left), top: parseFloat(node.style.top), width: parseFloat(node.style.width), height: parseFloat(node.style.height) }));
  return { viewer, scroller, $, pageField, zoom, pages, posted };
}

describe('BrowserPdfViewer', () => {
  test("names its toolbar as Chromium's, and opens at the zoom that fits the page's width, at most 100 %", async () => {
    documents.pages = Array.from({ length: 6 }, () => ({ width: 612, height: 792 }));
    documents.outline = [];
    documents.info = { Title: 'Fixture guide' };
    const { viewer, $, pageField, zoom } = mount();
    await until(() => $('#pagelength').textContent === '6', 'the document');
    const labels = [...viewer.element.querySelectorAll('#toolbar [aria-label]')].map(node => node.getAttribute('aria-label'));
    expect(labels).toEqual(['Toggle sidebar', 'Page number', 'Zoom out', 'Zoom level', 'Zoom in', 'Fit to page', 'Rotate counterclockwise', 'Download', 'Print', 'More actions']);
    expect([$('#title').textContent, pageField(), zoom()]).toEqual(['Fixture guide', '1', '73%']);
    expect($('#scroller').getAttribute('aria-label')).toBe('PDF document containing 6 pages');
    viewer.dispose();
  });

  test('pages by the field, zooms by the presets, and fits to page, then to width, in turn', async () => {
    documents.pages = Array.from({ length: 6 }, () => ({ width: 612, height: 792 }));
    const { viewer, scroller, $, pageField, zoom } = mount();
    await until(() => $('#pagelength').textContent === '6', 'the document');
    const field = $<HTMLInputElement>('#pageSelector');
    field.value = '4';
    field.dispatchEvent(new DomEvent('change'));
    await until(() => pageField() === '4', 'page 4');
    expect(scroller.scrollTop).toBeGreaterThan(0);
    $<HTMLButtonElement>('#zoom-in').click();
    expect(zoom()).toBe('75%');
    $<HTMLButtonElement>('#zoom-in').click();
    expect(zoom()).toBe('80%');
    $<HTMLButtonElement>('#zoom-out').click();
    $<HTMLButtonElement>('#zoom-out').click();
    expect(zoom()).toBe('67%');
    const fit = $<HTMLButtonElement>('#fit');
    fit.click();
    // Fit to page: the page box (792 × 4/3 and its 10 px of shadow) in the 644 px tall area.
    expect([zoom(), fit.getAttribute('aria-label'), pageField()]).toEqual([`${Math.round(644 / (1056 + 10) * 100)}%`, 'Fit to width', '4']);
    fit.click();
    expect([zoom(), fit.getAttribute('aria-label')]).toEqual([`${Math.round(599 / 826 * 100)}%`, 'Fit to page']);
    viewer.dispose();
  });

  test('turns the pages a quarter counterclockwise, and lays them in pairs in two page view', async () => {
    documents.pages = Array.from({ length: 3 }, () => ({ width: 612, height: 792 }));
    const { viewer, $, pages } = mount();
    await until(() => pages().length > 0, 'the pages');
    const upright = pages()[0]!;
    $<HTMLButtonElement>('#rotate').click();
    const turned = pages()[0]!;
    expect(turned.width / turned.height).toBeCloseTo(upright.height / upright.width, 3);
    $<HTMLButtonElement>('#rotate').click();
    $<HTMLButtonElement>('#rotate').click();
    $<HTMLButtonElement>('#rotate').click();
    $<HTMLButtonElement>('#more').click();
    const items = [...viewer.element.querySelectorAll('#menu .dropdown-item')];
    expect(items.map(item => [item.textContent, item.getAttribute('role'), item.getAttribute('aria-checked'), (item as HTMLButtonElement).disabled])).toEqual([
      ['Two page view', 'menuitemcheckbox', 'false', false], ['Annotations', 'menuitemcheckbox', 'true', false],
      ['Present', 'menuitem', null, false], ['Document properties', 'menuitem', null, false],
    ]);
    (items[0] as HTMLButtonElement).click();
    expect($('#menu').hidden).toBe(true);
    const [first, second, third] = pages();
    // Pages 1 and 2 side by side, either side of the middle; page 3 below page 1.
    expect([first!.top === second!.top, first!.left + first!.width < second!.left, third!.top > first!.top + first!.height]).toEqual([true, true, true]);
    viewer.dispose();
  });

  test("shows thumbnails, and the document outline's tab when the PDF has one", async () => {
    documents.pages = Array.from({ length: 4 }, () => ({ width: 612, height: 792 }));
    documents.outline = [
      { title: 'Introduction', dest: [{ num: 0 }, { name: 'XYZ' }, 0, 792, 0], items: [] },
      { title: 'Configuration', dest: [{ num: 2 }, { name: 'XYZ' }, 0, 792, 0], items: [{ title: 'Environment variables', dest: [{ num: 2 }, { name: 'XYZ' }, 0, 400, 0], items: [] }] },
    ];
    const { viewer, $, pageField } = mount();
    await until(() => viewer.element.querySelectorAll('.thumbnail').length === 4, 'the thumbnails');
    intersectAll();
    const thumbs = [...viewer.element.querySelectorAll('.thumbnail')];
    expect(thumbs.map(thumb => [thumb.getAttribute('aria-label'), thumb.getAttribute('aria-selected')])).toEqual([
      ['Thumbnail for page 1', 'true'], ['Thumbnail for page 2', 'false'], ['Thumbnail for page 3', 'false'], ['Thumbnail for page 4', 'false'],
    ]);
    (thumbs[2]!.querySelector('.thumbnail-box') as HTMLElement).click();
    await until(() => pageField() === '3', 'page 3 from its thumbnail');
    const tabs = [...viewer.element.querySelectorAll('#icons [role="tab"]')];
    expect(tabs.map(tab => [tab.getAttribute('title'), tab.getAttribute('aria-selected')])).toEqual([['Thumbnails', 'true'], ['Document outline', 'false']]);
    (tabs[1] as HTMLButtonElement).click();
    expect([$('#thumbnails').hidden, $('#outline').hidden]).toEqual([true, false]);
    const titles = () => [...viewer.element.querySelectorAll('.bookmark-title')].map(node => node.textContent);
    expect(titles()).toEqual(['Introduction', 'Configuration']);
    (viewer.element.querySelectorAll<HTMLButtonElement>('.bookmark .expand')[1]!).click();
    expect(titles()).toEqual(['Introduction', 'Configuration', 'Environment variables']);
    (viewer.element.querySelectorAll<HTMLElement>('.bookmark-item')[0]!).click();
    await until(() => pageField() === '1', 'page 1 from the outline');
    $<HTMLButtonElement>('#sidenavToggle').click();
    expect([$('#sidenav-container').hasAttribute('data-closed'), $('#sidenavToggle').getAttribute('aria-expanded')]).toEqual([true, 'false']);
    $<HTMLButtonElement>('#sidenavToggle').click();
    viewer.dispose();
  });

  test('asks the app to save the PDF, and lists its properties', async () => {
    documents.pages = [{ width: 612, height: 792 }];
    documents.outline = [];
    documents.info = { Title: 'Fixture guide', Producer: 'macOS Quartz PDFContext', PDFFormatVersion: '1.3', IsLinearized: false };
    const { viewer, $, posted } = mount();
    await until(() => $('#pagelength').textContent === '1', 'the document');
    $<HTMLButtonElement>('#save').click();
    expect(posted.map(message => JSON.parse(message).type)).toContain('download');
    $<HTMLButtonElement>('#more').click();
    [...viewer.element.querySelectorAll<HTMLButtonElement>('#menu .dropdown-item')].find(item => item.textContent === 'Document properties')!.click();
    await until(() => viewer.element.querySelectorAll('.dialog tr').length === 14, 'the properties');
    const rows = Object.fromEntries([...viewer.element.querySelectorAll('.dialog tr')].map(row => [row.firstElementChild!.textContent, row.lastElementChild!.textContent]));
    expect(rows).toMatchObject({ 'File name:': 'guide.pdf', 'File size:': '4 B', 'Title:': 'Fixture guide', 'Author:': '-', 'PDF producer:': 'macOS Quartz PDFContext', 'PDF version:': '1.3', 'Page count:': '1', 'Fast web view:': 'No' });
    expect(rows['Page size:']).toMatch(/^(8\.50 × 11\.00 in|216 × 279 mm) \(portrait\)$/);
    viewer.element.querySelector<HTMLButtonElement>('.dialog .action-button')!.click();
    expect(viewer.element.querySelector('.dialog')).toBeNull();
    viewer.dispose();
  });

  test("prints the PDF's bytes in a hidden frame, titled with the file's name, then takes the frame away", async () => {
    documents.pages = [{ width: 612, height: 792 }];
    const { viewer, $ } = mount();
    await until(() => $('#pagelength').textContent === '1', 'the document');
    $<HTMLButtonElement>('#print').click();
    await until(() => document.querySelector('iframe.print-frame') !== null, 'the print frame');
    const frame = document.querySelector<HTMLIFrameElement>('iframe.print-frame')!;
    expect([frame.getAttribute('aria-hidden'), frame.src.startsWith('blob:')]).toEqual(['true', true]);
    const printed: string[] = [];
    Object.defineProperty(frame, 'contentWindow', { configurable: true, get: () => ({ print: () => printed.push(document.title) }) });
    const title = document.title;
    $<HTMLButtonElement>('#print').click();
    expect(document.querySelectorAll('iframe.print-frame').length).toBe(1);
    frame.dispatchEvent(new DomEvent('load'));
    await until(() => printed.length === 1, 'the print');
    expect([printed, document.title, frame.isConnected]).toEqual([['guide.pdf'], title, false]);
    viewer.dispose();
  });
});
