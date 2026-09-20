import { mkdir, readFile, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { cache, root, run } from './build.mjs';
import { startServer } from './serve.mjs';
const chromePath='/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
class CDP {
  next=0;pending=new Map();events=[];
  constructor(url) {
    this.socket=new WebSocket(url);
    this.opened=new Promise((resolve,reject)=>{this.socket.onopen=resolve;this.socket.onerror=reject;});
    this.socket.onmessage=e=>{const v=JSON.parse(e.data);if(v.id){const p=this.pending.get(v.id);if(!p)return;this.pending.delete(v.id);clearTimeout(p.timer);v.error?p.reject(new Error(JSON.stringify(v.error))):p.resolve(v.result);}else this.events.push(v);};
  }
  async call(method,params={}) {
    await this.opened;const id=++this.next;
    return new Promise((resolve,reject)=>{const timer=setTimeout(()=>{this.pending.delete(id);reject(new Error(`CDP timeout: ${method}`));},180000);this.pending.set(id,{resolve,reject,timer});this.socket.send(JSON.stringify({id,method,params}));});
  }
  async evaluate(expression) {
    const response=await this.call('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true});
    if(response.exceptionDetails)throw new Error(JSON.stringify(response.exceptionDetails));
    return response.result.value;
  }
  close(){this.socket.close();}
}
await mkdir(cache,{recursive:true});
const report={attempts:[],comparisons:[],failures:[],sizes:await Bun.file(join(cache,'sizes.json')).json()};
await run(['cargo','build','-p','clod-view','--example','web_compare']);
const server=await startServer(0);
// HTTP contract checks report every failure before GPU work.
report.http=[];
for (const [range,status,length] of [['bytes=0-191',206,192],['bytes=-80',206,80],['bytes=999999999999-',416,0],['bytes=3-1',416,0]]) {
  const response=await fetch(`http://127.0.0.1:${server.port}/asset.clod`,{headers:{Range:range}});
  const bytes=await response.arrayBuffer();const row={range,status:response.status,bytes:bytes.byteLength};report.http.push(row);
  if(response.status!==status || bytes.byteLength!==length)report.failures.push(`Range ${range}: ${JSON.stringify(row)}`);
}
let done=false;
try {
  // First headless Metal, then headed Metal, finally headed with explicit WebGPU feature.
  for(let attempt=0;attempt<3 && !done;attempt++) {
    const profile=join(cache,`chrome-profile-${attempt}`);
    await rm(profile,{recursive:true,force:true});await mkdir(profile,{recursive:true});
    const flags=[`--user-data-dir=${profile}`,'--remote-debugging-port=0','--no-first-run','--no-default-browser-check',
      '--enable-unsafe-webgpu','--use-angle=metal','--disable-background-timer-throttling','--disable-renderer-backgrounding','--disable-backgrounding-occluded-windows',
      '--window-size=1280,720',...(attempt===0?['--headless=new']:[]),...(attempt===2?['--enable-features=WebGPU']:[]),'about:blank'];
    const chrome=Bun.spawn([chromePath,...flags],{stdout:'ignore',stderr:'pipe'});
    const stderrPromise=new Response(chrome.stderr).text();
    const record={pid:chrome.pid,flags};report.attempts.push(record);console.log(JSON.stringify({chrome:record}));
    let cdp;
    try {
      let port;
      for(let i=0;i<150;i++) {
        try{port=(await readFile(join(profile,'DevToolsActivePort'),'utf8')).split('\n')[0];break;}catch{}
        if(chrome.exitCode!==null)throw new Error(`Chrome exited ${chrome.exitCode}`);
        await sleep(200);
      }
      if(!port)throw new Error('Chrome did not publish DevToolsActivePort in 30 seconds');
      const target=await (await fetch(`http://127.0.0.1:${port}/json/new?about:blank`,{method:'PUT'})).json();
      cdp=new CDP(target.webSocketDebuggerUrl);
      await cdp.call('Runtime.enable');await cdp.call('Page.enable');
      await cdp.call('Emulation.setDeviceMetricsOverride',{width:1280,height:720,deviceScaleFactor:1,mobile:false});
      await cdp.call('Page.navigate',{url:`http://127.0.0.1:${server.port}/?proof&width=1280&height=720`});
      let state;
      for(let i=0;i<600;i++) {
        state=await cdp.evaluate('window.clod?.state ?? null');
        if(state?.error)throw new Error(state.error);
        if(state?.full)break;
        await sleep(200);
      }
      if(!state?.full)throw new Error(`Full residency timeout: ${JSON.stringify(state)}`);
      record.state=state;console.log(JSON.stringify({loaded:state}));
      if(state.firstFramePages!==1 || state.pageEvents.filter(p=>p.verified).length!==state.pages)report.failures.push('Coarse-first/integrity counters failed');
      for(const [t,view] of [[0,'lit'],[.45,'lit'],[.75,'lit'],[1,'lit'],[.75,'clusters']]) {
        const name=`gaul-${t}-${view}`;
        try {
          const png=await cdp.evaluate(`clod.frame(${t},${JSON.stringify(view)})`);
          const web=join(cache,`${name}-web.png`),native=join(cache,`${name}-native.png`);
          await Bun.write(web,Buffer.from(png.split(',')[1],'base64'));
          await run([join(root,'target/debug/clod-view'),'render',join(process.env.HOME,'Library/Caches/exact2-cluster-lod/out/gaul-4.clod'),'--out',native,'--size','1280x720','--path','hero','--layout','avenue:25','--t',String(t),'--threshold-px','1','--view',view]);
          const compare=Bun.spawn([join(root,'target/debug/examples/web_compare'),web,native],{stdout:'pipe',stderr:'inherit'});
          console.log(JSON.stringify({compare_pid:compare.pid}));
          const comparison=JSON.parse(await new Response(compare.stdout).text());
          if(await compare.exited)throw new Error('PNG comparison failed');
          const row={t,view,...comparison};report.comparisons.push(row);console.log(JSON.stringify({comparison:row}));
          // Report backend float/coverage deviations, fail clearly divergent or blank images.
          if(row.mean>.002 || row.fraction_above_2>.02)report.failures.push(`${name}: parity gate exceeded`);
        } catch(e) {report.failures.push(`${name}: ${e}`);}
      }
      try {report.pacing=await cdp.evaluate('clod.benchmark(600)');console.log(JSON.stringify({pacing:report.pacing}));if(report.pacing.overflow)report.failures.push('Pacing overflow');}catch(e){report.failures.push(`pacing: ${e}`);}
      // Exercise every documented key, real pointer orbit, lazy naive, and DPR resizing.
      report.controls=[];
      const press=async(code,expected)=>{
        const result=await cdp.evaluate(`(()=>{window.dispatchEvent(new KeyboardEvent('keydown',{code:${JSON.stringify(code)}}));return {...clod.state};})()`);
        const pass=Object.entries(expected).every(([k,v])=>typeof v==='number'?Math.abs(result[k]-v)<1e-5:result[k]===v);
        report.controls.push({code,pass});if(!pass)report.failures.push(`key ${code}: ${JSON.stringify(result)}`);
      };
      await cdp.evaluate('clod.frame(.5)');
      await press('Space',{paused:false});await press('Space',{paused:true});
      await press('ArrowRight',{t:.505});await press('ArrowLeft',{t:.5});
      for(const [code,view] of [['KeyC','clusters'],['KeyD','depth'],['KeyT','triangles']]) {await press(code,{view});await cdp.evaluate('clod.capture()');await press(code,{view:'lit'});}
      await press('BracketLeft',{threshold:.5});await press('BracketRight',{threshold:1});
      for(const [code,layout] of [['Digit1','single'],['Digit2','ring:12'],['Digit3','avenue:25'],['Digit4','grid:400']]) {await press(code,{layout});await cdp.evaluate('clod.capture()');}
      await cdp.evaluate('clod.frame(.75)');
      const before=await cdp.evaluate('clod.capture()');
      await cdp.call('Input.dispatchMouseEvent',{type:'mousePressed',x:640,y:360,button:'left',clickCount:1});
      await cdp.call('Input.dispatchMouseEvent',{type:'mouseMoved',x:690,y:390,button:'left',buttons:1});
      await cdp.call('Input.dispatchMouseEvent',{type:'mouseReleased',x:690,y:390,button:'left',clickCount:1});
      const after=await cdp.evaluate('clod.capture()');
      report.controls.push({code:'drag',pass:before!==after});if(before===after)report.failures.push('Orbit did not change pixels');
      try {
        await cdp.evaluate('clod.frame(.75)');
        await cdp.evaluate('clod.toggleNaive()');
        const naive=await cdp.evaluate('clod.capture()');
        await Bun.write(join(cache,'gaul-naive-web.png'),Buffer.from(naive.split(',')[1],'base64'));
        const result=await cdp.evaluate('clod.state');report.naive={enabled:result.naive,triangles:result.triangles};
        if(!result.naive || !result.triangles)report.failures.push('Naive mode did not draw');
        await cdp.evaluate('clod.toggleNaive()');
      }catch(e){report.failures.push(`naive: ${e}`);}
      // Re-navigation drops the first device and validates the selectable high-memory asset.
      await cdp.call('Page.navigate',{url:`http://127.0.0.1:${server.port}/?proof&asset=washington&width=1280&height=720`});
      let hero;
      for(let i=0;i<900;i++) {
        hero=await cdp.evaluate('window.clod?.state ?? null');
        if(hero?.error || hero?.full)break;await sleep(200);
      }
      report.hero=hero;
      if(hero?.full) {
        const png=await cdp.evaluate("clod.frame(.75,'lit')");await Bun.write(join(cache,'washington-portrait-web.png'),Buffer.from(png.split(',')[1],'base64'));
      } else if(!hero?.error) report.failures.push('Washington load timed out without a message');
      // Normal animation and DPR sizing use the same host, without the proof clock.
      await cdp.call('Emulation.setDeviceMetricsOverride',{width:640,height:360,deviceScaleFactor:2,mobile:false});
      await cdp.call('Page.navigate',{url:`http://127.0.0.1:${server.port}/`});
      let interactive;
      for(let i=0;i<600;i++) {
        interactive=await cdp.evaluate('window.clod?.state ?? null');if(interactive?.error || interactive?.full)break;await sleep(200);
      }
      if(interactive?.error)report.failures.push(`interactive: ${interactive.error}`);
      else {
        const firstT=interactive?.t;await sleep(250);
        report.interactive=await cdp.evaluate('({t:clod.state.t,frames:clod.state.frames,width:document.querySelector("canvas").width,height:document.querySelector("canvas").height,dpr:devicePixelRatio,error:clod.state.error})');
        if(!(report.interactive.t>firstT) || report.interactive.width!==1280 || report.interactive.height!==720 || report.interactive.error)report.failures.push('DPR or animation failed');
        await cdp.call('Emulation.setDeviceMetricsOverride',{width:480,height:270,deviceScaleFactor:2,mobile:false});
        await sleep(100);
        report.resize=await cdp.evaluate('({width:document.querySelector("canvas").width,height:document.querySelector("canvas").height,error:clod.state.error})');
        if(report.resize.width!==960 || report.resize.height!==540 || report.resize.error)report.failures.push('Resize failed');
      }
      record.events=cdp.events.filter(x=>['Runtime.exceptionThrown','Runtime.consoleAPICalled'].includes(x.method));
      done=true;
    } catch(e) {record.error=String(e);record.events=cdp?.events;console.error(e);if(!/adapter|DevToolsActivePort|Chrome exited/i.test(record.error)){report.failures.push(record.error);break;}}
    finally {
      cdp?.close();
      // Always SIGKILL exactly our recorded Chrome PID, then reap it before another launch.
      chrome.kill('SIGKILL');record.exitCode=await chrome.exited;record.signal=chrome.signalCode;
      await Bun.write(join(cache,`chrome-${attempt}.log`),await stderrPromise);
      record.reaped=true;
      console.log(JSON.stringify({chrome_finished:record.pid,exit:record.exitCode,signal:record.signal}));
    }
  }
} finally { server.stop(true); }
if(!done)report.failures.push('All three Chrome attempts failed');
if(report.comparisons.length!==5)report.failures.push(`Only ${report.comparisons.length}/5 image comparisons`);
await Bun.write(join(cache,'proof.json'),JSON.stringify(report,null,2));
console.log(JSON.stringify(report));
process.exitCode=report.failures.length?1:0;
