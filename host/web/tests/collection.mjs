// Opt-in real wasm + DOM integration, invoked by the Rust collection test.
// EXACT_COLLECTION_GALLERY=1 uses an existing gallery plan/data without Cargo.
// EXACT_COLLECTION_JS optionally selects a frozen JS baseline for RED replay.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { cpSync, readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { inflateSync } from 'node:zlib';
import { resolve } from 'node:path';
import { Cdp } from '../../../scripts/agent.mjs';
import { webContentType } from '../serve.mjs';
const dir = process.env.EXACT_COLLECTION_TEST, dist = resolve(dir, 'dist');
const gallery = process.env.EXACT_COLLECTION_GALLERY === '1';
const javascript = resolve(process.env.EXACT_COLLECTION_JS ?? 'host/web');
assert(process.env.EXACT_COLLECTION_DIST, 'set EXACT_COLLECTION_DIST to a built pure Rust web dist');
cpSync(process.env.EXACT_COLLECTION_DIST, dist, { recursive: true });
for (const name of ['glue.js', 'navigation.js']) cpSync(resolve(javascript,name), dist + '/' + name);
const hash = path => createHash('sha256').update(readFileSync(path)).digest('hex');
writeFileSync(dir + '/identity.json', JSON.stringify({carrier: resolve(process.env.EXACT_COLLECTION_DIST), javascript,
  files: Object.fromEntries(['app.wasm','app.plan','glue.js','navigation.js'].map(name => [name, {carrier: hash(resolve(process.env.EXACT_COLLECTION_DIST,name)), tested: hash(dist+'/'+name)}]))}, null, 2));
function fixture(plan) {
  const instantiate = WebAssembly.instantiateStreaming, RO = ResizeObserver;
  globalThis.ResizeObserver = class extends RO {
    constructor(callback) { super((entries, observer) => {
      if (collectionSmoke.delayResize && entries.some(e => e.target === collectionSmoke.port)) {
        collectionSmoke.delayed++; requestAnimationFrame(() => callback(entries, observer));
      } else callback(entries, observer);
    }); }
  };
  globalThis.collectionSmoke = { calls: 0, snapshots: [], maxRows: 0, delayed: 0, postFeedback: [] };
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
        if (!plan) return capture(w.exact_boot(width, height, n));
        const launch = new Uint8Array(w.memory.buffer, input, n).slice();
        const ptr = w.exact_in(plan.length + n), bytes = new Uint8Array(w.memory.buffer, ptr, plan.length + n);
        bytes.set(plan); bytes.set(launch, plan.length);
        return capture(w.exact_boot_plan(plan.length, width, height, n));
      },
      exact_collection_feedback(n) {
        const d = new DataView(w.memory.buffer, input, n);
        collectionSmoke.focus = d.getUint32(56, true); collectionSmoke.interaction = d.getUint32(60, true);
        collectionSmoke.calls++; const len = capture(w.exact_collection_feedback(n));
        queueMicrotask(() => { if (collectionSmoke.geometry) collectionSmoke.postFeedback.push(collectionSmoke.geometry()); });
        return len;
      },
      exact_dispatch(...args) { return capture(w.exact_dispatch(...args)); },
    } } };
  };
}
const html = readFileSync(dist + '/index.html', 'utf8').replace('<script type="module" src="./glue.js"></script>',
  `<script>(${fixture})(${JSON.stringify(gallery ? null : [...readFileSync(dir + '/app.plan')])})</script><script type="module" src="./glue.js"></script>`);
writeFileSync(dist + '/index.html', html);
const server = createServer((req, res) => {
  // Identified local carrier; no filesystem-helper build needed for this test.
  const path = new URL(req.url, 'http://localhost').pathname, relative = path === '/' ? '/index.html' : path;
  if (relative.includes('..')) { res.writeHead(404); res.end(); return; }
  try { const bytes = readFileSync(dist + relative); res.writeHead(200, {'content-type':webContentType(relative)}); res.end(bytes); }
  catch { res.writeHead(404); res.end(); }
});
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
  await call('Runtime.enable'); await call('Page.enable'); await call('Page.bringToFront');
  await call('Emulation.setDeviceMetricsOverride', {width:1180,height:860,deviceScaleFactor:1,mobile:false});
  const evaluate = async expression => {
    const r = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
    if (r.exceptionDetails) throw Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text);
    return r.result.value;
  };
  const until = async expression => {
    for (let n = 0; n < 1000; n++) {
      if (await evaluate(expression).catch(() => false)) return;
      await new Promise(r => setTimeout(r, 10));
    }
    throw Error('timed out: ' + expression + '\n' + errors.join('\n') + '\n' + JSON.stringify(await evaluate('({state:collectionSmoke,active:document.activeElement.dataset.testid})')));
  };
  // Agent mode seeks every WAAPI animation after feedback; this regression
  // needs ordinary browser presentation while only its test animation is paused.
  await call('Page.navigate', { url: `http://127.0.0.1:${server.address().port}/` });
  await until('globalThis.exact?.root?.dataset.moduleReady === "true"');
  if (!gallery) await until('collectionSmoke.calls >= 1');
  if (process.env.EXACT_COLLECTION_EDGES === '1') {
    await until(`document.querySelector('[data-testid="steps"]').textContent === '6'
      && document.querySelector('[data-testid="ends"]').textContent === '1'
      && collectionSmoke.snapshots[0].rows.every(r=>r.measured)`);
    const result = await evaluate(`(() => {const p=document.querySelector('[data-testid="list"]');
      return {feedbackCalls:collectionSmoke.calls,maxRows:collectionSmoke.maxRows,
        scrollable:p.scrollHeight>p.clientHeight,steps:+document.querySelector('[data-testid="steps"]').textContent,
        ends:+document.querySelector('[data-testid="ends"]').textContent};})()`);
    assert.equal(result.scrollable,false); assert.equal(result.maxRows,2);
    assert(result.feedbackCalls>=7 && result.feedbackCalls<=12,JSON.stringify(result));
    await new Promise(r=>setTimeout(r,150));
    assert.equal(await evaluate('collectionSmoke.calls'),result.feedbackCalls,'settled edges become idle');
    writeFileSync(dir+'/result.json',JSON.stringify(result,null,2));
    console.log(JSON.stringify({passed:true,...result}));
  } else if (gallery) {
    await evaluate(`document.querySelector('[data-testid="count-25000"]').click(); document.querySelector('[data-testid="mode-sheet"]').click()`);
    await until('collectionSmoke.snapshots.some(s=>s.count===25000)');
    await resizeCoverage(call, evaluate, until, cdp, sessionId, true);
  } else {
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
  // Confirmed heights can still leave a geometry callback queued (for example
  // an anchor acknowledgement). Drain the bounded callbacks before asserting idle.
  const measured = await evaluate('({calls:collectionSmoke.calls,correction:collectionSmoke.snapshots[0].correction})');
  await evaluate('new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))');
  const end = await evaluate('collectionSmoke.calls');
  console.log(JSON.stringify({measured,settledFeedbackCalls:end}));
  await new Promise(r => setTimeout(r, 150));
  assert.equal(await evaluate('collectionSmoke.calls'), end, 'settled geometry must become idle');
  await evaluate(`document.querySelector('[data-testid="hide"]').click()`);
  await until('collectionSmoke.snapshots.length === 0');
  assert.equal(await evaluate(`document.querySelector('[data-testid="list"]') === null`), true);
  await evaluate(`document.querySelector('[data-testid="hide"]').click()`);
  await until('collectionSmoke.snapshots.length === 1 && collectionSmoke.snapshots[0].rows.some(r=>r.index===0)');
  assert.deepEqual(errors, []);
  console.log(JSON.stringify({ passed: true, initialRows: initial.snapshots[0].rows.length, maxRows: middle.state.maxRows, logicalRows: 1000, feedbackCalls: end, traversed: ['top', 'middle', 'end'], unmount: true, remount: true, pins: 2, typing: true, resizeAnchor: true }));
  await resizeCoverage(call, evaluate, until, cdp, sessionId, false);
  }
  assert.deepEqual(errors, []);
} finally {
  if (child.exitCode === null) { child.kill(); await exited; }
  await new Promise(r => server.close(r));
}

// Decode Chrome's 8-bit RGB/RGBA PNGs outside the page: inspecting evidence
// must not pump another browser frame and accidentally hide a one-frame gap.
function pixels(base64) {
  const png = Buffer.from(base64, 'base64'), chunks = [];
  let width, height, channels;
  for (let at = 8; at < png.length;) {
    const size = png.readUInt32BE(at), tag = png.toString('ascii', at + 4, at + 8), data = png.subarray(at + 8, at + 8 + size);
    if (tag === 'IHDR') {
      width = data.readUInt32BE(0); height = data.readUInt32BE(4);
      assert.equal(data[8], 8); assert([2, 6].includes(data[9])); assert.equal(data[12], 0);
      channels = data[9] === 2 ? 3 : 4;
    }
    if (tag === 'IDAT') chunks.push(data);
    at += 12 + size;
  }
  const bytes = inflateSync(Buffer.concat(chunks)), stride = width * channels, out = Buffer.alloc(stride * height);
  const paeth = (a,b,c) => { const p=a+b-c, x=Math.abs(p-a), y=Math.abs(p-b), z=Math.abs(p-c); return x<=y&&x<=z?a:y<=z?b:c; };
  for (let y=0; y<height; y++) {
    const filter = bytes[y*(stride+1)]; assert(filter <= 4);
    for (let x=0; x<stride; x++) {
      const at=y*stride+x, a=x>=channels?out[at-channels]:0, b=y?out[at-stride]:0, c=y&&x>=channels?out[at-stride-channels]:0;
      out[at]=(bytes[y*(stride+1)+1+x]+[0,a,b,Math.floor((a+b)/2),paeth(a,b,c)][filter])&255;
    }
  }
  return (x,y) => { const at=(Math.floor(y)*width+Math.floor(x))*channels; return [...out.subarray(at,at+3)]; };
}

function installResize(gallery) {
  const smoke=collectionSmoke, snapshot=smoke.snapshots[0], list=document.querySelector(`[data-view="${snapshot.view}"]`);
  const port=gallery?document.querySelector('[data-testid="sheet-scroll"]'):list;
  const panel=gallery?document.querySelector('[data-testid="reading-sheet"]'):port;
  smoke.port=port; smoke.panel=panel;
  document.activeElement?.blur(); port.scrollTop=0;
  const selector=`[data-view="${snapshot.view}"] > [data-view]:has(> [data-view])`;
  const style=document.createElement('style');
  style.textContent=`${selector}{height:124px!important;min-height:124px!important;box-sizing:border-box!important;background:#00cc00!important} ${selector} > *{visibility:hidden!important}`;
  document.head.append(style);
  Object.assign(panel.style,{position:'fixed',bottom:'120px',top:'auto',left:'20px',width:'400px',boxSizing:'border-box',height:gallery?'180px':'105px',transition:'none'});
  port.style.outline='2px solid #0000ff'; port.style.background='#ff00ff'; list.style.background='#ff00ff';
  smoke.geometry=()=>{
    const rect=port.getBoundingClientRect(), top=rect.top+port.clientTop, bottom=top+port.clientHeight;
    const rows=smoke.snapshots[0].rows.map(r=>document.querySelector(`[data-view="${r.view}"]`).getBoundingClientRect()).sort((a,b)=>a.top-b.top);
    let end=top, gap=0;
    for(const row of rows) { if(row.bottom<=top||row.top>=bottom)continue; gap=Math.max(gap,row.top-end);end=Math.max(end,row.bottom); }
    gap=Math.max(gap,bottom-end);
    return {phase:smoke.phase,at:performance.now(),top:rect.top,bottom:rect.bottom,left:rect.left,width:rect.width,height:port.clientHeight,
      rows:rows.length,revision:smoke.snapshots[0].revision,gap,covered:gap<=1,calls:smoke.calls};
  };
  // Paused WAAPI changes actual layout at a controlled sample, not an elapsed
  // timer. No host animation clock or synthetic collection feedback is used.
  smoke.animation=panel.animate([{height:gallery?'180px':'105px'},{height:gallery?'331px':'256px'}],{duration:1000,fill:'both'});
  smoke.animation.pause(); smoke.animation.currentTime=0;
  const marker=document.createElement('div');
  marker.style.cssText='position:fixed;left:0;top:0;width:4px;height:4px;z-index:2147483647;pointer-events:none';
  document.body.append(marker);
  smoke.resize=(delayed,phase)=>new Promise(resolve=>requestAnimationFrame(()=>{
    smoke.phase=delayed?'delayed':'normal'; smoke.delayResize=delayed; smoke.postFeedback=[];
    smoke.animation.currentTime=1000; smoke.rendering=true;
    let frame=0;
    const mark=()=>{
      if(!smoke.rendering||frame>=120)return;
      marker.style.background=`rgb(${phase},${++frame},0)`;
      requestAnimationFrame(mark);
    };
    mark(); resolve(smoke.geometry());
  }));
  return smoke.geometry();
}

async function resizeCoverage(call, evaluate, until, cdp, sessionId, gallery) {
  await evaluate(`(${installResize})(${gallery})`);
  await until('collectionSmoke.geometry().height===105 && collectionSmoke.geometry().rows===2 && collectionSmoke.geometry().covered');
  const frames=[];
  cdp.listeners.push(event=>{
    if(event.sessionId!==sessionId||event.method!=='Page.screencastFrame')return;
    frames.push(event.params);
    call('Page.screencastFrameAck',{sessionId:event.params.sessionId}).catch(()=>{});
  });
  await call('Page.startScreencast',{format:'png',everyNthFrame:1});
  const results=[];
  // Run the negative control first. Its *first* resized rendered frame must
  // expose magenta; waiting for the eventual recovered frame would pass falsely.
  for(const delayed of [true,false]) {
    await evaluate(`collectionSmoke.delayResize=false; collectionSmoke.animation.currentTime=0`);
    await until('collectionSmoke.geometry().height===105 && collectionSmoke.geometry().rows===2');
    await evaluate('new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))');
    const start=frames.length, phase=delayed?'delayed':'normal';
    const phaseId=delayed?1:2;
    const geometry=await evaluate(`collectionSmoke.resize(${delayed},${phaseId})`);
    assert.equal(geometry.height,256);
    let selected;
    for(let attempt=0;attempt<200&&!selected;attempt++) {
      for(const frame of frames.slice(start)) {
        const pixel=pixels(frame.data);
        if(pixel(geometry.left+10,geometry.top-1).join(',')==='0,0,255') { selected={frame,pixel};break; }
      }
      if(!selected)await new Promise(r=>setTimeout(r,10));
    }
    assert(selected, 'must capture first resized rendered frame; never substitute an eventual screenshot');
    writeFileSync(dir+`/resize-${phase}.png`,Buffer.from(selected.frame.data,'base64'));
    // Screencast can drop frames under load. Fail closed if that happened: a
    // later covered frame is not evidence that the first frame was covered.
    const marker=selected.pixel(1,1);
    assert.deepEqual(marker,[phaseId,1,0], 'capture must contain rendering opportunity one');
    await evaluate('collectionSmoke.rendering=false');
    const color=selected.pixel(geometry.left+5,geometry.bottom-3);
    await until('collectionSmoke.postFeedback.some(g=>g.height===256&&g.covered)');
    const post=await evaluate('collectionSmoke.postFeedback');
    const result={phase,marker,firstFrameTimestamp:selected.frame.metadata.timestamp,bottomPixel:color,beforeFeedback:geometry,postFeedback:post};
    results.push(result); writeFileSync(dir+'/resize-coverage.json',JSON.stringify(results,null,2));
    assert(post.every(g=>g.height!==256||g.covered), 'accepted feedback must synchronously cover the port');
    assert(post.every(g=>g.rows<=24), 'resize retains bounded rows');
    assert.deepEqual(color,delayed?[255,0,255]:[0,204,0],`${phase}: first resized rendered frame`);
  }
  await call('Page.stopScreencast');
  await evaluate('new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))');
  const calls=await evaluate('collectionSmoke.calls'); await new Promise(r=>setTimeout(r,150));
  assert.equal(await evaluate('collectionSmoke.calls'),calls,'resize feedback becomes idle');
  console.log(JSON.stringify({resizeCoverage:true,gallery,results}));
}
