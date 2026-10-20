// Several live copies of one dev page (LLP 1122 T7, *1.14*), on both web
// dev loops: each copy is an iframe of the dev loop's page that the page
// framing it names (`name`, which never enters the app's URL). Each copy
// keeps its own build id across reloads, though same-origin frames share
// one sessionStorage, and posts it to its parent on registering, so the
// parent finds which frame is which build by the message's source. Each
// copy's status says where it is (`location`), as the app moves it too (a
// link, Back, a fragment). `navigate` routes one copy in place through the
// app's `navigate` handler, its state kept, while the others stay; a
// handler's redirect keeps the app's place; a location that is no route is
// that client's `unsupported`.
import { afterEach, expect, test } from 'bun:test';
import { cpSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { chromium } from './agent-launch.mjs';
import { spawnOwned, stopAll } from './test-children.mjs';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
afterEach(() => stopAll());

const APP = `routes nav
  tab home "/"
    post "/post/:id"
component App
  state taps = 0
  action back
    nav = back(nav)
  action tap
    taps = taps + 1
  action follow(url: string)
    nav = go(nav, url == "/post/0" ? "/post/9" : url)
  action dim
    setMode("dark-style", "dim")
  action dark
    setScheme("dark")
  action big
    setRootFontSize(24)
  view
    main navigationKey=\`\${top(nav).id}\` navigationBack="back" navigate=follow width="100%" height="100%" position="relative"
      for t in nav.tabs key = t.name
        for e in t.stack key = e.id
          column navigationKey=\`\${e.id}\` position="absolute" inset=0 display="flex" flex-direction="column" background-color="surface.page" testId="page"
            if e.name == "home"
              text "Home" role="heading" aria-level=1 testId="home"
              link href="/post/3" testId="link"
                text "Post 3"
            if e.name == "post"
              text \`Post \${params(nav, "id").join("")}\` role="heading" aria-level=1 testId="post"
            button press=tap testId="tap"
              text \`Taps \${taps}\`
            button press=dim testId="dim"
              text "Dim"
            button press=dark testId="dark"
              text "Dark"
            button press=big testId="big"
              text "Big"
`;

const free = async () => { const s = createServer(); await new Promise(ok => s.listen(0, '127.0.0.1', ok)); const { port } = s.address(); await new Promise(ok => s.close(ok)); return port; };

async function copies(wasm) {
  // The JS loop runs a routed app of the test's own; the wasm loop builds an
  // app in the Cargo workspace, so it runs the theme fixture, which has no
  // router: there `navigate` moves the fragment alone, and a path is no route.
  // (Its route step is the link's own, input-glue.js, as js-link-press and the
  // router's tests hold.)
  const app = wasm ? resolve(ROOT, 'apps/theme-fixture') : mkdtempSync(join(tmpdir(), 'exact-dev-copies-'));
  if (!wasm) {
    for (const f of ['app.tamber', 'app.json']) cpSync(resolve(ROOT, 'apps/theme-fixture', f), join(app, f));
    writeFileSync(join(app, 'app.contract'), APP);
  }
  const dist = mkdtempSync(join(tmpdir(), 'exact-dev-copies-dist-'));
  const port = await free(), base = `http://127.0.0.1:${port}`;
  // The page framing the copies (as the design tool: the origin the dev
  // loop's frame-ancestors names, EXACT_TOOL_PORT): another origin, three named frames; each
  // build's word kept with the frame it came from, by its source alone.
  const names = ['exact-copy-1', 'exact-copy-2', 'exact-copy-3'];
  // (And at /hosts, six copies each at a loopback name of its own, as the design tool frames them: theme2's F1 ruling.)
  const hosts = Array.from({ length: 6 }, (_, i) => `exact-copy-${i + 1}`);
  const parent = createServer((req, res) => {
    res.writeHead(200, { 'content-type': 'text/html' });
    if (req.url === '/hosts') return res.end(`<!doctype html><script>window.words=[];addEventListener('message',e=>{if(e.data?.type!=='exact-build')return;const name=${JSON.stringify(hosts)}.find(n=>window.frames[n]===e.source);words.push({name:name??null,id:e.data.id,origin:e.origin})})</script>${hosts.map(n => `<iframe name="${n}" src="http://${n}.localhost:${port}/" width="390" height="400"></iframe>`).join('')}`);
    res.end(`<!doctype html><script>window.words=[];addEventListener('message',e=>{if(e.data?.type!=='exact-build')return;const name=${JSON.stringify(names)}.find(n=>window.frames[n]===e.source);words.push({name:name??null,id:e.data.id,origin:e.origin})})</script>${[...names, 'app-own'].map(n => `<iframe name="${n}" src="${base}/" width="390" height="400"></iframe>`).join('')}`);
  });
  await new Promise(ok => parent.listen(0, '127.0.0.1', ok));
  const dev = spawnOwned(process.execPath, [resolve(ROOT, 'host/web/dev.mjs'), ...(wasm ? ['--app', 'theme-fixture', '--wasm'] : []), '--port', String(port)], { cwd: ROOT, env: { ...process.env, ...(wasm ? {} : { EXACT_APP_DIR: app }), EXACT_WEB_DIST: dist, EXACT_TOOL_PORT: String(parent.address().port) }, stdio: ['ignore', 'pipe', 'pipe'] });
  let said = '';
  dev.stdout.on('data', d => { said = (said + d).slice(-8000); });
  dev.stderr.on('data', d => { said = (said + d).slice(-8000); });
  for (const end = Date.now() + 600_000; !said.includes(`127.0.0.1:${port}/`);) { if (Date.now() > end || dev.exitCode !== null) throw new Error(`the dev loop never served: ${said}`); await Bun.sleep(100); }
  const browser = await (await import('playwright-core')).chromium.launch({ executablePath: chromium().executable, headless: true });
  try {
    const token = (await fetch(`${base}/__tamber/token`).then(r => r.json())).token;
    const call = body => fetch(`${base}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: JSON.stringify(body) }).then(async r => ({ status: r.status, ...(await r.json()) }));
    const events = () => fetch(`${base}/__tamber/events`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: JSON.stringify({ after: 0, wait: false, client: 'copies' }) }).then(r => r.json()).then(r => r.events ?? []);
    // Present throughout: a subscription held open (a poll's presence lapses in 35 s).
    const presence = new AbortController();
    fetch(`${base}/__tamber/subscribe`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: JSON.stringify({ client: 'copies' }), signal: presence.signal }).then(r => r.body?.pipeTo(new WritableStream())).catch(() => {});
    await events();
    let logs = [];
    const until = async (read, pass, what, ms = 120_000) => { for (const end = Date.now() + ms; ;) { const v = await read(); if (pass(v)) return v; if (Date.now() > end) throw new Error(`${what}: ${JSON.stringify(v)?.slice(0, 2000)}\nthe pages said: ${logs.slice(-20).join('\n')}\nevents: ${JSON.stringify((await events()).filter(e => e.event === 'unsupported'))}`); await Bun.sleep(250); } };
    const page = await browser.newPage();
    // (The copies' own words, said with a failure.)
    page.on('console', m => logs.push(`${m.type()}: ${m.text()}`.slice(0, 500)));
    await page.goto(`http://127.0.0.1:${parent.address().port}/`);
    const words = () => page.evaluate(() => window.words);
    const byName = ws => Object.fromEntries(ws.filter(w => w.name).map(w => [w.name, w.id]));
    const first = byName(await until(words, ws => Object.keys(byName(ws)).length === 3, 'each copy says its build'));
    // Three builds, one a copy, each said from the dev loop's origin (the app's own frame says its build too).
    expect(new Set(Object.values(first)).size).toBe(3);
    expect((await words()).every(w => w.origin === base)).toBe(true);
    const listed = async () => (await call({ method: 'hello' })).ok.builds;
    const ids = Object.values(first);
    const builds = await until(listed, bs => ids.every(id => bs.find(b => b.id === id)?.status?.location === '/'), 'each copy listed at /');
    expect(builds.filter(b => ids.includes(b.id)).every(b => b.capabilities.includes('navigate'))).toBe(true);
    // Reloaded together: each frame its own build again.
    await page.evaluate(() => { window.words = []; });
    await page.reload();
    expect(byName(await until(words, ws => Object.keys(byName(ws)).length === 3, 'each copy says its build again'))).toEqual(first);
    // One copy reloaded alone: its own build again.
    await page.evaluate(() => { window.words = []; });
    await page.frame({ name: 'exact-copy-1' }).evaluate(() => location.reload());
    expect(byName(await until(words, ws => ws.some(w => w.name === 'exact-copy-1'), 'copy 1 says its build again'))).toEqual({ 'exact-copy-1': first['exact-copy-1'] });
    // Each id kept under its copy's name; a frame the app named otherwise (or a tab) keeps the one id, as before.
    const keys = await page.frame({ name: 'app-own' }).evaluate(() => Object.keys(sessionStorage).filter(k => k.startsWith('exact.dev.build')).sort());
    expect(keys).toEqual(['exact.dev.build', ...names.map(n => `exact.dev.build:${n}`)]);
    // Copy 2 routed in place: its state kept (a tap, and a mark on its window), the others where they were.
    const frame = page.frame({ name: 'exact-copy-2' });
    await frame.waitForSelector(`[data-testid="${wasm ? 'page' : 'home'}"]`);
    if (!wasm) await frame.click('[data-testid="tap"]');
    await frame.evaluate(() => { window.__mark = 'kept'; });
    const to = wasm ? '/#c' : '/post/7?from=grid#c';
    const sent = await call({ method: 'navigate', client: 'copies', build: first['exact-copy-2'], location: to });
    expect(sent.ok).toMatchObject({ navigated: true, build: first['exact-copy-2'] });
    await until(listed, bs => bs.find(b => b.id === first['exact-copy-2'])?.status?.location === to, 'copy 2 says it moved');
    expect(await frame.evaluate(() => location.pathname + location.search + location.hash)).toBe(to);
    if (!wasm) expect(await frame.textContent('[data-testid="post"]')).toBe('Post 7');
    if (!wasm) expect(await frame.textContent('[data-testid="tap"]')).toBe('Taps 1');
    expect(await frame.evaluate(() => window.__mark)).toBe('kept');
    expect((await listed()).filter(b => b.id !== first['exact-copy-2'] && ids.includes(b.id)).map(b => b.status.location)).toEqual(['/', '/']);
    // Back where it was, the fragment gone too.
    await call({ method: 'navigate', client: 'copies', build: first['exact-copy-2'], location: '/' });
    await until(listed, bs => bs.find(b => b.id === first['exact-copy-2'])?.status?.location === '/', 'copy 2 back at /');
    const at = id => async () => (await listed()).find(b => b.id === id)?.status?.location;
    // The app moving a copy itself is said too: a fragment where no router
    // owns the history (the theme fixture; the JS fixture's router takes a
    // hand-made entry back), a link and Back where one does.
    const one = page.frame({ name: 'exact-copy-1' });
    if (wasm) {
      await one.evaluate(() => { location.hash = '#z'; });
      await until(at(first['exact-copy-1']), l => l === '/#z', 'copy 1 says its fragment');
    } else {
      await one.click('[data-testid="link"]');
      await until(at(first['exact-copy-1']), l => l === '/post/3', 'copy 1 says the link it followed');
      await one.evaluate(() => history.back());
      await until(at(first['exact-copy-1']), l => l === '/', 'copy 1 says it went back');
      // The app's handler sends /post/0 to /post/9: the app's place kept, the fragment not added to it.
      await call({ method: 'navigate', client: 'copies', build: first['exact-copy-3'], location: '/post/0#x' });
      await until(at(first['exact-copy-3']), l => l === '/post/9', 'copy 3 where the app sent it');
      expect(await page.frame({ name: 'exact-copy-3' }).textContent('[data-testid="post"]')).toBe('Post 9');
    }
    // No route: said to this client, the copy where it was.
    const before = page.frame({ name: 'exact-copy-3' }).url();
    await call({ method: 'navigate', client: 'copies', build: first['exact-copy-3'], location: '/nowhere' });
    const refused = await until(events, es => es.find(e => e.event === 'unsupported' && e.what === 'navigate'), 'a refusal');
    expect(refused.find(e => e.event === 'unsupported' && e.what === 'navigate')).toMatchObject({ build: first['exact-copy-3'], detail: expect.stringContaining('no route') });
    expect(page.frame({ name: 'exact-copy-3' }).url()).toBe(before);
    // Six copies at loopback names of their own: each registers and reports, none starved of connections.
    await page.goto(`http://127.0.0.1:${parent.address().port}/hosts`);
    const six = await until(async () => byName(await words()), ws => Object.keys(ws).length === 6, 'six copies at their own names say their builds');
    expect((await words()).every(w => w.origin === `http://${w.name}.localhost:${port}`)).toBe(true);
    await until(listed, bs => Object.values(six).every(id => bs.find(b => b.id === id)?.status?.location === '/'), 'six copies report');
    // A pinned copy holds its pin over the app's own setMode (theme2's F2 ruling), says what the app asked, and takes the app's choice when handed back.
    const c1 = six['exact-copy-1'], status1 = async () => (await listed()).find(b => b.id === c1)?.status;
    // (The JS fixture also sets its scheme and root size; the theme fixture its mode alone.)
    const pinned = wasm ? { scheme: 'dark', mode: { 'dark-style': 'black' } } : { scheme: 'light', mode: { 'dark-style': 'black' }, rootFontSize: 18 };
    await call({ method: 'pin', client: 'copies', build: c1, pin: pinned });
    await until(status1, st => st?.mode?.['dark-style'] === 'black' && st?.scheme === pinned.scheme && (wasm || st?.rootFontSize === 18), 'copy 1 pinned');
    const host1 = page.frame({ name: 'exact-copy-1' });
    for (const id of wasm ? ['dim'] : ['dim', 'dark', 'big']) await host1.click(`[data-testid="${id}"]`);
    const held = await until(status1, st => st?.app?.mode?.['dark-style'] === 'dim' && (wasm || (st.app.scheme === 'dark' && st.app.rootFontSize === '24')), 'copy 1 says what the app asked');
    expect([held.mode['dark-style'], held.scheme, wasm || held.rootFontSize]).toEqual(['black', pinned.scheme, wasm || 18]);
    // What paints, not only what the status says: the page's background in Black, the root size the pin's.
    const paint = () => host1.evaluate(() => ({ bg: [...document.querySelectorAll('[data-testid="page"], [data-testid="page"] *')].map(e => getComputedStyle(e).backgroundColor).find(c => c !== 'rgba(0, 0, 0, 0)') ?? 'none', root: parseFloat(getComputedStyle(document.documentElement).fontSize) }));
    const blackPaint = await paint();
    if (!wasm) expect(blackPaint.root).toBe(18);
    // A pin that holds less: the mode it drops is the app's latest choice at once; the scheme stays held.
    await call({ method: 'pin', client: 'copies', build: c1, pin: { scheme: pinned.scheme } });
    await until(status1, st => st?.mode?.['dark-style'] === 'dim' && st?.scheme === pinned.scheme, 'copy 1, its mode released, takes the app\'s dim');
    // (The wasm copy is dark, so its background moves; the JS copy is light, so its page's own modes say it.)
    if (wasm) await until(paint, pt => pt.bg !== blackPaint.bg, 'copy 1 paints the app\'s dim, not Black');
    else await until(() => host1.evaluate(() => document.documentElement.getAttribute('data-x-modes')), m => /dark-style=dim/.test(m ?? ''), 'copy 1\'s page is in the app\'s dim');
    await call({ method: 'pin', client: 'copies', build: c1, pin: null });
    const back = await until(status1, st => st?.mode?.['dark-style'] === 'dim' && (wasm || (st.scheme === 'dark' && st.rootFontSize === 24)), 'copy 1 handed back takes the app\'s latest');
    expect(back.app ?? null).toBe(null);
    if (!wasm) expect((await paint()).root).toBe(24);
    presence.abort();
  } finally {
    await browser.close();
    parent.close();
    await dev.stop(15_000);
    if (!wasm) rmSync(app, { recursive: true, force: true });
    rmSync(dist, { recursive: true, force: true });
  }
}

test('copies on the JS loop: each frame its own build, said to its parent; one routed in place by navigate', () => copies(false), 900_000);
test('copies on the wasm loop: each frame its own build, said to its parent; one moved in place by navigate, a path no route', () => copies(true), 900_000);
