// A navigation to a location whose last segment has a dot is the app's
// document, not a missing file (bsky5, b32: `/profile/jay.bsky.team` 404'd
// on the dev server); a request that is no navigation (an asset's fetch)
// still finds nothing, and a file that exists is still that file.
import {afterAll, afterEach, beforeAll, describe, test, expect, spyOn} from 'bun:test';
import {mkdirSync, mkdtempSync, rmSync, readFileSync, renameSync, writeFileSync} from 'node:fs';
import {createServer} from 'node:net';
import {createServer as httpServer} from 'node:http';
import * as filesystem from '../../scripts/filesystem.mjs';
import {spawnOwned, stopAll} from '../../scripts/test-children.mjs';
import {spawnSync} from 'node:child_process';
import {createInterface} from 'node:readline';
import {Readable} from 'node:stream';
import {createApp} from '../../game/new.mjs';
import {developmentBuildEnv} from '../../scripts/app.mjs';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {acceptsHTML, appDocumentPath, DirectoryOrigin, webRootPath} from '../../scripts/origin.mjs';
import {publishRoot} from '../../scripts/deploy.mjs';
import {canonical, locationOf, matchPattern} from '../web-js/route-pattern.js';
import {buildTreeFile, buildTreeFileAsync, planRouteTable, readWebRequest, serveBuildTree} from './serve.mjs';

test('a navigation to a dotted deep link is the document; a fetch of one is not', async () => {
  const dist = mkdtempSync(resolve(tmpdir(), 'exact-deep-link-'));
  try {
    writeFileSync(resolve(dist, 'index.html'), '<!doctype html><title>app</title>');
    // A JS-target build's shell and route table (host/web-js/build.mjs).
    writeFileSync(resolve(dist, 'shell.html'), '<!doctype html><title>app</title>');
    writeFileSync(resolve(dist, 'routes.json'), JSON.stringify({routes: [{pattern: '/', notfound: false}, {pattern: '/profile/:handle', notfound: false}]}));
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
    expect(buildTreeFile(dist, '/profile/jay.bsky.team', html)?.route).toBe('/shell.html');
    expect(buildTreeFile(dist, '/profile/jay.bsky.team')).toBe(null);
    // A build tree's shell is a navigation's (LLP 1153 D3.2): a fetch of a route's path, dotted or not, is no page.
    expect(buildTreeFile(dist, '/profile/alice', html)?.route).toBe('/shell.html');
    for (const accept of ['', '*/*', 'application/json']) expect(buildTreeFile(dist, '/profile/alice', accept), accept).toBe(null);
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
// The plan, and the shell and route table a JS-target build writes beside it (host/web-js/build.mjs).
function plan(dist, pattern) {
  const source = resolve(dist, 'routes.contract');
  writeFileSync(source, sourceFor(pattern));
  const built = spawnSync(resolve(process.env.CARGO_TARGET_DIR ?? 'target', 'debug/contract'),
    ['build', source, '-o', resolve(dist, 'app.plan')], {encoding: 'utf8'});
  if (built.error || built.status !== 0) throw new Error(`build the contract binary first: ${built.error ?? built.stderr}`);
  writeFileSync(resolve(dist, 'routes.json'), JSON.stringify({routes: planRouteTable(readFileSync(resolve(dist, 'app.plan')))}));
  writeFileSync(resolve(dist, 'shell.html'), '<!doctype html><title>shell</title>');
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
    // A static origin's document is index.html; a JS-target build's tree serves a route's shell.html.
    for (const [read, page] of [[async (path, accept) => (await readWebRequest(dist, path, accept)).found, '/index.html'],
      [async (path, accept) => buildTreeFile(dist, path, accept), '/shell.html'],
      [async (path, accept) => (await readWebRequest(published, path, accept)).found, '/index.html']]) {
      for (const path of ['/profile/photos.zip', '/profile/photos%2Ezip', '/profile/a%2Fb.zip', '/caf%C3%A9/report.pdf']) {
        expect((await read(path, 'text/html'))?.route, path).toBe(page);
        expect(await read(path, '*/*'), path).toBe(null);
      }
      for (const path of ['/deck/missing.html', '/missing.pdf', '/profile/photos.zip/', '/profile//photos.zip',
        '/profile%2Fphotos.zip', '/caf%c3%a9/report.pdf', '/profile/a/photos.zip', '/profile/%2e%2e/photos.zip', '/profile/.hidden.zip', '/profile/a%2f.hidden.zip',
        '/profile/%00.zip', '/profile/a%5Cb.zip', '/profile/%zz.zip']) {
        expect(await read(path, 'text/html'), path).toBe(null);
      }
      expect((await read('/assets/real.pdf', 'text/html'))?.route).toBe('/assets/real.pdf');
      expect((await read('/assets/missing.pdf', 'text/html'))?.route).toBe(page);
    }
    expect((await readWebRequest(published, '/profile/photos.zip', 'text/html')).found?.immutable).toBe(false);
    plan(dist, '/changed/:handle');
    await publishRoot({origin, row: {}, web: dist});
    for (const path of ['/profile/photos.zip', '/changed/photos.zip']) {
      const expected = path.startsWith('/changed/') ? '/index.html' : undefined;
      expect((await readWebRequest(dist, path, 'text/html')).found?.route).toBe(expected);
      expect(buildTreeFile(dist, path, 'text/html')?.route).toBe(expected && '/shell.html');
      expect((await readWebRequest(published, path, 'text/html')).found?.route).toBe(expected);
    }
    writeFileSync(resolve(dist, 'app.plan'), 'invalid plan');
    writeFileSync(resolve(dist, 'routes.json'), 'invalid route table');
    expect((await readWebRequest(dist, '/changed/photos.zip', 'text/html')).found).toBe(null);
    expect(buildTreeFile(dist, '/changed/photos.zip', 'text/html')).toBe(null);
  } finally { rmSync(dist, {recursive: true, force: true}); }
});

// LLP 1153 D3: a JS-target build is served as any static host serves it: a file, else for an HTML navigation a declared
// route's shell with 200, else 404.html with 404. With no 404.html a page's miss gets the shell, whose client draws
// the notfound route; a request for anything but HTML at a route's path is a miss.
test('a build tree answers a miss with 404 and its 404.html, or a page with the shell', async () => {
  const dist = mkdtempSync(resolve(tmpdir(), 'exact-hosting-rule-'));
  let server;
  try {
    plan(dist, '/profile/:handle');
    writeFileSync(resolve(dist, 'index.html'), '<title>home</title>');
    writeFileSync(resolve(dist, '404.html'), '<title>not found</title>');
    server = httpServer((req, res) => serveBuildTree(dist, req, res));
    await new Promise(ok => server.listen(0, '127.0.0.1', ok));
    const get = async (path, accept = '*/*') => {
      const r = await fetch(`http://127.0.0.1:${server.address().port}${path}`, {headers: {accept}});
      return [r.status, await r.text()];
    };
    const shell = '<!doctype html><title>shell</title>', missing = '<title>not found</title>';
    expect(await get('/')).toEqual([200, '<title>home</title>']);
    expect(await get('/profile/alice', 'text/html')).toEqual([200, shell]);
    expect(await get('/profile/alice.png', 'text/html')).toEqual([200, shell]);
    for (const accept of ['*/*', 'application/json']) expect(await get('/profile/alice', accept), accept).toEqual([404, missing]);
    for (const path of ['/no/such/route', '/nope.png', '/profile/alice.png', '/profile/alice/']) expect(await get(path), path).toEqual([404, missing]);
    expect(await get('/no/such/route', 'text/html')).toEqual([404, missing]);
    rmSync(resolve(dist, '404.html'));
    expect(await get('/no/such/route', 'text/html')).toEqual([404, shell]);
    for (const path of ['/nope.png', '/no/such/route']) expect(await get(path, 'application/json'), path).toEqual([404, '']);
    // Without a 404 page, a request for a page that does not ask for HTML (curl's) is told how to ask.
    expect(await get('/profile/alice')).toEqual([404, "/profile/alice is a page: ask for it as a browser does, with Accept: text/html (curl -H 'Accept: text/html' …).\n"]);
    // Asked without `miss`, a miss is nothing, as before: the dev gallery and the tool serve only what is there.
    expect(buildTreeFile(dist, '/no/such/route', 'text/html')).toBe(null);
    expect(buildTreeFile(dist, '/no/such/route', 'text/html', {miss: true})).toMatchObject({route: '/shell.html', status: 404});
  } finally {
    if (server) { server.closeAllConnections(); await new Promise(ok => server.close(ok)); }
    rmSync(dist, {recursive: true, force: true});
  }
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
// Hold producer replies so navigation can be tested before admission and after a host rebuild.
const holdProducer = `
import {ChildProcess} from 'node:child_process';
const spawn = ChildProcess.prototype.spawn;
ChildProcess.prototype.spawn = function(options) {
  const result = spawn.call(this, options);
  if (options.file.endsWith('/exact-js-bake') && options.args.includes('--serve')) {
    const stream = this.stdout, emit = stream.emit, buffered = [];
    let held = true;
    stream.emit = function(event, ...args) {
      if (held && event === 'data') { buffered.push(args); return true; }
      return emit.call(this, event, ...args);
    };
    const admit = input => {
      if (String(input).trim() !== 'admit') return;
      held = false;
      for (const args of buffered.splice(0)) emit.call(stream, 'data', ...args);
    };
    process.stdin.on('data', admit);
    this.once('exit', () => process.stdin.off('data', admit));
  }
  return result;
};
`;
// Fixture compilation is preparation; every wait on a running loop is bounded (30 s; the wasm loop's
// stream waits 2 min). The wasm loop's case is in the slow lane (EXACT_SLOW_TESTS=1): its new app's
// first start builds the module producer and the app's wasm crates cold (the warm-up below), and its
// `r` rebuilds the wasm, which on a loaded machine runs past any cap the default lane can afford.
const SLOW = process.env.EXACT_SLOW_TESTS === '1';
for (const wasm of [false, true]) describe.skipIf(wasm && !SLOW)(`${wasm ? 'wasm' : 'JS'} dev routes`, () => {
  let dir, app, dist, source, env, preload;
  const root = resolve(import.meta.dir, '../..');
  beforeAll(async () => {
    dir = mkdtempSync(resolve(tmpdir(), 'exact-deep-link-dev-'));
    app = resolve(dir, 'deep-links'); dist = resolve(dir, 'dist');
    if (wasm) {
      preload = resolve(dir, 'hold-producer.mjs');
      writeFileSync(preload, holdProducer);
      createApp(app, {id: 'com.exact.deep-links', title: 'Deep links'});
      writeFileSync(resolve(app, 'app.ts'), "export const appId = 'com.exact.deep-links';\nexport const grants = '';\nexport const answer = () => { throw new Error('no sources'); };\n");
    } else {
      mkdirSync(app);
      writeFileSync(resolve(app, 'app.json'), JSON.stringify({name: 'Deep links', app: {id: 'com.exact.deep-links', name: 'Deep links'}}));
    }
    source = resolve(app, 'app.contract');
    writeFileSync(source, sourceFor('/profile/:handle'));
    env = {...developmentBuildEnv(), EXACT_APP_DIR: app, EXACT_WEB_DIST: dist,
      EXACT_UPDATE_TRUST: 'development', EXACT_WEB_LINK: 'all', EXACT_DEV_BAKE: resolve(dist, 'bake.json')};
    const prepare = (command, args) => new Promise((ok, no) => {
      const child = spawnOwned(command, args, {cwd: root, env, stdio: ['ignore', 'pipe', 'pipe']});
      let said = '';
      const hear = data => { said = (said + data).slice(-12000); };
      child.stdout.on('data', hear); child.stderr.on('data', hear);
      child.once('error', no);
      child.once('exit', code => code === 0 ? ok() : no(new Error(`fixture preparation failed: ${said}`)));
    });
    await prepare(process.execPath, ['host/web/build.mjs', app, '--render', 'none', ...(wasm ? ['--wasm'] : [])]);
    if (wasm) {
      await prepare('cargo', ['build', '-q', '--release', '-p', 'exact-js-bake']);
      // A loop's first start builds the module producer and the app's wasm crates into the app's own target
      // (dev.mjs `toolingEnv`), minutes for a new app on a loaded machine: preparation too, so start it once
      // here, past the 30-second waits, until it listens.
      const listener = createServer();
      await new Promise(ok => listener.listen(0, '127.0.0.1', ok));
      const port = listener.address().port;
      await new Promise(ok => listener.close(ok));
      const warm = spawnOwned(process.execPath, ['--preload', preload, 'host/web/dev.mjs', '--app', app, '--port', String(port), '--wasm'],
        {cwd: root, env, stdio: ['pipe', 'pipe', 'pipe']});
      await new Promise((ok, no) => {
        let said = '';
        const hear = data => { said = (said + data).slice(-12000); if (said.includes(`http://127.0.0.1:${port}/`)) ok(); };
        warm.stdout.on('data', hear); warm.stderr.on('data', hear);
        warm.once('exit', code => no(new Error(`the warm-up loop exited ${code}: ${said}`)));
      });
      await warm.stop(30_000);
    }
  }, 900_000);
  afterAll(async () => { await stopAll(); if (dir) rmSync(dir, {recursive: true, force: true}); });

  test('serves admitted routes after an edit and reports a failed rebuild', async () => {
    const listener = createServer();
    await new Promise(ok => listener.listen(0, '127.0.0.1', ok));
    const port = listener.address().port;
    await new Promise(ok => listener.close(ok));
    const url = `http://127.0.0.1:${port}`;
    const dev = spawnOwned(process.execPath, [...(wasm ? ['--preload', preload] : []), 'host/web/dev.mjs', '--app', app, '--port', String(port), ...(wasm ? ['--wasm'] : [])],
      {cwd: root, env, stdio: ['pipe', 'pipe', 'pipe']});
    const controller = new AbortController();
    let streamTimer;
    try {
      await new Promise((ok, no) => {
        let said = '';
        const finish = error => {
          clearTimeout(timer); dev.off('exit', exited);
          dev.stdout.off('data', hear); dev.stderr.off('data', hear);
          dev.stdout.resume(); dev.stderr.resume();
          error ? no(error) : ok();
        };
        const timer = setTimeout(() => finish(new Error(`dev did not listen: ${said}`)), 30_000);
        const exited = code => finish(new Error(`dev exited ${code}: ${said}`));
        const hear = data => { said = (said + data).slice(-12000); if (said.includes(url + '/')) finish(); };
        dev.stdout.on('data', hear); dev.stderr.on('data', hear); dev.once('exit', exited);
      });
      // Each wait on the stream gets its own bound; a loaded box takes longer than that for the whole
      // sequence of rebuilds. The wasm loop's waits include a wasm rebuild (about 16 s by hand at load ~20):
      // 2 minutes each, in the slow lane where it runs.
      const wait = wasm ? 120_000 : 30_000;
      const bound = () => { clearTimeout(streamTimer); streamTimer = setTimeout(() => controller.abort(new Error('dev build notification did not arrive')), wait); };
      // An abort ends the fetch, but a line iterator over its body does not end with it: race the two.
      const aborted = new Promise((_, no) => controller.signal.addEventListener('abort', () => no(controller.signal.reason), {once: true}));
      aborted.catch(() => {});
      bound();
      const response = await fetch(url + (wasm ? '/__dev' : '/__dev/page'), {signal: controller.signal});
      const lines = createInterface({input: Readable.fromWeb(response.body)})[Symbol.asyncIterator]();
      const nextBuild = async predicate => {
        bound();
        for (;;) {
          const {value: line, done} = await Promise.race([lines.next(), aborted]);
          if (done) throw new Error('dev event stream closed');
          if (!line.startsWith('data: ')) continue;
          const frame = JSON.parse(line.slice(6));
          if (frame.error) throw new Error(frame.error);
          if (predicate(frame)) return frame;
        }
      };
      const sequence = frame => wasm ? frame.seq : frame.reload;
      const status = async (path, accept = 'text/html') => (await fetch(url + path, {headers: {accept}, signal: AbortSignal.timeout(30_000)})).status;
      let initial;
      if (wasm) {
        const hello = await nextBuild(frame => frame.hello);
        expect(hello.ready).toBe(false);
        const duringAdmission = async () => {
          expect(await status('/')).toBe(200);
          expect(await status('/exact.json')).toBe(404);
          return status('/profile/photos.zip');
        };
        const startup = await duringAdmission();
        dev.stdin.write('admit\n');
        const first = await nextBuild(frame => sequence(frame) > 0);
        dev.stdin.write('r\n');
        await nextBuild(frame => frame.rebuilt);
        const rebuilt = await duringAdmission();
        dev.stdin.write('admit\n');
        initial = await nextBuild(frame => sequence(frame) > sequence(first));
        expect([startup, rebuilt]).toEqual([200, 200]);
      } else initial = await nextBuild(frame => sequence(frame) > 0);
      expect(await status('/profile/photos.zip')).toBe(200);
      expect(await status('/deck/missing.html')).toBe(404);
      expect(await status('/missing.pdf')).toBe(404);
      expect(await status('/profile/photos.zip', '*/*')).toBe(404);
      const before = readFileSync(resolve(dist, 'app.plan'));
      writeFileSync(source, sourceFor('/changed/:handle'));
      const changed = await nextBuild(frame => sequence(frame) > sequence(initial));
      if (wasm) {
        expect(changed.digest).not.toBe(initial.digest);
        expect(readFileSync(resolve(dist, 'app.plan')).equals(before)).toBe(true);
      }
      expect(await status('/changed/photos.zip')).toBe(200);
      expect(await status('/profile/photos.zip')).toBe(404);
      writeFileSync(source, 'invalid contract !!!');
      await expect(nextBuild(frame => sequence(frame) > sequence(changed))).rejects.toThrow('syntax-expected-declaration');
      expect(dev.exitCode).toBe(null);
      expect(await status('/changed/photos.zip')).toBe(200);
    } finally {
      clearTimeout(streamTimer); controller.abort();
      await dev.stop(30_000);
    }
  }, wasm ? 900_000 : 300_000); // (the slow lane's cap for the wasm loop's rebuilds, above)
});

test('HTTP route caches read each plan asynchronously once and refresh after edits', async () => {
  const dist = mkdtempSync(resolve(tmpdir(), 'exact-route-cache-'));
  const sync = spyOn(filesystem, 'filesystem'), reads = spyOn(filesystem, 'filesystemRead');
  let server;
  try {
    writeFileSync(resolve(dist, 'index.html'), '<title>app</title>');
    plan(dist, '/profile/:handle');
    server = httpServer((req, res) => serveBuildTree(dist, req, res));
    await new Promise(ok => server.listen(0, '127.0.0.1', ok));
    const url = `http://127.0.0.1:${server.address().port}`;
    const tree = async path => (await fetch(url + path, {headers: {accept: 'text/html'}})).status;
    const staticRead = async path => (await readWebRequest(dist, path, 'text/html')).found ? 200 : 404;
    // A build's tree reads its route table from routes.json; a static origin from the plan.
    for (const [read, table] of [[tree, 'routes.json'], [staticRead, 'app.plan']]) {
      sync.mockClear(); reads.mockClear();
      expect(await read('/profile/photos.zip')).toBe(200);
      const planReads = () => reads.mock.calls.filter(([request]) => request.path === table).length;
      expect(sync.mock.calls.length).toBe(0);
      expect(planReads()).toBe(1);
      expect(await read('/profile/photos.zip')).toBe(200);
      expect(await read('/deck/missing.html')).toBe(404);
      expect(planReads()).toBe(1);
      plan(dist, '/changed/:handle');
      expect(await read('/changed/photos.zip')).toBe(200);
      expect(await read('/profile/photos.zip')).toBe(404);
      expect(planReads()).toBe(2);
      renameSync(dist, dist + '.previous');
      mkdirSync(dist);
      writeFileSync(resolve(dist, 'index.html'), '<title>replacement</title>');
      plan(dist, '/replaced/:handle');
      expect(await read('/replaced/photos.zip')).toBe(200);
      expect(await read('/changed/photos.zip')).toBe(404);
      expect(planReads()).toBe(3);
      rmSync(dist + '.previous', {recursive: true, force: true});
      plan(dist, '/profile/:handle');
    }
  } finally {
    sync.mockRestore(); reads.mockRestore();
    if (server) { server.closeAllConnections(); await new Promise(ok => server.close(ok)); }
    rmSync(dist + '.previous', {recursive: true, force: true});
    rmSync(dist, {recursive: true, force: true});
  }
});


for (const failure of ['published plan', 'published pointer', 'local plan', 'tree plan']) {
  test(`route lookup retries a failed ${failure} read without a file change`, async () => {
    const dist = mkdtempSync(resolve(tmpdir(), 'exact-route-read-failure-'));
    const original = filesystem.filesystemRead;
    let reads;
    try {
      writeFileSync(resolve(dist, 'index.html'), '<title>app</title>');
      plan(dist, '/profile/:handle');
      const published = failure.startsWith('published');
      const root = published ? resolve(dist, 'origin') : dist;
      if (published) await publishRoot({origin: new DirectoryOrigin(root), row: {}, web: dist});
      // Local static reads and a build tree's route table retry four times inside one request; published reads fail
      // closed immediately.
      let remaining = failure === 'local plan' || failure === 'tree plan' ? 4 : 1, pointers = 0;
      reads = spyOn(filesystem, 'filesystemRead').mockImplementation(request => {
        const selected = failure === 'published pointer' ? request.path === webRootPath && ++pointers === 2
          : failure === 'tree plan' ? request.path === 'routes.json'
            : request.path === 'app.plan' || request.path.endsWith('/app.plan');
        if (remaining && selected) { remaining--; throw new Error('injected EMFILE'); }
        return original(request);
      });
      const read = async path => failure === 'tree plan'
        ? (await buildTreeFileAsync(root, path, 'text/html'))?.route
        : (await readWebRequest(root, path, 'text/html')).found?.route;
      expect(await read('/profile/photos.zip')).toBe(undefined);
      expect(remaining).toBe(0);
      expect(await read('/index.html')).toBe('/index.html');
      expect(await read('/profile/photos.zip')).toBe(failure === 'tree plan' ? '/shell.html' : '/index.html');
      expect(await read('/deck/missing.html')).toBe(undefined);
      if (failure === 'tree plan') {
        // A route table that stays unreadable is no answer yet when serving (503), never a page's miss; one read that
        // fails is asked again, and the route's shell served.
        writeFileSync(resolve(dist, 'routes.json'), readFileSync(resolve(dist, 'routes.json'), 'utf8') + '\n');
        remaining = 4;
        expect(await buildTreeFileAsync(root, '/profile/photos.zip', 'text/html', {miss: true})).toEqual({status: 503});
        writeFileSync(resolve(dist, 'routes.json'), readFileSync(resolve(dist, 'routes.json'), 'utf8') + '\n');
        remaining = 1;
        expect([await read('/profile/photos.zip'), remaining]).toEqual(['/shell.html', 0]);
      }
    } finally {
      reads?.mockRestore();
      rmSync(dist, {recursive: true, force: true});
    }
  });
}
