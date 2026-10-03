// A drag timeline's release on the web (LLP 1057.003 D3): a consumer plays a
// copy of its keyframes with the `linear()` easing `timelineEasing` gives
// for the spring's frames. The first tests evaluate that easing against the
// directed progress written out by hand; the browser test holds it, between
// frames too, to Chrome seeking the paused animation to the timeline's
// progress, as the glue does while a drag is held: delays, iterations,
// directions and keyframe easing included.
import { test, expect } from 'bun:test';
import { spawn, spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { Cdp, open } from '../../../scripts/agent.mjs';
import { chromium } from '../../../scripts/agent-launch.mjs';
import { timelineEasing } from '../motion-glue.js';

const ROOT = resolve(new URL('../../..', import.meta.url).pathname);
const WEB = resolve(new URL('..', import.meta.url).pathname);
const WEB_JS = resolve(WEB, '../web-js');
const GLUE = readFileSync(resolve(WEB, 'glue.js'), 'utf8');
const operationSource = GLUE.slice(GLUE.indexOf('function apply(batch)'), GLUE.indexOf('function applyBatch(batch)', GLUE.indexOf('function apply(batch)')));
const applySource = GLUE.slice(GLUE.indexOf('function applyBatch(batch)'), GLUE.indexOf('\nfunction send(', GLUE.indexOf('function applyBatch(batch)')));
// The wasm host's real operation switch and commit tail, with the unrelated
// host pieces inert. Keeping both functions in this one closure matters:
// the `timelines` operation sets the flag that the commit tail consumes.
const hostCommitBody = `
  let timelinesMoved = false;
  const exact = globalThis.exact ??= {}, retiredViews = new WeakSet(), followedScrolls = new Map(), pendingScrolls = new Map();
  const root = document.getElementById('root'), listSelection = null, textflow = null, page = null, collectionOp = null;
  const collections = {commit() {}}, arrange = {commit() {}, destroy() {}, binding() {}, state() {}}, presence = {hold: () => false, live: null};
  const prepareContexts = () => {}, runFocusCommands = () => {}, inertAncestor = () => false, refreshSymbols = () => {};
  const focusAutofocus = () => {}, positionContexts = () => {}, markScrollDocument = () => {}, syncLists = () => {};
  const followScroll = () => {}, settleFollow = () => {}, settleValue = () => {}, letGo = () => {}, flowBatch = () => {};
  const navigation = {project() {}, apply() {}}, log = () => {}, inputReady = false, agentMode = false, frameSampler = null;
  const viewFor = (_, id) => views.get(id), applyProps = () => {}, attach = () => {}, listView = () => {};
  ${operationSource}
  ${applySource}
  return applyBatch;
`;
const { executable: chrome, unavailable } = chromium();
if (unavailable) console.warn(`SKIP: ${unavailable}`);
const check = unavailable ? (name, ...args) => test.skip(`${name} — ${unavailable}`, ...args) : test;

// A `linear()` easing at input x (0–1): the last point at or before x, then
// linearly on to the next; two points at one input are a jump.
function evaluate(easing, x) {
  const points = easing.slice(7, -1).split(', ').map((s) => s.split(' ')).map(([y, at]) => [parseFloat(at) / 100, Number(y)]);
  let i = points.findLastIndex(([at]) => at <= x);
  if (i === points.length - 1) return points[i][1];
  const [x0, y0] = points[i], [x1, y1] = points[i + 1];
  return y0 + (y1 - y0) * (x - x0) / (x1 - x0);
}

// The timeline's progress at x, between the spring's evenly spaced frames.
const between = (p, x) => {
  const at = x * (p.length - 1), j = Math.min(Math.floor(at), p.length - 2);
  return p[j] + (p[j + 1] - p[j]) * (at - j);
};

function worst(timing, p, directed) {
  const easing = timelineEasing(timing, p);
  let e = 0;
  // Off every frame and every crossing: 997 is prime.
  for (let i = 0; i <= 997; i++) e = Math.max(e, Math.abs(evaluate(easing, i / 997) - directed(between(p, i / 997) * timing.endTime)));
  return e;
}

const clamp = (v) => Math.min(1, Math.max(0, v));

test('one iteration, filled both ways: the progress, clamped, with a point at each crossing', () => {
  const timing = { delay: 0, duration: 1000, iterations: 1, direction: 'normal', fill: 'both', endTime: 1000 };
  // A spring back past its start and home: it crosses 0 twice.
  const p = [0.3, 0.1, -0.05, 0.02, -0.005, 0];
  expect(worst(timing, p, (t) => clamp(t / 1000))).toBeLessThan(1e-9);
  // Six frames and three crossings; a kink is one point, a jump two.
  expect(timelineEasing(timing, p).split(', ').length).toBe(9);
});

test('a delay, and iterations that wrap or alternate', () => {
  const delayed = { delay: 500, duration: 500, iterations: 1, direction: 'normal', fill: 'both', endTime: 1000 };
  expect(worst(delayed, [0.9, 0.4, 0.2, 0.6, 1.2], (t) => clamp((t - 500) / 500))).toBeLessThan(1e-9);
  const wraps = { delay: 0, duration: 250, iterations: 4, direction: 'normal', fill: 'both', endTime: 1000 };
  const wrap = (t) => (t >= 1000 ? 1 : t <= 0 ? 0 : (t % 250) / 250);
  expect(worst(wraps, [0, 0.5, 0.26, 0.99, 0.4], wrap)).toBeLessThan(1e-6);
  const alternates = { delay: 0, duration: 500, iterations: 2, direction: 'alternate', fill: 'both', endTime: 1000 };
  const triangle = (t) => (t <= 0 ? 0 : t >= 1000 ? 0 : t < 500 ? t / 500 : 1 - (t - 500) / 500);
  expect(worst(alternates, [0, 0.3, 0.75, 1.1, 0.5], triangle)).toBeLessThan(1e-6);
});

test('an endless animation holds its start; a fill that leaves it out of effect cannot be said', () => {
  expect(timelineEasing({ delay: 0, duration: 1000, iterations: Infinity, fill: 'both', endTime: Infinity }, [0, 1])).toBeUndefined();
  const none = { delay: 0, duration: 1000, iterations: 1, direction: 'normal', fill: 'none', endTime: 1000 };
  expect(timelineEasing(none, [0.5, -0.1, 0])).toBeNull(); // before the start, out of effect
  expect(timelineEasing(none, [0.5, 1, 0.9])).toBeNull(); // at the end, out of effect
  expect(worst(none, [0.5, 0.1, 0.9], (t) => t / 1000)).toBeLessThan(1e-9); // never leaves it
});

check('in Chrome, followers match timeline progress and stop when a consumer resolves elsewhere', async () => {
  const page = `<style>
@keyframes fade { from { opacity: 1 } to { opacity: 0.2 } }
@keyframes pulse { 0% { opacity: 0.1 } 40% { opacity: 0.9; animation-timing-function: ease-in } 100% { opacity: 0.3 } }
</style><div id="root"></div>
<script type="module">
  import { motionController, timelineEasing } from './motion-glue.js';
  import { engine as jsMotionEngine } from './js/motion.js';
  window.compare = (animation, p) => {
    const make = () => { const el = document.createElement('div'); el.style.animation = animation; el.style.animationPlayState = 'paused'; document.getElementById('root').append(el); return el; };
    const reference = make(), follower = make();
    const [a] = reference.getAnimations(), [b] = follower.getAnimations(), timing = b.effect.getComputedTiming();
    const easing = timelineEasing(timing, p);
    if (easing == null) return { easing: easing === null ? 'null' : 'undefined' };
    const f = follower.animate(b.effect.getKeyframes().map(({ computedOffset, ...k }) => k), { duration: 1000, easing, fill: 'both' });
    f.pause();
    let worst = 0;
    for (let i = 0; i <= 997; i++) {
      const x = i / 997, at = x * (p.length - 1), j = Math.min(Math.floor(at), p.length - 2);
      a.currentTime = (p[j] + (p[j + 1] - p[j]) * (at - j)) * timing.endTime;
      f.currentTime = x * 1000;
      worst = Math.max(worst, Math.abs(parseFloat(getComputedStyle(reference).opacity) - parseFloat(getComputedStyle(follower).opacity)));
    }
    reference.remove(); follower.remove();
    return { easing, worst };
  };
  const sourceStyle = '--exact-drag-timeline:--drag x;translate:0 0;width:10px;height:10px';
  const consumerStyle = 'opacity:.55;--exact-animation-timeline:--drag;--exact-animation-range:0 100;animation:fade 1s linear both;animation-play-state:paused';
  const makeController = async target => {
    const root = document.getElementById('root');
    root.innerHTML = '<div id="scope"><div id="old"></div><div id="next"></div><div id="consumer"></div></div>';
    const scope = document.getElementById('scope'), old = document.getElementById('old');
    const next = document.getElementById('next'), consumer = document.getElementById('consumer');
    scope.style.setProperty('--exact-timeline-scope', '--drag, --other');
    old.style.cssText = sourceStyle;
    next.style.cssText = 'translate:80px 0;width:10px;height:10px';
    consumer.style.cssText = consumerStyle;
    const views = new Map([[1, old], [2, next], [3, consumer], [4, scope]]);
    let frames = 0;
    const requestFrame = window.requestAnimationFrame;
    window.requestAnimationFrame = (...args) => { frames++; return requestFrame(...args); };
    let runtime;
    if (target === 'js') {
      const instantiate = WebAssembly.instantiate;
      const memory = new WebAssembly.Memory({initial:1});
      WebAssembly.instantiate = async () => ({instance:{exports:{memory,m_in:()=>0,m_out:()=>0,m_lower:()=>0}}});
      globalThis.__files = () => new ArrayBuffer(0);
      globalThis.exact ??= {}; globalThis.exact.After ??= {};
      try {
        runtime = await jsMotionEngine({clock:{agent:false,now:0},wall:()=>performance.now(),views,viewId:el=>Number(el.dataset.view),
          hooks:{},say:()=>{},inflight:{n:0}});
      } finally { WebAssembly.instantiate = instantiate; delete globalThis.__files; }
    }
    const motion = runtime?.api ?? motionController({ views, now: () => performance.now(), generation: () => 1,
      request: facts => facts.op === 'begin' ? {token:'7',value:[50,0]} : facts.op === 'live' ? {accepted:true} : {},
      applyBatch: () => {}, inert: () => false });
    return {root,scope,old,next,consumer,views,motion,runtime,frames:()=>frames,restore:()=>{motion.reset();window.requestAnimationFrame=requestFrame;}};
  };
  const startFollower = f => {
    f.motion.animate({ id: 1, property: 'translate', values: [[0, 0], [100, 80]], delay: 0, duration: 100000 });
    const source = f.old.getAnimations().find(a => a.animationName === undefined);
    const follower = f.consumer.getAnimations().find(a => a.animationName === undefined);
    source.currentTime = 50000; follower.currentTime = 50000;
  };
  const followerCount = f => f.consumer.getAnimations().filter(a => a.animationName === undefined).length;
  const stylesFor = (f, change) => {
    if (change === 'other-source') return [[1, 'translate:50px 0;width:10px;height:10px'], [2, '--exact-drag-timeline:--drag x;translate:80px 0;width:10px;height:10px']];
    if (change === 'renamed-axis') return [[1, '--exact-drag-timeline:--other y;translate:0 80px;width:10px;height:10px'], [3, consumerStyle.replaceAll('--drag', '--other')]];
    if (change === 'axis') return [[1, '--exact-drag-timeline:--drag y;translate:0 80px;width:10px;height:10px']];
    if (change === 'inactive') return [[1, 'translate:50px 0;width:10px;height:10px']];
    if (change === 'removed') return [[1, 'translate:50px 0;width:10px;height:10px'], [3, 'opacity:.55'], [4, '']];
    if (change === 'range') return [[3, consumerStyle.replace('0 100', '0 200')]];
    return [];
  };
  window.reconcileFollower = async (target, change) => {
    const f = await makeController(target); startFollower(f);
    const before = parseFloat(getComputedStyle(f.consumer).opacity), styles = stylesFor(f, change);
    if (target === 'wasm') {
      const commit = new Function('motion', 'views', ${JSON.stringify(hostCommitBody)})(f.motion, f.views);
      commit({ops:[...styles.map(([id,css])=>({op:'style',id,css})), {op:'timelines'}]});
    } else {
      for (const [id, css] of styles) f.views.get(id).style.cssText = css;
      f.runtime.flush();
    }
    const result = { before, after: parseFloat(getComputedStyle(f.consumer).opacity), followers:followerCount(f), frames:f.frames() };
    f.restore();
    return result;
  };
  window.cancelFollower = async operation => {
    const f = await makeController('wasm'); startFollower(f);
    let threw = null;
    try {
      if (operation === 'catch') f.motion.begin(1, 'translate');
      else if (operation === 'spring') f.motion.animate({id:1,property:'translate',values:[[50,0],[0,0]],delay:0,duration:100000});
      else if (operation === 'retire') f.motion.retire(1, 'translate');
      else if (operation === 'destroy') f.motion.destroy(1);
      else if (operation === 'finish') { f.consumer.getAnimations().find(a=>a.animationName===undefined).finish(); await new Promise(ok=>setTimeout(ok,0)); }
      else f.motion.reset();
    } catch (error) { threw = String(error); }
    const result = {threw,followers:followerCount(f)};
    f.restore(); return result;
  };
  window.ready = true;
</script>`;
  const server = createServer((req, res) => {
    if (req.url === '/') { res.writeHead(200, { 'content-type': 'text/html' }); res.end(page); return; }
    const path = decodeURIComponent(new URL(req.url, 'http://x').pathname);
    const file = path === '/motion-glue.js' || path === '/js/motion-glue.js' ? resolve(WEB, 'motion-glue.js')
      : path.startsWith('/js/') ? resolve(WEB_JS, path.slice(4)) : null;
    if (!file) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': 'text/javascript' }); res.end(readFileSync(file));
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const profile = mkdtempSync(resolve(tmpdir(), 'exact-timeline-'));
  const child = spawn(chrome, ['--headless=new', '--remote-debugging-pipe', `--user-data-dir=${profile}`,
    '--no-sandbox', '--no-first-run', '--disable-background-networking', 'about:blank'], { stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
  try {
    const cdp = new Cdp(child.stdio[3], child.stdio[4]);
    const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
    const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
    const evaluate = async (expression) => {
      const reply = await cdp.send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true }, sessionId);
      if (reply.exceptionDetails) throw new Error(reply.exceptionDetails.exception?.description ?? reply.exceptionDetails.text);
      return reply.result.value;
    };
    await cdp.send('Page.navigate', { url: `http://127.0.0.1:${server.address().port}/` }, sessionId);
    for (let i = 0; !(await evaluate('window.ready === true')); i++) { if (i > 2000) throw new Error('page never ready'); await Bun.sleep(5); }
    const compare = (animation, p) => evaluate(`compare(${JSON.stringify(animation)}, ${JSON.stringify(p)})`);
    for (const [animation, p] of [
      ['fade 1s linear both', [0.3, 0.1, -0.05, 0.02, -0.005, 0]],
      ['fade 1s linear 0.5s both', [0.9, 0.4, 0.2, 0.6, 1.2]],
      ['pulse 0.5s linear 2 alternate both', [0, 0.3, 0.75, 1.1, 0.5]],
      ['pulse 0.25s linear 4 both', [0, 0.5, 0.26, 0.99, 0.4]],
      ['pulse 0.4s linear 0.2s 1.5 reverse forwards', [0.3, 0.9, 1.3, 0.95]],
      ['fade 1s linear', [0.5, 0.1, 0.9]],
    ]) {
      const { easing, worst } = await compare(animation, p);
      expect(easing.startsWith('linear(')).toBe(true);
      // getComputedStyle serializes opacity to about six digits.
      expect(worst, animation).toBeLessThan(1e-4);
    }
    expect((await compare('fade 1s linear', [0.5, -0.1, 0])).easing).toBe('null');
    for (const operation of ['catch', 'spring', 'retire', 'destroy', 'finish', 'reset']) {
      const result = await evaluate(`cancelFollower(${JSON.stringify(operation)})`);
      expect(result.threw, `${operation}: cancelling a live follower does not throw`).toBeNull();
      expect(result.followers, `${operation}: the old follower is gone`).toBe(operation === 'spring' ? 1 : 0);
    }
    for (const target of ['wasm', 'js']) for (const [change, after, followers] of [
      ['other-source', 0.36, 0],
      ['renamed-axis', 0.68, 0],
      ['axis', 0.68, 0],
      ['inactive', 0.55, 0],
      ['removed', 0.55, 0],
      ['range', 0.8, 0],
      ['unchanged', 0.6, 1],
    ]) {
      const result = await evaluate(`reconcileFollower(${JSON.stringify(target)}, ${JSON.stringify(change)})`);
      expect(result.before, `${target} ${change}: the release follower is in flight`).toBeCloseTo(0.6, 3);
      expect(result.after, `${target} ${change}: the current timeline wins immediately`).toBeCloseTo(after, 3);
      expect(result.followers, `${target} ${change}: only a still-bound consumer keeps its follower`).toBe(followers);
      expect(result.frames, `${target} ${change}: only a changed moving resolution enters per-frame seeking`).toBe(['renamed-axis', 'axis', 'range'].includes(change) ? 1 : 0);
    }
  } finally {
    child.kill();
    server.close();
    rmSync(profile, { recursive: true, force: true });
  }
}, 60000);

const FOLLOWER_SOURCE = `keyframes fade
  from opacity=1
  to opacity=0.2

component App
  state mode = 0
  state tick = 0
  state x = 0
  state y = 0
  state otherX = 0
  state otherY = 0
  action released
    x = 260
    y = 180
    otherX = 220
    otherY = 140
  action reset
    mode = 0
    x = 0
    y = 0
    otherX = 0
    otherY = 0
  action axis
    mode = 1
  action range
    mode = 2
  action rebind
    mode = 3
  action remove
    mode = 4
  action direction
    mode = 5
  action inactive
    mode = 6
  action unchanged
    tick = tick + 1
  view
    column width=420 height=700 gap=8 padding=8
      row gap=6 flex-wrap="wrap"
        button testId="axis" press=axis
          text "axis"
        button testId="range" press=range
          text "range"
        button testId="rebind" press=rebind
          text "rebind"
        button testId="remove" press=remove
          text "remove"
        button testId="direction" press=direction
          text "direction"
        button testId="inactive" press=inactive
          text "inactive"
        button testId="unchanged" press=unchanged
          text "unchanged"
        button testId="reset" press=reset
          text "reset"
      box testId="scope" timeline-scope="--drag, --other, --inactive" width=360 height=500 position="relative" overflow="hidden"
        box testId="consumer" position="absolute" left=0 top=0 width=40 height=40 opacity=0.55 background-color="#000000" animation=(mode == 4 ? "none" : (mode == 5 ? "fade 1s linear reverse both" : "fade 1s linear both")) animation-timeline=(mode == 4 ? "auto" : (mode == 3 ? "--other" : (mode == 6 ? "--inactive" : "--drag"))) animation-range=(mode == 2 ? "0px 600px" : "0px 300px")
        box testId="handle" swiperight=released touch-action="pan-y" position="absolute" left=0 top=50 width=300 height=80 transition="translate spring(300, 30, 1)"
        box id="other" testId="other" position="absolute" left=0 top=230 width=300 height=200 translate=\`\${otherX}px \${otherY}px\` transition="translate spring(300, 30, 1)" drag-timeline="--other x"
        box id="source" testId="source" position="absolute" left=0 top=140 width=300 height=80 translate=\`\${x}px \${y}px\` transition="translate spring(300, 30, 1)" drag-timeline=(mode == 1 ? "--drag y" : "--drag x")
`;

check('a real commit rebinds a spring follower on the wasm and JS runtimes', async () => {
  const tmp = mkdtempSync(resolve(tmpdir(), 'exact-follower-runtime-'));
  const plan = resolve(tmp, 'follower.plan'), jsDist = resolve(tmp, 'js'), wasmDist = resolve(tmp, 'wasm');
  const run = (command, args, env = {}) => {
    const result = spawnSync(command, args, { cwd: ROOT, env: { ...process.env, ...env }, encoding: 'utf8', maxBuffer: 64 << 20 });
    expect(result.status, `${command} ${args.join(' ')}\n${result.stderr}`).toBe(0);
  };
  let session;
  try {
    writeFileSync(resolve(tmp, 'follower.contract'), FOLLOWER_SOURCE);
    run('cargo', ['run', '-q', '-p', 'contract', '--', 'build', resolve(tmp, 'follower.contract'), '-o', plan]);
    run(process.execPath, ['host/web/build.mjs', 'interaction-gallery', '--render', 'none'], { EXACT_WEB_DIST: jsDist });
    run(process.execPath, ['host/web/build.mjs', 'interaction-gallery', '--wasm'], { EXACT_WEB_DIST: wasmDist });
    for (const [target, dist] of [['wasm', wasmDist], ['js', jsDist]]) {
      session = await open({ host: 'web', plan, app: 'interaction-gallery', webDist: dist });
      await session.clock('settle');
      const read = () => session.carrier.evaluate(`(() => {
        const vector = id => { const v=getComputedStyle(document.querySelector('[data-testid="'+id+'"]')).translate.trim().split(/\\s+/); return v[0]==='none'?[0,0]:[parseFloat(v[0]),parseFloat(v[1]??'0')]; };
        const consumer=document.querySelector('[data-testid="consumer"]'), animations=consumer.getAnimations();
        return {source:vector('source'),other:vector('other'),opacity:parseFloat(getComputedStyle(consumer).opacity),
          followers:animations.filter(a=>a.animationName===undefined).length,
          consumerAnimations:animations.map(a=>[a.animationName,a.playState,a.currentTime]),
          sourceAnimations:document.querySelector('[data-testid="source"]').getAnimations().map(a=>[a.animationName,a.playState,a.currentTime]),
          syncs:globalThis.__followerSyncs??0,
          sameBasis:globalThis.__followerBasis===undefined||globalThis.__followerBasis===animations.find(a=>a.animationName!==undefined)};
      })()`);
      const expected = (kind, sample) => {
        if (kind === 'remove' || kind === 'inactive') return 0.55;
        const value = kind === 'axis' ? sample.source[1] : kind === 'rebind' ? sample.other[0] : sample.source[0];
        const p = Math.max(0, Math.min(1, value / (kind === 'range' ? 600 : 300)));
        return kind === 'direction' ? 0.2 + 0.8 * p : 1 - 0.8 * p;
      };
      for (const kind of ['axis', 'range', 'rebind', 'remove', 'direction', 'inactive', 'unchanged']) {
        await session.tap('reset'); await session.clock('settle');
        await session.tap('handle', { down: true });
        await session.pointer('move', { dx: 90, dy: 0, ms: 80 });
        await session.pointer('up');
        await session.clock('+120');
        const before = await read();
        expect(before.followers, `${target} ${kind}: release made a compositor follower: ${JSON.stringify(before)}`).toBe(1);
        await session.carrier.evaluate(`globalThis.__followerBasis=document.querySelector('[data-testid="consumer"]').getAnimations().find(a=>a.animationName!==undefined)`);
        if (target === 'js') await session.carrier.evaluate(`globalThis.__followerSyncs=0;if(!globalThis.__followerOriginalSync){globalThis.__followerOriginalSync=exact.synced;exact.synced=()=>{globalThis.__followerSyncs++;return globalThis.__followerOriginalSync()}}`);
        await session.tap(kind);
        const samples = [await read()];
        for (const advance of ['+120', '+240', '+480']) { await session.clock(advance); samples.push(await read()); }
        await session.clock('settle'); samples.push(await read());
        for (const [i, sample] of samples.entries()) expect(sample.opacity, `${target} ${kind} sample ${i}`).toBeCloseTo(expected(kind, sample), 3);
        if (target === 'js') expect(samples[0].syncs, `${target} ${kind}: the agent does not replace post-commit reconciliation`).toBe(0);
        expect(samples[0].followers, `${target} ${kind}: only an unchanged binding retains its follower`).toBe(kind === 'unchanged' ? 1 : 0);
        if (kind === 'direction') expect(samples[0].sameBasis, `${target}: direction changed the CSS animation in place`).toBe(true);
        if (!['remove', 'inactive', 'unchanged'].includes(kind)) {
          const values = samples.map(s => kind === 'axis' ? s.source[1] : kind === 'rebind' ? s.other[0] : s.source[0]);
          expect(Math.max(...values) - Math.min(...values), `${target} ${kind}: the new source keeps moving after the commit`).toBeGreaterThan(1);
        }
      }
      await session.close(); session = null;
    }
  } finally {
    await session?.close();
    rmSync(tmp, { recursive: true, force: true });
  }
}, 600000);
