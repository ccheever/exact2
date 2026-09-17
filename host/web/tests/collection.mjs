// Opt-in real wasm + DOM integration, invoked by the Rust collection test.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { cpSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { Cdp } from '../../../scripts/agent.mjs';
import { serveStatic } from '../serve.mjs';
const dir = process.env.EXACT_COLLECTION_TEST, dist = resolve(dir, 'dist');
assert(process.env.EXACT_COLLECTION_DIST, 'set EXACT_COLLECTION_DIST to a built pure Rust web dist');
cpSync(process.env.EXACT_COLLECTION_DIST, dist, { recursive: true });
for (const name of ['glue.js', 'navigation.js']) cpSync('host/web/' + name, dist + '/' + name);
function fixture(plan) {
  const instantiate = WebAssembly.instantiateStreaming;
  globalThis.collectionSmoke = { calls: 0, snapshots: [], maxRows: 0 };
  WebAssembly.instantiateStreaming = async (...args) => {
    const result = await instantiate(...args), w = result.instance.exports;
    let input;
    const capture = len => {
      const batch = JSON.parse(new TextDecoder().decode(new Uint8Array(w.memory.buffer, w.exact_out(), len)));
      for (const op of batch.ops ?? []) if (op.op === 'collections') {
        collectionSmoke.snapshots = op.items;
        for (const s of op.items) collectionSmoke.maxRows = Math.max(collectionSmoke.maxRows, s.rows.length);
      }
      return len;
    };
    return { ...result, instance: { exports: { ...w,
      exact_in(n) { input = w.exact_in(n); return input; },
      exact_boot(width, height, n) {
        const launch = new Uint8Array(w.memory.buffer, input, n).slice();
        const ptr = w.exact_in(plan.length + n), bytes = new Uint8Array(w.memory.buffer, ptr, plan.length + n);
        bytes.set(plan); bytes.set(launch, plan.length);
        return capture(w.exact_boot_plan(plan.length, width, height, n));
      },
      exact_collection_feedback(n) {
        const d = new DataView(w.memory.buffer, input, n);
        collectionSmoke.focus = d.getUint32(56, true); collectionSmoke.interaction = d.getUint32(60, true);
        collectionSmoke.calls++; return capture(w.exact_collection_feedback(n));
      },
      exact_dispatch(...args) { return capture(w.exact_dispatch(...args)); },
    } } };
  };
}
const html = readFileSync(dist + '/index.html', 'utf8').replace('<script type="module" src="./glue.js"></script>',
  `<script>(${fixture})(${JSON.stringify([...readFileSync(dir + '/app.plan')])})</script><script type="module" src="./glue.js"></script>`);
writeFileSync(dist + '/index.html', html);
const server = createServer((req, res) => serveStatic(dist, req, res));
await new Promise(r => server.listen(0, '127.0.0.1', r));
const child = spawn(process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  ['--headless=new', '--no-sandbox', '--remote-debugging-pipe', '--disable-background-networking', `--user-data-dir=${dir}/chrome`, 'about:blank'],
  { stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
const cdp = new Cdp(child.stdio[3], child.stdio[4]), errors = [];
const exited = new Promise(r => child.on('exit', () => { cdp.fail('Chrome closed'); r(); }));
try {
  const { targetInfos } = await cdp.send('Target.getTargets');
  const { sessionId } = await cdp.send('Target.attachToTarget', { targetId: targetInfos.find(t => t.type === 'page').targetId, flatten: true });
  const call = (method, params) => cdp.send(method, params, sessionId);
  cdp.listeners.push(msg => {
    if (msg.sessionId !== sessionId) return;
    if (msg.method === 'Runtime.exceptionThrown') errors.push(msg.params.exceptionDetails.exception?.description ?? msg.params.exceptionDetails.text);
    if (msg.method === 'Runtime.consoleAPICalled' && msg.params.type === 'error') errors.push(msg.params.args.map(a => a.value ?? a.description).join(' '));
  });
  await call('Runtime.enable');
  const evaluate = async expression => {
    const r = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
    if (r.exceptionDetails) throw Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text);
    return r.result.value;
  };
  const until = async expression => {
    for (let n = 0; n < 200; n++) {
      if (await evaluate(expression).catch(() => false)) return;
      await new Promise(r => setTimeout(r, 10));
    }
    throw Error('timed out: ' + expression + '\n' + errors.join('\n') + '\n' + JSON.stringify(await evaluate('({state:collectionSmoke,active:document.activeElement.dataset.testid})')));
  };
  await call('Page.navigate', { url: `http://127.0.0.1:${server.address().port}/?agent=1` });
  await until('globalThis.exact?.root?.dataset.bootMs && collectionSmoke.calls >= 1');
  await until('globalThis.exact.root.dataset.moduleReady === "true"');
  const initial = await evaluate('collectionSmoke');
  assert.equal(initial.snapshots[0].count, 1000);
  assert(initial.maxRows <= 24, JSON.stringify(initial));
  await evaluate(`document.querySelector('[data-testid="row-0"]').focus({preventScroll:true}); document.querySelector('[data-testid="row-1"]').dispatchEvent(new PointerEvent('pointerdown',{bubbles:true,pointerId:8}));`);
  await until('collectionSmoke.focus > 0 && collectionSmoke.interaction > 0');
  await evaluate(`document.querySelector('[data-testid="list"]').scrollTop=16000`);
  await until('collectionSmoke.snapshots[0]?.rows.some(r => r.index > 490)');
  const middle = await evaluate(`({state:collectionSmoke,top:document.querySelector('[data-testid="list"]').scrollTop,anchor:getComputedStyle(document.querySelector('[data-testid="list"]')).overflowAnchor})`);
  assert.equal(middle.top, 16000);
  assert.equal(middle.anchor, 'none');
  assert.equal(await evaluate('document.activeElement.dataset.testid'), 'row-0', 'focused pinned row must keep DOM focus');
  assert.equal(await evaluate(`document.querySelector('[data-testid="row-1"]') !== null`), true, 'one distant interaction pin');
  await call('Input.insertText', { text: 'typing across a virtual window' });
  await until(`document.querySelector('[data-testid="echo"]').textContent === 'typing across a virtual window'`);
  // Inserting into an offscreen focused editor intentionally reveals its caret.
  // Scroll away again before checking that releasing pins retires distant rows.
  await evaluate(`document.querySelector('[data-testid="list"]').scrollTop=16000`);
  await until('collectionSmoke.snapshots[0]?.rows.some(r => r.index > 490)');
  await evaluate(`document.activeElement.blur(); document.dispatchEvent(new PointerEvent('pointerup',{bubbles:true,pointerId:8}));`);
  await until('collectionSmoke.focus === 0 && collectionSmoke.interaction === 0');
  await until(`!document.querySelector('[data-testid="row-0"]') && !document.querySelector('[data-testid="row-1"]')`);
  const anchored = await evaluate(`(() => { const port=document.querySelector('[data-testid="list"]'), top=port.getBoundingClientRect().top; const row=collectionSmoke.snapshots[0].rows.find(r=>{const b=document.querySelector('[data-view="'+r.view+'"]').getBoundingClientRect();return b.bottom>top;}); return {index:row.index,offset:document.querySelector('[data-view="'+row.view+'"]').getBoundingClientRect().top-top}; })()`);
  await evaluate(`document.querySelector('[data-testid="grow"]').click()`);
  await until('collectionSmoke.snapshots[0]?.rows.every(r => r.measured && r.height === 64)');
  const afterGrow = await evaluate(`(() => {const row=collectionSmoke.snapshots[0].rows.find(r=>r.index===${anchored.index}); return document.querySelector('[data-view="'+row.view+'"]').getBoundingClientRect().top-document.querySelector('[data-testid="list"]').getBoundingClientRect().top;})()`);
  assert(Math.abs(afterGrow - anchored.offset) < 1, `anchor moved: ${anchored.offset} -> ${afterGrow}`);
  assert(middle.state.maxRows <= 26, JSON.stringify(middle));
  await evaluate(`const list=document.querySelector('[data-testid="list"]'); list.scrollTop=list.scrollHeight`);
  await until('collectionSmoke.snapshots[0]?.rows.some(r => r.index === 999)');
  await until('collectionSmoke.snapshots[0]?.rows.every(r => r.measured)');
  const end = await evaluate('collectionSmoke.calls');
  await new Promise(r => setTimeout(r, 150));
  assert.equal(await evaluate('collectionSmoke.calls'), end, 'settled geometry must become idle');
  await evaluate(`document.querySelector('[data-testid="hide"]').click()`);
  await until('collectionSmoke.snapshots.length === 0');
  assert.equal(await evaluate(`document.querySelector('[data-testid="list"]') === null`), true);
  await evaluate(`document.querySelector('[data-testid="hide"]').click()`);
  await until('collectionSmoke.snapshots.length === 1 && collectionSmoke.snapshots[0].rows.some(r=>r.index===0)');
  assert.deepEqual(errors, []);
  console.log(JSON.stringify({ passed: true, initialRows: initial.snapshots[0].rows.length, maxRows: middle.state.maxRows, logicalRows: 1000, feedbackCalls: end, traversed: ['top', 'middle', 'end'], unmount: true, remount: true, pins: 2, typing: true, resizeAnchor: true }));
} finally {
  if (child.exitCode === null) { child.kill(); await exited; }
  await new Promise(r => server.close(r));
}
