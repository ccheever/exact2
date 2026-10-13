// LLP 1116.004 D5.2 in a real browser: a value chosen from a few is a `radiogroup` of native
// `radio` buttons, which a default build's stylesheet (look.mjs) draws as a segmented control (a track, the
// checked radio raised in it) and the input piece (input-glue.js, which both web targets load)
// gives one Tab stop and the arrow keys, as WAI-ARIA's radio group has them. The nodes are as
// the hosts write them; a press checks its radio as the app's commit would.
import { test, expect } from 'bun:test';
import { spawn } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { Cdp } from '../../../scripts/agent.mjs';
import { chromium } from '../../../scripts/agent-launch.mjs';
import { DEFAULT_LOOK, withLook } from '../look.mjs';
import { need } from '../../../scripts/coverage.mjs';

const WEB = resolve(new URL('..', import.meta.url).pathname);
const { executable: chrome, unavailable } = chromium();
const check = need('chrome', unavailable);

const radio = (id, label, checked, extra = '') =>
  `<button type="button" data-button-style="bordered" data-native role="radio" aria-checked="${checked}" id="${id}"${extra}><span data-exact-text>${label}</span></button>`;
const page = withLook(readFileSync(resolve(WEB, 'index.html'), 'utf8'), DEFAULT_LOOK).match(/<style>[\s\S]*?<\/style>/)[0] + `
<div id="exact-root">
  <button type="button" data-button-style="bordered" data-native id="before">Before</button>
  <div role="radiogroup" aria-label="Tip" id="tip" style="display:flex">
    ${radio('t15', '15%', false)}${radio('t18', '18%', true)}${radio('t20', '20%', false, ' disabled')}${radio('t25', '25%', false)}
  </div>
  <button type="button" data-button-style="bordered" data-native id="after">After</button>
  <span id="track" style="background-color:var(--exact-tertiary-fill)"></span><span id="ground" style="background-color:var(--exact-background)"></span>
</div>
<script type="module">
  import { createInputHandlers } from './input-glue.js';
  const root = document.getElementById('exact-root');
  window.pressed = [];
  for (const r of root.querySelectorAll('[role=radio]')) r.addEventListener('click', () => {
    pressed.push(r.id);
    for (const o of r.parentElement.children) o.setAttribute('aria-checked', String(o === r));
  });
  createInputHandlers({ root, views: new Map(), retiredViews: new Set(), ready: () => true, inertAncestor: el => el.closest('[inert]'), dispatch() {} });
  window.ready = true;
</script>`;

/** The page in headless Chrome: `test(evaluate, call)` with its `Runtime.evaluate` and its CDP session. */
async function inChrome(test) {
  const server = createServer((req, res) => {
    if (req.url === '/') { res.writeHead(200, { 'content-type': 'text/html' }); res.end(page); return; }
    res.writeHead(200, { 'content-type': 'text/javascript' }); res.end(readFileSync(resolve(WEB, 'input-glue.js')));
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const profile = mkdtempSync(resolve(tmpdir(), 'exact-radio-group-'));
  const child = spawn(chrome, ['--headless=new', '--remote-debugging-pipe', '--window-size=600,400', `--user-data-dir=${profile}`,
    '--no-sandbox', '--no-first-run', '--disable-background-networking', 'about:blank'], { stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
  try {
    const cdp = new Cdp(child.stdio[3], child.stdio[4]);
    const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
    const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
    const call = (method, params) => cdp.send(method, params, sessionId);
    const evaluate = async (expression) => {
      const reply = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
      if (reply.exceptionDetails) throw new Error(reply.exceptionDetails.exception?.description ?? reply.exceptionDetails.text);
      return reply.result.value;
    };
    await call('Page.navigate', { url: `http://127.0.0.1:${server.address().port}/` });
    for (let i = 0; !(await evaluate('window.ready === true')); i++) { if (i > 2000) throw new Error('page never ready'); await Bun.sleep(5); }
    await test(evaluate, call);
  } finally {
    child.kill();
    server.close();
    rmSync(profile, { recursive: true, force: true });
  }
}

const CODES = { Tab: 9, ArrowLeft: 37, ArrowUp: 38, ArrowRight: 39, ArrowDown: 40 };
const key = async (call, name, modifiers = 0) => {
  for (const type of ['keyDown', 'keyUp']) await call('Input.dispatchKeyEvent', { type, key: name, code: name, windowsVirtualKeyCode: CODES[name], modifiers });
};

check('a radio group is a segmented control with one Tab stop, and the arrows move its choice', () => inChrome(async (evaluate, call) => {
  // The look: a track (the look's `-exact-tertiary-fill`, 8 px corners) no wider than its segments, the checked radio
  // a chip of the page's ground raised in it (shadowed, 6 px corners), the others on the track.
  const look = id => evaluate(`(s => [s.backgroundColor, s.boxShadow !== 'none', s.borderRadius])(getComputedStyle(document.getElementById('${id}')))`);
  const [track, ground] = await evaluate(`['track', 'ground'].map(id => getComputedStyle(document.getElementById(id)).backgroundColor)`);
  expect([track === 'rgba(0, 0, 0, 0)', /^rgb\(/.test(ground)]).toEqual([false, true]); // the sheet carries both roles (neutral-look.test.mjs holds their values)
  expect(await evaluate(`(s => [s.backgroundColor, s.borderRadius])(getComputedStyle(document.getElementById('tip')))`)).toEqual([track, '8px']);
  expect(await evaluate(`document.getElementById('tip').getBoundingClientRect().width < document.getElementById('exact-root').getBoundingClientRect().width / 2`)).toBe(true);
  expect(await look('t18')).toEqual([ground, true, '6px']);
  expect(await look('t15')).toEqual(['rgba(0, 0, 0, 0)', false, '6px']);
  // The accessibility tree: a radio group of radios, the checked one checked.
  const { nodes } = await call('Accessibility.getFullAXTree', {});
  const radios = nodes.filter(n => n.role?.value === 'radio').map(n => [n.name?.value, n.properties?.find(p => p.name === 'checked')?.value.value]);
  expect(nodes.some(n => n.role?.value === 'radiogroup' && n.name?.value === 'Tip')).toBe(true);
  expect(radios).toEqual([['15%', 'false'], ['18%', 'true'], ['20%', 'false'], ['25%', 'false']]);

  // One Tab stop, the checked radio: Tab from Before lands on 18%, the next Tab leaves the group; Shift-Tab returns to it.
  await evaluate(`document.getElementById('before').focus()`);
  await key(call, 'Tab');
  expect(await evaluate('document.activeElement.id')).toBe('t18');
  await key(call, 'Tab');
  expect(await evaluate('document.activeElement.id')).toBe('after');
  await key(call, 'Tab', 8);
  expect(await evaluate('document.activeElement.id')).toBe('t18');

  // The arrows: the next radio past the disabled one, focused and pressed; wrapping at either end; Up and Down too.
  await key(call, 'ArrowRight');
  expect(await evaluate(`[document.activeElement.id, pressed.splice(0), document.getElementById('t25').getAttribute('aria-checked')]`)).toEqual(['t25', ['t25'], 'true']);
  expect(await look('t25')).toEqual([ground, true, '6px']);
  await key(call, 'ArrowRight');
  expect(await evaluate(`[document.activeElement.id, pressed.splice(0)]`)).toEqual(['t15', ['t15']]);
  await key(call, 'ArrowLeft');
  expect(await evaluate(`[document.activeElement.id, pressed.splice(0)]`)).toEqual(['t25', ['t25']]);
  await key(call, 'ArrowUp');
  expect(await evaluate(`[document.activeElement.id, pressed.splice(0)]`)).toEqual(['t18', ['t18']]);
  await key(call, 'ArrowDown');
  expect(await evaluate(`[document.activeElement.id, pressed.splice(0)]`)).toEqual(['t25', ['t25']]);
  // The Tab stop follows the choice: in from After, the checked 25%.
  await evaluate(`document.getElementById('after').focus()`);
  await key(call, 'Tab', 8);
  expect(await evaluate('document.activeElement.id')).toBe('t25');

  // Right to left, Left is the next radio.
  await evaluate(`document.getElementById('tip').dir = 'rtl'`);
  await key(call, 'ArrowLeft');
  expect(await evaluate(`[document.activeElement.id, pressed.splice(0)]`)).toEqual(['t15', ['t15']]);
  // A key's own handler that prevents it keeps it.
  await evaluate(`document.getElementById('t15').addEventListener('keydown', e => e.preventDefault())`);
  await key(call, 'ArrowLeft');
  expect(await evaluate(`[document.activeElement.id, pressed.splice(0)]`)).toEqual(['t15', []]);
}), 30000);
