// The previews' frame sources (media-frame.ts): a PDF frame shows a local file, so a signed PDF is fetched and a draft's
// staged copy copied into app:/cache under a name that gives its type; a sent page is framed at its signed URL.
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { forgetMediaFrames, isAssetUrl, mediaFrameSource, stagedCopyName } from '../../src/media/media-frame';
import { memoryAppFiles, serveAssets } from '../support/asset-server';

const ID = '11111111-2222-3333-4444-555555555555';
const text = (value: string) => new TextEncoder().encode(value);
let server: ReturnType<typeof serveAssets>;
beforeEach(() => { forgetMediaFrames(); server = serveAssets(url => ({ body: `bytes of ${url}` })); });
afterEach(() => server.restore());

describe('a preview frame\'s source', () => {
  test('only a signed asset route of an http(s) server, and only a staged copy\'s own name', () => {
    expect(['http://127.0.0.1:1/api/assets/a', 'https://host/api/assets/t/b.pdf?sig=1'].map(isAssetUrl)).toEqual([true, true]);
    expect(['file:///etc/hosts', 'http://127.0.0.1:1/api/orchestration/shell', 'https://user@host/api/assets/x', 'app:/data/x', 'nonsense'].map(isAssetUrl))
      .toEqual([false, false, false, false, false]);
    expect([`t3-staged:${ID}`, `t3-staged:${ID.toUpperCase()}`, `t3-staged:${ID}.mp4`, 't3-staged:../t3-code.json', `x:${ID}`].map(stagedCopyName))
      .toEqual([ID, ID, null, null, null]);
  });

  test('a sent page is framed at its URL; anything else is refused', async () => {
    expect(await mediaFrameSource('html', 'http://127.0.0.1:1/api/assets/t/page.html')).toEqual({ src: 'http://127.0.0.1:1/api/assets/t/page.html', error: '' });
    expect(await mediaFrameSource('html', 'file:///etc/hosts')).toEqual({ src: '', error: 'Could not load this file.' });
    expect(await mediaFrameSource('pdf', 'http://127.0.0.1:1/api/other')).toEqual({ src: '', error: 'Could not load this file.' });
    expect(await mediaFrameSource('audio', 'http://127.0.0.1:1/api/assets/a')).toEqual({ src: '', error: '' });
    expect(server.calls).toEqual([]);
  });

  test('a signed PDF is fetched once into app:/cache under a .pdf name', async () => {
    const files = memoryAppFiles(), url = 'http://127.0.0.1:1/api/assets/t/report.pdf?sig=1';
    const first = await mediaFrameSource('pdf', url);
    expect(first.src).toMatch(/^app:\/cache\/media-preview\/\d+\.pdf$/);
    expect(new TextDecoder().decode(files.get(first.src))).toBe(`bytes of ${url}`);
    expect(await mediaFrameSource('pdf', url)).toEqual(first);
    expect(server.calls).toHaveLength(1);
  });

  test('a draft\'s PDF and page are their staged copies under .pdf and .html names; nothing is fetched', async () => {
    const files = memoryAppFiles(new Map([[`app:/data/composer-files/${ID}`, text('<a href="https://example.com/">x</a>')]]));
    const page = await mediaFrameSource('html', `t3-staged:${ID}`);
    expect(page.src).toMatch(/^app:\/cache\/media-preview\/\d+\.html$/);
    expect(files.get(page.src)).toEqual(files.get(`app:/data/composer-files/${ID}`));
    forgetMediaFrames();
    expect((await mediaFrameSource('pdf', `t3-staged:${ID}`)).src).toMatch(/\.pdf$/);
    expect(server.calls).toEqual([]);
  });

  test('a copy that cannot be made is not kept, so the next view tries again', async () => {
    const files = memoryAppFiles();
    expect(await mediaFrameSource('pdf', `t3-staged:${ID}`)).toEqual({ src: '', error: 'Could not load this file.' });
    files.set(`app:/data/composer-files/${ID}`, text('%PDF-1.4'));
    expect((await mediaFrameSource('pdf', `t3-staged:${ID}`)).error).toBe('');
  });
});
