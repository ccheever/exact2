// A `contextmenu` from a control in a real browser (issues/closed/20261012-web-an-exact-contextmenu-inside-a.md): a
// right-click, or on a Mac a Control-click, on a checkbox, switch, radio, range, date or time input inside a node
// with `contextmenu` runs that node's handler and keeps the browser's own menu shut. Chrome sends the event from each
// of them and macOS runs the node's (#487). A `contextPopover` above one opens. A text field and a textarea keep the
// browser's edit menu: no handler runs and nothing prevents it (review b5-b 3). The app is compiled from Contract,
// built for the JS target and driven by the agent, as `exact` does; the wasm host takes the same rule
// (navigation.js `editMenu`), which conformance's `menus` plan compares with this one.
import { test, expect } from 'bun:test';
import { chromium as playwrightChromium } from 'playwright-core';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { open } from '../../../scripts/agent.mjs';
import { chromium } from '../../../scripts/agent-launch.mjs';
import { serveBuildTree } from '../serve.mjs';
import { need } from '../../../scripts/coverage.mjs';

const ROOT = resolve(new URL('../../..', import.meta.url).pathname);
const { unavailable } = chromium();
const check = need('chrome', unavailable);

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

// The helper both targets call: real HTML popover methods, including omitted
// source and the browser's queued, coalesced ToggleEvent (issue #319).
check('popover commands preserve HTML source, focus, idempotence and toggle coalescing', async () => {
  const browser = await playwrightChromium.launch({ executablePath: chromium().executable, headless: true });
  try {
    const page = await browser.newPage();
    await page.setContent('<main id="root"><button id="trigger" data-view="1">Menu</button><div popover="auto" id="menu" data-view="2"><button id="first">Done</button></div></main>');
    const source = readFileSync(resolve(ROOT, 'host/web/navigation.js')).toString('base64');
    const result = await page.evaluate(async source => {
      const { popoverCommand: command, navigation } = await import('data:text/javascript;base64,' + source);
      const menu = document.getElementById('menu'), root = document.getElementById('root'), events = [], errors = [];
      menu.addEventListener('toggle', e => events.push([e.oldState, e.newState]));
      const changed = () => new Promise(resolve => menu.addEventListener('toggle', resolve, { once: true }));
      const run = (method, id, source) => command(method, id, source, message => errors.push(message));
      let next = changed();
      run('showPopover', 'menu', 'trigger'); run('showPopover', 'menu', 'trigger');
      document.getElementById('first').focus();
      const focus = document.activeElement.id, open = navigation.observation(root).popover;
      if (errors.length) throw new Error(errors.join('; '));
      await next;
      next = changed(); run('hidePopover', 'menu'); await next;
      next = changed(); run('showPopover', 'menu');
      const unsourced = navigation.observation(root).popover;
      run('hidePopover', 'menu');
      if (errors.length) throw new Error(errors.join('; '));
      await next;
      run('showPopover', 'absent'); run('hidePopover', 'trigger'); run('showPopover', 'menu', 'absent');
      return { focus, open, unsourced, events, errors };
    }, source);
    expect(result.focus).toBe('first');
    expect(result.open).toEqual({ phase: 'open', popover: 2, source: 1 });
    expect(result.unsourced).toEqual({ phase: 'open', popover: 2, source: null });
    expect(result.events).toEqual([['closed', 'open'], ['open', 'closed'], ['closed', 'closed']]);
    expect(result.errors).toHaveLength(3);
    expect(result.errors.every(message => message.includes('refused:'))).toBe(true);
  } finally { await browser.close(); }
}, 60000);

// Exercise the wasm dispatch arm and its end-of-batch queue over real DOM focus.
check('wasm popover and focus commands preserve authored order', async () => {
  const glue = readFileSync(resolve(ROOT, 'host/web/glue.js'), 'utf8');
  const dispatch = glue.slice(glue.indexOf('        else if (op.name === "focus"'), glue.indexOf('        else if (op.name === "preventDefault"'));
  const drain = glue.slice(glue.indexOf('  // Focusing can dispatch an action:'), glue.indexOf('  focusAutofocus(); inputHandlers?.arrive();'));
  const browser = await playwrightChromium.launch({ executablePath: chromium().executable, headless: true });
  try {
    const page = await browser.newPage();
    await page.setContent('<main id="root"><button id="trigger">Menu</button><div popover="auto" id="menu"><input id="first" autofocus></div></main>');
    const navigation = readFileSync(resolve(ROOT, 'host/web/navigation.js')).toString('base64');
    const result = await page.evaluate(async ({ navigation, dispatch, drain }) => {
      const { popoverCommand, runFocusCommands, inertAncestor } = await import('data:text/javascript;base64,' + navigation);
      const root = document.getElementById('root'), log = message => { throw new Error(message); };
      const apply = new Function('ops', 'root', 'popoverCommand', 'runFocusCommands', 'inertAncestor', 'log',
        'const inputReady = true, focusCommands = []; for (const op of ops) { if (false) {} ' + dispatch + ' } ' + drain);
      const command = (name, ...args) => ({ name, args });
      const run = ops => { apply(ops, root, popoverCommand, runFocusCommands, inertAncestor, log); return document.activeElement.id; };
      const forward = run([command('focus', 'trigger'), command('showPopover', 'menu')]);
      document.getElementById('menu').hidePopover();
      const reverse = run([command('showPopover', 'menu'), command('focus', 'trigger')]);
      return [forward, reverse];
    }, { navigation, dispatch, drain });
    expect(result).toEqual(['first', 'trigger']);
  } finally { await browser.close(); }
}, 60000);
