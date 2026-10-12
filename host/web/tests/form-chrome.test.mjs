// LLP 1116.005 D8 in a real browser: the form chrome a well-made web form has, as the shell's stylesheet
// (index.html, both web targets) and the JS target's runtime (rt.js) draw it. focus-timer lost partly on its
// form: a disabled Save that still drew the accent's fill under grey ink, no outline on the invalid field,
// Enter that did not save. A disabled button looks disabled in each style and scheme whatever its accent,
// and shows no hover or press; an authored colour on it still wins. A field marked `aria-invalid` is
// outlined in the system red, its focus ring red, unless its border is authored. Enter in a single-line
// field with a `submit` submits once. The nodes are as the hosts write them.
import { test, expect } from 'bun:test';
import { spawn } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { Cdp } from '../../../scripts/agent.mjs';
import { chromium } from '../../../scripts/agent-launch.mjs';

const WEB = resolve(new URL('..', import.meta.url).pathname);
const WEB_JS = resolve(WEB, '../web-js');
const { executable: chrome, unavailable } = chromium();
if (unavailable) console.warn(`SKIP: ${unavailable}`);
const check = unavailable ? (name, ...args) => test.skip(`${name} — ${unavailable}`, ...args) : test;

// focus-timer's accent, as the JS target writes `accent-color` (style.rs): the property and `--exact-accent`.
const ACCENT = 'accent-color:light-dark(#116f64, #62d5bd);--exact-accent:light-dark(#116f64, #62d5bd)';
const STYLES = ['filled', 'bordered-prominent', 'tinted', 'bordered-tinted', 'plain', 'borderless', 'bordered', 'gray', 'glass', 'prominent-glass'];
const DESTRUCTIVE = ['gray', 'bordered', 'tinted', 'bordered-tinted', 'plain', 'borderless', 'filled', 'bordered-prominent'];
const button = (id, look, extra = '', style = '') => `<button type="button" data-native data-button-style="${look}" id="${id}" style="display:grid;${style}"${extra}><span data-exact-text>Save settings</span></button>`;
const scheme = s => `<div style="color-scheme:${s};${ACCENT}" id="${s}">
  ${STYLES.map(look => button(`${s}-${look}-on`, look) + button(`${s}-${look}-off`, look, ' disabled')).join('')}
  ${button(`${s}-authored`, 'bordered-prominent', ' disabled', 'color:rgb(255, 0, 0);background-color:rgb(0, 0, 255)')}
  ${DESTRUCTIVE.map(look => button(`${s}-destructive-${look}`, look, ' data-destructive="true"')).join('')}
  <span id="${s}-gray" style="color:GrayText"></span>
  <input data-native id="${s}-valid" value="25">
  <input data-native id="${s}-invalid" value="0" aria-invalid="true" aria-describedby="${s}-error"><span id="${s}-error">Enter a whole number from 1 to 60.</span>
  <input data-native id="${s}-false" value="25" aria-invalid="false">
  <textarea data-native id="${s}-area" aria-invalid="true"></textarea>
  <select id="${s}-select" aria-invalid="true"><option>One</option></select>
  <input type="date" data-exact-field id="${s}-date" aria-invalid="true">
  <input data-native id="${s}-bordered" value="0" aria-invalid="true" style="border-color:rgb(0, 0, 255)">
</div>`;
const page = readFileSync(resolve(WEB, 'index.html'), 'utf8').match(/<style>[\s\S]*?<\/style>/)[0] + `
<div id="exact-root">${scheme('light')}${scheme('dark')}
  <input data-native id="draft" value="25"><input data-native id="kept" value="5">
</div>
<script type="module">
  import { createInputHandlers } from './input-glue.js';
  import { on, onSubmit } from './rt.js';
  const root = document.getElementById('exact-root');
  createInputHandlers({ root, views: new Map(), retiredViews: new Set(), ready: () => true, inertAncestor: el => el.closest('[inert]'), dispatch() {} });
  // The generated module's binding of \`submit=save\` (emit.rs): the action reads the field's text at the submit.
  window.saved = [];
  on(document.getElementById('draft'), 'submit', () => saved.push(document.getElementById('draft').value), onSubmit);
  // A key handler that prevents Enter keeps it from submitting, as HTML's implicit submission is Enter's default.
  const kept = document.getElementById('kept');
  on(kept, 'submit', () => saved.push('kept ' + kept.value), onSubmit);
  kept.addEventListener('keydown', e => { if (e.key === 'Enter') e.preventDefault(); });
  window.ready = true;
</script>`;

// rt.js beside the modules it imports: the real ones it binds with, stand-ins for the rest (as js-link-press.test.mjs).
const REAL = ['rt.js', 'font-family.js', 'grid.js', 'overlay.js', 'roster.js', 'router.js', 'schedule.js', 'budget.js'];
const STUBS = {
  'navigation.js': ['renderMarkup', 'reportPlace', 'onSelection', 'textField', 'editMenu', 'settleRadios', 'installFits', 'contextAt', 'animationClocks', 'launchLocation'],
  'shape.js': ['conforms', 'eq', 'equal', 'failureCode'], 'pointer.js': ['pointer', 'record'], 'commands.js': ['commands'],
  'focus.js': ['autofocus', 'press', 'hold', 'within'], 'paint.js': ['paintList', 'paintFacts', 'paintFlush', 'paintOwn'],
  'document.js': ['Docs', 'Head', 'head', 'markDocument', 'projectRoots'], 'svg-transform.js': ['svgTransform'], 'backdrop.js': ['backdropValue'],
  'dataset.js': ['ds', 'x_querySelectorAll'], 'hatches.js': ['ht'], 'perf.js': ['pf'], 'format.js': ['x_formatTime', 'x_formatDate', 'x_formatNumber', 'x_toFixed', 'x_formatDecimal'],
  'media.js': ['media', 'mediaProp', 'mediaOn', 'mediaPiece', 'requestFullscreen'],
};
const module = name => name === 'input-glue.js' ? readFileSync(resolve(WEB, name))
  : REAL.includes(name) ? readFileSync(resolve(WEB_JS, name))
  : STUBS[name] ? STUBS[name].map(n => `export const ${n} = () => {};`).join('\n') + (name === 'media.js' ? '\nexport const MEDIA_EVENTS = new Set();' : '')
  : null;

/** The page in headless Chrome: `test(evaluate, call)` with its `Runtime.evaluate` and its CDP session. */
async function inChrome(test) {
  const server = createServer((req, res) => {
    if (req.url === '/') { res.writeHead(200, { 'content-type': 'text/html' }); res.end(page); return; }
    const body = module(req.url.slice(1));
    if (body === null) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': 'text/javascript' }); res.end(body);
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const profile = mkdtempSync(resolve(tmpdir(), 'exact-form-chrome-'));
  const child = spawn(chrome, ['--headless=new', '--remote-debugging-pipe', '--window-size=900,1400', `--user-data-dir=${profile}`,
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

const look = (evaluate, id) => evaluate(`(s => ({ background: s.backgroundColor, color: s.color }))(getComputedStyle(document.getElementById('${id}')))`);
const centre = (evaluate, id) => evaluate(`(r => [r.x + r.width / 2, r.y + r.height / 2])(document.getElementById('${id}').getBoundingClientRect())`);
const NEUTRAL = { light: 'rgba(118, 118, 128, 0.12)', dark: 'rgba(118, 118, 128, 0.24)' }; // `bordered`'s fill, Apple's tertiary system fill
const RED = { light: 'rgb(255, 56, 60)', dark: 'rgb(255, 66, 69)' }; // -exact-system-red, as iOS 27 measures it (LLP 1115.000 D3.2)

check('a disabled button looks disabled in every style and scheme, whatever its accent; an authored colour wins', () => inChrome(async (evaluate, call) => {
  for (const s of ['light', 'dark']) {
    const gray = (await look(evaluate, `${s}-gray`)).color;
    for (const style of STYLES) {
      const [on, off] = [await look(evaluate, `${s}-${style}-on`), await look(evaluate, `${s}-${style}-off`)];
      expect([s, style, off.color]).toEqual([s, style, gray]); // the browser's disabled ink, never the accent's
      expect(off.color).not.toBe(on.color);
      // LLP 1116.007 D3.10: the unsaid button's ink is the label's (`ButtonText`, as iOS 27 draws `.gray()` and
      // `.bordered()`), never the accent's: a light system accent was 1.3:1 to 2.5:1 on its grey fill.
      if (style === 'gray' || style === 'bordered') expect([s, style, on.color]).toEqual([s, style, s === 'light' ? 'rgb(0, 0, 0)' : 'rgb(255, 255, 255)']);
      // Where the style fills with the accent, the neutral fill instead; a neutral or glass style keeps its own.
      if (['filled', 'bordered-prominent', 'tinted', 'bordered-tinted'].includes(style)) {
        expect([s, style, off.background]).toEqual([s, style, NEUTRAL[s]]);
        expect(off.background).not.toBe(on.background);
      } else if (style === 'prominent-glass') expect(off.background).toBe((await look(evaluate, `${s}-glass-off`)).background);
      else expect([s, style, off.background]).toEqual([s, style, on.background]);
    }
    // focus-timer's Save: a teal fill under white ink enabled; disabled, neither the accent nor white.
    const save = await look(evaluate, `${s}-bordered-prominent-on`);
    expect(save.background).toBe(s === 'light' ? 'rgb(17, 111, 100)' : 'rgb(98, 213, 189)');
    // The author's own colour and fill on the disabled button still win.
    expect(await look(evaluate, `${s}-authored`)).toEqual({ background: 'rgb(0, 0, 255)', color: 'rgb(255, 0, 0)' });
  }
  // No hover: the pointer over the disabled prominent button leaves its fill; over the enabled one it darkens.
  for (const [id, changes] of [['light-bordered-prominent-off', false], ['light-bordered-prominent-on', true]]) {
    const before = (await look(evaluate, id)).background;
    const [x, y] = await centre(evaluate, id);
    await call('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y });
    expect([id, (await look(evaluate, id)).background !== before]).toEqual([id, changes]);
    await call('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 1, y: 1 });
  }
  // No press: a pointer down on the disabled button marks nothing pressed (input-glue.js dims a pressed face).
  const [x, y] = await centre(evaluate, 'light-bordered-prominent-off');
  await call('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', clickCount: 1 });
  expect(await evaluate(`document.querySelectorAll('[data-pressed]').length`)).toBe(0);
  await call('Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'left', clickCount: 1 });
}), 30000);

// LLP 1116.008 D7: a destructive button passes `check` as the platform draws it (a control's label at 3:1 on its
// own fill, over the card a row of buttons sits on), in every style and both schemes: team-kanban's Delete was
// #ff383c at 2.7:1 on the grey fill over -exact-secondary-background, so its builder removed `destructive=true`.
check('a destructive button reads at 3:1 on its own fill over a card, in every style and scheme', () => inChrome(async (evaluate) => {
  const contrast = (id, card) => evaluate(`(() => {
    const x = document.createElement('canvas').getContext('2d', { willReadFrequently: true }), e = document.getElementById('${id}'), s = getComputedStyle(e);
    const read = () => [...x.getImageData(0, 0, 1, 1).data].slice(0, 3);
    x.fillStyle = '${card}'; x.fillRect(0, 0, 1, 1); x.fillStyle = s.backgroundColor; x.fillRect(0, 0, 1, 1);
    const bg = read(); x.clearRect(0, 0, 1, 1); x.fillStyle = s.color; x.fillRect(0, 0, 1, 1);
    const ink = read(), lum = c => { const [r, g, b] = c.map(v => { v /= 255; return v <= .04045 ? v / 12.92 : ((v + .055) / 1.055) ** 2.4; }); return .2126 * r + .7152 * g + .0722 * b; };
    const [hi, lo] = [lum(ink), lum(bg)].sort((a, b) => b - a); return (hi + .05) / (lo + .05); })()`);
  for (const [s, card] of [['light', '#f2f2f7'], ['dark', '#1c1c1e']]) {
    for (const look of DESTRUCTIVE) expect([s, look, (await contrast(`${s}-destructive-${look}`, card)) >= 3]).toEqual([s, look, true]);
    // The red is still red: not the label's ink on a neutral fill.
    expect((await look(evaluate, `${s}-destructive-gray`)).color).not.toBe((await look(evaluate, `${s}-gray-on`)).color);
  }
}), 30000);

check('a field marked aria-invalid is outlined in the system red, its focus ring red, in both schemes', () => inChrome(async (evaluate) => {
  const field = id => evaluate(`(e => { const s = getComputedStyle(e); return { border: s.borderTopColor, width: s.borderTopWidth, outline: s.outlineColor, style: s.outlineStyle, ring: s.outlineWidth }; })(document.getElementById('${id}'))`);
  for (const s of ['light', 'dark']) {
    const valid = await field(`${s}-valid`);
    for (const id of ['invalid', 'area', 'select', 'date']) expect([s, id, (await field(`${s}-${id}`)).border]).toEqual([s, id, RED[s]]);
    expect((await field(`${s}-invalid`)).width).toBe(valid.width); // the same line, only its colour
    expect((await field(`${s}-false`)).border).toBe(valid.border);
    expect(valid.border).not.toBe(RED[s]);
    expect((await field(`${s}-bordered`)).border).toBe('rgb(0, 0, 255)'); // an authored border wins
    // Focused, the ring is a 2 px red line where the valid field's is the browser's own (`auto`, which Firefox
    // and WebKit draw in their focus colour whatever `outline-color` says).
    await evaluate(`document.getElementById('${s}-valid').focus()`);
    const ring = await field(`${s}-valid`);
    await evaluate(`document.getElementById('${s}-invalid').focus()`);
    const invalid = await field(`${s}-invalid`);
    expect([s, invalid.outline, invalid.style, invalid.ring]).toEqual([s, RED[s], 'solid', '2px']);
    expect([ring.outline, ring.style]).not.toEqual([RED[s], 'solid']);
    // Its error text stays its description.
    expect(await evaluate(`document.getElementById('${s}-invalid').getAttribute('aria-describedby')`)).toBe(`${s}-error`);
  }
}), 30000);

check('Enter in a single-line field with a submit submits once; a key handler that prevents it keeps it', () => inChrome(async (evaluate, call) => {
  const key = async (name, text) => { for (const type of ['keyDown', 'keyUp']) await call('Input.dispatchKeyEvent', { type, key: name, code: name, windowsVirtualKeyCode: 13, ...(type === 'keyDown' && text ? { text } : {}) }); };
  await evaluate(`document.getElementById('draft').focus()`);
  await key('Enter', '\r');
  for (let i = 0; i < 200 && !(await evaluate('saved.length')); i++) await Bun.sleep(5);
  expect(await evaluate('saved')).toEqual(['25']);
  await Bun.sleep(50);
  expect(await evaluate('saved')).toEqual(['25']); // once
  await evaluate(`document.getElementById('kept').focus()`);
  await key('Enter', '\r');
  await Bun.sleep(50);
  expect(await evaluate('saved')).toEqual(['25']);
}), 30000);
