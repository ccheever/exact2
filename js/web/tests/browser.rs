//! Real Chrome coverage of the private module realm, not a simulated DOM.
use std::path::Path;
use std::process::Command;
#[path = "support/caltrain.rs"]
mod caltrain;

#[test]
fn browser_modules_guard_their_own_builtins_and_refuse_bad_candidates() {
    let chrome = std::env::var("CHROME")
        .unwrap_or_else(|_| "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".into());
    if !Path::new(&chrome).exists() {
        eprintln!("browser module sweep unavailable: set CHROME");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let result = Command::new("node")
        .args(["--input-type=module", "-e", PROBE])
        .env("CHROME", chrome)
        .env(
            "EXACT_SCOPED_PLAN",
            serde_json::to_string(
                &contract::compile(include_str!("../../tests/fixtures/scoped.contract"))
                    .unwrap()
                    .encode(),
            )
            .unwrap(),
        )
        .env(
            "EXACT_PARITY",
            serde_json::to_string(&caltrain::oracle()).unwrap(),
        )
        .current_dir(root)
        .output()
        .unwrap();
    eprintln!("{}", String::from_utf8_lossy(&result.stdout));
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

const PROBE: &str = r#"
import { Cdp } from './scripts/agent.mjs';
import { spawn, execFileSync } from 'node:child_process';
import { createServer } from 'node:http';
import { readFileSync, writeFileSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, dirname } from 'node:path';
import assert from 'node:assert/strict';
const messages=execFileSync('cargo',['build','--locked','--offline','-p','exact-js-web','--target','wasm32-unknown-unknown','--message-format=json'],{encoding:'utf8',maxBuffer:16*1024*1024}).trim().split('\n').map(line=>JSON.parse(line));
const artifacts=new Map(messages.filter(m=>m.reason==='compiler-artifact').map(m=>[m.target.name,m.filenames.find(f=>f.endsWith('.rlib'))]));
const wasmDir=mkdtempSync(resolve(tmpdir(),'exact-scoped-wasm-'));
writeFileSync(resolve(wasmDir,'app.plan'),new Uint8Array(JSON.parse(process.env.EXACT_SCOPED_PLAN)));
execFileSync('rustc',['--edition=2021','--crate-type=cdylib','--target','wasm32-unknown-unknown','js/web/tests/support/scoped.rs','--out-dir',wasmDir,'-L',`dependency=${dirname(artifacts.get('exact_runner'))}`,...['exact_js_web','exact_kernel','exact_runner','exact_plan','serde_json'].flatMap(name=>['--extern',`${name}=${artifacts.get(name)}`])],{env:{...process.env,EXACT_SCOPED_PLAN_FILE:resolve(wasmDir,'app.plan')}});
const routes = {'/module-glue.js':'host/web/module-glue.js','/module-prelude.js':'js/src/prelude.js','/scoped.wasm':resolve(wasmDir,'scoped.wasm')};
routes['/startup/glue.js']='host/web/glue.js';
const hostPage=readFileSync('host/web/index.html','utf8');
const modulePage=hostPage.replace(/<script type="module" src="\.\/glue\.js"><\/script>/,'');
const startupStub=()=>{
  const memory=new WebAssembly.Memory({initial:2});
  const out=value=>{const bytes=new TextEncoder().encode(JSON.stringify(value));new Uint8Array(memory.buffer,65536,bytes.length).set(bytes);return bytes.length;};
  globalThis.startup={dispatch:[],activated:false,release:null};
  globalThis.startupGate=new Promise(resolve=>startup.release=resolve);
  const create=(id,tag,props,css,handlers=[])=>({op:'create',id,tag,props,css,handlers});
  const batch={ops:[
    create(1,'main',{id:'scroller'},'height:100%;overflow:auto'),
    create(2,'button',{id:'action',text:'Act'},'height:40px;width:200px',['press']),
    create(3,'input',{id:'editor',value:'baked'},'height:40px;width:200px',['change']),
    create(4,'button',{id:'disabled',text:'Disabled',disabled:'true'},'height:40px;width:200px',['press']),
    create(6,'input',{id:'range',type:'range',min:'0',max:'100',value:'50'},'appearance:auto;height:40px;width:200px',['change']),
    create(7,'button',{id:'disabled-on-activation',text:'Will disable'},'height:40px;width:200px',['press']),
    create(8,'button',{id:'enabled-on-activation',text:'Will enable',disabled:'true'},'height:40px;width:200px',['press']),
    create(5,'div',{text:'A long baked page'},'height:2200px'),
    {op:'children',id:1,ids:[2,3,4,6,7,8,5]},{op:'roots',ids:[1]}
  ]};
  WebAssembly.instantiateStreaming=async response=>{
    await response;
    return {instance:{exports:{memory,exact_out:()=>65536,exact_in:()=>0,
      exact_compat:()=>out({}),exact_logic:()=>out({appId:'test.startup',grants:''}),
      exact_plan:()=>out([]),exact_plan_fonts:()=>out([]),exact_boot:()=>out(batch),
      exact_data_ready:()=>{startup.activated=true;return out({ops:[
        {op:'props',id:7,set:{disabled:'true'},clear:[]},
        {op:'props',id:8,set:{},clear:['disabled']}
      ]});},
      exact_dispatch:(id,kind,length)=>{startup.dispatch.push({id,kind,value:new TextDecoder().decode(new Uint8Array(memory.buffer,0,length))});return out({ops:[]});}
    }}};
  };
};
const startupPage=hostPage.replace('<script type="module" src="./glue.js"></script>',`<script>(${startupStub.toString()})()</script><script type="module" src="/startup/glue.js"></script>`);
for(const name of ['storage.js','storage-fs.js','storage-sqlite.js','storage-worker.js'])routes['/'+name]='host/web/'+name;
routes['/sqlite3.mjs']='node_modules/@sqlite.org/sqlite-wasm/dist/index.mjs';
routes['/sqlite3.wasm']='node_modules/@sqlite.org/sqlite-wasm/dist/sqlite3.wasm';
const fixtures=Object.fromEntries(['inputs','castle','caltrain','ambient-init','storage'].map(name=>[name,execFileSync('./node_modules/.bin/rolldown',[`js/tests/fixtures/${name}.ts`,'--format','iife'],{encoding:'utf8',stdio:['ignore','pipe','pipe']})]));
fixtures.oracle=JSON.parse(process.env.EXACT_PARITY);
const server = createServer((req,res)=>{
  if(req.url.startsWith('/startup/module-glue.js')){
    res.setHeader('content-type','text/javascript');
    res.end(`globalThis.exact.moduleRuntime={baked:async()=>{await globalThis.startupGate;if(location.search.includes('fail'))throw new Error('controlled loader failure');return {};},prepare:async()=>({id:0,dispose(){}})};`);return;
  }
  res.setHeader('content-type', req.url.endsWith('.wasm') ? 'application/wasm' : routes[req.url] ? 'text/javascript' : 'text/html');
  res.end(routes[req.url] ? readFileSync(routes[req.url]) : req.url.startsWith('/startup') ? startupPage : modulePage);
});
await new Promise(r=>server.listen(0,'127.0.0.1',r));
const profile = mkdtempSync(resolve(tmpdir(),'exact-module-browser-'));
const child = spawn(process.env.CHROME, ['--headless=new','--no-sandbox','--remote-debugging-pipe','--no-first-run','--disable-background-networking',`--user-data-dir=${profile}`,'about:blank'],{detached:true,stdio:['ignore','ignore','ignore','pipe','pipe']});
const cdp = new Cdp(child.stdio[3],child.stdio[4]);
const exited = new Promise(r=>child.on('exit',()=>{cdp.fail('browser closed');r();}));
try {
  const { targetInfos } = await cdp.send('Target.getTargets');
  const { sessionId } = await cdp.send('Target.attachToTarget',{targetId:targetInfos.find(t=>t.type==='page').targetId,flatten:true});
  const call = (method,params)=>cdp.send(method,params,sessionId);
  await call('Page.enable');
  await call('Page.navigate',{url:`http://127.0.0.1:${server.address().port}/`});
  const probe = async (fixtures) => {
    await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));
    globalThis.exact ??= {};
    const {prepare,call,run} = await import('/module-glue.js');
    const checkpoint=async result=>{for(let i=0;result.continuation&&i<20;i++)result=await run(result.continuation);return result;};
    {
      const bytes=s=>new TextEncoder().encode(s), text=b=>new TextDecoder().decode(b);
      const script=bytes(fixtures.castle), identity={appId:'xyz.castle.test',grants:'net.fetch https://api.castle.xyz\nsecret.keep castle.session\n'};
      const digest=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',script)),b=>b.toString(16).padStart(2,'0')).join('');
      const receipt=bytes(JSON.stringify({version:1,abi:1,...identity,module:{sha256:'a'.repeat(64)},web:{file:'app.js',bytes:script.length,sha256:digest}}));
      let wasm, reply, beforePixel=true;
      ({instance:wasm}=await WebAssembly.instantiate(await (await fetch('/scoped.wasm')).arrayBuffer(),{exact_js:{call(op,ptr,len){
        if(beforePixel)throw new Error('wasm called JavaScript before first pixel');
        if(op===0){reply=bytes(JSON.stringify(call(JSON.parse(text(new Uint8Array(wasm.exports.memory.buffer,ptr,len))))));return reply.length;}
        new Uint8Array(wasm.exports.memory.buffer,ptr,len).set(reply);return len;
      }}}));
      const step=(op,ticket=0,body='')=>{const input=bytes(body), ptr=wasm.exports.input(input.length);new Uint8Array(wasm.exports.memory.buffer,ptr,input.length).set(input);const len=wasm.exports.step(op,BigInt(ticket));return JSON.parse(text(new Uint8Array(wasm.exports.memory.buffer,wasm.exports.output(),len)));};
      const boot=step(0);if(boot.requests.length)throw new Error('first frame requested JS work');
      beforePixel=false;
      const realm=await prepare({script,receipt},identity,0);
      const advance=async rows=>{const http=[];for(const row of rows){if(row.continuation){const ready=await run(row.continuation);http.push(...await advance(step(2,row.ticket,JSON.stringify(ready)).requests));}else http.push(row);}return http;};
      const first=await advance(step(1).requests);
      if(first.length!==2||first.some(r=>r.url!=='https://api.castle.xyz/child'))throw new Error('wasm siblings did not both request equal arguments');
      let state;
      for(const [index,who] of [[1,'B'],[0,'A']]) {
        const chain=await advance(step(2,first[index].ticket,who).requests);
        if(chain.length!==1||chain[0].url!==`https://api.castle.xyz/child/${who}`)throw new Error('wasm chained caller changed');
        state=step(2,chain[0].ticket,'done');await advance(state.requests);
      }
      const value=(tree,id)=>tree.nodes.find(n=>n.props?.testId===id||n.testId===id)?.props?.text;
      // Read rendered values through the real kernel's tree, after checkpoints.
      state=step(4);
      if(value(state.tree,'value-a')!=='A'||value(state.tree,'value-b')!=='B')throw new Error('wasm sibling values differ: '+JSON.stringify(state.tree));
      const stale=await advance(step(3,0,'refresh-a').requests);
      const saved=await advance(step(3,0,'save-b').requests);
      const fresh=await advance(step(3,0,'refresh-a').requests);
      if(stale.length!==1||saved.length!==1||fresh.length!==1)throw new Error('wasm resource/mutation did not remain separate');
      if(step(2,stale[0].ticket,'STALE').requests.length)throw new Error('stale wasm child resumed');
      for(const [request,who] of [[saved[0],'saved B'],[fresh[0],'new A']]) {
        const chain=await advance(step(2,request.ticket,who).requests);
        await advance(step(2,chain[0].ticket,'done').requests);
      }
      state=step(4);
      if(value(state.tree,'value-a')!=='new A'||value(state.tree,'value-b')!=='B'||value(state.tree,'saved-b')!=='saved B'||value(state.tree,'saved-a')!=='none')throw new Error('wasm scoped cancellation crossed child owners');
      realm.dispose();
    }
    const oldDate=Date, oldNow=Date.now, oldRandom=Math.random;
    const guest=document.createElement('iframe');document.getElementById('exact-root').append(guest);
    const guestBox=guest.getBoundingClientRect(), pageHeight=document.documentElement.scrollHeight;
    if(guestBox.width!==300||guestBox.height!==150)throw new Error('guest iframe lost its 300x150 box');
    const guestDate=guest.contentWindow.Date;
    const identity={appId:'test.browser.module',grants:'secret.keep token'};
    const encode=s=>new TextEncoder().encode(s);
    const hash=async bytes=>Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),b=>b.toString(16).padStart(2,'0')).join('');
    const payload=async (source,admitted=identity)=>{
      const script=encode(source);
      return {script,receipt:encode(JSON.stringify({version:1,abi:1,...admitted,module:{sha256:'a'.repeat(64)},web:{file:'app.js',bytes:script.length,sha256:await hash(script)}}))};
    };
    const source=`const captured=Date.now; globalThis.exact={abi:1,appId:'test.browser.module',grants:'secret.keep token',answer(source,args,store){
      if(source==='alias')return captured();
      if(source==='random')return globalThis['Math']['random']();
      if(source==='constructor')return new (new Date(0).constructor)();
      if(source==='intl')return new Intl.DateTimeFormat().format();
      if(source==='explicit')return new Date(args[0]).getUTCFullYear();
      if(source==='write'){const old=store.get('token');store.set('token','next');return old;}
      if(source==='refused')return store.set('other','secret');
      if(source==='async')return Promise.resolve('later');
      return 'ok';}};`;
    const module = await prepare(await payload(source),identity);
    const privateFrames=[...document.querySelectorAll('iframe')].filter(frame=>frame!==guest);
    if(privateFrames.length!==1||privateFrames.some(frame=>frame.getClientRects().length))throw new Error('private module iframe participates in layout');
    if(document.documentElement.scrollHeight!==pageHeight)throw new Error('private module grew document scroll height');
    const answer=(source,args=[])=>call({op:'answer',id:module.id,source,args,store:[['token','old']],grants:['token']});
    const results=['alias','random','constructor','intl'].map(name=>answer(name));
    if(!results.every(r=>r.message?.includes('pass time or a random seed')))throw new Error(JSON.stringify(results));
    if(answer('explicit',[0]).value!==1970)throw new Error('explicit date arithmetic changed');
    const written=answer('write');
    if(written.value!=='old'||written.reads[0]!=='token'||written.writes[0][1]!=='next')throw new Error('store seam changed');
    if(!answer('refused').message?.includes('not granted'))throw new Error('store grant bypass');
    if((await checkpoint(answer('async'))).value!=='later')throw new Error('Promise resolution failed');
    if(Date!==oldDate||Date.now!==oldNow||Math.random!==oldRandom||Date.now()<=0||guest.contentWindow.Date!==guestDate||guest.contentWindow.Date.now()<=0)throw new Error('page/guest globals changed');
    const count=document.querySelectorAll('iframe').length;
    for(const bad of [await payload(source.replace('appId:\'test.browser.module\'','appId:\'another.app\'')), {...await payload(source),script:encode('corrupt')}]) {
      let refused=false;try{await prepare(bad,identity);}catch{refused=true;}
      if(!refused||document.querySelectorAll('iframe').length!==count)throw new Error('bad module leaked an environment');
    }
    module.dispose();
    if(!call({op:'answer',id:module.id,source:'explicit',args:[0]}).error)throw new Error('disposed environment is callable');

    const inputsIdentity={appId:'test.explicit-inputs',grants:'net.fetch https://fixture.exact.test\n'};
    const inputs=await prepare(await payload(fixtures.inputs,inputsIdentity),inputsIdentity);
    const invoke=(realm,source,args=[],store=[],grants=[],outcome)=>checkpoint(call({id:realm.id,op:outcome?'resume':'answer',source,args,store,grants,outcome}));
    const response=(body='',status=200)=>({response:{status,headers:[],body,bodyBase64:btoa(body)}});
    const forms=['now','new','call','call-with-arg','random','alias-now','alias-random','alias-date','prototype-constructor','computed-now','computed-random','bound-now','bound-new','reflect','intl-format','intl-format-undefined','intl-parts','intl-parts-undefined','intl-format-alias','intl-format-alias-undefined','intl-parts-alias','intl-parts-alias-undefined','intl-format-getter','intl-format-computed','intl-parts-prototype'];
    for(const form of forms){
      const initial=await invoke(inputs,'atInit',[form]);
      const direct=await invoke(inputs,'ambient',[form]);
      const pending=await invoke(inputs,'ambientLater',[form]);
      if(pending.request?.url!=='https://fixture.exact.test/value')throw new Error('missing ambient request');
      const resumed=await invoke(inputs,'ambientLater',[form],[],[],response());
      if(!initial.value?.includes('as an argument')||direct.tag!==2||direct.message!==initial.value||resumed.tag!==2||resumed.message!==initial.value)throw new Error(`guard parity ${form}: ${JSON.stringify({initial,direct,resumed})}`);
    }
    let initRefused=false;try{await prepare(await payload(fixtures['ambient-init'],inputsIdentity),inputsIdentity);}catch{initRefused=true;}
    if(!initRefused)throw new Error('uncaught init did not refuse');
    for(const [ms,year] of [[0,'1970'],[1709210096789,'2024'],[-1,'1969']])if((await invoke(inputs,'intl',[ms])).value!==Array(4).fill(year).join('/'))throw new Error('Intl explicit parity');
    if((await invoke(inputs,'utc')).value!=='2024-02-29T12:34:56.789Z/12/1709210096789/true')throw new Error('UTC parity');
    const argumentsList=[[0,7],[1000,8]];
    const starters=await Promise.all(argumentsList.map(args=>invoke(inputs,'explicitLater',args)));
    if(starters.some(r=>r.tag!==1))throw new Error('concurrent start failed');
    for(const args of argumentsList.toReversed()){
      const actual=await invoke(inputs,'explicitLater',args,[],[],response());
      if(actual.value!==(await invoke(inputs,'explicit',args)).value)throw new Error('interleaved explicit inputs changed');
    }
    const stale=call({op:'answer',id:inputs.id,source:'explicitLater',args:[0,7],store:[],grants:[]});
    const draining=run(stale.continuation);await Promise.resolve();
    inputs.dispose();let dropped=false;try{await draining;}catch{dropped=true;}
    if(!dropped)throw new Error('disposed continuation executed');

    const castleIdentity={appId:'xyz.castle.test',grants:'net.fetch https://api.castle.xyz\nsecret.keep castle.session\n'};
    const castle=await prepare(await payload(fixtures.castle,castleIdentity),castleIdentity);
    const grants=['castle.session'], loginArgs=['ada','pw'];let store=[];
    const ask=(source,args=[],outcome)=>invoke(castle,source,args,store,grants,outcome);
    if((await ask('login',['','pw'])).value.error!=='Enter a username and a password')throw new Error('empty async login');
    let login=await ask('login',loginArgs);
    if(login.request.method!=='POST'||!login.request.body.includes('"who":"ada"'))throw new Error('login request');
    login=await ask('login',loginArgs,response('{"data":{"loginV2":{"token":"t0k","username":"ada"}}}'));
    if(login.value.username!=='ada'||login.writes[0][0]!=='castle.session')throw new Error('async store write');
    store=login.writes;
    if((await ask('remember')).value.username!=='ada')throw new Error('remember after await');
    if((await ask('profile')).request.url!=='https://api.castle.xyz/me')throw new Error('first fetch');
    if((await ask('profile',[],response('{"username":"ada"}'))).request.url!=='https://api.castle.xyz/profile/ada')throw new Error('second fetch');
    if((await ask('profile',[],response('hello'))).value.error!=='hello')throw new Error('sequential response');
    for(const kind of ['Refused','Unsupported','Network','Aborted']){
      await ask('login',loginArgs);
      const result=await ask('login',loginArgs,{failed:{kind,message:'test'}});
      if(!result.value?.error)throw new Error('fetch failure not data');
    }
    if((await ask('stuck')).tag!==2)throw new Error('pending on nothing must refuse');
    if((await ask('bogus')).kind!=='UnknownSource'||(await ask('login',[1,'pw'])).kind!=='BadArguments')throw new Error('error kind parity');
    if((await ask('refused')).message!=='refused on purpose')throw new Error('sync error');
    await ask('refusedLater');if((await ask('refusedLater',[],response())).message!=='after the fetch')throw new Error('async error');
    if((await ask('parallel')).request.url!=='https://api.castle.xyz/a')throw new Error('parallel first request');
    if((await ask('parallel',[],response('\u0000\u00ff'))).request.url!=='https://api.castle.xyz/b')throw new Error('parallel second request');
    if((await ask('parallel',[],response('ok'))).value.error!=='0,255/ok')throw new Error('parallel binary body');
    if((await ask('logout')).writes[0][1]!==null)throw new Error('forget');
    {
    // Distinct u64 contexts cross the JSON bridge as strings, including values
    // beyond JavaScript's exact-integer range. Equal arguments remain independent.
    const scoped=(context,source,args=[],outcome)=>checkpoint(call({id:castle.id,context,op:outcome?'resume':'answer',source,args,store,grants,outcome}));
    const first='18446744073709551614', second='18446744073709551615';
    const same=['same','pw'];
    const starts=await Promise.all([scoped(first,'login',same),scoped(second,'login',same)]);
    if(starts.some(r=>r.tag!==1))throw new Error('equal-argument scoped starts did not park');
    for(const [context,who] of [[second,'second'],[first,'first']]) {
      const result=await scoped(context,'login',same,response(JSON.stringify({data:{loginV2:{token:who,username:who}}})));
      if(result.value?.username!==who||!result.writes[0][1].includes(who))throw new Error('scoped response reached another caller');
    }
    await Promise.all([scoped('201','profile'),scoped('202','profile')]);
    for(const [context,who] of [['202','second'],['201','first']]) {
      const result=await scoped(context,'profile',[],response(JSON.stringify({username:who})));
      if(result.request?.url!==`https://api.castle.xyz/profile/${who}`)throw new Error('scoped chain lost its caller');
    }
    call({id:castle.id,op:'cancel',context:'201'});
    if(!(await scoped('201','profile',[],response('stale'))).error)throw new Error('cancelled call still resumed');
    if((await scoped('202','profile',[],response('second profile'))).value?.error!=='second profile')throw new Error('cancelling sibling cancelled survivor');
    const scopedAbandoned=call({id:castle.id,op:'answer',context:'301',source:'login',args:same,store,grants});
    const survivor=call({id:castle.id,op:'answer',context:'302',source:'login',args:same,store,grants});
    call({id:castle.id,op:'cancel',context:'301'});
    let cancelled=false;try{await run(scopedAbandoned.continuation);}catch{cancelled=true;}
    if(!cancelled||(await checkpoint(survivor)).tag!==1)throw new Error('checkpoint cancellation lost sibling');
    if((await scoped('302','login',same,response('{"data":{"loginV2":{"token":"live","username":"live"}}}'))).value?.username!=='live')throw new Error('surviving queued call lost');
    }
    castle.dispose();
    const NativeWorker=globalThis.Worker;
    let workersCreated=0, workersTerminated=0;
    globalThis.Worker=class extends NativeWorker {
      constructor(...args){super(...args);workersCreated++;}
      terminate(){workersTerminated++;return super.terminate();}
    };
    const storageIdentity={appId:'dev.exact.storage-test',grants:'fs.read app:/data\nfs.write app:/data\nsqlite.open app:/data/notes.db\nnet.fetch https://example.test\nsecret.keep session\n'};
    const beforeStorage=(await indexedDB.databases()).length;
    let storage=await prepare(await payload(fixtures.storage,storageIdentity),storageIdentity);
    if((await indexedDB.databases()).length!==beforeStorage)throw new Error('prepare opened storage');
    const storageCall=async(op,value='')=>{
      const result=await invoke(storage,'work',[op,value],[],['session']);
      if(result.tag!==0)throw new Error(`storage ${op}: ${JSON.stringify(result)}`);
      if(result.externalRead!==true||result.reads.length!==0)throw new Error('storage dependency must not invent a secret read');
      return result.value.text;
    };
    if(await storageCall('file','hello')!=='hello')throw new Error('file bytes');
    if(await storageCall('add','remember')!=='remember')throw new Error('prepared insert');
    if(await storageCall('rollback','discard')!=='remember')throw new Error('transaction rollback');
    if(await storageCall('refused')!=='denied')throw new Error('filesystem grant');
    if(await storageCall('types')!=='9223372036854775807/-9223372036854775808/1.25/0,255')throw new Error('SQLite value parity');
    if(await storageCall('sql-refusals')!=='Unavailable/Unavailable/Unavailable/Unavailable')throw new Error('SQLite policy parity');
    const fetchStart=await invoke(storage,'work',['fetch','a'],[],['session']);
    if(fetchStart.request?.url!=='https://example.test/a')throw new Error('storage to fetch continuation');
    const fetchEnd=await invoke(storage,'work',['fetch','a'],[],['session'],response('reply'));
    if(fetchEnd.value?.text!=='a:reply'||fetchEnd.writes[0]?.[1]!=='a')throw new Error(`fetch to storage continuation: ${JSON.stringify(fetchEnd)}`);
    const paired=await Promise.all(['b','c'].map(value=>invoke(storage,'work',['fetch',value],[],['session'])));
    for(let i=0;i<paired.length;i++)if(paired[i].request?.url!=='https://example.test/'+['b','c'][i])throw new Error('interleaved storage request attribution');
    const replies=await Promise.all(['c','b'].map(value=>invoke(storage,'work',['fetch',value],[],['session'],response(value))));
    for(let i=0;i<replies.length;i++)if(replies[i].value?.text!==['c:c','b:b'][i]||replies[i].writes[0]?.[1]!==['c','b'][i])throw new Error('interleaved storage result attribution');
    const workerCount=workersCreated;
    const replacement=await prepare(await payload(fixtures.storage,storageIdentity),storageIdentity);
    storage.dispose();storage=replacement;

    if(await storageCall('read')!=='hello'||await storageCall('list')!=='remember')throw new Error('storage reload persistence');
    if(workersCreated!==workerCount)throw new Error('overlapping realm replaced SQLite worker');
    storage.dispose();
    const secondIdentity={...storageIdentity,appId:'dev.exact.storage-other'};
    storage=await prepare(await payload(fixtures.storage.replaceAll(storageIdentity.appId,secondIdentity.appId),secondIdentity),secondIdentity);
    if(await storageCall('list')!=='')throw new Error('app storage isolation');
    storage.dispose();
    history.replaceState(null,'','/?agent=1');
    storage=await prepare(await payload(fixtures.storage,storageIdentity),storageIdentity);
    if(!(await storageCall('bake')).includes('unavailable in agent mode'))throw new Error('agent mode disk access');
    storage.dispose();history.replaceState(null,'','/');
    const abandonedSource=`let invocation=0;globalThis.exact={abi:1,appId:'dev.exact.storage-test',grants:${JSON.stringify(storageIdentity.grants)},answer(source,args,store,storage){
      if(invocation++===0){void storage.fs.stat('app:/data').then(()=>store.set('session','orphan'));throw new Error('abandoned');}
      return storage.fs.stat('app:/data').then(()=>({text:'current'}));}};`;
    const abandoned=await prepare(await payload(abandonedSource,storageIdentity),storageIdentity);
    const first=await invoke(abandoned,'work',[],[],['session']);
    const next=await invoke(abandoned,'work',[],[],['session']);
    if(first.tag!==2||next.value?.text!=='current'||next.writes.length)throw new Error('abandoned completion entered another invocation');
    abandoned.dispose();
    const abandonedOpenSource=`let invocation=0;globalThis.exact={abi:1,appId:'dev.exact.storage-test',grants:${JSON.stringify(storageIdentity.grants)},answer(source,args,store,storage){
      if(invocation++===1){void storage.sqlite.open('app:/data/notes.db').then(db=>{store.set('session','orphan');return db.close();});throw new Error('abandoned open');}
      return storage.sqlite.open('app:/data/notes.db').then(db=>db.close()).then(()=>({text:'current'}),()=>({text:'busy'}));}};`;
    const abandonedOpen=await prepare(await payload(abandonedOpenSource,storageIdentity),storageIdentity);
    await invoke(abandonedOpen,'work',[],[],['session']);
    if((await invoke(abandonedOpen,'work',[],[],['session'])).tag!==2)throw new Error('open abandonment fixture');
    let released=false;
    for(let attempt=0;attempt<20&&!released;attempt++){
      const result=await invoke(abandonedOpen,'work',[],[],['session']);
      if(result.writes.length)throw new Error('abandoned open ran app callback');
      released=result.value?.text==='current';
    }
    if(!released)throw new Error('abandoned open kept its database locked');
    abandonedOpen.dispose();
    const {createFileSystem}=await import('/storage-fs.js');
    const {createSqlite}=await import('/storage-sqlite.js');
    const fsApp='test.browser.file-operations', fsGrants='fs.read app:/\nfs.write app:/\nsqlite.open app:/data';
    const fs=createFileSystem(fsApp,fsGrants), sql=createSqlite(fsApp,fsGrants);
    const refused=async promise=>{try{await promise;}catch(e){if(e.kind!=='Unavailable')throw e;return;}throw new Error('operation should refuse');};
    await fs.mkdir('app:/data/dir');
    await fs.writeFile('app:/data/dir/a',new Uint8Array([1,2]));
    await Promise.all(Array.from({length:8},()=>fs.appendFile('app:/data/dir/a',new Uint8Array([3]))));
    if((await fs.stat('app:/data/dir/a')).size!==10)throw new Error('concurrent append lost bytes');
    await fs.copyFile('app:/data/dir/a','app:/cache/copy');
    await fs.rename('app:/data/dir','app:/data/moved');
    if((await fs.readdir('app:/data/moved')).join()!=='a'||await fs.realpath('app:/data//moved/a/')!=='app:/data/moved/a')throw new Error('directory rename/canonical path');
    for(const path of ['app:/data/../cache/escape','app:/database/escape','/tmp/escape','app:/data/./escape'])await refused(fs.writeFile(path,new Uint8Array([1])));
    const narrow=createFileSystem(fsApp,'fs.read app:/data/move\nfs.write app:/data/move');
    await refused(narrow.readFile('app:/data/moved/a'));narrow.dispose();
    await fs.rm('app:/data/moved');await fs.rm('app:/data/missing');
    if((await fs.readdir('app:/data')).length)throw new Error('recursive removal');
    const db=await sql.open('app:/data/live.db');
    await db.execute('CREATE TABLE t (value INTEGER)');
    await db.execute('INSERT INTO t VALUES (?)',[42n]);
    await refused(fs.atomicWriteFile('app:/data/live.db',new Uint8Array([0])));
    await refused(fs.rm('app:/data'));
    await fs.writeFile('app:/cache/unrelated',new Uint8Array([1]));
    const secondSql=createSqlite(fsApp,fsGrants);await refused(secondSql.open('app:/data/live.db'));secondSql.dispose();
    await db.close();
    await fs.copyFile('app:/data/live.db','app:/data/copied.db');
    if(!new TextDecoder().decode(await fs.readFile('app:/data/copied.db')).startsWith('SQLite format 3'))throw new Error('database is not a SQLite file');
    const copied=await sql.open('app:/data/copied.db');
    if((await copied.query('SELECT value FROM t')).rows[0][0]!==42n)throw new Error('copied database contents');
    const prepared=await copied.prepare('SELECT value FROM t');await copied.close();
    await refused(prepared.query());
    const reloadApp='test.browser.sqlite-overlap';
    let current=createSqlite(reloadApp,fsGrants);
    const coldStart=performance.now();
    let live=await current.open('app:/data/reload.db');
    const coldMs=performance.now()-coldStart;
    await live.execute('CREATE TABLE t (value INTEGER)');
    await live.execute('INSERT INTO t VALUES (?)',[91n]);
    let oldStatement=await live.prepare('SELECT value FROM t');
    const warmMs=[], expectedWorkers=workersCreated;
    for(let i=0;i<5;i++){
      const next=createSqlite(reloadApp,fsGrants);
      await refused(next.open('app:/data/reload.db'));
      const other=await next.open('app:/data/other.db');
      await other.close();
      const late=live.query('SELECT value FROM t');
      const start=performance.now();current.dispose();
      await refused(late);await refused(live.query('SELECT value FROM t'));await refused(oldStatement.query());
      live=await next.open('app:/data/reload.db');
      if((await live.query('SELECT value FROM t')).rows[0][0]!==91n)throw new Error('warm reload lost SQLite data');
      warmMs.push(performance.now()-start);
      oldStatement=await live.prepare('SELECT value FROM t');current=next;
    }
    if(workersCreated!==expectedWorkers)throw new Error('same app overlap spawned workers');
    current.dispose();
    if(workersCreated!==workersTerminated+1)throw new Error('last owner retained a SQLite worker');
    sql.dispose();fs.dispose();
    if(workersCreated!==workersTerminated)throw new Error('SQLite worker leaked');
    globalThis.Worker=NativeWorker;
    const caltrainIdentity={appId:'com.exact.caltrain',grants:''};
    const train=await prepare(await payload(fixtures.caltrain,caltrainIdentity),caltrainIdentity);
    const canonical=v=>JSON.stringify(v,(_key,value)=>value&&typeof value==='object'&&!Array.isArray(value)?Object.fromEntries(Object.entries(value).sort(([a],[b])=>a.localeCompare(b))):value);
    for(const test of fixtures.oracle){
      const result=await invoke(train,test.source,test.args);
      const actual=result.tag===0?{tag:0,value:result.value}:{tag:result.tag,kind:result.kind,message:result.message};
      if(canonical(actual)!==canonical(test.expected))throw new Error(`Caltrain parity ${test.source}: ${JSON.stringify({actual,expected:test.expected})}`);
    }
    train.dispose();
    return {scopedWasm:true,scopedContexts:true,guards:forms.length,caltrain:fixtures.oracle.length,store:true,isolated:true,refusals:true,async:true,storage:true,sqliteReload:{coldMs,warmMs}};
  };
  const result=await call('Runtime.evaluate',{expression:`(${probe.toString()})(${JSON.stringify(fixtures)})`,returnByValue:true,awaitPromise:true});
  assert.equal(result.exceptionDetails,undefined,JSON.stringify(result.exceptionDetails));
  console.log(JSON.stringify(result.result.value.sqliteReload));delete result.result.value.sqliteReload;
  assert.deepEqual(result.result.value,{scopedWasm:true,scopedContexts:true,guards:25,caltrain:20,store:true,isolated:true,refusals:true,async:true,storage:true});
  await call('Page.reload');
  const persisted=await call('Runtime.evaluate',{expression:`(async()=>{
    await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));
    const {createFileSystem}=await import('/storage-fs.js');
    const {createSqlite}=await import('/storage-sqlite.js');
    const fs=createFileSystem('dev.exact.storage-test','fs.read app:/data');
    const sql=createSqlite('dev.exact.storage-test','sqlite.open app:/data/notes.db');
    const bytes=new TextDecoder().decode(await fs.readFile('app:/data/note'));
    const db=await sql.open('app:/data/notes.db');
    const rows=await db.query('SELECT body FROM notes');await db.close();sql.dispose();fs.dispose();
    return bytes==='hello'&&rows.rows[0][0]==='remember';
  })()`,returnByValue:true,awaitPromise:true});
  assert.equal(persisted.exceptionDetails,undefined,JSON.stringify(persisted.exceptionDetails));
  assert.equal(persisted.result.value,true,'storage survives full page reload');
  console.log(JSON.stringify(result.result.value));
  const evaluate=async expression=>{
    const result=await call('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});
    assert.equal(result.exceptionDetails,undefined,JSON.stringify(result.exceptionDetails));
    return result.result.value;
  };
  const frames='await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))';
  const click=async id=>{
    const point=await evaluate(`(()=>{const r=document.getElementById(${JSON.stringify(id)}).getBoundingClientRect();return {x:r.x+20,y:r.y+20};})()`);
    await call('Input.dispatchMouseEvent',{type:'mousePressed',button:'left',clickCount:1,...point});
    await call('Input.dispatchMouseEvent',{type:'mouseReleased',button:'left',clickCount:1,...point});
  };
  const scroll=async()=>{
    await call('Input.dispatchMouseEvent',{type:'mouseWheel',x:350,y:200,deltaX:0,deltaY:320});
    const moved=await evaluate(`(async()=>{for(let i=0;i<30;i++){${frames};if(document.getElementById('scroller').scrollTop>0)return true;}return false;})()`);
    assert.equal(moved,true,'baked page accepts wheel scrolling while module is unavailable');
    await evaluate(`document.getElementById('scroller').scrollTop=0`);
    await evaluate(`(async()=>{${frames};})()`);
  };
  const rangeKeyboard=async()=>{
    await evaluate(`document.getElementById('range').focus()`);
    await call('Input.dispatchKeyEvent',{type:'keyDown',key:'ArrowRight',code:'ArrowRight',windowsVirtualKeyCode:39});
    await call('Input.dispatchKeyEvent',{type:'keyUp',key:'ArrowRight',code:'ArrowRight',windowsVirtualKeyCode:39});
  };
  const blockedRange=async()=>{
    await rangeKeyboard();
    assert.equal(await evaluate(`document.getElementById('range').value`),'50','unready range rejects native keyboard editing');
    await click('range');
    assert.equal(await evaluate(`document.getElementById('range').value`),'50','unready range rejects native pointer editing');
  };
  for(const fails of [false,true]){
    await call('Page.navigate',{url:`http://127.0.0.1:${server.address().port}/startup${fails?'?fail=1':''}`});
    assert.equal(await evaluate(`(async()=>{for(let i=0;i<120;i++){${frames};if(document.getElementById('exact-root').dataset.bootMs&&globalThis.exact.moduleRuntime)return true;}return false;})()`),true,'real glue paints before the controlled module is released');
    assert.equal(await evaluate('startup.activated'),false);
    await scroll();
    await click('action');
    await click('editor');
    await call('Input.insertText',{text:'early'});
    await blockedRange();
    assert.deepEqual(await evaluate(`({dispatch:startup.dispatch,value:document.getElementById('editor').value})`),{dispatch:[],value:'baked'},'unready app neither dispatches nor edits baked input');
    await evaluate('startup.release()');
    assert.equal(await evaluate(`(async()=>{for(let i=0;i<120;i++){${frames};const root=document.getElementById('exact-root');if(root.dataset.${fails?'error':'moduleReady'})return true;}return false;})()`),true,'module release settles readiness');
    if(fails){
      await scroll();
      await click('action');await click('editor');await call('Input.insertText',{text:'failed'});
      await blockedRange();
      assert.deepEqual(await evaluate(`({active:startup.activated,dispatch:startup.dispatch,value:document.getElementById('editor').value})`),{active:false,dispatch:[],value:'baked'},'failed module remains gated without disabling scrolling');
    }else{
      await click('action');await click('editor');await call('Input.insertText',{text:'ready'});
      const active=await evaluate(`({dispatch:startup.dispatch,value:document.getElementById('editor').value,disabled:document.getElementById('disabled').disabled})`);
      assert.equal(active.dispatch.some(event=>event.id===2&&event.kind===0),true,'ready button dispatches');
      assert.equal(active.dispatch.some(event=>event.id===3&&event.kind===1),true,'ready input dispatches edits');
      assert.equal(active.value.includes('ready'),true);assert.equal(active.disabled,true,'authored disabled state survives activation');
      assert.deepEqual(await evaluate(`['disabled-on-activation','enabled-on-activation'].map(id=>document.getElementById(id).disabled)`),[true,false],'activation prop changes override originally authored disabled state');
      await rangeKeyboard();
      assert.equal(await evaluate(`document.getElementById('range').value`),'51','activated range accepts native keyboard editing');
      await click('range');
      assert.equal(await evaluate(`Number(document.getElementById('range').value)<51`),true,'activated range accepts native pointer editing');
      assert.equal(await evaluate(`startup.dispatch.filter(event=>event.id===6&&event.kind===1).length`),2,'keyboard and pointer range edits both dispatch');
      await click('enabled-on-activation');
      assert.equal(await evaluate(`startup.dispatch.some(event=>event.id===8&&event.kind===0)`),true,'activation can enable an originally disabled button');
      const count=await evaluate('startup.dispatch.length');await click('disabled');await click('disabled-on-activation');
      assert.equal(await evaluate('startup.dispatch.length'),count,'authored disabled button stays noninteractive');
    }
  }
  console.log('startup: private iframe layout, pre-activation scroll, input gating, successful and failed activation');
} finally {
  process.kill(-child.pid,'SIGKILL');await exited;server.closeAllConnections();await new Promise(r=>server.close(r));rmSync(wasmDir,{recursive:true,force:true});rmSync(profile,{recursive:true,force:true,maxRetries:3,retryDelay:100});
}
"#;
