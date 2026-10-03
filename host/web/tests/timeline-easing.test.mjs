// A drag timeline's release on the web (LLP 1057.003 D3): a consumer plays a
// copy of its keyframes with the `linear()` easing `timelineEasing` gives
// for the spring's frames. The first tests evaluate that easing against the
// directed progress written out by hand; the browser test holds it, between
// frames too, to Chrome seeking the paused animation to the timeline's
// progress, as the glue does while a drag is held: delays, iterations,
// directions and keyframe easing included.
import { test, expect } from 'bun:test';
import { spawn } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { Cdp } from '../../../scripts/agent.mjs';
import { chromium } from '../../../scripts/agent-launch.mjs';
import { timelineEasing } from '../motion-glue.js';

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
      expect(result.frames, `${target} ${change}: a spring release schedules no per-frame callbacks`).toBe(0);
    }
  } finally {
    child.kill();
    server.close();
    rmSync(profile, { recursive: true, force: true });
  }
}, 60000);
