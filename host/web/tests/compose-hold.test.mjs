// LLP 1115.000 §5, ruling 6, on the web, in a real browser with a real composition (CDP's
// `Input.imeSetComposition`, which fires `compositionstart`, `compositionupdate` and `input` as an input method does;
// `Input.insertText` commits it, an empty composition cancels it): the app hears `input` at every change, the marked
// text included, and a value it writes meanwhile never ends the composition. It is held, the last one, and written
// once the composition has ended, committed or cancelled, after the app heard the text.
//
// The JS target's runtime (host/web-js/rt.js `writeValue`) runs here beside stand-ins for the modules it imports, as
// form-chrome.test.mjs runs it, so the glue step holds the rule. With a wasm dist and a JS dist of Caltrain
// (EXACT_WEB_DIST, EXACT_WEB_JS_DIST: what accessibility-tree.test.mjs reads; the wasm one linked whole,
// `EXACT_WEB_LINK=all`, as conformance builds it for its synthetic plans) the same drive runs a compiled plan on both
// targets, whose glue (host/web/glue.js `writeValue`) and runtime must then agree.
import { test, expect } from 'bun:test';
import { spawn, spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { Cdp, open } from '../../../scripts/agent.mjs';
import { chromium } from '../../../scripts/agent-launch.mjs';

const ROOT = resolve(new URL('../../..', import.meta.url).pathname);
const WEB_JS = resolve(ROOT, 'host/web-js');
const { executable: chrome, unavailable } = chromium();
if (unavailable) console.warn(`SKIP: ${unavailable}`);
const check = unavailable ? (name, ...args) => test.skip(`${name} — ${unavailable}`, ...args) : test;

// A field the app rewrites in lower case as it hears it, and what it heard: the app `hosted_tests.rs` gives Android.
const CONTRACT = `component Hold
  state low = ""
  state heard = ""
  action hush(value: string)
    low = toLowerCase(value)
    heard = \`\${heard}[\${value}]\`
  view
    column width=320 align-items="flex-start"
      input testId="low" value=low input=hush
      text heard testId="log"
`;
// The same app as the generated module binds it (emit.rs), and a field whose value only the app writes.
const page = `<!doctype html><meta charset="utf-8"><div id="exact-root">
  <input data-testid="low"><span data-testid="log"></span><input data-testid="told"></div>
<script type="module">
  import { P, on, onValue, sig, W, commit } from './rt.js';
  const low = sig(''), heard = sig(''), told = sig('');
  const [field, log, other] = ['low', 'log', 'told'].map(t => document.querySelector('[data-testid="' + t + '"]'));
  P(field, 'value', () => low()); P(log, 'text', () => heard()); P(other, 'value', () => told());
  on(field, 'input', v => commit(() => { W(low, v.toLowerCase()); W(heard, heard() + '[' + v + ']'); }), onValue);
  window.tell = v => commit(() => W(told, v));
  window.ready = true;
</script>`;
const REAL = ['rt.js', 'font-family.js', 'grid.js', 'overlay.js', 'roster.js', 'router.js', 'schedule.js', 'budget.js', 'shape.js'];
const STUBS = {
  'navigation.js': ['renderMarkup', 'reportPlace', 'onSelection', 'textField', 'editMenu', 'settleRadios', 'installFits', 'contextAt', 'animationClocks', 'launchLocation'],
  'pointer.js': ['pointer', 'record'], 'commands.js': ['commands'], 'focus.js': ['autofocus', 'press', 'hold', 'within'],
  'paint.js': ['paintList', 'paintFacts', 'paintFlush', 'paintOwn'], 'document.js': ['Docs', 'Head', 'head', 'markDocument', 'projectRoots'],
  'svg-transform.js': ['svgTransform'], 'backdrop.js': ['backdropValue'], 'dataset.js': ['ds', 'x_querySelectorAll'], 'hatches.js': ['ht'], 'perf.js': ['pf'],
  'format.js': ['x_formatTime', 'x_formatDate', 'x_formatNumber', 'x_toFixed', 'x_formatDecimal'], 'media.js': ['media', 'mediaProp', 'mediaOn', 'mediaPiece', 'requestFullscreen'],
};
// A module the runtime came to import after this list was written (observers.js did, with `intersect=`) is stood in
// for by the names the real modules take from it, so a new import does not leave the page unable to load.
const taken = name => [...new Set(REAL.flatMap(real => [...readFileSync(resolve(WEB_JS, real), 'utf8').matchAll(/(?:import|export)\s*\{([^}]*)\}\s*from\s*['"]\.\/([\w-]+\.js)['"]/g)]
  .filter(m => m[2] === name).flatMap(m => m[1].split(',').map(n => n.trim().split(/\s+as\s+/)[0]).filter(Boolean))))];
const stand = names => names.map(n => `export const ${n} = () => {};`).join('\n');
const module = name => REAL.includes(name) ? readFileSync(resolve(WEB_JS, name))
  : STUBS[name] ? stand(STUBS[name]) + (name === 'media.js' ? '\nexport const MEDIA_EVENTS = new Set();' : '')
  : taken(name).length ? stand(taken(name))
  : null;

/** The stand-in page in headless Chrome: `drive(evaluate, call)` with its `Runtime.evaluate` and its CDP session. */
async function inChrome(drive) {
  const server = createServer((req, res) => {
    if (req.url === '/') { res.writeHead(200, { 'content-type': 'text/html' }); res.end(page); return; }
    const body = module(req.url.slice(1));
    if (body === null) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': 'text/javascript' }); res.end(body);
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const profile = mkdtempSync(resolve(tmpdir(), 'exact-compose-hold-'));
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
    await drive(evaluate, call);
  } finally {
    child.kill();
    server.close();
    rmSync(profile, { recursive: true, force: true });
  }
}

const at = t => `document.querySelector('[data-testid="${t}"]')`;
/** The field's text, what the app heard, and how many compositions began and ended in it. */
const shown = (evaluate, t = 'low') => evaluate(`new Promise(ok => setTimeout(() => ok([${at(t)}.value, ${at('log')}.textContent, window.began, window.ended]), 30))`);
const mark = (call, text) => call('Input.imeSetComposition', { text, selectionStart: text.length, selectionEnd: text.length });
async function focus(evaluate, call, t) {
  await call('Emulation.setFocusEmulationEnabled', { enabled: true });
  await evaluate(`(() => { window.began = window.ended = 0; const e = ${at(t)}; e.addEventListener('compositionstart', () => window.began++); e.addEventListener('compositionend', () => window.ended++); e.focus(); })()`);
}

/** A field the app lower-cases, composed in: on every web target the same. */
async function lowered(evaluate, call) {
  await focus(evaluate, call, 'low');
  // The input method marks "K": the app hears it and writes "k", which is held. The marked text stands.
  await mark(call, 'K');
  expect(await shown(evaluate)).toEqual(['K', '[K]', 1, 0]);
  // The marked text grows in the same composition (a value written into the field would have ended it, and "KY" then
  // began another beside what was left).
  await mark(call, 'KY');
  expect(await shown(evaluate)).toEqual(['KY', '[K][KY]', 1, 0]);
  // It is committed: the app hears the text, and its value, the last it wrote, is the field's.
  await call('Input.insertText', { text: 'KYO' });
  expect(await shown(evaluate)).toEqual(['kyo', '[K][KY][KYO]', 1, 1]);
  // A second composition, cancelled: the marked text goes, the app hears what is left, and that is what stands (the
  // value held meanwhile, "kyox", was not the last).
  await mark(call, 'X');
  expect(await shown(evaluate)).toEqual(['kyoX', '[K][KY][KYO][kyoX]', 2, 1]);
  await mark(call, '');
  expect(await shown(evaluate)).toEqual(['kyo', '[K][KY][KYO][kyoX][kyo]', 2, 2]);
}

check('the JS target holds a value the app writes while a field composes, and writes it when the composition ends', () => inChrome(async (evaluate, call) => {
  await lowered(evaluate, call);
  // A value written from elsewhere (an answer, a timer) while a field composes: held through a commit and a cancel.
  await focus(evaluate, call, 'told');
  await mark(call, 'A');
  await evaluate(`tell('first')`); await evaluate(`tell('second')`);
  expect((await shown(evaluate, 'told')).filter((_, i) => i !== 1)).toEqual(['A', 1, 0]);
  await mark(call, '');
  expect((await shown(evaluate, 'told')).filter((_, i) => i !== 1)).toEqual(['second', 1, 1]);
  await mark(call, 'B');
  await evaluate(`tell('third')`);
  expect((await shown(evaluate, 'told'))[0]).toBe('secondB');
  await call('Input.insertText', { text: 'B' });
  expect((await shown(evaluate, 'told')).filter((_, i) => i !== 1)).toEqual(['third', 2, 2]);
  // With no composition a value is written at once, as before.
  await evaluate(`tell('fourth')`);
  expect((await shown(evaluate, 'told'))[0]).toBe('fourth');
}), 60000);

// A compiled plan on each target, when its dist is named.
const dists = { wasm: process.env.EXACT_WEB_DIST, JS: process.env.EXACT_WEB_JS_DIST };
for (const [target, dist] of Object.entries(dists)) {
  (dist && !unavailable ? test : test.skip)(`the ${target} target's compiled plan holds the app's value through a composition${dist ? '' : ' (no dist named)'}`, async () => {
    const tmp = mkdtempSync(resolve(tmpdir(), 'exact-compose-plan-'));
    try {
      const source = resolve(tmp, 'hold.contract'), plan = resolve(tmp, 'hold.plan');
      writeFileSync(source, CONTRACT);
      const c = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'build', source, '-o', plan], { cwd: ROOT, encoding: 'utf8' });
      if (c.status !== 0) throw new Error('the fixture does not compile: ' + c.stderr);
      const s = await open({ host: 'web', plan, webDist: resolve(dist) });
      try { await lowered(s.carrier.evaluate, s.carrier.call); } finally { await s.close(); }
    } finally { rmSync(tmp, { recursive: true, force: true }); }
  }, 240000);
}
