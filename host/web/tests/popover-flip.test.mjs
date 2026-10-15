// Compiled Contract CSS and Apple's placement helper against Chrome's own
// invoker popovers. Build the compiler before this test (docs/testing.md).
import { test, expect, beforeAll, afterAll } from 'bun:test';
import { spawn, spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { chromium } from 'playwright-core';
import { chromium as browserPath } from '../../../scripts/agent-launch.mjs';

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
];
let server, native;
beforeAll(async () => {
  const source = `component App
  state flips = "${tries}"
  action clear
    flips = "none"
  view
    column
      button "Open" id="anchor" testId="anchor" popovertarget="pop" position="absolute" left=332 top=560 width=80 height=32
      button "Clear" testId="clear" press=clear
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
    const stub = `struct Style { var string: String? }
class NodeView { var style: [String:Style] = [:]; func number(_ name: String) -> CGFloat { 0 } }
`;
    const calls = cases.map(([area, fallback, x, y, w, h, m = [0, 0, 0, 0]]) => `do {
let p = PositionArea.origin(${JSON.stringify(area)}, anchor: CGRect(x:${x},y:${y},width:80,height:32), size:CGSize(width:322,height:122), margins:.init(top:${m[0]},right:${m[1]},bottom:${m[2]},left:${m[3]}), fallbacks:${JSON.stringify(fallback)}, in:CGRect(x:0,y:0,width:${w},height:${h}))
print("\\(p.x),\\(p.y)")
}`).join('\n');
    const swift = spawnSync('xcrun', ['swift', '-'], { input: readFileSync(resolve(root, 'host/apple/Sources/ExactKit/PositionArea.swift'), 'utf8') + '\n' + stub + calls, encoding: 'utf8', timeout: 60000 });
    expect(swift.status, swift.stderr).toBe(0);
    native = swift.stdout.trim().split('\n').map(s => s.split(',').map(Number));
  }
}, 180000);
afterAll(() => { server?.stop(true); rmSync(dir, { recursive: true, force: true }); });

test('authored fallback CSS reaches Chrome; native rectangles match its edge cases', async () => {
  const browser = await chromium.launch({ executablePath: browserPath().executable, headless: true });
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
    for (const [index, [area, fallback, x, y, width, height, margins = [0, 0, 0, 0]]] of cases.entries()) {
      await page.setViewportSize({ width, height });
      const oracle = await page.evaluate(({ area, fallback, x, y, margins }) => {
        const anchor = document.createElement('button'), pop = document.createElement('div');
        anchor.style.cssText = `position:absolute;left:${x}px;top:${y}px;width:80px;height:32px`;
        pop.popover = 'manual';
        pop.style.cssText = `position:absolute;padding:0;width:320px;height:120px;border:1px solid;margin:${margins.map(v => v + 'px').join(' ')};position-area:${area};position-try-fallbacks:${fallback}`;
        document.body.append(anchor, pop); pop.showPopover({ source: anchor });
        const r = pop.getBoundingClientRect(); pop.remove(); anchor.remove();
        return [r.x, r.y];
      }, { area, fallback, x, y, margins });
      if (native) expect(native[index], JSON.stringify(cases[index])).toEqual(oracle);
    }
  } finally { await browser.close(); }
}, 60000);
