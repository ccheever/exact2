// Optional live WebGPU timing: run after the selected fixture web proof.
import {spawn} from 'node:child_process';
import {createServer} from 'node:http';
import {mkdtempSync,rmSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {Cdp} from '../../../scripts/agent.mjs';
import {serveStatic} from '../../../host/web/serve.mjs';
const name=process.argv[2] ?? 'particles-fixture', root=resolve(import.meta.dir,'../../..');
if(!['particles-fixture','sprites-fixture'].includes(name)) throw Error('Use particles-fixture or sprites-fixture');
const server=createServer((req,res)=>serveStatic(resolve(root,'game/games',name,'dist'),req,res));
await new Promise(ok=>server.listen(0,'127.0.0.1',ok));
const profile=mkdtempSync('/tmp/p1-chrome-');
const child=spawn('/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',['--headless=new','--remote-debugging-pipe',`--user-data-dir=${profile}`,'--no-first-run','--no-default-browser-check','--enable-unsafe-webgpu','--disable-background-networking','about:blank'],{stdio:['ignore','ignore','ignore','pipe','pipe']});
console.log('Chrome PID',child.pid);
const exited=new Promise(ok=>child.on('exit',ok)), cdp=new Cdp(child.stdio[3],child.stdio[4]);
function observe(){
  const state=window.p1={active:false,samples:[],errors:[]};
  const request=GPUAdapter.prototype.requestDevice;
  GPUAdapter.prototype.requestDevice=function(d={}){
    state.supported=this.features.has('timestamp-query');
    return request.call(this,{...d,requiredFeatures:[...new Set([...(d.requiredFeatures??[]),...(state.supported?['timestamp-query']:[])])]});
  };
  const create=GPUDevice.prototype.createCommandEncoder,submit=GPUQueue.prototype.submit;
  const pending=new WeakMap();
  GPUDevice.prototype.createCommandEncoder=function(...args){
    const encoder=create.apply(this,args);
    if(!state.active||!this.features.has('timestamp-query'))return encoder;
    const d=this,queries=d.createQuerySet({type:'timestamp',count:32});
    const result=d.createBuffer({size:256,usage:GPUBufferUsage.QUERY_RESOLVE|GPUBufferUsage.COPY_SRC});
    const read=d.createBuffer({size:256,usage:GPUBufferUsage.COPY_DST|GPUBufferUsage.MAP_READ});
    let count=0;const labels=[];
    const begin=encoder.beginRenderPass.bind(encoder),finish=encoder.finish.bind(encoder);
    encoder.beginRenderPass=desc=>{
      const at=count;count+=2;labels.push(desc.label);
      return begin({...desc,timestampWrites:{querySet:queries,beginningOfPassWriteIndex:at,endOfPassWriteIndex:at+1}});
    };
    encoder.finish=(...args)=>{
      if(count){encoder.resolveQuerySet(queries,0,count,result,0);encoder.copyBufferToBuffer(result,0,read,0,count*8);}
      const cmd=finish(...args);
      pending.set(cmd,async()=>{
        try{if(count){await read.mapAsync(GPUMapMode.READ);const times=new BigUint64Array(read.getMappedRange());
          const ms=Array.from({length:count/2},(_,i)=>Number(times[i*2+1]-times[i*2])/1e6);
          if(labels.some(x=>x==='game forward'))state.samples.push({labels,ms,total:ms.reduce((a,b)=>a+b,0)});}}
        catch(e){state.errors.push(String(e));}finally{read.destroy();result.destroy();queries.destroy();}
      });return cmd;
    };return encoder;
  };
  GPUQueue.prototype.submit=function(buffers){submit.call(this,buffers);for(const b of buffers){pending.get(b)?.();}};
}
try{
 const {targetInfos}=await cdp.send('Target.getTargets');const {sessionId}=await cdp.send('Target.attachToTarget',{targetId:targetInfos.find(t=>t.type==='page').targetId,flatten:true});
 const call=(method,params)=>cdp.send(method,params,sessionId);
 await call('Runtime.enable');await call('Page.enable');
 const evaluate=async expression=>{const r=await call('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true});if(r.exceptionDetails)throw Error(JSON.stringify(r.exceptionDetails));return r.result.value;};
 await call('Emulation.setDeviceMetricsOverride',{width:1280,height:720,deviceScaleFactor:1,mobile:false});
 await call('Page.addScriptToEvaluateOnNewDocument',{source:`(${observe.toString()})();`});
 await call('Page.navigate',{url:`http://127.0.0.1:${server.address().port}/`});
 for(let i=0;i<600;i++){if(await evaluate('!!globalThis.exact && !!document.querySelector("[data-testid=play]")'))break;await new Promise(r=>setTimeout(r,100));}
 await evaluate('exact.ready');await evaluate('document.querySelector("[data-testid=play]").click()');
 for(let i=0;i<600;i++){if(await evaluate('!!document.querySelector("[data-gpu-input]") && !!exact.gpu?.wantsInput(Number(document.querySelector("[data-gpu-input]").dataset.view))'))break;await new Promise(r=>setTimeout(r,100));}
 // Warm to 1,000 particles per emitter. Keep the live browser clock throughout.
 await new Promise(r=>setTimeout(r,6500));
 if(name==='sprites-fixture') { await evaluate('document.querySelector("[data-gpu-input]").focus()'); await call('Input.dispatchKeyEvent',{type:'keyDown',code:'KeyD',key:'d',windowsVirtualKeyCode:68}); }
 await evaluate('window.p1view=Number(document.querySelector("[data-gpu-input]").dataset.view);exact.gpu.agent(p1view,{op:"state",perf_reset:true});p1.active=true;');
 for(let i=0;i<600;i++){if(await evaluate('p1.samples.length>=180'))break;await new Promise(r=>setTimeout(r,100));}
 const result=await evaluate('p1.active=false;({gpu:p1,world:exact.gpu.agent(p1view,{op:"state"}).world,canvas:[innerWidth,innerHeight],devicePixelRatio})');
 const nums=result.gpu.samples.map(s=>s.total).sort((a,b)=>a-b);result.gpu.summary={count:nums.length,p50:nums[Math.floor(nums.length*.5)],p95:nums[Math.floor(nums.length*.95)]};
 writeFileSync(resolve(root,'game/games',name,'artifacts/perf-web.json'),JSON.stringify(result,null,2));console.log(JSON.stringify({gpu:result.gpu.summary,errors:result.gpu.errors,perf:result.world?.perf},null,2));
}finally{try{await cdp.send('Browser.close');}catch{}child.kill('SIGKILL');await exited;server.close();rmSync(profile,{recursive:true,force:true});}
