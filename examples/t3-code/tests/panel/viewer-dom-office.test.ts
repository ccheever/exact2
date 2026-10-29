// The viewer page's Office viewers under happy-dom (MIT reference, see LICENSE-T3: T3 Code 7c3b3b3605ef
// office/OfficePages.tsx, WordDocument.tsx, Spreadsheet.tsx, Presentation.tsx, OfficeViewer.tsx and DocumentPages.tsx):
// what each keeps from a redraw as React keeps its keyed nodes, and what it asks of its worker again.
import { describe, expect, test } from 'bun:test';
import { domWindow, FakeWorker, intersectAll, network, page, resizeAll, sized, until, viewerDom } from './viewer-dom';

viewerDom();
const { OfficePages } = await import(`${page}/office/office-pages.ts`);
const { WordDocument } = await import(`${page}/office/word-document.ts`);
const { Spreadsheet } = await import(`${page}/office/spreadsheet.ts`);
const { Presentation } = await import(`${page}/office/presentation.ts`);
const { officeViewer } = await import(`${page}/office/office-viewer.ts`);
const { h } = await import(`${page}/chrome.ts`);

const DomKeyboard = domWindow.KeyboardEvent;
const letter = () => ({ width: 600, height: 800 });
/** An engine behind worker-rpc.ts: `answer` returns each method's value. */
function engine(name: string, answer: (method: string, input: any) => unknown | Promise<unknown>) {
  const calls: { method: string; input: any }[] = [];
  FakeWorker.answers[name] = (message, worker) => {
    if (!('method' in message)) return;
    calls.push({ method: message.method, input: message.input });
    void Promise.resolve(answer(message.method, message.input)).then(value => worker.reply({ id: message.id, ok: true, value, transferred: [] }));
  };
  return calls;
}
/** The pages laid out at 800 by 600 and near the viewport; a page paints from then on. */
function settle(root: HTMLElement) {
  for (const node of [root, ...root.querySelectorAll('.office-pages')]) if (node.matches('.office-pages')) sized(node, 800, 600);
  resizeAll(800, 600);
  intersectAll(true);
}

describe('OfficePages', () => {
  test("a redraw keeps a page's overlay nodes, and a selection in them; a new page object is painted again", async () => {
    const painted: number[] = [];
    const layers = new Map<number, Node>();
    const props = {
      pages: [letter(), letter()], paint: async (index: number) => { painted.push(index); }, label: 'Page', onError: (cause: unknown) => { throw cause; },
      overlay: (index: number) => { if (!layers.has(index)) layers.set(index, h('div', { class: 'layer' }, h('span', {}, `text of page ${index + 1}`))); return [layers.get(index)!]; },
    };
    const pages = new OfficePages(props);
    document.body.append(pages.element);
    settle(pages.element);
    await until(() => painted.length === 2, 'both pages painted');
    expect(painted).toEqual([0, 1]);
    const span = pages.element.querySelector('.layer span')!;
    document.getSelection()!.selectAllChildren(span);
    pages.redraw();
    expect([pages.element.querySelector('.layer span') === span, document.getSelection()!.toString()]).toEqual([true, 'text of page 1']);
    // The same props again paint nothing; the Word layout's later pass brings new page objects, which paint again.
    pages.update({ ...props });
    expect(painted).toEqual([0, 1]);
    pages.update({ ...props, pages: [letter(), letter()] });
    expect(painted).toEqual([0, 1, 0, 1]);
    expect([...pages.element.querySelectorAll('[role="group"]')].map(row => row.getAttribute('aria-label'))).toEqual(['Page 1', 'Page 2']);
    pages.dispose();
  });
});

describe('WordDocument', () => {
  test("the full layout's pages are painted and their text asked for again; the prefix's text shows until it comes", async () => {
    let full = false, laidOut: (() => void) | null = null;
    const releases: (() => void)[] = [];
    const calls = engine('document.worker.js', async (method, input) => {
      if (method === 'open') return { pages: [letter(), letter()], loading: true };
      if (method === 'layout') { await new Promise<void>(resolve => { laidOut = resolve; }); full = true; return { done: true, pages: [letter(), letter(), letter()] }; }
      if (method === 'text') {
        if (full) await new Promise<void>(resolve => { releases.push(resolve); });
        return [{ text: full ? `full page ${input.index + 1}` : `page ${input.index + 1}`, left: 0, top: 0, size: 0.02, transform: '', dir: 'ltr' }];
      }
      return undefined;
    });
    const details: string[] = [];
    const word = new WordDocument(new ArrayBuffer(8), (text: string) => details.push(text), (cause: unknown) => { throw cause; }, () => h('div'));
    document.body.append(word.element);
    await until(() => word.element.querySelector('.office-pages'), 'the pages');
    settle(word.element);
    const asked = (method: string) => calls.filter(call => call.method === method && call.input?.index === 0).length;
    const text = () => word.element.querySelector('.word-text span')?.textContent;
    await until(() => text() === 'page 1', "the prefix's text");
    expect([details, asked('paint'), asked('text')]).toEqual([['2+ pages'], 1, 1]);
    await until(() => laidOut, 'the layout step');
    laidOut!();
    await until(() => asked('text') === 2, 'the text asked again');
    expect([details, asked('paint'), text()]).toEqual([['2+ pages', '3 pages'], 2, 'page 1']);
    for (const release of releases) release();
    await until(() => text() === 'full page 1', "the full layout's text");
    word.dispose();
  });

  test("a page's text layer goes when the page leaves the viewport, and is asked for again when it comes back", async () => {
    const calls = engine('document.worker.js', async (method, input) => {
      if (method === 'open') return { pages: [letter(), letter()], loading: false };
      if (method === 'text') return [{ text: `page ${input.index + 1}`, left: 0, top: 0, size: 0.02, transform: '', dir: 'ltr' }];
      return undefined;
    });
    const word = new WordDocument(new ArrayBuffer(8), () => {}, (cause: unknown) => { throw cause; }, () => h('div'));
    document.body.append(word.element);
    await until(() => word.element.querySelector('.office-pages'), 'the pages');
    settle(word.element);
    const asked = () => calls.filter(call => call.method === 'text' && call.input?.index === 0).length;
    await until(() => word.element.querySelector('.word-text span'), 'the text layers');
    intersectAll(false);
    expect(word.element.querySelector('.word-text')).toBe(null);
    intersectAll(true);
    await until(() => asked() === 2, 'the text asked again');
    await until(() => word.element.querySelector('.word-text span'), 'the text layer back');
    word.dispose();
  });
});

describe('Spreadsheet', () => {
  test('the sheet tabs stay the same buttons as sheets show, so the arrow keys keep moving the focus along them', async () => {
    const layout = (sheet: number) => ({
      info: { activeSheet: sheet, sheetNames: ['Alpha', 'Beta', 'Gamma'], frozenRows: 0, frozenCols: 0, initialScrollX: 0, initialScrollY: 0, contentWidth: 2000, contentHeight: 2000 },
      merges: [], hidden: { rows: [], cols: [] }, used: { top: 0, left: 0, bottom: 10, right: 5 }, chartWidth: 0, chartHeight: 0,
    });
    const shown: number[] = [];
    engine('spreadsheet.worker.js', (method, input) => {
      if (method === 'open') return { tabs: [{ index: 0, name: 'Alpha' }, { index: 1, name: 'Beta' }, { index: 2, name: 'Gamma' }], layout: layout(0) };
      if (method === 'show') { shown.push(input.sheet); return layout(input.sheet); }
      if (method === 'cells') return [];
      return undefined;
    });
    const sheet = new Spreadsheet(new ArrayBuffer(8), () => {}, (cause: unknown) => { throw cause; }, () => h('div'));
    document.body.append(sheet.element);
    const tabs = () => [...sheet.element.querySelectorAll<HTMLButtonElement>('[role="tab"]')];
    await until(() => tabs().length === 3, 'the tabs');
    const first = tabs();
    first[0]!.focus();
    for (const at of [1, 2]) {
      document.activeElement!.dispatchEvent(new DomKeyboard('keydown', { key: 'ArrowRight', bubbles: true, cancelable: true }));
      await until(() => tabs()[at]!.getAttribute('aria-selected') === 'true', `sheet ${at + 1} shown`);
    }
    expect(shown).toEqual([1, 2]);
    expect([tabs().every((tab, at) => tab === first[at]), (document.activeElement as HTMLElement).textContent, tabs().map(tab => tab.getAttribute('aria-selected'))])
      .toEqual([true, 'Gamma', ['false', 'false', 'true']]);
    sheet.dispose();
  });
});

describe('Presentation', () => {
  test('Previous and Next step through the slides; a slide that could not be drawn says so once', async () => {
    engine('presentation.worker.js', (method, input) => {
      if (method === 'open') return [{ ...letter(), notes: '', hidden: false }, { ...letter(), notes: 'Say hello', hidden: false }, { ...letter(), notes: '', hidden: true }];
      if (method === 'paint') return input.index === 1 ? { kind: 'failed' } : { kind: 'cancelled' };
      return undefined;
    });
    const details: string[] = [];
    const deck = new Presentation(new ArrayBuffer(8), (text: string) => details.push(text), (cause: unknown) => { throw cause; }, () => h('div'));
    document.body.append(deck.element);
    await until(() => deck.element.querySelector('.office-pages'), 'the slides');
    settle(deck.element);
    await until(() => deck.element.querySelector('.slide-failed'), "the failed slide's line");
    const counter = () => deck.element.querySelector('.slide-counter')?.textContent;
    const button = (label: string) => deck.element.querySelector<HTMLButtonElement>(`[aria-label="${label}"]`)!;
    expect([details, counter(), button('Previous slide').disabled]).toEqual([['3 slides'], 'Slide 1 of 3', true]);
    button('Next slide').click();
    button('Next slide').click();
    expect([counter(), button('Next slide').disabled]).toEqual(['Slide 3 of 3', true]);
    const failed = deck.element.querySelector('.slide-failed')!;
    expect([failed.textContent, deck.element.querySelector('.slide-notes')?.textContent, deck.element.querySelectorAll('.slide-hidden').length]).toEqual(['This slide could not be drawn.', 'Say hello', 1]);
    resizeAll(820, 600);
    expect(deck.element.querySelector('.slide-failed')).toBe(failed);
    deck.dispose();
  });
});

describe('OfficeViewer (OpenDocument, RTF, EPUB)', () => {
  network.respond = () => new Response(new Uint8Array([80, 75, 3, 4]), { headers: { 'content-length': '4' } });
  function serve(result: (kind: string) => unknown) {
    network.respond = () => new Response(new Uint8Array([80, 75, 3, 4]), { headers: { 'content-length': '4' } });
    FakeWorker.answers['office-preview.worker.js'] = (message, worker) => {
      if (message.type === 'open') worker.reply(result(message.kind));
      else if (message.type === 'sheet-page') worker.reply({ type: 'sheet-page', sheet: message.sheet, index: message.index, rows: [] });
    };
  }
  test("Next slide keeps the focus as the slide turns, and the reading size goes from slide to slide", async () => {
    serve(() => ({ type: 'document', navigation: 'slide', pages: [1, 2, 3].map(n => ({ title: `Slide ${n}`, html: `<p>slide ${n}</p>` })) }));
    const viewer = officeViewer({ src: 'http://127.0.0.1:1/deck.odp', name: 'deck.odp', kind: 'odp', sizeBytes: 4, scheme: 'light' });
    document.body.append(viewer.element);
    await until(() => viewer.element.querySelector('[aria-label="Next slide"]'), 'the navigation');
    const next = viewer.element.querySelector<HTMLButtonElement>('[aria-label="Next slide"]')!;
    const counter = () => viewer.element.querySelector('.counter')?.textContent;
    next.focus();
    next.click();
    expect([counter(), document.activeElement === next, next.isConnected]).toEqual(['Slide 2 of 3', true, true]);
    viewer.element.querySelector<HTMLButtonElement>('[aria-label="Increase reading size"]')!.click();
    const size = viewer.element.querySelector('.zoom')?.textContent;
    next.click();
    expect([counter(), next.disabled, viewer.element.querySelector('.zoom')?.textContent]).toEqual(['Slide 3 of 3', true, size]);
    viewer.dispose();
  });

  test("an EPUB's chapters are a menu with Previous and Next; an RTF of one page has neither", async () => {
    serve(kind => (kind === 'epub'
      ? { type: 'document', navigation: 'chapter', pages: ['Opening', 'Middle', 'Close'].map(title => ({ title, html: `<h1>${title}</h1>` })) }
      : { type: 'document', navigation: 'page', pages: [{ title: 'Page 1', html: '<p>only</p>' }] }));
    const book = officeViewer({ src: 'http://127.0.0.1:1/book.epub', name: 'book.epub', kind: 'epub', sizeBytes: 4, scheme: 'light' });
    document.body.append(book.element);
    await until(() => book.element.querySelector('.office-nav'), 'the navigation');
    const nav = book.element.querySelector('.office-nav')!;
    nav.querySelector<HTMLButtonElement>('[aria-label="Next chapter"]')!.click();
    expect([book.element.querySelector('.office-nav') === nav, nav.textContent?.includes('Middle'), book.element.querySelector('.surface-notice')]).toEqual([true, true, null]);
    book.dispose();
    const rtf = officeViewer({ src: 'http://127.0.0.1:1/note.rtf', name: 'note.rtf', kind: 'rtf', sizeBytes: 4, scheme: 'light' });
    document.body.append(rtf.element);
    await until(() => rtf.element.querySelector('.surface-notice'), 'the page');
    expect([rtf.element.querySelector('.office-nav'), rtf.element.querySelector('.surface-notice')?.textContent]).toEqual([null, 'Text and images. Layout and embedded objects may differ from the original.']);
    rtf.dispose();
  });

  test("a workbook's sheet table stays as the theme changes, so its scroll does too", async () => {
    serve(() => ({ type: 'ods', sheets: [{ name: 'Stock', rows: Array.from({ length: 32 }, (_, row) => [`a${row}`, `b${row}`]), truncated: false, rowCount: 400, columns: 2 }, { name: 'Prices', rows: [['x']], truncated: false, rowCount: 1, columns: 1 }] }));
    const viewer = officeViewer({ src: 'http://127.0.0.1:1/stock.ods', name: 'stock.ods', kind: 'ods', sizeBytes: 4, scheme: 'light' });
    document.body.append(viewer.element);
    await until(() => viewer.element.querySelector('.vt'), 'the sheet table');
    const table = viewer.element.querySelector('.vt');
    expect([viewer.element.querySelector('.status-line')?.textContent, !!table]).toEqual(['Sheet 1 of 2 · 400 rows · 2 columns · 1 KB', true]);
    viewer.setScheme('dark');
    expect(viewer.element.querySelector('.vt')).toBe(table);
    viewer.dispose();
  });
});
