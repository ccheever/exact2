// A navigation to a location whose last segment has a dot is the app's
// document, not a missing file (bsky5, b32: `/profile/jay.bsky.team` 404'd
// on the dev server); a request that is no navigation (an asset's fetch)
// still finds nothing, and a file that exists is still that file.
import {test, expect} from 'bun:test';
import {mkdirSync, mkdtempSync, rmSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {acceptsHTML, appDocumentPath} from '../../scripts/origin.mjs';
import {buildTreeFile, readWebRequest} from './serve.mjs';

test('a navigation to a dotted deep link is the document; a fetch of one is not', async () => {
  const dist = mkdtempSync(resolve(tmpdir(), 'exact-deep-link-'));
  try {
    writeFileSync(resolve(dist, 'index.html'), '<!doctype html><title>app</title>');
    mkdirSync(resolve(dist, 'assets'));
    writeFileSync(resolve(dist, 'assets/app.css'), 'body{}');
    const html = 'text/html,application/xhtml+xml,*/*;q=0.8';
    expect(appDocumentPath('/profile/jay.bsky.team')).toBe(false);
    expect(appDocumentPath('/profile/jay.bsky.team', true)).toBe(true);
    expect(appDocumentPath('/.exact/x', true)).toBe(false);
    expect((await readWebRequest(dist, '/profile/jay.bsky.team', html)).found?.route).toBe('/index.html');
    expect((await readWebRequest(dist, '/profile/jay.bsky.team')).found).toBe(null);
    expect((await readWebRequest(dist, '/assets/app.css', html)).found?.route).toBe('/assets/app.css');
    expect(buildTreeFile(dist, '/profile/jay.bsky.team', html)?.route).toBe('/index.html');
    expect(buildTreeFile(dist, '/profile/jay.bsky.team')).toBe(null);
    expect(buildTreeFile(dist, '/profile/alice')?.route).toBe('/index.html');
    // A missing file a page or an iframe navigates to is still missing.
    expect((await readWebRequest(dist, '/deck/missing.html', html)).found).toBe(null);
    expect((await readWebRequest(dist, '/assets/missing.pdf', html)).found).toBe(null);
    expect(buildTreeFile(dist, '/deck/missing.html', html)).toBe(null);
  } finally { rmSync(dist, {recursive: true, force: true}); }
});

test('acceptsHTML reads the media type, any case, at a quality above zero', () => {
  expect(acceptsHTML('TEXT/HTML')).toBe(true);
  expect(acceptsHTML('text/html;q=0.5, */*')).toBe(true);
  expect(acceptsHTML('text/html;q=0')).toBe(false);
  expect(acceptsHTML('application/json;profile="text/html"')).toBe(false);
  expect(acceptsHTML(undefined)).toBe(false);
  expect(acceptsHTML('*/*')).toBe(false);
});
