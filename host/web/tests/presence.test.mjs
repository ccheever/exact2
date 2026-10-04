// The production presence controller under Chrome's layout and animation
// clocks, including the host's resize entry and child reconciliation.
import { beforeAll, afterAll, test, expect } from 'bun:test';
import { spawn } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, existsSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { Cdp } from '../../../scripts/agent.mjs';
import { chromium } from '../../../scripts/agent-launch.mjs';
import { comparePresence } from '../parity.mjs';

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
const { executable: chrome, unavailable } = chromium();
if (unavailable) console.warn(`SKIP: ${unavailable}`);
const check = unavailable ? (name, ...args) => test.skip(`${name} — ${unavailable}`, ...args) : test;
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
  const positionContexts = () => {}, onPreferences = () => {}, onFold = () => {}, foldBits = () => 0;
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

let server, child, profile, evaluate;
beforeAll(async () => {
  if (unavailable) return;
  server = createServer((req, res) => {
    if (req.url === '/favicon.ico') { res.writeHead(204); res.end(); return; }
    res.writeHead(200, { 'content-type': req.url === '/' ? 'text/html' : 'text/javascript' });
    const path = req.url.startsWith('/js/') ? req.url.slice(4) : req.url.slice(1);
    const js = resolve(WEB, '../web-js', path);
    res.end(req.url === '/' ? page : readFileSync(req.url.startsWith('/js/') && existsSync(js) ? js : resolve(WEB, path)));
  });
  await new Promise(ok => server.listen(0, '127.0.0.1', ok));
  profile = mkdtempSync(resolve(tmpdir(), 'exact-presence-'));
  child = spawn(chrome, ['--headless=new', '--remote-debugging-pipe', '--window-size=500,500', `--user-data-dir=${profile}`,
    '--no-sandbox', '--no-first-run', '--disable-background-networking', 'about:blank'], { stdio:['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
  const cdp = new Cdp(child.stdio[3], child.stdio[4]);
  const { targetId } = await cdp.send('Target.createTarget', { url:'about:blank' });
  const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten:true });
  const call = (method, params) => cdp.send(method, params, sessionId);
  evaluate = async expression => {
    const reply = await call('Runtime.evaluate', { expression, returnByValue:true, awaitPromise:true });
    if (reply.exceptionDetails) throw new Error(reply.exceptionDetails.exception?.description ?? reply.exceptionDetails.text);
    return reply.result.value;
  };
  await call('Page.navigate', { url:`http://127.0.0.1:${server.address().port}/` });
  for (let i = 0; !(await evaluate('window.ready === true')); i++) {
    if (i > 2000) throw new Error('presence page never ready');
    await Bun.sleep(5);
  }
});
afterAll(() => {
  child?.kill(); server?.close();
  if (profile) rmSync(profile, { recursive:true, force:true });
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
  const spring = card.replace('1000 0 linear', '0 100 spring(300, 30, 1)');
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
