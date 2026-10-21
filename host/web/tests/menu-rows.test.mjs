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
  <button id="more" popovertarget="submenu">More</button>
  <button id="close" popovertarget="menu" popovertargetaction="hide">Close</button>
</div>
<div id="submenu" popover role="menu"><button id="deep">Deep</button></div>
<div id="picker" popover><button id="emoji">👍</button></div>
<script src="/menu-rows.js"></script>
<script>
  globalThis.exact.menuRows();
  window.pressed = [];
  document.getElementById('react').addEventListener('click', () => pressed.push('react'));
  window.open = () => ['menu', 'submenu', 'picker'].filter(id => document.getElementById(id).matches(':popover-open'));
  window.ready = true;
</script>`;

async function inChrome(test) {
  const server = createServer((req, res) => {
    if (req.url === '/') { res.writeHead(200, { 'content-type': 'text/html' }); res.end(page); return; }
    if (req.url !== '/menu-rows.js') { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': 'text/javascript' }); res.end(readFileSync(resolve(WEB, 'menu-rows.js')));
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
      await evaluate('new Promise(r => setTimeout(r, 50))');
    };
    await test(evaluate, press);
  } finally {
    await suite.close();
    server.close();
  }
}

check('a menu row opens its popover from the menu\'s invoker, the menu closes and the popover stays', () => inChrome(async (evaluate, press) => {
  await press('opener');
  expect(await evaluate('open()')).toEqual(['menu']);
  await press('react');
  expect(await evaluate('pressed')).toEqual(['react']);
  expect(await evaluate('open()')).toEqual(['picker']);
  expect(await evaluate(`document.getElementById('picker').exactOpener?.id`)).toBe('opener');
  // A press in the picker keeps it open: the menu, now closed, was never its ancestor.
  await press('emoji');
  expect(await evaluate('open()')).toEqual(['picker']);
}), 60000);

check('a submenu stays nested in its menu, and a row that hides the menu hides it', () => inChrome(async (evaluate, press) => {
  await press('opener');
  await press('more');
  expect(await evaluate('open()')).toEqual(['menu', 'submenu']);
  await evaluate(`document.getElementById('menu').hidePopover()`);
  expect(await evaluate('open()')).toEqual([]);
  await press('opener');
  await press('close');
  expect(await evaluate('open()')).toEqual([]);
}), 60000);
