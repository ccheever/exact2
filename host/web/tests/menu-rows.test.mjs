// exact2's menu rule in a real browser (LLP 1021; theme2 2026-10-21 19:47,
// 22:05): a popover a row of a menu (`role="menu"`) opens is opened as if by
// the menu's own invoker, so the menu closing as its row is chosen doesn't
// take it along; a submenu, itself a menu, stays nested in its menu; a row
// that hides its menu is unchanged. Both web targets run host/web/menu-rows.js.
import { expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { resolve } from 'node:path';
import { cdpBrowser, chromium } from '../../../scripts/agent-launch.mjs';
import { need } from '../../../scripts/coverage.mjs';
import { readyPage } from './page.mjs';

const WEB = resolve(new URL('..', import.meta.url).pathname);
const { unavailable } = chromium();
const check = need('chrome', unavailable);

const page = `<!doctype html>
<button id="opener" popovertarget="menu">Message</button>
<div id="menu" popover role="menu">
  <button id="react" popovertarget="picker">Add Reaction…</button>
  <button id="commanded" commandfor="picker" command="show-popover">Pick (command)</button>
  <button id="fallback" commandfor="" popovertarget="picker">Pick (no command target)</button>
  <button id="switch" popovertarget="submenu">Switches to the picker</button>
  <button id="toggle" popovertarget="manual">Toggle the manual one</button>
  <button id="more" popovertarget="submenu">More</button>
  <button id="close" popovertarget="menu" popovertargetaction="hide">Close</button>
  <button id="confirm" commandfor="sure" command="show-modal">Clear…</button>
  <button id="toDialog" popovertarget="picker">Becomes the dialog's</button>
  <button id="notDialog" commandfor="plain" command="show-modal">A dialog's command on no dialog</button>
  <button id="cancelled" commandfor="picker" command="show-popover">Its command event cancelled</button>
  <fieldset id="fs"><button id="fsRow" popovertarget="picker">Its press disables its fieldset</button></fieldset>
</div>
<div id="plain">Plain</div>
<dialog id="sure">Sure?</dialog>
<div id="submenu" popover role="menu"><button id="deep">Deep</button><button id="deeper" popovertarget="picker">Pick from the submenu</button></div>
<div id="picker" popover><button id="emoji">👍</button></div>
<div id="manual" popover="manual">Manual</div>
<script type="module">
  import '/menu-rows.js';
  globalThis.exact.menuRows();
  window.pressed = [];
  document.getElementById('react').addEventListener('click', () => pressed.push('react'));
  // A press that changes what its row opens, from a submenu to the picker.
  document.getElementById('switch').addEventListener('click', e => e.currentTarget.setAttribute('popovertarget', 'picker'));
  // A press that retargets its row from a popover to a dialog's command (the stated limit: what it named at the click).
  document.getElementById('toDialog').addEventListener('click', e => { e.currentTarget.removeAttribute('popovertarget'); e.currentTarget.setAttribute('commandfor', 'sure'); e.currentTarget.setAttribute('command', 'show-modal'); });
  window.commands = [];
  document.getElementById('picker').addEventListener('command', e => { commands.push('picker ' + e.command + ' ' + e.source.id); if (e.source.id === 'cancelled') e.preventDefault(); });
  document.getElementById('fsRow').addEventListener('click', () => { document.getElementById('fs').disabled = true; });
  document.getElementById('plain').addEventListener('command', e => commands.push('plain ' + e.command));
  document.getElementById('sure').addEventListener('command', e => commands.push(e.command));
  window.open = () => ['menu', 'submenu', 'picker', 'manual'].filter(id => document.getElementById(id).matches(':popover-open'));
  // What opened each popover, as its beforetoggle says.
  window.sources = {};
  document.addEventListener('beforetoggle', e => { if (e.newState === 'open') sources[e.target.id] = e.source?.id ?? null; }, true);
  window.ready = true;
</script>`;

async function inChrome(test) {
  const server = createServer((req, res) => {
    if (req.url === '/') { res.writeHead(200, { 'content-type': 'text/html' }); res.end(page); return; }
    const file = ['menu-rows.js', 'navigation.js', 'grant-admission.js'].find(name => req.url === '/' + name);
    if (!file) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': 'text/javascript' }); res.end(readFileSync(resolve(WEB, file)));
  });
  await new Promise(ok => server.listen(0, '127.0.0.1', ok));
  const suite = await cdpBrowser({ args: ['--window-size=600,900'] }), { cdp } = suite;
  try {
    const { sessionId } = await suite.page();
    const call = (method, params) => cdp.send(method, params, sessionId);
    const evaluate = async expression => {
      const reply = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
      if (reply.exceptionDetails) throw new Error(reply.exceptionDetails.exception?.description ?? reply.exceptionDetails.text);
      return reply.result.value;
    };
    await readyPage(cdp, sessionId, `http://127.0.0.1:${server.address().port}/`);
    const press = async id => {
      const at = await evaluate(`(() => { const r = document.getElementById(${JSON.stringify(id)}).getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()`);
      for (const type of ['mousePressed', 'mouseReleased']) await call('Input.dispatchMouseEvent', { type, button: 'left', clickCount: 1, ...at });
    };
    const enter = async id => {
      await evaluate(`document.getElementById(${JSON.stringify(id)}).focus()`);
      for (const type of ['keyDown', 'keyUp']) await call('Input.dispatchKeyEvent', { type, key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13, ...(type === 'keyDown' ? { text: '\r' } : {}) });
    };
    // Until `expression` is `want` (the rule opens after the row's press, a task later).
    const until = async (expression, want) => {
      for (let i = 0; i < 100; i++) { if (JSON.stringify(await evaluate(expression)) === JSON.stringify(want)) return; await evaluate('new Promise(requestAnimationFrame)'); }
      expect(await evaluate(expression)).toEqual(want);
    };
    await test(evaluate, press, until, enter);
  } finally {
    await suite.close();
    server.close();
  }
}

check('a menu row opens its popover from the menu\'s invoker, the menu closes and the popover stays', () => inChrome(async (evaluate, press, until) => {
  await press('opener');
  await until('open()', ['menu']);
  await press('react');
  await until('open()', ['picker']);
  expect(await evaluate('pressed')).toEqual(['react']);
  expect(await evaluate('sources.picker')).toBe('opener');
  // A press in the picker keeps it open: the menu, now closed, was never its ancestor.
  await press('emoji');
  await until('open()', ['picker']);
}), 60000);

check('a button is marked open while the popover it opened is open, a submenu\'s row too', () => inChrome(async (evaluate, press, until) => {
  const marked = `['opener', 'more', 'react'].filter(id => document.getElementById(id).hasAttribute('data-exact-open'))`;
  await press('opener');
  await until(marked, ['opener']);
  await press('more');
  await until(marked, ['opener', 'more']);
  await press('plain');
  await until(marked, []);
  // The picker a row opens is the menu's invoker's, so the invoker stays marked as the menu gives way to it.
  await press('opener');
  await press('react');
  await until('open()', ['picker']);
  await until(marked, ['opener']);
  await press('plain');
  await until(marked, []);
  // Shown again from another button in the same task, only that one is marked; and a context menu marks none.
  await evaluate(`document.getElementById('menu').showPopover({ source: document.getElementById('opener') })`);
  await until(marked, ['opener']);
  await evaluate(`{ const m = document.getElementById('menu'); m.hidePopover(); m.showPopover({ source: document.getElementById('react') }); }`);
  await until(marked, ['react']);
  await evaluate(`{ const m = document.getElementById('menu'); m.hidePopover(); m.style.setProperty('--exact-context-x', '0px'); m.showPopover({ source: document.getElementById('opener') }); }`);
  await until('open()', ['menu']);
  expect(await evaluate(marked)).toEqual([]);
  await evaluate(`{ const m = document.getElementById('menu'); m.hidePopover(); m.style.removeProperty('--exact-context-x'); }`);
  // Two open popovers from one button: it stays marked until both close.
  await evaluate(`document.body.insertAdjacentHTML('beforeend', '<div id="manual2" popover="manual">Manual 2</div>'); for (const id of ['manual', 'manual2']) document.getElementById(id).showPopover({ source: document.getElementById('opener') })`);
  await until(marked, ['opener']);
  await evaluate(`document.getElementById('manual').hidePopover()`);
  await until('open()', []);
  expect(await evaluate(marked)).toEqual(['opener']);
  await evaluate(`document.getElementById('manual2').hidePopover()`);
  await until(marked, []);
  // A popover removed while open, which closes with no toggle.
  await evaluate(`document.getElementById('picker').showPopover({ source: document.getElementById('opener') })`);
  await until(marked, ['opener']);
  await evaluate(`document.getElementById('picker').remove()`);
  await until(marked, []);
}), 60000);

check('a commandfor row and a key\'s press take the rule too', () => inChrome(async (evaluate, press, until, enter) => {
  await press('opener');
  await until('open()', ['menu']);
  await press('commanded');
  await until('open()', ['picker']);
  expect(await evaluate('commands')).toEqual(['picker show-popover commanded']);
  expect(await evaluate('sources.picker')).toBe('opener');
  await evaluate(`document.getElementById('picker').hidePopover()`);
  await press('opener');
  await until('open()', ['menu']);
  await enter('react');
  await until('open()', ['picker']);
}), 60000);

check('a submenu stays nested in its menu, and a row that hides the menu hides it', () => inChrome(async (evaluate, press, until) => {
  await press('opener');
  await until('open()', ['menu']);
  await press('more');
  await until('open()', ['menu', 'submenu']);
  await evaluate(`document.getElementById('menu').hidePopover()`);
  await until('open()', []);
  await press('opener');
  await until('open()', ['menu']);
  await press('close');
  await until('open()', []);
}), 60000);

check('what a row opens after its press, a commandfor naming nothing, and a toggle that closes', () => inChrome(async (evaluate, press, until) => {
  await press('opener');
  await until('open()', ['menu']);
  await press('switch');
  await until('open()', ['picker']);
  expect(await evaluate('sources.picker')).toBe('opener');
  await evaluate(`document.getElementById('picker').hidePopover(); document.getElementById('switch').setAttribute('popovertarget', 'submenu')`);
  await press('opener');
  await until('open()', ['menu']);
  await press('fallback');
  await until('open()', ['picker']);
  await evaluate(`document.getElementById('picker').hidePopover(); document.getElementById('manual').showPopover()`);
  await press('opener');
  await until('open()', ['menu', 'manual']);
  await press('toggle');
  await until('open()', ['menu']); // the manual one toggled closed; the menu, as HTML has it, stays
}), 60000);

check('a submenu\'s row opens its popover from the outermost menu\'s invoker, and both menus close', () => inChrome(async (evaluate, press, until) => {
  await press('opener');
  await until('open()', ['menu']);
  await press('more');
  await until('open()', ['menu', 'submenu']);
  await press('deeper');
  await until('open()', ['picker']);
  expect(await evaluate('sources.picker')).toBe('opener');
}), 60000);

check('a row that invokes no popover, a dialog\'s command, keeps HTML\'s own activation', () => inChrome(async (evaluate, press, until) => {
  await press('opener');
  await until('open()', ['menu']);
  await press('confirm');
  await until(`document.getElementById('sure').open && document.getElementById('sure').matches(':modal')`, true);
}), 60000);

check('a row its press retargets from a popover to a dialog acts on the popover it named at the click', () => inChrome(async (evaluate, press, until) => {
  await press('opener');
  await until('open()', ['menu']);
  await press('toDialog');
  await until('open()', ['picker']);
  expect(await evaluate('sources.picker')).toBe('opener');
  expect(await evaluate(`document.getElementById('sure').open`)).toBe(false);
}), 60000);

check('a dialog\'s command on what is no dialog does nothing and fires no event', () => inChrome(async (evaluate, press, until) => {
  await press('opener');
  await until('open()', ['menu']);
  await press('notDialog');
  await evaluate('new Promise(r => setTimeout(() => setTimeout(r)))'); // past the rule's task
  expect(await evaluate('commands')).toEqual([]);
  expect(await evaluate('open()')).toEqual(['menu']);
}), 60000);

check('a cancelled command event, and a row its press disables by its fieldset, open nothing', () => inChrome(async (evaluate, press, until) => {
  await press('opener');
  await until('open()', ['menu']);
  await press('cancelled');
  await evaluate('new Promise(r => setTimeout(() => setTimeout(r)))'); // past the rule's task
  expect(await evaluate('commands')).toEqual(['picker show-popover cancelled']);
  expect(await evaluate(`document.getElementById('picker').matches(':popover-open')`)).toBe(false);
  await evaluate(`document.getElementById('menu').hidePopover()`);
  await press('opener');
  await until('open()', ['menu']);
  await press('fsRow');
  await evaluate('new Promise(r => setTimeout(() => setTimeout(r)))');
  expect(await evaluate(`document.getElementById('picker').matches(':popover-open')`)).toBe(false);
}), 60000);
