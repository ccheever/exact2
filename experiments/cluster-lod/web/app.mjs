import init, { Viewer, fetch_range } from '/pkg/clod_web.js';
const canvas = document.querySelector('canvas');
const stats = document.querySelector('#stats');
const params = new URLSearchParams(location.search);
const asset = params.get('asset') === 'washington' ? 'washington' : 'gaul';
const start = 0; // Navigation-relative: includes module and Wasm download.
const state = { asset, resident:0, pages:0, full:false, firstFrameMs:null, fullDetailMs:null, error:null,
  t:0, threshold:1, view:'lit', naive:false, paused:params.has('proof'), layout:'avenue:25', triangles:0, frameMs:0, frames:0, overflow:0 };
let viewer, scenes, sourceTriangles, ready = false, direction = 1, yaw = 0, pitch = 0, last = performance.now(), lastStats = 0, statsBusy = false;
const fail = e => { state.error = String(e); stats.textContent = state.error; ready = false; console.error(e); };
window.addEventListener('error', e => fail(e.message));
window.addEventListener('unhandledrejection', e => fail(e.reason));
const resize = () => {
  const w = params.has('width') ? Number(params.get('width')) : Math.round(innerWidth*devicePixelRatio);
  const h = params.has('height') ? Number(params.get('height')) : Math.round(innerHeight*devicePixelRatio);
  if (canvas.width!==w || canvas.height!==h) { canvas.width=w; canvas.height=h; viewer?.resize(w,h); }
};
resize(); addEventListener('resize',resize);
const digest = async bytes => Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),x=>x.toString(16).padStart(2,'0')).join('');
const paint = () => { viewer.draw(state.t,state.threshold,state.view,state.naive,yaw,pitch); state.frames++; };
function hud() { stats.textContent = `${state.triangles.toLocaleString()} / ${sourceTriangles.toLocaleString()} × ${scenes[state.layout].instances.length} triangles · ${state.frameMs.toFixed(2)} ms · ${state.resident}/${state.pages} pages · ${state.threshold} px · ${state.naive?'naive':state.view}${state.paused?' · paused':''}`; }
async function counters() { Object.assign(state,JSON.parse(await viewer.counters())); hud(); }
function tick(now) {
  if (ready) {
    try {
      state.frameMs = now-last;
      if (!state.paused) {
        state.t += (now-last)/40000*direction;
        if (state.t>=1) {state.t=1;direction=-1;}
        if (state.t<=0) {state.t=0;direction=1;}
      }
      paint();
      if (now-lastStats>500 && !statsBusy) {
        lastStats=now;statsBusy=true;
        counters().catch(fail).finally(()=>statsBusy=false);
      }
      const error = viewer.error(); if (error) fail(error);
    } catch(e) { fail(e); }
  }
  last=now;
  if (!params.has('proof')) requestAnimationFrame(tick);
}
const setLayout = name => { viewer.layout(JSON.stringify(scenes[name])); state.layout=name; state.t=0; yaw=pitch=0; };
let naiveLoaded = false;
let naiveLoading = false;
async function toggleNaive() {
  if (!state.full || naiveLoading) return;
  if (!naiveLoaded) {
    stats.textContent='Loading conventional buffers…';naiveLoading=true;
    const response=await fetch(`/naive.bin?asset=${asset}`);
    if (!response.ok) { stats.textContent=await response.text();naiveLoading=false;return; }
    viewer.baseline(new Uint8Array(await response.arrayBuffer()));naiveLoaded=true;naiveLoading=false;
  }
  state.naive=!state.naive;state.view='lit';
}
addEventListener('keydown',async e=>{
  if (!ready) return;
  try {
    if (['Space','ArrowLeft','ArrowRight'].includes(e.code)) e.preventDefault();
    if (e.code==='Space') {state.paused=!state.paused;yaw=pitch=0;}
    if (['ArrowLeft','ArrowRight'].includes(e.code)) {state.paused=true;state.t=Math.min(1,Math.max(0,state.t+(e.code==='ArrowRight'?.005:-.005)));yaw=pitch=0;}
    const views={KeyC:'clusters',KeyD:'depth',KeyT:'triangles'};
    if (views[e.code]) {state.view=state.view===views[e.code]?'lit':views[e.code];state.naive=false;}
    if (e.code==='KeyN') await toggleNaive();
    if (e.code==='BracketLeft') state.threshold=Math.max(.125,state.threshold/2);
    if (e.code==='BracketRight') state.threshold=Math.min(16,state.threshold*2);
    const layouts={Digit1:'single',Digit2:'ring:12',Digit3:'avenue:25',Digit4:'grid:400'};
    if (layouts[e.code]) setLayout(layouts[e.code]);
  } catch(e) {fail(e);}
});
canvas.addEventListener('pointerdown',e=>{ if(e.button===0) canvas.setPointerCapture(e.pointerId); });
canvas.addEventListener('pointermove',e=>{ if (state.paused && canvas.hasPointerCapture(e.pointerId)) {yaw-=e.movementX*.005;pitch=Math.max(-1.2,Math.min(1.2,pitch+e.movementY*.005));} });
canvas.addEventListener('pointerup',e=>canvas.releasePointerCapture(e.pointerId));
window.clod = {state, async capture() {
  paint();const png=canvas.toDataURL('image/png');await viewer.completed();await counters();return png;
}, async frame(t,view='lit',threshold=1,layout='avenue:25') {
  state.paused=true; if(state.layout!==layout) setLayout(layout); state.t=t;state.view=view;state.threshold=threshold;state.naive=false;yaw=pitch=0;
  paint(); const png=canvas.toDataURL('image/png'); await viewer.completed(); await counters(); return png;
}, async benchmark(n=600) {
  setLayout('avenue:25');state.view='lit';state.threshold=1;state.naive=false;
  const intervals=[], completed=[], encode=[]; let previous;
  for(let i=-10;i<n;i++) {
    await new Promise(requestAnimationFrame); const now=performance.now();
    state.t=Math.max(0,i)/Math.max(1,n-1); const begin=performance.now();paint();const submitted=performance.now();await viewer.completed();const end=performance.now();
    if(i>=0) {intervals.push(now-previous);encode.push(submitted-begin);completed.push(end-begin);} previous=now;
  }
  await counters();
  const distribution=values=>{values.sort((a,b)=>a-b);return {mean:values.reduce((a,b)=>a+b,0)/values.length,p50:values[Math.ceil(values.length*.5)-1],p95:values[Math.ceil(values.length*.95)-1],p99:values[Math.ceil(values.length*.99)-1],max:values.at(-1)};};
  return {frames:n,intervalMs:distribution(intervals),submitMs:distribution(encode),completedMs:distribution(completed),overflow:state.overflow};
}, toggleNaive, setLayout };
try {
  if (!navigator.gpu) throw new Error('WebGPU is unavailable in this browser.');
  await init();
  const url=`/asset.clod?asset=${asset}`;
  const header=await fetch_range(url,0,191);
  const hv=new DataView(header.buffer,header.byteOffset,header.byteLength);
  if(new TextDecoder().decode(header.subarray(0,8))!=='CLOD0004') throw new Error('Unsupported CLOD format');
  const geometry=Number(hv.getBigUint64(168,true));
  const tables=await fetch_range(url,0,geometry-1);
  const tv=new DataView(tables.buffer,tables.byteOffset,tables.byteLength);
  const pageCount=tv.getUint32(72,true), pageOffset=Number(tv.getBigUint64(152,true));
  const companion=await (await fetch(`/scenes.json?asset=${asset}`)).json();
  if(await digest(tables)!==companion.metadata_sha256) throw new Error('Camera metadata does not match baked asset');
  scenes=companion.scenes; sourceTriangles=tv.getUint32(60,true);state.pages=pageCount;state.metadataBytes=geometry;state.assetBytes=Number(tv.getBigUint64(16,true));state.pageEvents=[];
  viewer=await Viewer.create(canvas,tables,JSON.stringify(scenes[state.layout]));
  state.adapter=viewer.adapter();
  const adapter=await navigator.gpu.requestAdapter(); state.browserAdapter=adapter?.info?Object.fromEntries(['vendor','architecture','device','description','subgroupMinSize','subgroupMaxSize'].map(k=>[k,adapter.info[k]])):{};
  for(let page=0;page<pageCount;page++) {
    const offset=pageOffset+page*80;
    const begin=Number(tv.getBigUint64(offset,true)), length=Number(tv.getBigUint64(offset+8,true));
    const sha=Array.from(tables.subarray(offset+16,offset+48),x=>x.toString(16).padStart(2,'0')).join('');
    const bytes=await fetch_range(url,begin,begin+length-1);
    if(await digest(bytes)!==sha) throw new Error(`Page ${page} SHA-256 mismatch`);
    viewer.upload(page,bytes);state.resident++;state.pageEvents.push({page,bytes:length,verified:true,uploadedMs:performance.now()});
    if(page===0) {
      paint();
      await viewer.completed();state.firstFrameMs=performance.now()-start;state.firstFramePages=state.resident;
      await counters();state.firstFrameTriangles=state.triangles;ready=true;last=performance.now();
      if(!params.has('proof')) requestAnimationFrame(tick);
      // Give the browser a presentation opportunity before fetching page one.
      await new Promise(requestAnimationFrame);
    }
  }
  paint();await viewer.completed();state.fullDetailMs=performance.now()-start;state.full=true;await counters();
} catch(e) {fail(e);}
