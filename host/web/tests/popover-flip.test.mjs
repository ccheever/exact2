// Compiled Contract CSS and Apple's placement helper against Chrome's own
// invoker popovers. Build the compiler before this test (docs/testing.md).
import { test, expect, beforeAll, afterAll } from 'bun:test';
import { spawn, spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { suiteBrowser } from '../../../scripts/agent-launch.mjs';

const root = resolve(new URL('../../..', import.meta.url).pathname);
const dir = mkdtempSync(resolve(tmpdir(), 'exact-popflip-'));
const dist = resolve(dir, 'dist');
const tries = 'flip-block, flip-inline, flip-block flip-inline';
const cases = [
  ['bottom span-right', 'none', 332, 560, 420, 600],
  ['bottom span-right', 'flip-block, flip-inline', 332, 560, 420, 600],
  ['bottom span-right', tries, 332, 560, 420, 600],
  ['bottom span-right', tries, 332, 560, 800, 800],
  ['top span-left', tries, 8, 8, 420, 600],
  ['left span-bottom', tries, 8, 200, 420, 600],
  ['right span-top', tries, 332, 560, 420, 600],
  ['bottom span-left', 'flip-block flip-inline, flip-inline', 8, 300, 420, 600],
  ['bottom span-left', 'flip-inline, flip-block flip-inline', 8, 300, 420, 600],
  ['top', tries, 8, 8, 420, 600],
  ['top left', tries, 8, 8, 420, 600],
  ['bottom span-right', tries, 332, 560, 420, 600, [10, 0, 0, 5]],
  ['bottom span-right', tries, 332, 560, 420, 600, [0, 0, 0, 110]],
  // flip-start, alone and composed in order; a square box, since the width and
  // height it exchanges are the laid-out size natively (LLP 1021 D2).
  ['bottom span-right', 'flip-start flip-block', 8, 500, 420, 600, [0, 0, 0, 0], [120, 120]],
  ['bottom span-right', 'flip-block flip-start', 8, 500, 420, 600, [0, 0, 0, 0], [120, 120]],
  ['bottom span-right', 'flip-start flip-block', 8, 500, 420, 600, [1, 2, 3, 4], [120, 120]],
  ['bottom', 'flip-start', 8, 540, 420, 600, [0, 0, 0, 0], [120, 120]],
  ['top', 'flip-start, flip-block', 8, 8, 420, 600, [0, 0, 0, 0], [120, 120]],
  ['bottom span-right', 'flip-inline flip-start flip-block, flip-start flip-inline flip-block', 332, 500, 420, 600, [0, 0, 0, 0], [120, 120]],
  // No option fits: shifted back over the top and left edges, and a fixed
  // box over the bottom and right ones too.
  ['top span-left', 'flip-start', 200, 8, 420, 600, [3, 0, 0, 5], [120, 120]],
  ['left', 'flip-start', 8, 300, 420, 600, [0, 0, 0, 0], [120, 120]],
  ['bottom span-right', 'none', 332, 560, 420, 600, [0, 0, 0, 0], [320, 120], 'fixed'],
  ['right', 'none', 332, 560, 420, 600, [0, 0, 0, 0], [120, 120], 'fixed'],
  ['bottom span-right', tries, 332, 560, 420, 600, [0, 0, 0, 0], [320, 120], 'fixed'],
  // The limit holds the area too: an invoker past the left edge keeps its box there.
  ['bottom span-right', 'none', -10, 200, 420, 600, [0, 0, 0, 0], [120, 120], 'fixed'],
  ['left', 'none', -60, 300, 420, 600, [0, 0, 0, 0], [120, 120], 'fixed'],
  ['top', 'none', 200, -10, 420, 600, [0, 0, 0, 0], [120, 120], 'fixed'],
  ['right', 'none', 400, 300, 420, 600, [0, 0, 0, 0], [120, 120], 'fixed'],
  // Scrolled 100: an absolute box is held at the page's top, a fixed one at the window's.
  ['top', 'none', 200, 108, 420, 600, [0, 0, 0, 0], [120, 120], 'absolute', 'ltr', 100],
  ['top', 'none', 200, 108, 420, 600, [0, 0, 0, 0], [120, 120], 'fixed', 'ltr', 100],
  // Right to left: flip-start's diagonal runs from the top-right corner, and
  // the right edge is the start edge an absolute box is held at.
  ['bottom span-left', 'flip-start flip-block', 332, 500, 420, 600, [0, 0, 0, 0], [120, 120], 'absolute', 'rtl'],
  ['bottom span-left', 'flip-start flip-block', 332, 500, 420, 600, [1, 2, 3, 4], [120, 120], 'absolute', 'rtl'],
  ['right', 'none', 332, 300, 420, 600, [0, 0, 0, 0], [120, 120], 'absolute', 'rtl'],
  ['left', 'none', 8, 300, 420, 600, [0, 0, 0, 0], [120, 120], 'absolute', 'rtl'],
];
let server, native;
beforeAll(async () => {
  const source = `component App
  state flips = "${tries}"
  action clear
    flips = "none"
  action start
    flips = "flip-start flip-block, flip-block"
  view
    column
      button "Open" id="anchor" testId="anchor" popovertarget="pop" position="absolute" left=332 top=560 width=80 height=32
      button "Clear" testId="clear" press=clear
      button "Start" testId="start" press=start
      button "Open fixed" id="anchor2" testId="anchor2" popovertarget="pop2" position="absolute" left=332 top=500 width=80 height=32
      column id="pop2" testId="pop2" popover="auto" position-area="bottom span-right" position-try-fallbacks="none" width=320 height=120 margin=0 padding=0 border="1px solid #888"
        text "Fixed"
      column id="pop" testId="pop" popover="auto" position="absolute" position-area="bottom span-right" position-try-fallbacks=flips width=320 height=120 margin=0 padding=0 border="1px solid #888"
        text "Popover"
`;
  writeFileSync(resolve(dir, 'app.contract'), source);
  writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Popover flips', app: { id: 'com.example.popflip', name: 'Popover flips' }, host: { web: {} } }));
  const built = await new Promise((done, fail) => {
    const child = spawn(process.execPath, ['host/web-js/build.mjs', 'popflip', '--out', dist, '--render', 'none'],
      { cwd: root, env: { ...process.env, EXACT_APP_DIR: dir, CARGO_BUILD_JOBS: '4' }, stdio: ['ignore', 'pipe', 'pipe'] });
    let log = '';
    child.stdout.on('data', b => { log += b; });
    child.stderr.on('data', b => { log += b; });
    child.on('error', fail); child.on('close', code => done({ code, log }));
  });
  expect(built.code, built.log).toBe(0);
  server = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch(req) {
    const path = new URL(req.url).pathname;
    return new Response(Bun.file(resolve(dist, path === '/' ? 'index.html' : path.slice(1))));
  } });
  if (process.platform === 'darwin') {
    const stub = `struct Style { var string: String?; func length(basis: CGFloat) -> CGFloat? { nil } }
class NodeView { var style: [String:Style] = [:]; func number(_ name: String) -> CGFloat { 0 } }
`;
    // The page is 1200 tall when scrolled; the viewport is the layer.
    const calls = cases.map(([area, fallback, x, y, w, h, m = [0, 0, 0, 0], [bw, bh] = [320, 120], position = 'absolute', dir = 'ltr', scroll = 0]) => `do {
let page = CGRect(x:0,y:${-scroll},width:${w},height:${scroll ? 1200 : h})
let p = PositionArea.origin(${JSON.stringify(area)}, anchor: CGRect(x:${x},y:${y - scroll},width:80,height:32), size:CGSize(width:${bw + 2},height:${bh + 2}), margins:.init(top:${m[0]},right:${m[1]},bottom:${m[2]},left:${m[3]}), fallbacks:${JSON.stringify(fallback)}, overflow:${position === 'absolute' ? '.absolute(page: page)' : '.fixed'}, rtl:${dir === 'rtl'}, in:CGRect(x:0,y:0,width:${w},height:${h}))
print("\\(p.x),\\(p.y)")
}`).join('\n');
    const swift = spawnSync('xcrun', ['swift', '-'], { input: readFileSync(resolve(root, 'host/apple/Sources/ExactKit/PositionArea.swift'), 'utf8') + '\n' + stub + calls, encoding: 'utf8', timeout: 60000 });
    expect(swift.status, swift.stderr).toBe(0);
    native = swift.stdout.trim().split('\n').map(s => s.split(',').map(Number));
  }
}, 180000);
afterAll(() => { server?.stop(true); rmSync(dir, { recursive: true, force: true }); });

test('authored fallback CSS reaches Chrome; native rectangles match its edge cases', async () => {
  const browser = await suiteBrowser();
  try {
    const page = await browser.newPage({ viewport: { width: 420, height: 600 } });
    await page.goto(`http://127.0.0.1:${server.port}/`);
    await page.locator('[data-testid="anchor"]').click();
    const rect = () => page.locator('[data-testid="pop"]').evaluate(el => { const r = el.getBoundingClientRect(); return [r.x, r.y, r.width, r.height]; });
    expect(await rect()).toEqual([90, 438, 322, 122]);
    await page.evaluate(async () => {
      document.body.style.height = '1200px';
      const moved = new Promise(resolve => window.addEventListener('scroll', resolve, { once: true }));
      window.scrollTo(0, 10); await moved;
      await new Promise(requestAnimationFrame);
    });
    expect(await rect()).toEqual([90, 428, 322, 122]);
    await page.evaluate(() => { window.scrollTo(0, 0); document.body.style.height = ''; });
    await page.setViewportSize({ width: 800, height: 800 });
    expect(await rect()).toEqual([90, 438, 322, 122]);
    await page.locator('[data-testid="clear"]').click();
    await page.setViewportSize({ width: 420, height: 600 });
    await page.locator('[data-testid="anchor"]').click();
    expect(await rect()).toEqual([332, 592, 322, 122]);
    // A dynamic value reaches the browser through the JS target's own check.
    const tries = () => page.locator('[data-testid="pop"]').evaluate(el => getComputedStyle(el).positionTryFallbacks);
    await page.locator('[data-testid="start"]').click();
    await page.locator('[data-testid="anchor"]').click();
    expect(await tries()).toBe('flip-start flip-block, flip-block');
    // A popover with no authored position keeps the UA's `position: fixed`,
    // which Apple's placement assumes for a box that is not absolute.
    await page.locator('[data-testid="anchor2"]').evaluate(el => el.click()); // under the open popover
    expect(await page.locator('[data-testid="pop2"]').evaluate(el => {
      const r = el.getBoundingClientRect();
      return [getComputedStyle(el).position, r.x, r.y];
    })).toEqual(['fixed', 98, 478]);
    for (const [index, [area, fallback, x, y, width, height, margins = [0, 0, 0, 0], [bw, bh] = [320, 120], position = 'absolute', dir = 'ltr', scroll = 0]] of cases.entries()) {
      await page.setViewportSize({ width, height });
      const oracle = await page.evaluate(async ({ area, fallback, x, y, margins, bw, bh, position, dir, scroll }) => {
        document.documentElement.dir = dir;
        if (scroll) { document.body.style.height = '1200px'; window.scrollTo(0, scroll); }
        const anchor = document.createElement('button'), pop = document.createElement('div');
        anchor.style.cssText = `position:absolute;left:${x}px;top:${y}px;width:80px;height:32px`;
        pop.popover = 'manual';
        pop.style.cssText = `position:${position};padding:0;width:${bw}px;height:${bh}px;border:1px solid;margin:${margins.map(v => v + 'px').join(' ')};position-area:${area};position-try-fallbacks:${fallback}`;
        document.body.append(anchor, pop); pop.showPopover({ source: anchor });
        await new Promise(requestAnimationFrame);
        const r = pop.getBoundingClientRect(); pop.remove(); anchor.remove();
        document.documentElement.dir = ''; window.scrollTo(0, 0); document.body.style.height = '';
        return [r.x, r.y];
      }, { area, fallback, x, y, margins, bw, bh, position, dir, scroll });
      if (native) expect(native[index], JSON.stringify(cases[index])).toEqual(oracle);
    }
  } finally { await browser.close(); }
}, 60000);
