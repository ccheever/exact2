import {killChildren} from './processes.mjs';
import {spawn} from 'node:child_process';
import {createServer} from 'node:http';
import {mkdtempSync,rmSync,writeFileSync,readFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {Cdp} from '../../../scripts/agent.mjs';
import {serveStatic} from '../../../host/web/serve.mjs';
const dir=resolve(import.meta.dirname,'../dist');
const interval=Number(process.argv[2]??10);
const server=createServer((q,r)=>{if(q.url==='/__plan'){r.setHeader('content-type','application/octet-stream');r.end(readFileSync(resolve(import.meta.dirname,`../evidence/plans/${interval}.plan`)));}else serveStatic(dir,q,r);});
await new Promise(r=>server.listen(0,'127.0.0.1',r));
const url=`http://127.0.0.1:${server.address().port}/`;
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
const instrument=`
window.x1={firstTick:null,queries:0,publications:[],advances:[]};
const original=WebAssembly.instantiateStreaming;
WebAssembly.instantiateStreaming=async(...a)=>{
 const result=await original.apply(WebAssembly,a),raw=result.instance.exports;
 window.x1raw=raw;
 window.x1read=(op)=>{const b=new TextEncoder().encode(JSON.stringify({op}));new Uint8Array(raw.memory.buffer,raw.exact_in(b.length),b.length).set(b);const n=raw.exact_agent(b.length);return JSON.parse(new TextDecoder().decode(new Uint8Array(raw.memory.buffer,raw.exact_out(),n)));};
 const exports={...raw};
 for(const [name,fn] of Object.entries(raw))if(typeof fn==='function'&&name.startsWith('exact_'))exports[name]=(...args)=>{
   const before=performance.now(),out=fn(...args),after=performance.now();
   if(!x1.firstTick&&raw.x1_first_tick?.())x1.firstTick={before,after,call:name};
   if(name==='exact_advance')x1.advances.push({at:after,to:args[0]});
   return out;
 };
 return {...result,instance:{exports}};
};
new MutationObserver(()=>{const el=[...document.querySelectorAll('*')].find(e=>e.childNodes.length===1&&e.textContent?.startsWith('Tick '));if(el&&x1.publications.at(-1)?.text!==el.textContent)x1.publications.push({at:performance.now(),text:el.textContent});}).observe(document,{subtree:true,childList:true,characterData:true});
`;
const result={cold:[],live:null};let profile;
async function browser(agent=false){
 profile=mkdtempSync(resolve(import.meta.dirname,'../evidence/profiles/web-'));
 const child=spawn('/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',['--headless=new','--remote-debugging-pipe',`--user-data-dir=${profile}`,'--no-sandbox','--no-first-run','--disable-background-networking','--disable-component-update','--disable-extensions','--disable-features=Translate','about:blank'],{detached:true,stdio:['ignore','ignore','pipe','pipe','pipe']});
 child.stderr.resume();const cdp=new Cdp(child.stdio[3],child.stdio[4]);
 child.on('exit',()=>cdp.fail('exited'));
 const targets=await cdp.send('Target.getTargets');const target=targets.targetInfos.find(x=>x.type==='page');
 const {sessionId}=await cdp.send('Target.attachToTarget',{targetId:target.targetId,flatten:true});
 const call=(m,p)=>cdp.send(m,p,sessionId);
 const evaluate=async expression=>{const r=await call('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});if(r.exceptionDetails)throw Error(r.exceptionDetails.exception?.description??r.exceptionDetails.text);return r.result.value;};
 await call('Page.enable');await call('Page.addScriptToEvaluateOnNewDocument',{source:instrument});
 await call('Page.navigate',{url:url+(agent?'?agent=1':'')});
 for(let i=0;i<1000;i++){if(await evaluate('!!globalThis.exact?.root?.dataset.bootMs&&!!globalThis.x1?.firstTick').catch(()=>false))break;await sleep(10);}
 await evaluate('exact.ready');await sleep(60);
 return {call,evaluate};
}
async function close(){await killChildren();if(profile)rmSync(profile,{recursive:true,force:true});profile=null;}
try {
 for(let i=0;i<(interval===10?10:0);i++){
  const {evaluate}=await browser(true);
  result.cold.push(await evaluate(`({firstTick:x1.firstTick,fcp:performance.getEntriesByName('first-contentful-paint')[0]?.startTime,bootMs:Number(exact.root.dataset.bootMs),wasmFetches:performance.getEntriesByType('resource').filter(x=>x.name.endsWith('.wasm')).map(x=>x.name)})`));
  await close();
 }
 const {evaluate,call}=await browser(false);
 if(interval!==10)await evaluate("fetch('/__plan').then(r=>r.arrayBuffer()).then(b=>exact.reload(new Uint8Array(b)))");
 const measure=async()=>evaluate(`({at:performance.now(),allocations:x1raw.x1_allocations(),state:x1read('state'),publications:x1.publications.length,advances:x1.advances.length})`);
 await sleep(500);const a=await measure();await sleep(3000);const b=await measure();
 result.live={from:a,to:b};
 await evaluate(`exact.views.get(x1read('tree').nodes.find(n=>n.props?.testId==='draw').id).click()`);await sleep(120);
 await evaluate(`exact.views.get(x1read('tree').nodes.find(n=>n.props?.testId==='hold').id).click()`);await sleep(120);
 await evaluate(`exact.views.get(x1read('tree').nodes.find(n=>n.props?.testId==='save').id).click()`);
 result.save=await evaluate(`({checkpoint:localStorage.getItem('exact.secret.world.checkpoint'),state:x1read('state')})`);
 await call('Page.reload',{ignoreCache:true});
 for(let i=0;i<1000;i++){if(await evaluate('!!globalThis.exact?.root?.dataset.bootMs').catch(()=>false))break;await sleep(10);}
 await evaluate('exact.ready');
 result.restore=await evaluate(`({checkpoint:localStorage.getItem('exact.secret.world.checkpoint'),state:x1read('state')})`);
 result.live.publicationSamples=await evaluate('x1.publications.slice(0,12)');
} catch(e){result.error=String(e.stack??e);console.error(e);}
finally{await close();server.close();writeFileSync(resolve(import.meta.dirname,`../evidence/web-measure${interval===10?'':'-'+interval}.json`),JSON.stringify(result,null,2));}
console.log(JSON.stringify({cold:result.cold.length,error:result.error,live:result.live&&{commits:result.live.to.state.epoch-result.live.from.state.epoch,ticks:result.live.to.state.resources.world.tick-result.live.from.state.resources.world.tick}}));
