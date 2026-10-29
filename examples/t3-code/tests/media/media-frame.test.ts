// The previews' frame sources (media-frame.ts): a sent page is framed at its signed URL and a draft page is its staged
// copy in app:/cache under an `.html` name; a draft the viewer page is given is copied there under its own extension.
// A draft's copies go with the draft, and the numbered copies an earlier launch left go before this launch's first.
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { cachedStagedCopy, forgetMediaFrames, forgetStagedCopies, isAssetUrl, mediaFrameSource, stagedCopyName } from '../../src/media/media-frame';
import { removeStaged } from '../../src/composer/composer-staged-copies';
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
    expect(await mediaFrameSource('http://127.0.0.1:1/api/assets/t/page.html')).toEqual({ src: 'http://127.0.0.1:1/api/assets/t/page.html', error: '' });
    expect(await mediaFrameSource('file:///etc/hosts')).toEqual({ src: '', error: 'Could not load this file.' });
    expect(await mediaFrameSource('')).toEqual({ src: '', error: '' });
    expect(server.calls).toEqual([]);
  });

  test('a draft page is its staged copy under an .html name; nothing is fetched', async () => {
    const files = memoryAppFiles(new Map([[`app:/data/composer-files/${ID}`, text('<a href="https://example.com/">x</a>')]]));
    const page = await mediaFrameSource(`t3-staged:${ID}`);
    expect(page.src).toMatch(/^app:\/cache\/media-preview\/\d+\.html$/);
    expect(files.get(page.src)).toEqual(files.get(`app:/data/composer-files/${ID}`));
    expect(server.calls).toEqual([]);
  });

  test('a draft the viewer page is given is copied once under its own extension', async () => {
    const files = memoryAppFiles(new Map([[`app:/data/composer-files/${ID}`, text('%PDF-1.4 large draft')]]));
    const given = await cachedStagedCopy(ID, 'Quarterly Report.PDF');
    expect([given, new TextDecoder().decode(files.get(given))]).toEqual([expect.stringMatching(/^app:\/cache\/media-preview\/\d+\.pdf$/), '%PDF-1.4 large draft']);
    expect(await cachedStagedCopy(ID, 'Quarterly Report.PDF')).toBe(given);
    forgetMediaFrames();
    expect(await cachedStagedCopy(ID, 'notes')).toMatch(/\.bin$/);
  });

  test("a draft's copies go when the draft is removed or sent, or when its preview asks for them afresh", async () => {
    const files = memoryAppFiles(new Map([[`app:/data/composer-files/${ID}`, text('<p>x</p>')]]));
    const page = (await mediaFrameSource(`t3-staged:${ID}`)).src, given = await cachedStagedCopy(ID, 'large.pdf');
    expect([files.has(page), files.has(given)]).toEqual([true, true]);
    // removeStaged is what a removed chip and a finished upload call.
    await removeStaged(ID);
    await new Promise(resolve => setTimeout(resolve, 0));
    expect([files.has(page), files.has(given), files.has(`app:/data/composer-files/${ID}`)]).toEqual([false, false, false]);
    // A copy gone from the cache while its path was kept is made again once it is forgotten (a preview's Try again).
    files.set(`app:/data/composer-files/${ID}`, text('<p>y</p>'));
    const again = await cachedStagedCopy(ID, 'large.pdf');
    files.delete(again);
    expect(await cachedStagedCopy(ID, 'large.pdf')).toBe(again);
    forgetStagedCopies(ID);
    const fresh = await cachedStagedCopy(ID, 'large.pdf');
    expect(new TextDecoder().decode(files.get(fresh))).toBe('<p>y</p>');
  });

  test('the numbered copies an earlier launch left go before the first copy, and nothing else in the folder', async () => {
    const files = memoryAppFiles(new Map([[`app:/data/composer-files/${ID}`, text('<p>x</p>')], ['app:/cache/media-preview/3.pdf', text('old')],
      ['app:/cache/media-preview/12.html', text('old')], ['app:/cache/media-preview/notes.txt', text('kept')], ['app:/cache/other/4.pdf', text('kept')]]));
    const page = (await mediaFrameSource(`t3-staged:${ID}`)).src;
    expect([...files.keys()].filter(path => path.startsWith('app:/cache/')).sort())
      .toEqual(['app:/cache/media-preview/notes.txt', 'app:/cache/other/4.pdf', page].sort());
    // Once a launch: a later copy leaves this launch's copies alone.
    const given = await cachedStagedCopy(ID, 'large.pdf');
    expect([files.has(page), files.has(given)]).toEqual([true, true]);
  });

  test('a copy that cannot be made is not kept, so the next view tries again', async () => {
    const files = memoryAppFiles();
    expect(await mediaFrameSource(`t3-staged:${ID}`)).toEqual({ src: '', error: 'Could not load this file.' });
    files.set(`app:/data/composer-files/${ID}`, text('<p>x</p>'));
    expect((await mediaFrameSource(`t3-staged:${ID}`)).error).toBe('');
  });
});
