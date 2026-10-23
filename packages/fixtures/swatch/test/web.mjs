// @fixture/swatch on the web (LLP 1133.001 D1, LLP 1133.008 D8 items 1 and
// 2): the fixture app made outside the repo, built for the web, and driven
// in each engine. The package's view is made by the session's instance of
// its implementation, which hears the view's displayed state and its `tint`
// binding change, each with the origin `app` (the web has no tool or wake
// door), and answers what it heard as `swatchReports`.
//
// usage: bun packages/fixtures/swatch/test/web.mjs [chromium] [firefox] [webkit]
//   (every engine by default; `bunx playwright@1.64.1 install firefox webkit` first)
import { rmSync } from 'node:fs';
import { dirname, extname, join, resolve } from 'node:path';
import { checker, makeFixture, root, run } from './fixture.mjs';

const engines = process.argv.slice(2).length ? process.argv.slice(2) : ['chromium', 'firefox', 'webkit'];
const { chromium, firefox, webkit } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));

const { app } = makeFixture();
run('bun', ['exact.mjs', 'web-build', '--render', 'none'], app);

const dist = join(app, 'target/web-dist');
const types = { '.js': 'text/javascript', '.html': 'text/html', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css' };
const server = Bun.serve({ port: 0, async fetch(req) {
  let path = new URL(req.url).pathname;
  if (path === '/') path = '/index.html';
  const file = Bun.file(join(dist, path));
  if (!(await file.exists())) return new Response('not found', { status: 404 });
  return new Response(file, { headers: { 'content-type': types[extname(path)] ?? 'application/octet-stream' } });
} });
const origin = `http://127.0.0.1:${server.port}`;
const { check, failed } = checker();

for (const engine of engines) {
  const browser = await { chromium, firefox, webkit }[engine].launch();
  const text = (page, id) => page.getByTestId(id).innerText({ timeout: 10000 }).catch(e => `(${e.message.split('\n')[0]})`);
  const until = async (page, id, want) => {
    const end = Date.now() + 10000;
    let got;
    while (Date.now() < end) { if (want(got = await text(page, id))) return got; await page.waitForTimeout(100); }
    return got;
  };
  const heard = (page, want) => until(page, 'heard', t => want(t.split('; ')));
  try {
    const page = await (await browser.newContext()).newPage();
    await page.goto(origin);
    const none = await until(page, 'heard', t => !t.startsWith('('));
    check(engine, 'nothing is reported before the view is placed', none === '', none);

    await page.getByTestId('show').click();
    const shown = await heard(page, l => l.some(x => x.startsWith('made ')) && l.some(x => x.startsWith('displayed ')) && l.some(x => x.startsWith('bound ')));
    const id = shown.split('; ').find(x => x.startsWith('made '))?.split(' ')[1];
    check(engine, 'the session\'s instance made the view', Boolean(id), shown);
    check(engine, 'shown by the app\'s commit: displayed true app', shown.includes(`displayed ${id} true app`), shown);
    check(engine, 'its binding prop reported with its origin', shown.includes(`bound ${id} tint app`), shown);
    const fill = () => page.getByTestId('box').evaluate(el => getComputedStyle(el.firstElementChild).backgroundColor).catch(e => e.message);
    check(engine, 'the view draws its tint', await fill() === 'rgb(221, 51, 51)', await fill());

    await page.getByTestId('recolor').click();
    const rebound = await heard(page, l => l.filter(x => x === `bound ${id} tint app`).length === 2);
    check(engine, 'a rebinding is reported again', rebound.split('; ').filter(x => x === `bound ${id} tint app`).length === 2, rebound);
    check(engine, 'the new tint reaches the view', await fill() === 'rgb(51, 102, 204)', await fill());

    await page.getByTestId('hide').click();
    const hidden = await heard(page, l => l.includes(`displayed ${id} false app`));
    check(engine, 'display: none is reported hidden', hidden.includes(`displayed ${id} false app`), hidden);
    await page.getByTestId('hide').click();
    const again = await heard(page, l => l.filter(x => x === `displayed ${id} true app`).length === 2);
    check(engine, 'shown again is reported again', again.split('; ').filter(x => x === `displayed ${id} true app`).length === 2, again);

    await page.getByTestId('box').click();
    const tapped = await until(page, 'last', t => t === 'tap');
    check(engine, 'the view\'s event reaches its handler', tapped === 'tap', tapped);

    await page.getByTestId('show').click();
    const removed = await heard(page, l => l.filter(x => x === `displayed ${id} false app`).length === 2);
    check(engine, 'a shown view leaving the tree is reported hidden', removed.split('; ').filter(x => x === `displayed ${id} false app`).length === 2, removed);
  } finally {
    await browser.close();
  }
}
server.stop();
rmSync(dirname(app), { recursive: true, force: true });
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
