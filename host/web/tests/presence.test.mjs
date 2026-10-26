// The production presence controller under Chrome's layout and animation
// clocks, including the host's resize entry and child reconciliation.
import { beforeAll, afterAll, test, expect } from 'bun:test';
import { readFileSync, existsSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { resolve, sep } from 'node:path';
import { cdpBrowser, chromium, suiteBrowser } from '../../../scripts/agent-launch.mjs';
import { comparePresence } from '../parity.mjs';
import { need } from '../../../scripts/coverage.mjs';
import { readyPage } from './page.mjs';

test('the timeline comparator rejects missing ghosts, drift, and invalid recordings', () => {
  const node = { x: 10, y: 20, w: 100, h: 40, opacity: .5, exiting: true };
  const reference = { steps: [{ advance: 100 }], samples: [{ name: 'exit-200', clock: 200, nodes: { card: node, sibling: { ...node, exiting: false } } }] };
  const changed = change => { const other = structuredClone(reference); change(other); return comparePresence(reference, other); };
  expect(comparePresence(reference, reference)).toEqual([]);
  expect(changed(r => { r.samples[0].nodes.card.x += .09; r.samples[0].nodes.card.opacity += .004; })).toEqual([]);
  for (const change of [
    r => { r.samples = []; },
    r => { r.steps[0].advance = 101; },
    r => { r.samples[0].clock++; },
    r => { r.samples[0].nodes.card = null; },
    r => { delete r.samples[0].nodes.sibling; },
    r => { r.samples[0].nodes.card.h += 1; },
    r => { r.samples[0].nodes.card.opacity += .02; },
    r => { r.samples[0].nodes.card.x = NaN; },
    r => { r.samples[0].nodes.card.exiting = false; },
  ]) expect(changed(change).length).toBeGreaterThan(0);
  expect(comparePresence({ samples: [] }, { samples: [] }).length).toBeGreaterThan(0);
});

const WEB = resolve(new URL('..', import.meta.url).pathname);
const { unavailable } = chromium();
const check = need('chrome', unavailable);
const glue = readFileSync(resolve(WEB, 'glue.js'), 'utf8');
const children = glue.slice(glue.indexOf('      case "children":'), glue.indexOf('      case "animate":'));
const resize = glue.slice(glue.indexOf('const mediaChanged ='), glue.indexOf('\nvisualViewport?.'));
const page = `<style>
  body { margin:0 } #exact-root { position:relative; padding:20px }
  #card { width:100px; height:40px; background:red; --exact-layout-transition:1000 0 linear }
  #card[data-pressed] { transform:scale(.8) }
  @keyframes pulse { from { opacity:1 } to { opacity:.2 } }
</style><div id="exact-root"></div>
<script type="module">
  import { presenceLoader, animationClock, settleValue } from './navigation.js';
  window.exact = {};
  let incarnation = 1;
  await import('./presence-glue.js');
  const root = document.getElementById('exact-root'), views = new Map();
  const presence = presenceLoader(async () => exact.presence, root, batch => applyBatch(batch), () => {});
  presence.hold({ ops:[{ op:'style', css:'--exact-layout-transition:1000 0 linear' }] });
  await Promise.resolve();
  const clock = animationClock(() => 0, () => 0, () => presence.live.sync?.());
  const viewFor = (_, id) => views.get(id);
  function applyBatch(batch) {
    presence.live.before(batch, views);
    for (const op of batch.ops ?? []) {
      switch (op.op) {
      case 'style': views.get(op.id).style.cssText = op.css; break;
      ${children}
      }
    }
    presence.live.after(batch, views);
  }
  const readOut = x => x, now = () => 0, preferences = () => 0;
  const positionContexts = () => {}, onPreferences = () => {}, foldBits = () => 0, onFold = () => {}; // no fold posture (LLP 1078 D6)
  const wasm = { exact_resize: () => JSON.stringify(window.resizeBatch) };
  ${resize}
  window.fixture = html => {
    for (const a of document.getAnimations()) a.cancel();
    root.innerHTML = html; views.clear();
    for (const el of root.querySelectorAll('[id]')) views.set(el.id, el);
    applyBatch({ ops:[...views].map(([id, el]) => ({ op:'style', id, css:el.style.cssText })) });
  };
  window.batch = applyBatch;
  window.seek = ms => { clock.register(0); clock.seek(ms); };
  window.resizePresence = batch => { window.resizeBatch = batch; mediaChanged(); };
  window.surface = () => root.querySelector('[data-exiting] > div');
  window.readPresence = () => presence.live.observation();
  window.leave = css => presence.live.exit(document.getElementById('card'), css);
  window.ready = true;
</script>`;

let server, suite, evaluate;
beforeAll(async () => {
  if (unavailable) return;
  server = createServer((req, res) => {
    if (req.url === '/favicon.ico') { res.writeHead(204); res.end(); return; }
    res.writeHead(200, { 'content-type': req.url === '/' ? 'text/html' : 'text/javascript' });
    if (req.url === '/js/names.js') { res.end('export const sourceTypes = {};'); return; }
    const path = req.url.startsWith('/js/') ? req.url.slice(4) : req.url.slice(1);
    const js = resolve(WEB, '../web-js', path);
    res.end(req.url === '/' ? page : readFileSync(req.url.startsWith('/js/') && existsSync(js) ? js : resolve(WEB, path)));
  });
  await new Promise(ok => server.listen(0, '127.0.0.1', ok));
  suite = await cdpBrowser({ args: ['--window-size=500,500'] });
  const { cdp } = suite, { sessionId } = await suite.page();
  const call = (method, params) => cdp.send(method, params, sessionId);
  evaluate = async expression => {
    const reply = await call('Runtime.evaluate', { expression, returnByValue:true, awaitPromise:true });
    if (reply.exceptionDetails) throw new Error(reply.exceptionDetails.exception?.description ?? reply.exceptionDetails.text);
    return reply.result.value;
  };
  await readyPage(cdp, sessionId, `http://127.0.0.1:${server.address().port}/`);
}, 60000);
afterAll(async () => {
  await suite?.close(); server?.close();
});

const card = 'width:100px;height:40px;background:red;--exact-layout-transition:1000 0 linear';
const fixture = html => evaluate(`fixture(${JSON.stringify(html)})`);
const style = (css, id = 'card') => evaluate(`batch({ops:[{op:'style',id:${JSON.stringify(id)},css:${JSON.stringify(css)}}]})`);

check('presence inspection reads the moving surface and the leaving ghost', async () => {
  await fixture(`<div id="card" data-view="7" style="${card}"></div>`);
  await style(card.replace('40px', '140px'));
  await evaluate('seek(250)');
  const read = () => evaluate('readPresence()');
  const moving = (await read())[0];
  expect(moving.id).toBe(7);
  expect(moving.h).toBeCloseTo(65, 2);
  expect(moving.opacity).toBe(1);
  await evaluate(`leave('pulse 1000ms linear both'); seek(500)`);
  const leaving = (await read())[0];
  expect(leaving.exiting).toBe(true);
  expect(leaving.opacity).toBeCloseTo(.6, 3);
});

check('resize snaps an active move and takes new boxes without a move', async () => {
  await fixture(`<div id="card" style="${card}"></div>`);
  await style(card.replace('40px', '140px'));
  await evaluate('seek(200)');
  expect(await evaluate('document.getAnimations().length')).toBeGreaterThan(0);
  await evaluate(`resizePresence({ops:[{op:'style',id:'card',css:${JSON.stringify(card.replace('40px', '200px'))}}]})`);
  expect(await evaluate('document.getAnimations().length')).toBe(0);
  expect(await evaluate('surface()')).toBeNull();
  expect(await evaluate('document.getElementById("card").offsetHeight')).toBe(200);
});

check('an interrupted spring uses the seeked time, including its delay and velocity', async () => {
  const spring = card.replace('1000 0 linear', '0 100 spring(300, 30, 1)'); // the private row (css.rs layout_transition_css; LLP 1081 D5)
  await fixture(`<div id="card" style="${spring}"></div>`);
  await style(spring + ';margin-left:100px');
  await evaluate('seek(250)');
  const before = await evaluate('document.getElementById("card").getBoundingClientRect().x');
  await style(spring + ';margin-left:180px');
  await evaluate('seek(0)');
  expect(await evaluate('document.getElementById("card").getBoundingClientRect().x')).toBeCloseTo(before, 1);
  await evaluate('seek(120)');
  const later = await evaluate('document.getElementById("card").getBoundingClientRect().x');
  // The inherited velocity carries the card right as the new spring starts.
  expect(later - before).toBeGreaterThan(2);
});

check('a growing surface follows live paint, opacity and pressed transforms', async () => {
  await fixture(`<div id="card" style="${card}"></div>`);
  await style(card.replace('40px', '140px'));
  await evaluate(`document.getElementById('card').animate([
    {backgroundColor:'red',borderColor:'red',boxShadow:'0px 0px 4px red',opacity:1},
    {backgroundColor:'blue',borderColor:'blue',boxShadow:'0px 0px 4px blue',opacity:.2}
  ], {duration:1000,fill:'both'}); seek(500);
  document.getElementById('card').setAttribute('data-pressed','');
  new Promise(requestAnimationFrame);`);
  expect(await evaluate('getComputedStyle(surface()).backgroundColor')).toBe('rgb(128, 0, 128)');
  expect(await evaluate('getComputedStyle(document.getElementById("card")).backgroundColor')).toBe('rgba(0, 0, 0, 0)');
  expect(await evaluate('getComputedStyle(surface()).opacity')).toBe('0.6');
  expect(await evaluate('getComputedStyle(surface()).boxShadow')).toContain('rgb(128, 0, 128)');
  expect(await evaluate('new DOMMatrix(getComputedStyle(surface()).transform).a')).toBe(0.8);
  await style(card.replace('40px', '140px') + ';border:2px solid green;border-radius:12px');
  await evaluate('seek(500)');
  expect(await evaluate('getComputedStyle(surface()).borderTopWidth')).toBe('2px');
});

check('paint transitions retain their clocks and restore their paint when size motion ends', async () => {
  const css = card + ';border:2px solid red;transition:background-color 1s linear,border-color 1s linear,opacity 1s linear';
  await fixture(`<div id="card" style="${css}"></div>`);
  await style(css.replaceAll('red', 'blue').replace('40px', '140px') + ';opacity:.2');
  await evaluate('seek(500)');
  expect(await evaluate('getComputedStyle(surface()).backgroundColor')).toBe('rgb(128, 0, 128)');
  expect(await evaluate('getComputedStyle(surface()).borderTopColor')).toBe('rgb(128, 0, 128)');
  expect(await evaluate('getComputedStyle(surface()).opacity')).toBe('0.6');
  expect(await evaluate('getComputedStyle(document.getElementById("card")).backgroundColor')).toBe('rgba(0, 0, 0, 0)');
  // A style-only update during the move must start from the paint on screen.
  await style(css.replaceAll('red', 'green').replace('40px', '140px') + ';opacity:.2');
  await evaluate('seek(0)');
  expect(await evaluate('getComputedStyle(surface()).backgroundColor')).toBe('rgb(128, 0, 128)');
  await evaluate('document.getAnimations().forEach(a => a.finish()); Promise.resolve()');
  expect(await evaluate('getComputedStyle(document.getElementById("card")).backgroundColor')).toBe('rgb(0, 128, 0)');
});

check('a shrinking clip reveals children up to the shown box, then restores overflow', async () => {
  await fixture(`<div id="card" style="${card.replace('40px', '140px')};overflow:hidden"><div id="body" style="height:140px;width:100px;background:blue"></div></div>`);
  await style(card + ';overflow:hidden');
  await evaluate('seek(500)');
  expect(await evaluate('document.elementFromPoint(40,90).id')).toBe('body');
  expect(await evaluate('document.elementFromPoint(40,120).id')).not.toBe('body');
  await evaluate('document.getAnimations().forEach(a => a.finish()); Promise.resolve()');
  expect(await evaluate('getComputedStyle(document.getElementById("card")).overflow')).toBe('hidden');
  expect(await evaluate('surface()')).toBeNull();
});

check('a same-name exit is a fresh finite play beside the unchanged infinite animation', async () => {
  for (const paused of [true, false]) {
    await fixture(`<div id="card" style="${card};animation:pulse 1s infinite linear"></div>`);
    await evaluate(`window.playing = document.getElementById('card').getAnimations()[0];
      ${paused ? 'seek(400)' : 'playing.currentTime = 400'};
      batch({ops:[{op:'exit',id:'card',css:'pulse 200ms linear both'}]});`);
    const animations = await evaluate('document.getAnimations().map(a => ({old:a === playing, time:a.currentTime, iterations:a.effect.getTiming().iterations, duration:a.effect.getTiming().duration}))');
    expect(animations.find(a => a.old)).toMatchObject({ old:true, iterations:null, duration:1000 });
    expect(animations.find(a => a.old).time).toBeGreaterThanOrEqual(400);
    expect(animations.find(a => !a.old)?.duration).toBe(200);
    await evaluate('document.getAnimations().filter(a => a !== playing).forEach(a => a.finish()); Promise.resolve()');
    expect(await evaluate('document.querySelector("[data-exiting]")')).toBeNull();
  }
});

check('reordering during a size move keeps the surface at its owner', async () => {
  await fixture(`<div id="row" style="display:flex;position:relative"><div id="card" style="${card}"></div><div id="other" style="width:50px;height:40px"></div></div>`);
  await style(card.replace('40px', '140px'));
  await evaluate('seek(300); batch({ops:[{op:"children",id:"row",ids:["other","card"]}]}); seek(0)');
  const boxes = await evaluate('(() => { const a = document.getElementById("card").getBoundingClientRect(), b = surface().getBoundingClientRect(); return [a.x,a.y,b.x,b.y]; })()');
  expect(boxes[2]).toBeCloseTo(boxes[0], 1);
  expect(boxes[3]).toBeCloseTo(boxes[1], 1);
});

check('reordering equal siblings keeps a surface anchored when its owner has the same box', async () => {
  await fixture(`<div id="row" style="position:relative">
    <div id="a" style="height:40px"></div><div id="card" style="${card}"></div><div id="b" style="height:40px"></div></div>`);
  await style(card.replace('40px', '140px'));
  await evaluate('seek(300)');
  const before = await evaluate('document.getElementById("card").getBoundingClientRect().y');
  await evaluate('batch({ops:[{op:"children",id:"row",ids:["b","card","a"]}]})');
  expect(await evaluate('document.getElementById("card").getBoundingClientRect().y')).toBe(before);
  expect(await evaluate('surface().getBoundingClientRect().y')).toBeCloseTo(before, 1);
});

check('ghosts stay above authored z and isolate their parent until its last exit ends', async () => {
  await fixture(`<div id="holder" style="position:relative">
    <div id="card" style="width:100px;height:40px"></div>
    <div id="second" style="width:100px;height:40px"></div>
    <div id="ranked" style="position:absolute;z-index:2147483645;width:100px;height:40px"></div>
  </div>`);
  await evaluate(`batch({ops:[{op:'exit',id:'card',css:'pulse 1000ms linear both'},
    {op:'exit',id:'second',css:'pulse 2000ms linear both'}]}); seek(0)`);
  expect(await evaluate('document.getElementById("holder").style.isolation')).toBe('isolate');
  expect(await evaluate('[...document.querySelectorAll("[data-exiting]")].map(el => el.style.zIndex)')).toEqual(['2147483646', '2147483646']);
  await evaluate(`document.getAnimations().find(a => a.effect.getTiming().duration === 1000).finish(); Promise.resolve()`);
  expect(await evaluate('document.querySelectorAll("[data-exiting]").length')).toBe(1);
  expect(await evaluate('document.getElementById("holder").style.isolation')).toBe('isolate');
  // Replacing cssText while a ghost lives cannot drop the temporary trap.
  await style('position:relative;color:blue', 'holder');
  expect(await evaluate('document.getElementById("holder").style.isolation')).toBe('isolate');
  await evaluate('document.getAnimations().forEach(a => a.finish()); Promise.resolve()');
  expect(await evaluate('document.getElementById("holder").style.isolation')).toBe('');
});

check('a finished or cancelled ghost restores the parents current isolation row', async () => {
  for (const initial of ['', 'auto', 'isolate']) {
    for (const updated of ['', 'auto', 'isolate']) {
      await fixture(`<div id="holder" style="position:relative;${initial ? `isolation:${initial}` : ''}"><div id="card" style="width:100px;height:40px"></div></div>`);
      await evaluate(`leave('pulse 1000ms linear both'); seek(0)`);
      await style(`position:relative;${updated ? `isolation:${updated}!important` : ''}`, 'holder');
      expect(await evaluate('document.getElementById("holder").style.isolation')).toBe('isolate');
      await evaluate('document.getAnimations().forEach(a => a.cancel()); Promise.resolve()');
      expect(await evaluate('document.getElementById("holder").style.isolation')).toBe(updated);
      expect(await evaluate('document.getElementById("holder").style.getPropertyPriority("isolation")')).toBe(updated ? 'important' : '');
    }
  }
});

check('layout resizing uses its parents structural isolation without animating it', async () => {
  await fixture(`<div id="holder" style="position:relative;isolation:isolate"><div id="card" style="${card};isolation:isolate"></div></div>`);
  await style(card.replace('40px', '140px') + ';isolation:isolate');
  await evaluate('seek(500)');
  expect(await evaluate('surface().getBoundingClientRect().height')).toBeCloseTo(90, 2);
  expect(await evaluate('document.getElementById("holder").getAnimations().length')).toBe(0);
  expect(await evaluate('document.getElementById("holder").style.isolation')).toBe('isolate');
});

check('JS commits keep ghost containment and restore the latest paint decision after the last exit', async () => {
  await fixture('');
  await evaluate(`(async () => {
    const { h, when, sig, W, act, pr, mount, pieces, usePaint } = await import('/js/rt.js');
    const { paintUse } = await import('/js/paint.js');
    usePaint(paintUse());
    const shown = sig(true), trapped = sig(false);
    window.jsShown = act(v => W(shown, v));
    window.jsIsolation = act(v => W(trapped, v));
    mount(root => {
      const app = h(root, 'div', 0, {'data-exact-f':'133'}, 0);
      window.jsHolder = h(app, 'div', 0, {'data-exact-f':'0'}, 0);
      when(jsHolder, shown, p => {
        for (const duration of [1000, 2000]) {
          const c = h(p, 'div', 0, {'data-exact-f':'4'}, 0);
          c.style.cssText = 'width:100px;height:40px;--exact-exit-animation:pulse ' + duration + 'ms linear both';
          pr(c);
        }
      }, () => {});
      // A live z descendant requires isolation independently of the ghosts.
      when(jsHolder, trapped, p => {
        const c = h(p, 'div', 0, {'data-exact-f':'513','data-exact-zi':'2'}, 0);
        c.style.cssText = 'position:absolute;z-index:2;width:10px;height:10px';
      }, () => {});
      const sibling = h(app, 'div', 0, {'data-exact-f':'513','data-exact-zi':'1'}, 0);
      sibling.style.cssText = 'position:absolute;z-index:1;width:100px;height:40px';
    });
    await pieces();
    if (!jsHolder.$paint?.own || jsHolder.$paint.own.outside) throw Error('the production paint pass must classify the holder');
  })()`);
  expect(await evaluate('jsHolder.style.isolation')).toBe('');
  for (const required of [true, false]) {
    await evaluate('jsShown(false); seek(0)');
    expect(await evaluate('document.querySelectorAll("[data-exiting]").length')).toBe(2);
    expect(await evaluate('jsHolder.style.isolation')).toBe('isolate');
    // These actions must commit the production pass themselves: no manual flush.
    for (const trapped of [true, false, true]) {
      await evaluate(`jsIsolation(${trapped})`);
      expect(await evaluate('jsHolder.$paint.isolated')).toBe(trapped);
      expect(await evaluate('jsHolder.$ghostIsolation.value')).toBe(trapped ? 'isolate' : '');
      expect(await evaluate('jsHolder.style.isolation')).toBe('isolate');
    }
    await evaluate('document.getAnimations().find(a => a.effect.getTiming().duration === 1000).finish(); Promise.resolve()');
    expect(await evaluate('document.querySelectorAll("[data-exiting]").length')).toBe(1);
    expect(await evaluate('jsHolder.style.isolation')).toBe('isolate');
    await evaluate(`jsIsolation(${required})`);
    expect(await evaluate('jsHolder.$ghostIsolation.value')).toBe(required ? 'isolate' : '');
    await evaluate(`document.getAnimations().forEach(a => a.${required ? 'finish' : 'cancel'}()); Promise.resolve()`);
    expect(await evaluate('document.querySelectorAll("[data-exiting]").length')).toBe(0);
    expect(await evaluate('!!jsHolder.$ghostIsolation')).toBe(false);
    expect(await evaluate('jsHolder.style.isolation')).toBe(required ? 'isolate' : '');
    await evaluate('jsIsolation(false); jsShown(true)');
  }
});

// This is the normal compiler, TypeScript data module, emitter, bundler and
// browser path. The literal control takes that same path first, so a build or
// data-fixture refusal cannot be mistaken for the bound-row refusal of #809.
const motionSource = row => [
  'shape Preferences',
  '  reducedMotion: bool',
  '  reads: number',
  '  writes: number',
  'component App',
  '  state curve = "none"',
  '  state offset = 0',
  '  state tall = false',
  '  state shown = true',
  '  resource prefs = preference() as shape Preferences else empty()',
  '  mutation prefsChanged as shape Preferences refreshes prefs',
  '  action enable',
  '    curve = "1000ms linear"',
  '    offset = 100',
  '  action grow',
  '    tall = true',
  '  action shift',
  '    offset = 200',
  '  action disable',
  '    curve = "none"',
  '    tall = false',
  '    offset = 300',
  '  action clear',
  '    curve = "unset"',
  '    offset = 450',
  '  action reenable',
  '    curve = "1000ms linear"',
  '    offset = 400',
  '  action reduce',
  '    offset = 500',
  '    send prefsChanged = setPreference(true)',
  '  action restoreMotion',
  '    offset = 600',
  '    send prefsChanged = setPreference(false)',
  '  action retire',
  '    shown = false',
  '    curve = "1000ms linear"',
  '    offset = 800',
  '  action restoreChild',
  '    shown = true',
  '    offset = 700',
  '  view',
  '    column position="relative" width=700 height=900',
  '      column position="absolute" left=200 top=0',
  ...['enable', 'grow', 'shift', 'disable', 'clear', 'reenable', 'reduce', 'restoreMotion', 'retire', 'restoreChild']
    .map(action => `        button "${action}" testId="${action}" press=${action}`),
  '        text `${prefs.reads}/${prefs.writes}` testId="counts"',
  '      if shown',
  `        column id="moving-card" testId="moving-card" position="absolute" left=20 top=(prefs.reducedMotion ? offset + 50 : offset) width=100 height=(tall ? 140 : 40) background-color="red" overflow="hidden" -exact-layout-transition=${row}`,
  '          text "Content" font-size=14',
  '',
].join('\n');
const motionData = `import type { Answer, Sources } from './app.contract.d.ts';
export const appId = 'test.dynamic-layout', grants = '';
let reducedMotion = false, reads = 0, writes = 0;
const sources: Sources = {
  preference: () => ({ reducedMotion, reads: ++reads, writes }),
  setPreference: async ([value]) => { reducedMotion = value; writes++; return { reducedMotion, reads, writes }; },
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args as never, store, storage, native) as never;
`;

async function buildMotionApp(dir, row) {
  writeFileSync(resolve(dir, 'app.contract'), motionSource(row));
  writeFileSync(resolve(dir, 'app.ts'), motionData);
  writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Dynamic layout', app: { id: 'test.dynamic-layout', name: 'Dynamic layout' }, host: { web: {} } }));
  const dist = resolve(dir, 'dist');
  const built = await new Promise((done, fail) => {
    const child = spawn(process.execPath, ['host/web-js/build.mjs', 'dynamic-layout', '--out', dist, '--render', 'none'],
      { cwd: resolve(WEB, '../..'), env: { ...process.env, EXACT_APP_DIR: dir }, stdio: ['ignore', 'pipe', 'pipe'] });
    // Inherit the test supervisor's group, including any compiler descendants:
    // its outer deadline can clean the whole group rather than only this Bun.
    console.info(JSON.stringify({ phase: 'presence-build-start', pid: child.pid, row }));
    let log = '', timedOut = false, kill;
    const signal = name => child.kill(name);
    const timeout = setTimeout(() => { timedOut = true; signal('SIGTERM'); kill = setTimeout(() => signal('SIGKILL'), 5000); }, 30000);
    child.stdout.on('data', chunk => { log += chunk; });
    child.stderr.on('data', chunk => { log += chunk; });
    child.on('error', error => { clearTimeout(timeout); clearTimeout(kill); fail(error); });
    child.on('close', (code, signal) => {
      clearTimeout(timeout); clearTimeout(kill);
      console.info(JSON.stringify({ phase: 'presence-build-end', pid: child.pid, code, signal, timedOut, log }));
      done({ code, signal, timedOut, pid: child.pid, log });
    });
  });
  expect({ timedOut: built.timedOut, signal: built.signal }, `infrastructure: builder completion ${JSON.stringify(built)}`)
    .toEqual({ timedOut: false, signal: null });
  if (built.code === 0) {
    const scan = new Bun.Transpiler({ loader: 'js' }), modules = {};
    for (const file of readdirSync(dist).filter(name => name.endsWith('.js'))) {
      const bytes = readFileSync(resolve(dist, file));
      modules[file] = { bytes: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex'), imports: scan.scanImports(bytes.toString()) };
    }
    const statics = new Set(), late = new Set();
    const visit = file => {
      if (statics.has(file)) return;
      statics.add(file);
      for (const item of modules[file]?.imports ?? []) {
        const target = item.path.replace(/^\.\//, '');
        if (item.kind === 'import-statement' && modules[target]) visit(target);
        if (item.kind === 'dynamic-import') late.add(target);
      }
    };
    visit('app.js');
    console.info(JSON.stringify({ phase: 'presence-emitted-graph', row, source: createHash('sha256').update(motionSource(row) + motionData).digest('hex'), modules, staticClosure: [...statics], late: [...late] }));
    for (const piece of ['shared', 'presence-glue']) {
      expect([...late].some(file => file.startsWith(piece + '-') || file === piece + '.js'), `the emitted ${piece} piece stays a dynamic import`).toBe(true);
      expect([...statics].some(file => file.startsWith(piece + '-') || file === piece + '.js'), `the emitted ${piece} piece is outside the static entry closure`).toBe(false);
    }
  }
  return { ...built, dist };
}

async function builtMotionPage(browser, dist) {
  const served = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch(request) {
    const path = new URL(request.url).pathname, file = resolve(dist, path === '/' ? 'index.html' : decodeURIComponent(path.slice(1)));
    return file.startsWith(dist + sep) && existsSync(file) ? new Response(Bun.file(file)) : new Response('not found', { status: 404 });
  } });
  const context = await browser.newContext(), p = await context.newPage(), errors = [];
  p.on('pageerror', error => errors.push(String(error)));
  try {
    await p.goto(`${served.url.href}?agent`);
    await p.waitForFunction(() => globalThis.exact?.ready && document.querySelector('[data-testid="counts"]'));
    const ready = await p.evaluate(async () => { await exact.ready; return exact.agentSettled({ op: 'clock', data: true }); });
    expect(ready.settled, `ordinary data readiness: ${JSON.stringify(ready)}`).toBe(true);
    const read = () => p.evaluate(() => {
      const card = document.getElementById('moving-card'), root = document.getElementById('exact-root').getBoundingClientRect();
      const stand = card?.previousElementSibling, surface = stand?.hasAttribute('data-exiting') ? stand.firstElementChild : card;
      return { y: card ? card.getBoundingClientRect().y - root.y : null, height: surface?.getBoundingClientRect().height ?? null,
        css: card?.style.getPropertyValue('--exact-layout-transition') ?? null, animations: document.getAnimations().filter(a => a.playState !== 'finished' && a.playState !== 'idle').length,
        stands: document.querySelectorAll('[data-exiting]').length, counts: document.querySelector('[data-testid="counts"]').textContent };
    });
    const press = async name => { await p.getByTestId(name).evaluate(el => el.click()); await p.evaluate(() => exact.agentSettled({ op: 'state' })); return read(); };
    const advance = async ms => { await p.evaluate(async ms => { exact.advance(exact.clock.now + ms, false); await exact.agentSettled({ op: 'state' }); }, ms); return read(); };
    return { p, read, press, advance, errors, close: async () => { await context.close(); served.stop(true); } };
  } catch (error) { await context.close(); served.stop(true); throw error; }
}

check('normal emitted layout rows gain, clear and re-enable through real controls and resource replies', async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-dynamic-layout-')), browser = await suiteBrowser();
  let live;
  try {
    const literal = await buildMotionApp(dir, '"1000ms linear"');
    expect(literal.code, `infrastructure: literal normal build ${JSON.stringify(literal)}`).toBe(0);
    live = await builtMotionPage(browser, literal.dist);
    const literalStart = await live.press('enable'), literalMiddle = await live.advance(500), literalEnd = await live.advance(500);
    const controlFailures = live.errors.map(error => `page error: ${error}`);
    for (const [sample, wanted] of [[literalStart, 0], [literalMiddle, 50], [literalEnd, 100]]) {
      if (typeof sample.y !== 'number' || !Number.isFinite(sample.y) || Math.abs(sample.y - wanted) > .05 || sample.counts !== '1/0') controlFailures.push(JSON.stringify({ sample, wanted }));
    }
    expect(controlFailures, 'infrastructure: literal motion/data control').toEqual([]);
    await live.close(); live = null;

    const bound = await buildMotionApp(dir, '(prefs.reducedMotion ? "none" : curve)');
    const knownRefusal = /a dynamic `layout_transition` \(Transitions\) is not one declaration; not in the JS target/.test(bound.log);
    expect(bound.code, `${knownRefusal && !bound.timedOut ? 'issue809 feature' : 'infrastructure: unexpected dynamic build outcome'}: ${JSON.stringify(bound)}`).toBe(0);
    live = await builtMotionPage(browser, bound.dist);
    const samples = {}, failures = [];
    const take = (name, sample, values) => {
      samples[name] = sample;
      for (const [key, wanted] of Object.entries(values)) {
        const got = sample[key];
        if (typeof wanted === 'number' ? typeof got !== 'number' || !Number.isFinite(got) || Math.abs(got - wanted) > .05 : got !== wanted) failures.push(`${name}.${key}: ${JSON.stringify(got)} != ${JSON.stringify(wanted)}`);
      }
    };
    take('initial', await live.read(), { y: 0, counts: '1/0', css: '', animations: 0 });
    take('first gain', await live.press('enable'), { y: 0, counts: '1/0' });
    take('first midpoint', await live.advance(500), { y: 50 });
    take('first end', await live.advance(500), { y: 100 });
    take('position start', await live.press('shift'), { y: 100 });
    take('position quarter', await live.advance(250), { y: 125 });
    take('disable interrupted position', await live.press('disable'), { y: 300, css: '', animations: 0, stands: 0 });
    take('gain before size', await live.press('reenable'), { y: 300 });
    await live.advance(1000);
    take('grow start', await live.press('grow'), { height: 40 });
    take('grow midpoint', await live.advance(500), { height: 90 });
    take('disable interrupted size', await live.press('disable'), { y: 300, height: 40, css: '', animations: 0, stands: 0 });
    take('re-enable after none', await live.press('reenable'), { y: 300 });
    take('re-enable midpoint', await live.advance(500), { y: 350 });
    take('re-enable end', await live.advance(500), { y: 400 });
    take('unset removal', await live.press('clear'), { y: 450, css: '', animations: 0, stands: 0 });
    take('re-enable after removal', await live.press('reenable'), { y: 450 });
    take('second midpoint', await live.advance(500), { y: 425 });
    await live.advance(500);
    await live.press('reduce');
    const reduced = await live.p.evaluate(() => exact.agentSettled({ op: 'clock', data: true }));
    if (!reduced.settled) failures.push(`reduced preference did not finish: ${JSON.stringify(reduced)}`);
    take('preference disabled', await live.read(), { y: 550, css: '', counts: '2/1', animations: 0, stands: 0 });
    await live.press('restoreMotion');
    const restored = await live.p.evaluate(() => exact.agentSettled({ op: 'clock', data: true }));
    if (!restored.settled) failures.push(`restored preference did not finish: ${JSON.stringify(restored)}`);
    take('preference gains with move', await live.read(), { y: 650, counts: '3/2' });
    take('preference midpoint', await live.advance(500), { y: 625 });
    take('preference end', await live.advance(500), { y: 600 });
    await live.p.evaluate(() => { globalThis.retiredCard = document.getElementById('moving-card'); });
    take('retired child', await live.press('retire'), { y: null, animations: 0, stands: 0, counts: '3/2' });
    failures.push(...await live.p.evaluate(() => retiredCard.isConnected || retiredCard.getAnimations().length ? ['retired child retained a live view or animation'] : []));
    take('new child', await live.press('restoreChild'), { y: 700, animations: 0, stands: 0, counts: '3/2' });
    failures.push(...live.errors.map(error => `page error: ${error}`));
    expect(failures, `issue809 lifecycle: ${JSON.stringify(samples)}`).toEqual([]);
  } finally { await live?.close(); await browser.close(); rmSync(dir, { recursive: true, force: true }); }
}, 120000);

// A direct-runtime supplement can count a style getter and derive, which an
// ordinary Contract intentionally cannot mutate. It never replaces the normal
// emitted program above and uses no prospective-style API or future export.
check('gaining presence never previews a source or a retired child effect, and ordinary styles keep their IDs', async () => {
  const browser = await suiteBrowser(), context = await browser.newContext(), p = await context.newPage();
  try {
    await p.goto(`http://127.0.0.1:${server.address().port}/`);
    await p.waitForFunction(() => globalThis.ready);
    const result = await p.evaluate(async () => {
      const { h, S, when, sig, memo, res, W, act, pr, mount, pieces, data, Views } = await import('/js/rt.js');
      const counts = { derives: 0, styles: 0, asks: 0 }, samples = {};
      const mode = sig(false), offset = sig(0), shown = sig(true), tint = sig(false);
      data.answer = (_, args) => { counts.asks++; return { v: args[0] }; };
      let card, original;
      mount(root => {
        const app = h(root, 'div', 0, {}, 0);
        app.style.cssText = 'position:relative;width:700px;height:900px';
        const seed = h(app, 'div', 0, {}, 0);
        seed.style.cssText = 'position:absolute;left:300px;width:10px;height:10px;--exact-layout-transition:1000 0 linear'; pr(seed);
        for (let n = 0; n < 40; n++) {
          const ordinary = h(app, 'div', 0, {}, 0);
          S(ordinary, 'background-color', '', () => tint() ? 'red' : 'blue');
        }
        when(app, shown, parent => {
          const resource = res('ownedProof', 'ownedProof', () => [1], undefined, undefined, 'n', 0);
          const curve = memo(() => { counts.derives++; resource(); return mode() ? '1000 0 linear' : null; });
          card = h(parent, 'div', 0, {}, 0); original ??= card;
          card.style.cssText = 'position:absolute;width:100px;height:40px;background:red';
          S(card, '--exact-layout-transition', '', () => { counts.styles++; return curve(); });
          S(card, 'top', 'px', offset); pr(card);
        }, () => {});
      });
      await pieces();
      const capture = name => { samples[name] = { ...counts, y: card.getBoundingClientRect().y - card.parentElement.getBoundingClientRect().y, connected: card.isConnected }; };
      capture('initial');
      act(() => { W(mode, true); W(offset, 100); })(); capture('gain');
      act(() => { W(shown, false); W(mode, false); })();
      samples.retired = { ...counts, connected: original.isConnected, animations: original.getAnimations().length };
      act(() => { W(shown, true); W(mode, true); W(offset, 400); })(); capture('newChild');
      const ids = Views.size;
      for (let n = 0; n < 10; n++) act(() => W(tint, !tint()))();
      return { samples, idsBefore: ids, idsAfter: Views.size };
    });
    const expected = {
      initial: { derives: 1, styles: 1, asks: 1, y: 0, connected: true },
      gain: { derives: 2, styles: 2, asks: 1, y: 0, connected: true },
      retired: { derives: 3, styles: 2, asks: 1, connected: false, animations: 0 },
      newChild: { derives: 4, styles: 3, asks: 2, y: 400, connected: true },
    };
    expect({ samples: result.samples, idGrowth: result.idsAfter - result.idsBefore },
      'issue809 ownership: normal settle/flush counts, first-gain box and ordinary style IDs').toEqual({ samples: expected, idGrowth: 0 });
  } finally { await context.close(); await browser.close(); }
});

check('ordinary dynamic styles neither load presence nor register view IDs', async () => {
  const browser = await suiteBrowser(), context = await browser.newContext(), p = await context.newPage();
  try {
    await p.goto(`http://127.0.0.1:${server.address().port}/`);
    await p.waitForFunction(() => globalThis.ready);
    const result = await p.evaluate(async () => {
      const { h, S, sig, W, act, mount, pieces, Views } = await import('/js/rt.js');
      const tint = sig(false), boxes = [];
      mount(root => {
        for (let n = 0; n < 40; n++) {
          const box = h(root, 'div', 0, {}, 0); boxes.push(box);
          S(box, 'background-color', '', () => tint() ? 'red' : 'blue');
        }
      });
      for (let n = 0; n < 10; n++) act(() => W(tint, !tint()))();
      await pieces();
      return { presence: !!globalThis.exact.presenceLive, ids: Views.size, boxes: boxes.length,
        colors: [...new Set(boxes.map(box => box.style.backgroundColor))] };
    });
    expect(result).toEqual({ presence: false, ids: 0, boxes: 40, colors: ['blue'] });
  } finally { await context.close(); await browser.close(); }
});

check('a real shared commit measures current queued gains only when its deferred tail runs', async () => {
  const browser = await suiteBrowser(), context = await browser.newContext({ reducedMotion: 'no-preference' }), p = await context.newPage();
  try {
    await p.goto(`http://127.0.0.1:${server.address().port}/`);
    await p.waitForFunction(() => globalThis.ready);
    const observed = await p.evaluate(async () => {
      const { h, S, when, sig, W, act, pr, mount, pieces } = await import('/js/rt.js');
      const branch = sig(true), enabled = sig(false), offset = sig(0);
      let moving, reads = 0;
      mount(root => {
        const app = h(root, 'div', 0, {}, 0); app.style.cssText = 'position:relative;width:500px;height:600px';
        const named = parent => {
          const e = h(parent, 'div', 0, { 'data-shared-element': 'handoff' }, 0);
          e.style.cssText = 'position:absolute;left:300px;width:10px;height:10px;--exact-layout-transition:1000 0 linear'; pr(e);
        };
        when(app, branch, named, named);
        moving = h(app, 'div', 0, {}, 0); moving.style.cssText = 'position:absolute;width:100px;height:40px';
        S(moving, '--exact-layout-transition', '', () => { reads++; return enabled() ? '1000 0 linear' : null; });
        S(moving, 'top', 'px', offset); pr(moving);
      });
      await pieces();
      const original = document.startViewTransition, updates = [];
      // Hold the browser's public callback. The shared module and both actual
      // runtime tails remain production code; no private flush hook is called.
      document.startViewTransition = update => {
        let finish;
        const done = new Promise(resolve => { finish = resolve; });
        updates.push(async () => { try { update(); } finally { finish(); } await done; });
        return { ready: done, updateCallbackDone: done, finished: done, skipTransition() {} };
      };
      const y = () => moving.getBoundingClientRect().y - moving.parentElement.getBoundingClientRect().y;
      try {
        act(() => { W(branch, false); W(enabled, true); W(offset, 100); })();
        act(() => W(offset, 200))(); // this tail waits behind the same capture
        const held = { updates: updates.length, reads, y: y(), css: moving.style.getPropertyValue('--exact-layout-transition') };
        await updates[0](); await exact.viewTransition(); seek(0);
        const start = { reads, y: y() };
        seek(500); const middle = y(); seek(1000); const end = y();
        return { held, start, middle, end };
      } finally { document.startViewTransition = original; }
    });
    expect(observed).toEqual({ held: { updates: 1, reads: 1, y: 0, css: '' }, start: { reads: 2, y: 0 }, middle: 100, end: 200 });
  } finally { await context.close(); await browser.close(); }
});

check('rebound list items, indices and match values gain presence before their first move', async () => {
  const browser = await suiteBrowser(), context = await browser.newContext(), p = await context.newPage();
  try {
    await p.goto(`http://127.0.0.1:${server.address().port}/`);
    await p.waitForFunction(() => globalThis.ready);
    const result = await p.evaluate(async () => {
      const { h, S, each, match, sig, W, act, pr, mount, pieces } = await import('/js/rt.js');
      const items = sig([[1, false, 0]]), indices = sig([1, 2]), value = sig([false, 0]);
      const cards = [], reads = [0, 0, 0];
      const row = (parent, slot, enabled, offset) => {
        const card = h(parent, 'div', 0, {}, 0);
        card.style.cssText = 'position:absolute;width:100px;height:40px;background:red';
        S(card, '--exact-layout-transition', '', () => { if (slot !== null) reads[slot]++; return enabled() ? '1000 0 linear' : null; });
        S(card, 'top', 'px', offset); pr(card);
        if (slot !== null) cards[slot] = card;
      };
      mount(root => {
        const group = () => {
          const e = h(root, 'div', 0, {}, 0);
          e.style.cssText = 'position:relative;width:400px;height:200px';
          return e;
        };
        each(group(), items, item => item()[0], (parent, item) => row(parent, 0, () => item()[1], () => item()[2]));
        each(group(), indices, item => item(), (parent, item, index) => row(parent, item() === 1 ? 1 : null, () => index() > 0, () => index() * 100));
        match(group(), value, (parent, bound) => row(parent, 2, () => bound()[0], () => bound()[1]));
      });
      await pieces();
      const boxes = () => cards.map(card => card.getBoundingClientRect().y - card.parentElement.getBoundingClientRect().y);
      const before = boxes(), originals = [...cards];
      act(() => { W(items, [[1, true, 100]]); W(indices, [2, 1]); W(value, [true, 100]); })();
      const start = boxes(); seek(500); const middle = boxes(); seek(1000); const end = boxes();
      return { before, start, middle, end, reads, kept: cards.every((card, i) => card === originals[i]) };
    });
    expect(result).toEqual({ before: [0, 0, 0], start: [0, 0, 0], middle: [50, 50, 50], end: [100, 100, 100], reads: [2, 2, 2], kept: true });
  } finally { await context.close(); await browser.close(); }
});
