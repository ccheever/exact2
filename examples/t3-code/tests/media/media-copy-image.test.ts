// Copy image's readMediaPng (mediaContent.ts, T3 Code 7c3b3b3605ef; MIT, see LICENSE-T3) as media-copy-image.ts runs it:
// the bytes `fetch` writes to a file, a PNG written as it is, anything else drawn into a PNG, the reference's sentences,
// and @exact/clipboard's `write`, here a fake.
import { afterEach, describe, expect, test } from 'bun:test';
import { copyImageFile, copyMediaImage, dataUrlBytes, FETCH_FAILED, NOT_PNG, TOO_LARGE, UNDECODABLE, useClipboardCalls, WEB_PAGE } from '../../src/media/media-copy-image';
import { adoptAppFiles } from '../../src/media/save-file';
import type { Obj } from '../../src/client/domain';

type Reply = { type?: string; status?: number } | 'offline';
const original = globalThis.fetch;
afterEach(() => { globalThis.fetch = original; useClipboardCalls(null); });

function copying(replies: Record<string, Reply>, options: { write?: () => Promise<void>; drawn?: { width: number; height: number } | { code: string } } = {}) {
  const fetched: Obj[] = [], written: Obj[][] = [], drawn: Obj[] = [], files = new Map<string, Uint8Array>(), removed: string[] = [];
  globalThis.fetch = (async (url: string, init: Obj) => {
    fetched.push({ url: String(url), to: init.exactBodyTo });
    const reply = replies[String(url)];
    if (!reply || reply === 'offline') throw new TypeError('Network connection lost');
    return new Response('', { status: reply.status ?? 200, headers: { 'content-type': reply.type ?? '' } });
  }) as unknown as typeof fetch;
  adoptAppFiles({ fs: {
    readFile: async () => new ArrayBuffer(0), mkdir: async () => {},
    writeFile: async (path: string, bytes: Uint8Array) => { files.set(path, bytes); },
    rm: async (path: string) => { removed.push(path); },
    transformImage: async (from: string, to: string, transform: Obj) => {
      drawn.push({ from, to, ...transform });
      const result = options.drawn ?? { width: 40, height: 24 };
      if ('code' in result) throw Object.assign(new Error(result.code), { code: result.code });
      return { path: to, ...result };
    },
  } });
  useClipboardCalls({ write: async items => { await options.write?.(); written.push(items as Obj[]); }, ClipboardItem: class { constructor(items: Obj) { Object.assign(this, items); } } });
  return { fetched, written, drawn, files, removed };
}
const refused = async (run: Promise<void>) => { try { await run; return 'copied'; } catch (error) { return (error as Error).message; } };
const svgNever = async () => { throw new Error('not an SVG'); };

describe('copyMediaImage', () => {
  test('a PNG goes to the clipboard as the file fetch wrote; anything else is drawn into a PNG at its own size first', async () => {
    const copy = copying({ 'https://cdn.example/a.png': { type: 'image/png' }, 'https://cdn.example/b.jpg': { type: 'image/jpeg; charset=binary' } });
    await copyMediaImage('https://cdn.example/a.png', svgNever);
    await copyMediaImage('https://cdn.example/b.jpg', svgNever);
    const [png, jpeg] = copy.fetched;
    expect(String(png!.to)).toMatch(/^app:\/tmp\/copy-image\/\d+-source$/);
    const jpegPng = String(jpeg!.to).replace(/-source$/, '.png');
    expect(copy.drawn).toEqual([{ from: jpeg!.to, to: jpegPng, type: 'image/png' }]);
    expect(copy.written).toEqual([[{ 'image/png': png!.to }], [{ 'image/png': jpegPng }]]);
    // Each copy's files go once it is written, so the folder does not grow over a session.
    expect(copy.removed).toEqual(expect.arrayContaining([png!.to, jpeg!.to, jpegPng]));
  });

  test('an image file by its path (a Browser artifact the module staged) is drawn into a PNG, written and removed', async () => {
    const copy = copying({});
    await copyImageFile('app:/tmp/copy-image/artifact-1.png');
    const [drawn] = copy.drawn;
    expect(drawn).toMatchObject({ from: 'app:/tmp/copy-image/artifact-1.png', type: 'image/png' });
    expect(copy.written).toEqual([[{ 'image/png': drawn!.to }]]);
    expect(copy.removed).toEqual(expect.arrayContaining(['app:/tmp/copy-image/artifact-1.png', drawn!.to]));
  });

  test('the reference\'s refusals: a web page, an HTTP error, no connection, no image, and too many pixels', async () => {
    copying({ 'https://h/page': { type: 'text/html; charset=utf-8' }, 'https://h/gone.png': { status: 404 }, 'https://h/off.png': 'offline' });
    expect(await refused(copyMediaImage('https://h/page', svgNever))).toBe(WEB_PAGE);
    expect(await refused(copyMediaImage('https://h/gone.png', svgNever))).toBe('The file could not be fetched (HTTP 404).');
    expect(await refused(copyMediaImage('https://h/off.png', svgNever))).toBe(FETCH_FAILED);
    copying({ 'https://h/x.jpg': { type: 'image/jpeg' } }, { drawn: { code: 'undecodable' } });
    expect(await refused(copyMediaImage('https://h/x.jpg', svgNever))).toBe(UNDECODABLE);
    copying({ 'https://h/x.jpg': { type: 'image/jpeg' } }, { drawn: { code: 'too-large' } });
    expect(await refused(copyMediaImage('https://h/x.jpg', svgNever))).toBe(TOO_LARGE);
    // The codec draws up to 64 Mi pixels; the reference refuses past 64,000,000.
    const big = copying({ 'https://h/x.webp': { type: 'image/webp' } }, { drawn: { width: 10_000, height: 6_500 } });
    expect(await refused(copyMediaImage('https://h/x.webp', svgNever))).toBe(TOO_LARGE);
    expect(big.written).toEqual([]);
  });

  test('the clipboard\'s DataError is the reference\'s sentence: too large, else not a PNG', async () => {
    copying({ 'https://h/a.png': { type: 'image/png' } }, { write: async () => { throw new DOMException('too large: 9000×9000 is over 64,000,000 pixels', 'DataError'); } });
    expect(await refused(copyMediaImage('https://h/a.png', svgNever))).toBe(TOO_LARGE);
    copying({ 'https://h/a.png': { type: 'image/png' } }, { write: async () => { throw new DOMException('image/png does not decode as PNG', 'DataError'); } });
    expect(await refused(copyMediaImage('https://h/a.png', svgNever))).toBe(NOT_PNG);
  });

  test('an SVG goes to the module, which draws it; an inline data: image is decoded here', async () => {
    const copy = copying({ 'https://h/mark.svg': { type: 'image/svg+xml' }, 'https://h/untyped.svg?v=2': {} });
    let drawnBySvg = 0;
    await copyMediaImage('https://h/mark.svg', async () => { drawnBySvg++; });
    await copyMediaImage('https://h/untyped.svg?v=2', async () => { drawnBySvg++; });
    expect([drawnBySvg, copy.written.length]).toEqual([2, 0]);
    await copyMediaImage('data:image/png;base64,iVBORw0KGgo=', svgNever);
    expect(copy.fetched.map(entry => entry.url)).toEqual(['https://h/mark.svg', 'https://h/untyped.svg?v=2']);
    const [path] = [...copy.files.keys()];
    expect([...copy.files.get(path!)!]).toEqual([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
    expect(copy.written).toEqual([[{ 'image/png': path }]]);
    expect(dataUrlBytes('data:,a%20b')).toEqual({ type: 'text/plain', bytes: new TextEncoder().encode('a b') });
    expect(dataUrlBytes('data:image/svg+xml;charset=utf-8,%3Csvg%2F%3E').type).toBe('image/svg+xml');
  });
});
