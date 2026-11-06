// The viewer page's archive and database viewer under happy-dom (MIT reference, see LICENSE-T3: T3 Code 7c3b3b3605ef
// BinaryViewer.tsx, its ArchiveFile and PreviewImageContent.tsx): the archive's bar and scroll area kept as folders open,
// an entry sent to the app to draw, and the pictures this page draws itself.
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { domWindow, FakeWorker, network, page, until, viewerDom } from './viewer-dom';

viewerDom();
const { BinaryViewer, archiveEntry, APP_IMAGE_BYTES } = await import(`${page}/binary/binary-viewer.ts`);

const entries = [
  { path: 'docs/', size: 0, modified: null, directory: true },
  ...Array.from({ length: 30 }, (_, index) => ({ path: `docs/page-${String(index).padStart(2, '0')}.md`, size: 1200, modified: '2026-10-01T00:00:00Z', directory: false })),
  { path: 'logo.svg', size: 300, modified: null, directory: false },
  { path: 'photo.heic', size: 9000, modified: null, directory: false },
  { path: 'notes.txt', size: 12, modified: null, directory: false },
];
let posted: Record<string, unknown>[] = [];
const parent = domWindow.parent as unknown as { postMessage: (data: string) => void };
const original = parent.postMessage;
beforeEach(() => { posted = []; parent.postMessage = (data: string) => { posted.push(JSON.parse(data)); }; });
afterEach(() => { parent.postMessage = original; });

function openArchive() {
  network.respond = () => new Response(new Uint8Array([80, 75, 5, 6]), { headers: { 'content-length': '4' } });
  FakeWorker.answers['binary-preview.worker.js'] = (message, worker) => {
    if (message.type === 'open') worker.reply({ type: 'archive', entries });
    if (message.type === 'entry') worker.reply({ type: 'entry', path: message.path, bytes: new TextEncoder().encode(message.path === 'logo.svg' ? '<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"/>' : 'hello') });
  };
  const viewer = new BinaryViewer({ src: 'http://127.0.0.1:1/bundle.zip', name: 'bundle.zip', kind: 'archive' });
  document.body.append(viewer.element);
  return viewer;
}
const press = (root: HTMLElement, name: string) => [...root.querySelectorAll<HTMLButtonElement>('.entry-open')].find(button => button.textContent === name)!.click();

describe('BinaryViewer', () => {
  test("a folder opens in the archive's one scroll area, which keeps its place; Parent folder goes back up", async () => {
    const viewer = openArchive();
    await until(() => viewer.element.querySelector('.entry-open'), 'the entries');
    const bar = viewer.element.querySelector('.archive-bar')!, scroll = viewer.element.querySelector('.scroll-both')!;
    const back = bar.querySelector<HTMLButtonElement>('.surface-action')!;
    expect([bar.querySelector('.archive-place')?.textContent, bar.querySelector('.archive-count')?.textContent, back.getAttribute('aria-label'), back.disabled])
      .toEqual(['Archive contents', '34 entries · 1 KB', 'Parent folder', true]);
    expect([...viewer.element.querySelectorAll('.entry-open')].map(button => button.textContent)).toEqual(['docs', 'logo.svg', 'notes.txt', 'photo.heic']);
    scroll.scrollTop = 120;
    press(viewer.element, 'docs');
    expect([viewer.element.querySelector('.scroll-both') === scroll, scroll.scrollTop, viewer.element.querySelector('.archive-bar') === bar, back.disabled, bar.querySelector('.archive-place')?.textContent])
      .toEqual([true, 120, true, false, 'docs/']);
    expect(viewer.element.querySelectorAll('.entry-open').length).toBe(30);
    back.click();
    expect([viewer.element.querySelectorAll('.entry-open').length, back.disabled]).toEqual([4, true]);
    viewer.dispose();
  });

  test("an entry's text goes to the app to draw under the bar, and Back to archive closes it", async () => {
    const viewer = openArchive();
    await until(() => viewer.element.querySelector('.entry-open'), 'the entries');
    press(viewer.element, 'notes.txt');
    await until(() => posted.length, 'the entry sent');
    const back = viewer.element.querySelector<HTMLButtonElement>('.archive-bar .surface-action')!;
    expect([posted.at(-1), back.getAttribute('aria-label'), viewer.element.querySelector('.archive-place')?.textContent])
      .toEqual([{ type: 'entry', path: 'notes.txt', view: 'text', text: 'hello', truncated: false }, 'Back to archive', 'notes.txt']);
    back.click();
    expect([posted.at(-1), viewer.element.querySelectorAll('.entry-open').length]).toEqual([{ type: 'entry-closed' }, 4]);
    // A HEIC picture is one the reference's Chromium cannot decode.
    press(viewer.element, 'photo.heic');
    await until(() => posted.at(-1)?.path === 'photo.heic', 'the entry sent');
    expect(posted.at(-1)).toEqual({ type: 'entry', path: 'photo.heic', view: 'failure', message: 'Unable to load image.' });
    viewer.dispose();
  });

  test('an SVG entry is drawn here under its size and zoom bar, and nothing goes to the app', async () => {
    const viewer = openArchive();
    await until(() => viewer.element.querySelector('.entry-open'), 'the entries');
    press(viewer.element, 'logo.svg');
    await until(() => viewer.element.querySelector('.picture img'), 'the picture');
    const picture = viewer.element.querySelector('.picture')!;
    const image = picture.querySelector<HTMLImageElement>('img')!;
    expect([image.getAttribute('src')?.startsWith('blob:'), image.getAttribute('alt'), picture.querySelector('.picture-bar .tabular')?.textContent, picture.querySelector('.picture-percent')?.textContent])
      .toEqual([true, 'logo.svg', 'Loading image…', '100%']);
    expect([...picture.querySelectorAll('.surface-action')].map(button => [button.getAttribute('aria-label'), button.getAttribute('aria-pressed')]))
      .toEqual([['Zoom out', null], ['Zoom in', null], ['Fit image', 'true'], ['Actual image size', 'false']]);
    // Its size comes when it loads: fit to the viewport, then zoomed in a step.
    Object.defineProperty(image, 'naturalWidth', { value: 400 });
    Object.defineProperty(image, 'naturalHeight', { value: 200 });
    image.dispatchEvent(new domWindow.Event('load'));
    picture.querySelector<HTMLButtonElement>('[aria-label="Zoom in"]')!.click();
    expect([picture.querySelector('.picture-bar .tabular')?.textContent, picture.querySelector('[aria-label="Fit image"]')?.getAttribute('aria-pressed')]).toEqual(['400 × 200', 'false']);
    expect(posted.filter(message => message.type === 'entry' || message.type === 'entry-closed')).toEqual([]);
    viewer.element.querySelector<HTMLButtonElement>('.archive-bar .surface-action')!.click();
    expect([viewer.element.querySelector('.picture'), viewer.element.querySelectorAll('.entry-open').length]).toEqual([null, 4]);
    viewer.dispose();
  });

  test('a picture is sent to the app up to 8 MB; a larger one, and an SVG, the page draws', () => {
    expect(archiveEntry('a.png', new Uint8Array(4))).toEqual({ view: 'image', data: 'AAAAAA==' });
    expect(archiveEntry('a.png', new Uint8Array(APP_IMAGE_BYTES + 1))).toEqual({ view: 'page' });
    expect(archiveEntry('icon.SVG', new Uint8Array(4))).toEqual({ view: 'page' });
    expect(archiveEntry('scan.tiff', new Uint8Array(4))).toEqual({ view: 'failure', message: 'Unable to load image.' });
  });

  test("a database's bar stays as its pages turn: Previous and Next rows, the range and the status", async () => {
    network.respond = () => new Response(new Uint8Array([83, 81, 76]), { headers: { 'content-length': '3' } });
    FakeWorker.answers['binary-preview.worker.js'] = (message, worker) => {
      if (message.type === 'open') worker.reply({ type: 'sqlite', tables: [{ name: 'orders', count: 120 }] });
      if (message.type === 'rows') worker.reply({ type: 'rows', table: message.table, page: message.page, columns: ['id', 'total'], rows: [[String(message.page * 50 + 1), '9.50']] });
    };
    const viewer = new BinaryViewer({ src: 'http://127.0.0.1:1/app.sqlite', name: 'app.sqlite', kind: 'sqlite' });
    document.body.append(viewer.element);
    await until(() => viewer.element.querySelector('.db-rows td'), 'the first rows');
    const next = viewer.element.querySelector<HTMLButtonElement>('[aria-label="Next rows"]')!;
    expect([viewer.element.querySelector('.range')?.textContent, viewer.element.querySelector('.status-line')?.textContent]).toEqual(['1–50 of 120', '120 rows · 2 columns · 1 KB']);
    next.click();
    await until(() => viewer.element.querySelector('.db-rows td')?.textContent === '51', 'the next rows');
    expect([viewer.element.querySelector('[aria-label="Next rows"]') === next, viewer.element.querySelector('.range')?.textContent, viewer.element.querySelector('.db-rows td')?.textContent])
      .toEqual([true, '51–100 of 120', '51']);
    viewer.dispose();
  });
});
