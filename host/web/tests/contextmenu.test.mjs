// A `contextmenu` from a control in a real browser (issues/closed/20261012-web-an-exact-contextmenu-inside-a.md): a
// right-click, or on a Mac a Control-click, on a checkbox, switch, radio, range, date or time input inside a node
// with `contextmenu` runs that node's handler and keeps the browser's own menu shut. Chrome sends the event from each
// of them and macOS runs the node's (#487). A `contextPopover` above one opens. A text field and a textarea keep the
// browser's edit menu: no handler runs and nothing prevents it (review b5-b 3). The app is compiled from Contract,
// built for the JS target and driven by the agent, as `exact` does; the wasm host takes the same rule
// (navigation.js `editMenu`), which conformance's `menus` plan compares with this one.
import { test, expect } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { open } from '../../../scripts/agent.mjs';
import { chromium } from '../../../scripts/agent-launch.mjs';
import { serveBuildTree } from '../serve.mjs';

const ROOT = resolve(new URL('../../..', import.meta.url).pathname);
const { unavailable } = chromium();
if (unavailable) console.warn(`SKIP: ${unavailable}`);
const check = unavailable ? (name, ...args) => test.skip(`${name} — ${unavailable}`, ...args) : test;

const MENUS = `component Menus
  state heard = ""
  state changes = 0
  action menu(name: string)
    heard = \`\${heard}\${name};\`
  action checked(value: bool)
    changes = changes + 1
  action picked(value: string)
    changes = changes + 1
  view
    column testId="root" gap=8 padding=8
      text heard testId="heard"
      text \`\${changes}\` testId="changes"
      row contextmenu=menu("row") testId="row" gap=8 padding=8 align-items="center" flex-wrap="wrap"
        input type="checkbox" change=checked aria-label="Check" testId="check"
        input type="checkbox" switch change=checked aria-label="Switch" testId="switch"
        input type="radio" name="r" value="a" change=picked aria-label="Radio" testId="radio"
        input type="range" min=0 max=100 value=20 aria-label="Level" width=120 testId="range"
        input type="date" value="2026-10-12" change=picked aria-label="Day" testId="date"
        input type="time" value="09:30" change=picked aria-label="Hour" testId="time"
        input value="" change=picked aria-label="Notes" testId="field"
        textarea value="" change=picked aria-label="Body" testId="area" height=40
      row contextPopover="pop" testId="held" gap=8 padding=8 align-items="center"
        text "Held"
        input type="checkbox" change=checked aria-label="Other" testId="other"
      column id="pop" popover="auto" testId="pop" padding=8
        text "Popover"
`;

check('a contextmenu from a checkbox, switch, radio, range, date or time input is its node\'s; a text field keeps the browser\'s menu', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-contextmenu-')), contract = join(dir, 'app.contract'), plan = join(dir, 'app.plan'), dist = join(dir, 'dist');
  writeFileSync(contract, MENUS);
  const compile = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'build', contract, '-o', plan], { cwd: ROOT, encoding: 'utf8' });
  expect(compile.status, compile.stderr).toBe(0);
  const build = spawnSync(process.execPath, ['host/web-js/build.mjs', 'caltrain', '--plan', plan, '--out', dist, '--render', 'none'], { cwd: ROOT, encoding: 'utf8' });
  expect(build.status, build.stderr).toBe(0);
  const server = createServer((request, response) => serveBuildTree(dist, request, response));
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  let s;
  try {
    s = await open({ host: 'web', url: `http://127.0.0.1:${server.address().port}/` });
    // Whether each `contextmenu` was prevented, read after every listener has run.
    await s.carrier.evaluate(`(() => { window.prevented = []; addEventListener('contextmenu', e => setTimeout(() => prevented.push(e.defaultPrevented)), true); })()`);
    // Each click, from the platform at the target's middle: what the app heard, whether a control changed, and
    // whether the browser's own menu was prevented; every case is compared at once.
    const got = [], want = [];
    let heard = '';
    const click = async (how, id, menu, prevented) => {
      await s.tap(id, how === 'right-click' ? { contextmenu: true } : { modifiers: 'Control' });
      await s.clock('+16');
      const { slots } = await s.state(), added = slots.heard.slice(heard.length);
      heard = slots.heard;
      got.push(`${how} ${id}: ${added || '-'} · changes ${slots.changes} · prevented ${await s.carrier.evaluate('prevented.at(-1)')}`);
      want.push(`${how} ${id}: ${menu} · changes 0 · prevented ${prevented}`);
    };
    const controls = ['check', 'switch', 'radio', 'range', 'date', 'time'];
    for (const id of controls) await click('right-click', id, 'row;', true);
    // A Control-click is the Mac's secondary click, which Chrome sends as `contextmenu` there (the range's own down
    // moves its value first, as Chrome's does).
    if (process.platform === 'darwin') for (const id of controls) await click('Control-click', id, 'row;', true);
    // A text field's own edit menu stays the browser's.
    for (const id of ['field', 'area']) await click('right-click', id, '-', false);
    // The popover a node names opens from a control inside it, and the control stays as it was.
    await click('right-click', 'other', '-', true);
    got.push(`popover open: ${await s.carrier.evaluate(`document.querySelector('[data-testid=pop]').matches(':popover-open')`)}`);
    want.push('popover open: true');
    expect(got).toEqual(want);
  } finally {
    await s?.close();
    server.close();
    rmSync(dir, { recursive: true, force: true });
  }
}, 120000);
