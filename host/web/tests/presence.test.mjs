// The production presence controller under Chrome's layout and animation
// clocks, including the host's resize entry and child reconciliation.
import { beforeAll, afterAll, test, expect } from 'bun:test';
import { readFileSync, existsSync } from 'node:fs';
import { createServer } from 'node:http';
import { resolve } from 'node:path';
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
    // `/js/` is the JS target's directory, beside this one; its modules import ours as `../web/`.
    const path = req.url.startsWith('/js/') ? req.url.slice(4) : req.url.replace(/^\/(web\/)?/, '');
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
// What paints on top at a point of the holder, the ghosts made hit-testable for the read
const topmost = (x, y) => evaluate(`(() => {
  const ghosts = [...document.querySelectorAll('[data-exiting]')];
  for (const g of ghosts) { g.inert = false; g.style.pointerEvents = 'auto'; }
  const r = document.getElementById('holder').getBoundingClientRect(), el = document.elementFromPoint(r.left + ${x}, r.top + ${y});
  for (const g of ghosts) { g.inert = true; g.style.pointerEvents = 'none'; }
  return el.id || el.exactExitId;
})()`);
// A motion controller from motion-glue.js that knows only the element `id`, as `window.motion`, over an engine that
// accepts every hold
const motionFor = id => evaluate(`(async () => {
  const { motionController } = await import('./motion-glue.js');
  window.motion = motionController({ views: new Map([['${id}', document.getElementById('${id}')]]), now: () => 0, generation: () => 0,
    request: facts => facts.op === 'begin' ? { token: 1, value: [0, 0] } : { accepted: true }, applyBatch: () => {}, inert: () => false });
})()`);
// Lifts the element `id` as an Arrange drag does, and reads its `z-index`
const lift = async id => {
  await motionFor(id);
  return evaluate(`motion.raiseReorder('${id}', true); getComputedStyle(document.getElementById('${id}')).zIndex`);
};

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

check('ghosts keep their paint order among their siblings and isolate their parent until its last exit ends', async () => {
  // After the exits: `plain` and `later` (moved up over the card's box) at 0-40, the ghosts at 0-40 and 80-120.
  await fixture(`<div id="holder" style="position:relative">
    <div id="card" style="width:200px;height:40px;z-index:5"></div>
    <div id="plain" style="width:200px;height:40px"></div>
    <div id="second" style="position:relative;z-index:2;width:200px;height:40px"></div>
    <div id="later" style="position:relative;top:-40px;width:100px;height:40px"></div>
  </div>`);
  await evaluate(`batch({ops:[{op:'exit',id:'card',css:'pulse 1000ms linear both'},
    {op:'exit',id:'second',css:'pulse 2000ms linear both'}]}); seek(0)`);
  expect(await evaluate('document.getElementById("holder").style.isolation')).toBe('isolate');
  expect(await topmost(150, 20)).toBe('card'); // a positioned box over an in-flow one
  expect(await topmost(50, 20)).toBe('later'); // level 0 in tree order: the z-index it had no use for stays unused
  expect(await evaluate('[...document.querySelectorAll("[data-exiting]")].map(el => getComputedStyle(el).zIndex)')).toEqual(['auto', '2']);
  // Each ghost starts at its own last box, though the first one's exit took it out of flow before the second's
  expect(await evaluate('[...document.querySelectorAll("[data-exiting]")].map(el => el.offsetTop)')).toEqual([0, 80]);
  await evaluate(`document.getAnimations().find(a => a.effect.getTiming().duration === 1000).finish(); Promise.resolve()`);
  expect(await evaluate('document.querySelectorAll("[data-exiting]").length')).toBe(1);
  expect(await evaluate('document.getElementById("holder").style.isolation')).toBe('isolate');
  // Replacing cssText while a ghost lives cannot drop the temporary trap.
  await style('position:relative;color:blue', 'holder');
  expect(await evaluate('document.getElementById("holder").style.isolation')).toBe('isolate');
  await evaluate('document.getAnimations().forEach(a => a.finish()); Promise.resolve()');
  expect(await evaluate('document.getElementById("holder").style.isolation')).toBe('');
});

check('a ghost paints its subtree in its turn, though only its position made it a stacking context', async () => {
  // After the exit, `later` (z-index 1) lies over the card's box, where the card's child has z-index 10. The exit
  // animates a colour, which makes no stacking context of its own, as an opacity animation would.
  await fixture(`<style>@keyframes tint { to { background-color: blue } }</style><div id="holder" style="position:relative">
    <div id="card" style="position:sticky;top:0;width:200px;height:40px"><div id="inner" style="position:relative;z-index:10;height:40px"></div></div>
    <div id="plain" style="width:200px;height:40px"></div>
    <div id="later" style="position:relative;z-index:1;top:-40px;width:100px;height:40px"></div>
  </div>`);
  await evaluate(`leave('tint 1000ms linear both'); seek(0)`);
  expect(await topmost(50, 20)).toBe('later');
});

check('a view that leaves while lifted stops being lifted, as Apple lets go of it', async () => {
  // After the exit, `later` lies over the card's box; the lift had raised the card over everything
  await fixture(`<div id="holder" style="position:relative">
    <div id="card" style="width:200px;height:40px"></div>
    <div id="plain" style="width:200px;height:40px"></div>
    <div id="later" style="position:relative;top:-40px;width:100px;height:40px"></div>
  </div>`);
  expect(await lift('card')).toBe('2147483647');
  await evaluate(`leave('pulse 1000ms linear both'); seek(0)`);
  expect(await topmost(50, 20)).toBe('later');
  expect(await evaluate('getComputedStyle(document.querySelector("[data-exiting]")).zIndex')).toBe('auto');
  // The drag ends after its view left, which motion let go of: the ghost stays out of flow
  await evaluate(`motion.raiseReorder('card', false)`);
  expect(await evaluate('document.querySelector("[data-exiting]").style.position')).toBe('absolute');
  // And a lifted row inside a view that leaves
  await fixture(`<div id="holder" style="position:relative"><div id="card" style="width:200px;height:80px">
    <div id="row" style="width:200px;height:40px"></div></div></div>`);
  expect(await lift('row')).toBe('2147483647');
  await evaluate(`leave('pulse 1000ms linear both'); seek(0)`);
  expect(await evaluate('(() => { const cs = getComputedStyle(document.querySelector("[data-exiting] > div")); return [cs.zIndex, cs.position]; })()')).toEqual(['auto', 'static']);
});

check('a hold that ends after its view left leaves the ghost as it was shown', async () => {
  await fixture(`<div id="holder" style="position:relative">
    <div id="card" style="width:200px;height:40px"></div>
    <div id="plain" style="width:200px;height:40px"></div>
  </div>`);
  await motionFor('card');
  const shown = () => evaluate('(() => { const el = document.getElementById("card") ?? document.querySelector("[data-exiting]"); return [el.style.position, getComputedStyle(el).translate]; })()');
  await evaluate(`window.hold = motion.begin('card', 'translate'); motion.move(hold, [30, 0])`);
  const [, translate] = await shown();
  expect(translate).not.toBe('none');
  await evaluate(`leave('pulse 1000ms linear both'); seek(0)`);
  await evaluate('motion.end(hold)');
  expect(await shown()).toEqual(['absolute', translate]);
});

check('a lift that ends on a ghosts parent keeps the parent isolated', async () => {
  // `over` (z-index 1) lies over the card, whose z-index 5 stays inside the row while the row is isolated
  await fixture(`<div id="holder" style="position:relative">
    <div id="row" style="display:flex;flex-direction:column;width:200px">
      <div id="card" style="position:relative;z-index:5;width:200px;height:40px"></div>
      <div id="other" style="width:200px;height:40px"></div>
    </div>
    <div id="over" style="position:absolute;z-index:1;top:0;left:0;width:200px;height:40px"></div>
  </div>`);
  expect(await lift('row')).toBe('2147483647');
  await evaluate(`leave('pulse 1000ms linear both'); seek(0)`);
  await evaluate(`motion.raiseReorder('row', false)`);
  expect(await evaluate('document.getElementById("row").style.isolation')).toBe('isolate');
  expect(await topmost(50, 20)).toBe('over');
});

check('a flex or grid items ghost paints at the z-index that ranked it', async () => {
  for (const display of ['flex', 'grid;grid-auto-flow:column;grid-template-columns:50px 50px 50px']) {
    // After the exit, `low` and `high` take the card's place, one half each
    await fixture(`<div id="holder" style="position:relative;display:${display}">
      <div id="card" style="flex:none;width:100px;height:40px;z-index:5"></div>
      <div id="low" style="flex:none;position:relative;z-index:3;width:50px;height:40px"></div>
      <div id="high" style="flex:none;position:relative;z-index:7;width:50px;height:40px"></div>
    </div>`);
    await evaluate(`leave('pulse 1000ms linear both'); seek(0)`);
    expect(await topmost(25, 20)).toBe('card');
    expect(await topmost(75, 20)).toBe('high');
  }
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

// A bound row as the JS target emits it (rows.rs): `S` writes what the row's
// map gives (style.rs `LAYOUT_TRANSITION_MAP`, which the Rust tests hold to the
// kernel's CSS for computed values), here that value itself, and `pr` loads presence.
check('a bound layout transition gains, stops and starts again on actions and on a reply', async () => {
  const browser = await suiteBrowser(), context = await browser.newContext(), p = await context.newPage();
  try {
    await p.goto(`http://127.0.0.1:${server.address().port}/`);
    await p.waitForFunction(() => globalThis.ready);
    const samples = await p.evaluate(async () => {
      const { h, S, sig, res, W, act, pr, mount, pieces, data } = await import('/js/rt.js');
      const curve = sig('1000 0 linear'), offset = sig(0), tall = sig(false);
      let card, asked, answer;
      // The preference's first answer is a reply in a commit of its own: until then the placeholder reduces motion.
      data.answer = () => asked = new Promise(ok => { answer = ok; });
      mount(root => {
        const app = h(root, 'div', 0, {}, 0);
        app.style.cssText = 'position:relative;width:700px;height:900px';
        const prefs = res('prefs', 'prefs', () => [], undefined, undefined, undefined, { reduced: true });
        card = h(app, 'div', 0, {}, 0);
        card.style.cssText = 'position:absolute;left:20px;width:100px;background:red;overflow:hidden';
        S(card, '--exact-layout-transition', '', () => prefs().reduced ? null : curve());
        S(card, 'top', 'px', () => prefs().reduced ? offset() + 50 : offset());
        S(card, 'height', 'px', () => tall() ? 140 : 40); pr(card);
      });
      await pieces();
      const read = () => {
        const stand = card.previousElementSibling, surface = stand?.hasAttribute('data-exiting') ? stand.firstElementChild : card;
        return { y: card.getBoundingClientRect().y - card.parentElement.getBoundingClientRect().y, height: surface.getBoundingClientRect().height,
          css: card.style.getPropertyValue('--exact-layout-transition'), animations: document.getAnimations().filter(a => a.playState !== 'finished' && a.playState !== 'idle').length,
          stands: document.querySelectorAll('[data-exiting]').length };
      };
      const out = {}, take = (name, f) => { f?.(); out[name] = read(); };
      take('reduced');
      const landed = new Promise(ok => { const o = new MutationObserver(() => { o.disconnect(); ok(); }); o.observe(card, { attributeFilter: ['style'] }); });
      answer({ reduced: false }); await asked; await landed;
      take('reply gains with move'); take('reply midpoint', () => seek(500)); take('reply end', () => seek(1000));
      take('shift', act(() => W(offset, 100))); take('shift quarter', () => seek(250));
      take('none stops the move', act(() => { W(curve, null); W(offset, 300); }));
      take('gain before move', act(() => { W(curve, '1000 0 linear'); W(offset, 400); })); take('gain midpoint', () => seek(500)); take('gain end', () => seek(1000));
      take('grow', act(() => W(tall, true))); take('grow midpoint', () => seek(500));
      take('none stops the size', act(() => { W(curve, null); W(tall, false); }));
      take('again after none', act(() => { W(curve, '1000 0 linear'); W(offset, 200); })); take('again midpoint', () => seek(500)); take('again end', () => seek(1000));
      return out;
    });
    const idle = { css: '', animations: 0, stands: 0 }, on = { css: '1000 0 linear' };
    const want = {
      'reduced': { y: 50, height: 40, ...idle },
      'reply gains with move': { y: 50, ...on }, 'reply midpoint': { y: 25 }, 'reply end': { y: 0 },
      'shift': { y: 0 }, 'shift quarter': { y: 25 },
      'none stops the move': { y: 300, ...idle },
      'gain before move': { y: 300, ...on }, 'gain midpoint': { y: 350 }, 'gain end': { y: 400 },
      'grow': { height: 40 }, 'grow midpoint': { height: 90 },
      'none stops the size': { y: 400, height: 40, ...idle },
      'again after none': { y: 400, ...on }, 'again midpoint': { y: 300 }, 'again end': { y: 200 },
    };
    const near = (got, wanted) => typeof wanted === 'number' ? Math.abs(got - wanted) <= .05 : got === wanted;
    const misses = Object.entries(want).flatMap(([name, values]) => Object.entries(values)
      .filter(([key, wanted]) => !near(samples[name][key], wanted)).map(([key, wanted]) => `${name}.${key}: ${JSON.stringify(samples[name][key])}, not ${JSON.stringify(wanted)}`));
    expect(misses).toEqual([]);
  } finally { await context.close(); await browser.close(); }
});

// The runtime driven directly can count a style getter's and a derive's runs,
// which a Contract cannot.
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
      "settle and flush counts, the first gain's box and ordinary styles' IDs").toEqual({ samples: expected, idGrowth: 0 });
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
