// A navigation to a location whose last segment has a dot is the app's
// document, not a missing file (bsky5, b32: `/profile/jay.bsky.team` 404'd
// on the dev server); a request that is no navigation (an asset's fetch)
// still finds nothing, and a file that exists is still that file.
import {afterEach, test, expect} from 'bun:test';
import {mkdirSync, mkdtempSync, rmSync, readFileSync, existsSync, writeFileSync} from 'node:fs';
import {createServer} from 'node:net';
import {spawnOwned, stopAll} from '../../scripts/test-children.mjs';
import {spawnSync} from 'node:child_process';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {acceptsHTML, appDocumentPath, DirectoryOrigin} from '../../scripts/origin.mjs';
import {publishRoot} from '../../scripts/deploy.mjs';
import {canonical, locationOf, matchPattern} from '../web-js/route-pattern.js';
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
    expect(appDocumentPath('/profile/photos.zip', true)).toBe(false);
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
    expect((await readWebRequest(dist, '/assets/hero.model', html)).found).toBe(null);
  } finally { rmSync(dist, {recursive: true, force: true}); }
});

test('acceptsHTML reads the media type, any case, at a quality above zero', () => {
  expect(acceptsHTML('TEXT/HTML')).toBe(true);
  expect(acceptsHTML('text/html;q=0.5, */*')).toBe(true);
  expect(acceptsHTML('text/html;q=0')).toBe(false);
  expect(acceptsHTML('application/json;profile="text/html"')).toBe(false);
  expect(acceptsHTML(undefined)).toBe(false);
  expect(acceptsHTML('*/*')).toBe(false);
  expect(acceptsHTML('application/json;profile="a,text/html,b"')).toBe(false);
  expect(acceptsHTML('application/json;profile="a\\",text/html"')).toBe(false);
  expect(acceptsHTML('application/vnd.exact.envelope+json, text/html;q=0.9')).toBe(true);
});

// Use the compiler's wire format, so the server's route reader is checked
// against what both web targets actually ship.
const sourceFor = pattern => `routes nav
  home "/"
  profile "${pattern}"
  literal "/café/report.pdf"
  asset "/assets/:name"
  notfound

component App
  view
    text top(nav).url
`;
function plan(dist, pattern) {
  const source = resolve(dist, 'routes.contract');
  writeFileSync(source, sourceFor(pattern));
  const built = spawnSync(resolve(process.env.CARGO_TARGET_DIR ?? 'target', 'debug/contract'),
    ['build', source, '-o', resolve(dist, 'app.plan')], {encoding: 'utf8'});
  if (built.error || built.status !== 0) throw new Error(`build the contract binary first: ${built.error ?? built.stderr}`);
}

test('declared routes disambiguate file extensions on both serving paths and follow rebuilt plans', async () => {
  const dist = mkdtempSync(resolve(tmpdir(), 'exact-route-extension-'));
  try {
    writeFileSync(resolve(dist, 'index.html'), '<!doctype html><title>app</title>');
    mkdirSync(resolve(dist, 'assets'));
    writeFileSync(resolve(dist, 'assets/real.pdf'), 'real file');
    plan(dist, '/profile/:handle');
    const published = resolve(dist, 'origin'), origin = new DirectoryOrigin(published);
    await publishRoot({origin, row: {}, web: dist});
    for (const read of [async (path, accept) => (await readWebRequest(dist, path, accept)).found,
      async (path, accept) => buildTreeFile(dist, path, accept),
      async (path, accept) => (await readWebRequest(published, path, accept)).found]) {
      for (const path of ['/profile/photos.zip', '/profile/photos%2Ezip', '/profile/a%2Fb.zip', '/caf%C3%A9/report.pdf']) {
        expect((await read(path, 'text/html'))?.route, path).toBe('/index.html');
        expect(await read(path, '*/*'), path).toBe(null);
      }
      for (const path of ['/deck/missing.html', '/missing.pdf', '/profile/photos.zip/', '/profile//photos.zip',
        '/profile%2Fphotos.zip', '/caf%c3%a9/report.pdf', '/profile/a/photos.zip', '/profile/%2e%2e/photos.zip', '/profile/.hidden.zip', '/profile/a%2f.hidden.zip',
        '/profile/%00.zip', '/profile/a%5Cb.zip', '/profile/%zz.zip']) {
        expect(await read(path, 'text/html'), path).toBe(null);
      }
      expect((await read('/assets/real.pdf', 'text/html'))?.route).toBe('/assets/real.pdf');
      expect((await read('/assets/missing.pdf', 'text/html'))?.route).toBe('/index.html');
    }
    expect((await readWebRequest(published, '/profile/photos.zip', 'text/html')).found?.immutable).toBe(false);
    plan(dist, '/changed/:handle');
    await publishRoot({origin, row: {}, web: dist});
    for (const path of ['/profile/photos.zip', '/changed/photos.zip']) {
      const expected = path.startsWith('/changed/') ? '/index.html' : undefined;
      expect((await readWebRequest(dist, path, 'text/html')).found?.route).toBe(expected);
      expect(buildTreeFile(dist, path, 'text/html')?.route).toBe(expected);
      expect((await readWebRequest(published, path, 'text/html')).found?.route).toBe(expected);
    }
    writeFileSync(resolve(dist, 'app.plan'), 'invalid plan');
    expect((await readWebRequest(dist, '/changed/photos.zip', 'text/html')).found).toBe(null);
    expect(buildTreeFile(dist, '/changed/photos.zip', 'text/html')).toBe(null);
  } finally { rmSync(dist, {recursive: true, force: true}); }
});

test('the shared pattern matcher retains the route corpus semantics without notfound', () => {
  const corpus = JSON.parse(readFileSync(new URL('../../route/tests/corpus.json', import.meta.url), 'utf8'));
  for (const {input, expected} of corpus.canonical) expect(canonical(input), input).toBe(expected);
  for (const {input, expected} of corpus.location_of) expect(locationOf(input), input).toBe(expected);
  for (const {table, location, expected} of corpus.matches) {
    const routes = corpus.tables[table].routes, params = Object.fromEntries(corpus.param_names[table].map(name => [name, '']));
    const index = matchPattern(routes, canonical(location), (name, value) => { params[name] = value; });
    const matched = index < 0 ? null : {name: routes[index].name, params};
    expect(matched, location).toEqual(expected?.name === 'notfound' ? null : expected);
  }
});

afterEach(() => stopAll());
test('JS dev loop serves declared file-like routes and updates them after an edit', async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-deep-link-dev-'));
  const app = resolve(dir, 'deep-links'), dist = resolve(dir, 'dist');
  mkdirSync(app);
  writeFileSync(resolve(app, 'app.json'), JSON.stringify({name: 'Deep links', app: {id: 'com.exact.deep-links', name: 'Deep links'}}));
  const source = resolve(app, 'app.contract');
  writeFileSync(source, sourceFor('/profile/:handle'));
  const listener = createServer();
  await new Promise(ok => listener.listen(0, '127.0.0.1', ok));
  const port = listener.address().port;
  await new Promise(ok => listener.close(ok));
  const root = resolve(import.meta.dir, '../..');
  const dev = spawnOwned(process.execPath, ['host/web/dev.mjs', '--app', app, '--port', String(port)],
    {cwd: root, env: {...process.env, EXACT_APP_DIR: app, EXACT_WEB_DIST: dist}, stdio: ['ignore', 'pipe', 'pipe']});
  let said = '', listening = false;
  const hear = data => { said = (said + data).slice(-12000); listening ||= said.includes(`127.0.0.1:${port}/`); };
  dev.stdout.on('data', hear); dev.stderr.on('data', hear);
  const until = async pass => {
    const end = Date.now() + 900_000;
    while (!pass()) {
      if (dev.exitCode !== null || Date.now() > end) throw new Error(`dev build did not arrive: ${said}`);
      await Bun.sleep(100);
    }
  };
  const status = async (path, accept = 'text/html') => (await fetch(`http://127.0.0.1:${port}${path}`, {headers: {accept}})).status;
  try {
    await until(() => listening && existsSync(resolve(dist, 'app.plan')));
    expect(await status('/profile/photos.zip')).toBe(200);
    expect(await status('/deck/missing.html')).toBe(404);
    expect(await status('/missing.pdf')).toBe(404);
    expect(await status('/profile/photos.zip', '*/*')).toBe(404);
    const before = readFileSync(resolve(dist, 'app.plan'));
    writeFileSync(source, sourceFor('/changed/:handle'));
    await until(() => { try { return !readFileSync(resolve(dist, 'app.plan')).equals(before); } catch { return false; } });
    expect(await status('/changed/photos.zip')).toBe(200);
    expect(await status('/profile/photos.zip')).toBe(404);
  } finally {
    await dev.stop(30_000);
    rmSync(dir, {recursive: true, force: true});
  }
}, 1_200_000);
