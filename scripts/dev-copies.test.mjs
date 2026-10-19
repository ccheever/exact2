// Several live copies of one dev page (LLP 1122 T7, *1.14*), on both web
// dev loops: each copy is an iframe of the dev loop's page that the page
// framing it names (`name`, which never enters the app's URL). Each copy
// keeps its own build id across reloads, though same-origin frames share
// one sessionStorage, and posts it to its parent on registering, so the
// parent finds which frame is which build by the message's source. Each
// copy's status says where it is (`location`). `navigate` routes one copy in
// place, its state kept, while the others stay; a location that is no
// route is that client's `unsupported`.
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
  view
    main navigationKey=\`\${top(nav).id}\` navigationBack="back" width="100%" height="100%" position="relative"
      for t in nav.tabs key = t.name
        for e in t.stack key = e.id
          column navigationKey=\`\${e.id}\` position="absolute" inset=0 display="flex" flex-direction="column" background-color="surface.page"
            if e.name == "home"
              text "Home" role="heading" aria-level=1 testId="home"
            if e.name == "post"
              text \`Post \${params(nav, "id").join("")}\` role="heading" aria-level=1 testId="post"
            button press=tap testId="tap"
              text \`Taps \${taps}\`
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
  const parent = createServer((req, res) => {
    res.writeHead(200, { 'content-type': 'text/html' });
    res.end(`<!doctype html><script>window.words=[];addEventListener('message',e=>{if(e.data?.type!=='exact-build')return;const name=${JSON.stringify(names)}.find(n=>window.frames[n]===e.source);words.push({name:name??null,id:e.data.id,origin:e.origin})})</script>${names.map(n => `<iframe name="${n}" src="${base}/" width="390" height="400"></iframe>`).join('')}`);
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
    await events(); // present
    let logs = [];
    const until = async (read, pass, what, ms = 120_000) => { for (const end = Date.now() + ms; ;) { const v = await read(); if (pass(v)) return v; if (Date.now() > end) throw new Error(`${what}: ${JSON.stringify(v)?.slice(0, 2000)}\nthe pages said: ${logs.slice(-20).join('\n')}\nevents: ${JSON.stringify((await events()).filter(e => e.event === 'unsupported'))}`); await Bun.sleep(250); } };
    const page = await browser.newPage();
    // (The copies' own words, said with a failure.)
    page.on('console', m => logs.push(`${m.type()}: ${m.text()}`.slice(0, 500)));
    await page.goto(`http://127.0.0.1:${parent.address().port}/`);
    const words = () => page.evaluate(() => window.words);
    const byName = ws => Object.fromEntries(ws.filter(w => w.name).map(w => [w.name, w.id]));
    const first = byName(await until(words, ws => Object.keys(byName(ws)).length === 3, 'each copy says its build'));
    // Three builds, one a frame, each said from the dev loop's origin.
    expect(new Set(Object.values(first)).size).toBe(3);
    expect((await words()).every(w => w.origin === base && w.name)).toBe(true);
    const listed = async () => (await call({ method: 'hello' })).ok.builds;
    const ids = Object.values(first);
    const builds = await until(listed, bs => ids.every(id => bs.find(b => b.id === id)?.status?.location === '/'), 'each copy listed at /');
    expect(builds.filter(b => ids.includes(b.id)).every(b => b.capabilities.includes('navigate'))).toBe(true);
    // Reloaded together: each frame its own build again.
    await page.evaluate(() => { window.words = []; });
    await page.reload();
    expect(byName(await until(words, ws => Object.keys(byName(ws)).length === 3, 'each copy says its build again'))).toEqual(first);
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
    // No route: said to this client, the copy where it was.
    await call({ method: 'navigate', client: 'copies', build: first['exact-copy-3'], location: '/nowhere' });
    const refused = await until(events, es => es.find(e => e.event === 'unsupported' && e.what === 'navigate'), 'a refusal');
    expect(refused.find(e => e.event === 'unsupported' && e.what === 'navigate')).toMatchObject({ build: first['exact-copy-3'], detail: expect.stringContaining('no route') });
    expect(page.frame({ name: 'exact-copy-3' }).url()).toBe(`${base}/`);
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
