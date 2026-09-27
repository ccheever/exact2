// Real compiled Host/Engine ABI and DOM pointer/WAAPI ownership. No network data.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { cpSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { Cdp } from '../../../scripts/agent.mjs';
import { serveStatic } from '../serve.mjs';
const dir=process.env.EXACT_MOTION_TEST,dist=resolve(dir,'dist');
assert(process.env.EXACT_MOTION_DIST,'set EXACT_MOTION_DIST to a current pure Rust web dist built with EXACT_WEB_LINK=all');
cpSync(process.env.EXACT_MOTION_DIST,dist,{recursive:true});
for(const name of ['glue.js','navigation.js','motion-glue.js','collection-glue.js'])cpSync('host/web/'+name,dist+'/'+name);
function fixture(plan) {
  const instantiate=WebAssembly.instantiateStreaming;
  globalThis.motionSmoke={requests:[],plan};
  WebAssembly.instantiateStreaming=async(...args)=>{
    const result=await instantiate(...args),w=result.instance.exports; let input;
    motionSmoke.raw=bytes=>{const ptr=w.exact_in(bytes.length);new Uint8Array(w.memory.buffer,ptr,bytes.length).set(bytes);const len=w.exact_motion(bytes.length);return JSON.parse(new TextDecoder().decode(new Uint8Array(w.memory.buffer,w.exact_out(),len)));};
    return {...result,instance:{exports:{...w,
      exact_in(n){input=w.exact_in(n);return input;},
      exact_boot(width,height,n){const launch=new Uint8Array(w.memory.buffer,input,n).slice(),ptr=w.exact_in(plan.length+n);new Uint8Array(w.memory.buffer,ptr,plan.length).set(plan);new Uint8Array(w.memory.buffer,ptr+plan.length,n).set(launch);return w.exact_boot_plan(plan.length,width,height,n);},
      exact_motion(n){const d=new DataView(w.memory.buffer,input,n),request={op:d.getUint32(4,true),view:d.getUint32(8,true),property:d.getUint32(12,true),token:String(d.getBigUint64(16,true)),x:d.getFloat64(24,true)};const len=w.exact_motion(n);request.reply=JSON.parse(new TextDecoder().decode(new Uint8Array(w.memory.buffer,w.exact_out(),len)));motionSmoke.requests.push(request);return len;},
    }}};
  };
}
writeFileSync(dist+'/index.html',readFileSync(dist+'/index.html','utf8').replace('<script type="module" src="./glue.js"></script>',`<script>(${fixture})(${JSON.stringify([...readFileSync(dir+'/app.plan')])})</script><script type="module" src="./glue.js"></script>`));
const server=createServer((req,res)=>serveStatic(dist,req,res));await new Promise(r=>server.listen(0,'127.0.0.1',r));
const child=spawn(process.env.CHROME??'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',['--headless=new','--no-sandbox','--remote-debugging-pipe','--disable-background-networking',`--user-data-dir=${dir}/chrome`,'about:blank'],{stdio:['ignore','ignore','ignore','pipe','pipe']});
const cdp=new Cdp(child.stdio[3],child.stdio[4]),errors=[];
const exited=new Promise(r=>child.on('exit',()=>{cdp.fail('Chrome closed');r();}));
try {
  const {targetInfos}=await cdp.send('Target.getTargets');
  const {sessionId}=await cdp.send('Target.attachToTarget',{targetId:(targetInfos.find(t=>t.type==='page')??await cdp.send('Target.createTarget',{url:'about:blank'})).targetId,flatten:true});
  const call=(method,params)=>cdp.send(method,params,sessionId);
  cdp.listeners.push(msg=>{if(msg.sessionId!==sessionId)return;if(msg.method==='Runtime.exceptionThrown')errors.push(msg.params.exceptionDetails.exception?.description??msg.params.exceptionDetails.text);if(msg.method==='Runtime.consoleAPICalled'&&msg.params.type==='error')errors.push(msg.params.args.map(a=>a.value??a.description).join(' '));});
  await call('Runtime.enable');
  const evaluate=async expression=>{const r=await call('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});if(r.exceptionDetails)throw Error(r.exceptionDetails.exception?.description??r.exceptionDetails.text);return r.result.value;};
  const until=async expression=>{for(let n=0;n<200;n++){if(await evaluate(expression).catch(()=>false))return;await new Promise(r=>setTimeout(r,10));}throw Error('timeout '+expression+' '+errors.join('\n'));};
  await call('Page.navigate',{url:`http://127.0.0.1:${server.address().port}/?agent=1`});
  await until('globalThis.exact?.root?.dataset.moduleReady==="true"');
  let row=await evaluate(`(()=>{const r=document.querySelector('[data-testid="row"]').getBoundingClientRect();return {x:r.left+20,y:r.top+15};})()`);
  const mouse=(type,dx)=>call('Input.dispatchMouseEvent',{type,x:row.x+dx,y:row.y,button:'left',buttons:type==='mouseReleased'?0:1,clickCount:type==='mouseMoved'?0:1});
  const position=()=>evaluate(`parseFloat(getComputedStyle(document.querySelector('[data-testid="row"]')).translate)||0`);
  await mouse('mousePressed',0);await mouse('mouseMoved',10);await mouse('mouseMoved',100);
  const held=await position();assert(held>64&&held<90,`held ${held}`);
  await evaluate(`document.querySelector('[data-testid="input"]').focus()`);await call('Input.insertText',{text:'still typing'});
  assert.equal(await evaluate(`document.querySelector('[data-testid="echo"]').textContent`),'still typing');
  assert.equal(await position(),held,'style commit must preserve translate hold');
  await mouse('mouseReleased',100);
  assert.equal(await evaluate(`document.querySelector('[data-testid="count"]').textContent`),'1');
  const release=await evaluate(`motionSmoke.requests.findLast(r=>[2,10].includes(r.op)&&r.property===0)`);
  const animation=release.reply.batch.ops.find(op=>op.op==='animate'&&op.property==='translate');assert(animation?.values.length>2,JSON.stringify(release));assert.equal(animation.delay,100);
  assert.equal(await position(),held,'delay must hold the released start');
  // Real playback is caught at its computed value, not the pointer-down value.
  await evaluate(`globalThis.returning=document.querySelector('[data-testid="row"]').getAnimations().find(a=>a.effect.getKeyframes().some(k=>k.translate));returning.pause();returning.currentTime=150;`);
  row=await evaluate(`(()=>{const r=document.querySelector('[data-testid="row"]').getBoundingClientRect();return {x:r.left+20,y:r.top+15};})()`);
  await mouse('mousePressed',0);
  const crossing=await evaluate(`returning.currentTime=200;parseFloat(getComputedStyle(document.querySelector('[data-testid="row"]')).translate)`);
  assert(crossing>0&&crossing<64,'returning sample below commit threshold');
  await mouse('mouseMoved',-10);assert(Math.abs(await position()-crossing)<0.001,'immediate left recognition must capture current WAAPI sample');
  await evaluate(`document.querySelector('[data-testid="plain"]').click()`);
  assert(Math.abs(await position()-crossing)<0.001,'transition:none commit must preserve hold '+JSON.stringify(await evaluate('motionSmoke.requests.slice(-6)')));
  await mouse('mouseReleased',-10);assert.equal(await position(),0,'none release snaps to latest target');
  // A committed disable or retained-route switch must consume every hold even
  // when no pointerup arrives. Re-enabling cannot revive that old gesture.
  for(const action of ['disable','deactivate']) {
    row=await evaluate(`(()=>{const r=document.querySelector('[data-testid="row"]').getBoundingClientRect();return {x:r.left+20,y:r.top+15};})()`);
    await mouse('mousePressed',0);await mouse('mouseMoved',10);await mouse('mouseMoved',100);
    const tokens=await evaluate(`motionSmoke.requests.filter(r=>r.op===0).slice(-3).map(r=>r.reply.token)`);
    assert.equal(tokens.length,3);
    await evaluate(`document.querySelector('[data-testid="${action}"]').click()`);
    for(const token of tokens) assert.equal(await evaluate(`import('/motion-glue.js').then(({motionBytes})=>motionSmoke.raw(motionBytes({op:'live',token:${JSON.stringify(token)}})).accepted)`),false,action+' consumes hold before pointerup');
    assert.equal(await evaluate(`import('/motion-glue.js').then(({motionBytes})=>motionSmoke.raw(motionBytes({op:'action',token:${JSON.stringify(tokens[0])},now:NaN})).accepted)`),false,'retired token cannot dispatch directly through Host');
    assert.equal(await evaluate(`document.querySelector('[data-testid="row"]')!==null`),true,'retained row remains mounted');
    if(action==='deactivate')assert.equal(await evaluate(`document.querySelector('[data-testid="held-route"]').inert`),true);
    await evaluate(`document.querySelector('[data-testid="enable"]').click()`);
    await mouse('mouseReleased',100);
    assert.equal(await evaluate(`document.querySelector('[data-testid="count"]').textContent`),'1','late pointerup cannot commit');
  }
  await evaluate(`document.querySelector('[data-testid="arm"]').click()`);
  row=await evaluate(`(()=>{const r=document.querySelector('[data-testid="row"]').getBoundingClientRect();return {x:r.left+20,y:r.top+15};})()`);
  await mouse('mousePressed',0);await mouse('mouseMoved',10);await mouse('mouseMoved',100);await mouse('mouseReleased',100);
  assert.equal(await evaluate(`document.querySelector('[data-testid="row"]')===null`),true);
  assert.equal(await evaluate(`document.querySelector('[data-testid="count"]').textContent`),'2');
  const stale=await evaluate(`motionSmoke.requests.findLast(r=>r.op===0&&r.property===0).reply.token`);
  const reject=()=>evaluate(`import('/motion-glue.js').then(({motionBytes})=>motionSmoke.raw(motionBytes({op:'action',token:${JSON.stringify(stale)},now:NaN})))`);
  assert.equal((await reject()).accepted,false,'deleted token cannot dispatch');
  await evaluate(`exact.reload(new Uint8Array(motionSmoke.plan))`);await until('exact.root.dataset.moduleReady==="true"');
  assert.equal((await reject()).accepted,false,'reloaded Host cannot accept old token');
  assert.deepEqual(errors,[]);
  console.log(JSON.stringify({passed:true,realWasm:true,realPointer:true,typingWhileHeld:true,delay:100,takeover:true,transitionNone:true,disabledCancellation:true,retainedRouteCancellation:true,deletion:true,staleReload:true}));
} finally {
  if(child.exitCode===null){child.kill();await exited;}
  await new Promise(r=>server.close(r));
}
