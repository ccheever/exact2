// The agent's tap and a sticky box (QUEUE "The agent's tap doesn't scroll a target covered by a sticky overlay clear of
// it"; theme2's 14:49 ruling): a target whose middle a sticky box of its own scroller covers is scrolled into the band
// the box leaves (`reveal`, host/web/agent-input.js), as a person scrolls a row out from under a docked bar, and pressed;
// a pass-through sticky (pointer-events: none), which the tap's own hit-test passes, scrolls nothing; a popover over a
// row is no scroll's to clear, and the tap still refuses there; nor a dock taller than the band it leaves.
import { test, expect } from 'bun:test';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { open } from '../../scripts/agent.mjs';
import { serveBuildTree } from './serve.mjs';

const ROOT = resolve(new URL('../..', import.meta.url).pathname);
const compiler = resolve(process.env.CARGO_TARGET_DIR ?? resolve(ROOT, 'target'), 'debug', process.platform === 'win32' ? 'contract.exe' : 'contract');
const rows = Array.from({ length: 20 }, (_, i) => i + 1).join(', ');
const CONTRACT = `component StickyFixture
  state pressed = ""
  action press(name: string)
    pressed = name
  view
    column testId="root" width=320
      scroll testId="port" height=400
        column testId="header" position="sticky" top=0 height=60 z-index=2 background-color="-exact-secondary-background"
          text "Header"
        for i in [${rows}] key=i
          column testId=\`card-\${i}\` overflow="hidden" border-radius=8
            button testId=\`row-\${i}\` press=press(\`row-\${i}\`) height=50 width="100%"
              text \`Row \${i}\`
        column testId="ghost" position="sticky" bottom=110 height=40 z-index=3 pointer-events="none"
          text "Pass-through"
        column testId="dock" position="sticky" bottom=0 height=100 z-index=2 background-color="-exact-secondary-background"
          text "Dock"
`;

async function serve() {
  const dir = mkdtempSync(join(tmpdir(), 'exact-sticky-')), contract = join(dir, 'app.contract'), plan = join(dir, 'app.plan'), dist = join(dir, 'dist');
  writeFileSync(contract, CONTRACT);
  const compile = spawnSync(compiler, ['build', contract, '-o', plan], { encoding: 'utf8' });
  if (compile.status !== 0) throw new Error(compile.stderr);
  const build = spawnSync(process.execPath, ['host/web-js/build.mjs', 'caltrain', '--plan', plan, '--out', dist, '--render', 'none'], { cwd: ROOT, encoding: 'utf8' });
  if (build.status !== 0) throw new Error(build.stderr);
  const server = createServer((request, response) => serveBuildTree(dist, request, response));
  await new Promise((done) => server.listen(0, '127.0.0.1', done));
  server.unref();
  return `http://127.0.0.1:${server.address().port}/`;
}

const drive = async (session) => {
  const pressed = async () => (await session.state()).slots.pressed;
  // Where each row's middle is, in the port: the first row whose middle lies in a band.
  const rowIn = (from, to) => session.carrier.evaluate(`(() => { const port = document.querySelector('[data-testid="port"]').getBoundingClientRect();
    for (let i = 1; i <= 20; i++) { const b = document.querySelector('[data-testid="row-' + i + '"]').getBoundingClientRect(), m = (b.top + b.bottom) / 2 - port.top; if (m >= ${from} && m < ${to}) return 'row-' + i; } return null; })()`);
  const scrollTop = () => session.carrier.evaluate(`document.querySelector('[data-testid="port"]').scrollTop`);
  // Under the dock (the port's bottom 100 px): scrolled clear, pressed.
  const docked = await rowIn(300, 400);
  expect(docked).not.toBe(null);
  const t1 = await session.tap(docked);
  expect(t1.scrolled).toBeDefined();
  expect(await pressed()).toBe(docked);
  // Under the header (its top 60 px), the port scrolled down a way first: scrolled clear the other way, pressed.
  await session.carrier.evaluate(`document.querySelector('[data-testid="port"]').scrollTop = 300`);
  const headed = await rowIn(0, 60);
  expect(headed).not.toBe(null);
  const before = await scrollTop(), t2 = await session.tap(headed);
  expect(t2.scrolled).toBeDefined();
  expect(await scrollTop()).toBeLessThan(before);
  expect(await pressed()).toBe(headed);
  // Under the pass-through box (the port's 250..290, clear of the dock): no scroll, pressed through it.
  await session.carrier.evaluate(`document.querySelector('[data-testid="port"]').scrollTop = 0`);
  const ghosted = await rowIn(252, 288);
  expect(ghosted).not.toBe(null);
  const t3 = await session.tap(ghosted);
  expect(t3.scrolled).toBeUndefined();
  expect(await pressed()).toBe(ghosted);
  // A popover over a row in the clear, opened from inside the dock (Astra's round 2): no scroll takes it away, and the
  // tap refuses, naming what covers it.
  const clearRow = await rowIn(100, 200);
  await session.carrier.evaluate(`(() => { const r = document.querySelector('[data-testid="${clearRow}"]').getBoundingClientRect(), p = document.createElement('div');
    p.id = 'sticky-test-popover'; p.setAttribute('popover', 'manual'); p.style.cssText = 'position:fixed;margin:0;inset:auto;left:' + r.left + 'px;top:' + (r.top - 10) + 'px;width:' + r.width + 'px;height:' + (r.height + 20) + 'px';
    document.querySelector('[data-testid="dock"]').append(p); p.showPopover(); })()`);
  const top = await scrollTop();
  await expect(session.tap(clearRow)).rejects.toThrow(/covers its middle/);
  expect(await scrollTop()).toBe(top);
  await session.carrier.evaluate(`document.getElementById('sticky-test-popover')?.remove()`);
  // A dock over more than half the port (each row in a card that clips, as Astra's round 1 had it): the band above it.
  await session.carrier.evaluate(`document.querySelector('[data-testid="dock"]').style.height = '240px'`);
  const tall = await rowIn(200, 300);
  expect(tall).not.toBe(null);
  expect((await session.tap(tall)).scrolled).toBeDefined();
  expect(await pressed()).toBe(tall);
  // A header across from it (100 px) narrows the band to the 60 px between them: the row lands there, not under it.
  await session.carrier.evaluate(`document.querySelector('[data-testid="header"]').style.height = '100px'`);
  const between = await rowIn(300, 400);
  expect(between).not.toBe(null);
  expect((await session.tap(between)).scrolled).toBeDefined();
  expect(await pressed()).toBe(between);
  const landed = await session.carrier.evaluate(`(() => { const port = document.querySelector('[data-testid="port"]').getBoundingClientRect(), b = document.querySelector('[data-testid="${between}"]').getBoundingClientRect(); return (b.top + b.bottom) / 2 - port.top; })()`);
  expect(landed >= 100 && landed < 160).toBe(true);
  // A dock taller than the band it would leave: nothing to scroll it clear into, so the tap refuses.
  await session.carrier.evaluate(`document.querySelector('[data-testid="dock"]').style.height = '340px'`);
  const still = await scrollTop();
  await expect(session.tap(await rowIn(300, 400))).rejects.toThrow(/covers its middle/);
  expect(await scrollTop()).toBe(still);
};

// (The JS page; the wasm page's glue.js calls the same `reveal`, from agent-input.js.)
test('a tap scrolls its target clear of a sticky box of its own scroller, and only then (theme2\'s 14:49 conditions)', async () => {
  const session = await open({ host: 'web', url: await serve() });
  try { await drive(session); } finally { await session.close(); }
}, 900000);
