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
            "EXACT_PARITY",
            serde_json::to_string(&caltrain::oracle()).unwrap(),
        )
        .current_dir(root)
        .output()
        .unwrap();
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
import { readFileSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import assert from 'node:assert/strict';
const routes = {'/module-glue.js':'host/web/module-glue.js','/module-prelude.js':'js/src/prelude.js'};
const fixtures=Object.fromEntries(['inputs','castle','caltrain','ambient-init','storage'].map(name=>[name,execFileSync('./node_modules/.bin/rolldown',[`js/tests/fixtures/${name}.ts`,'--format','iife'],{encoding:'utf8',stdio:['ignore','pipe','pipe']})]));
fixtures.oracle=JSON.parse(process.env.EXACT_PARITY);
const server = createServer((req,res)=>{
  res.setHeader('content-type', routes[req.url] ? 'text/javascript' : 'text/html');
  res.end(routes[req.url] ? readFileSync(routes[req.url]) : '<script>globalThis.exact={}</script>');
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
    const oldDate=Date, oldNow=Date.now, oldRandom=Math.random;
    const guest=document.createElement('iframe');document.body.append(guest);
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
    castle.dispose();
    const storageIdentity={appId:'dev.exact.storage-test',grants:'fs.read app:/data\nfs.write app:/data\nsqlite.open app:/data/notes.db\nnet.fetch https://example.test\nsecret.keep session\n'};
    const storage=await prepare(await payload(fixtures.storage,storageIdentity),storageIdentity);
    const unavailable=await invoke(storage,'work',['bake','']);
    if(unavailable.value?.text!=='Unavailable:storage is unsupported by this host'||unavailable.writes.length||unavailable.request)throw new Error(`browser storage refusal: ${JSON.stringify(unavailable)}`);
    storage.dispose();
    const caltrainIdentity={appId:'com.exact.caltrain',grants:''};
    const train=await prepare(await payload(fixtures.caltrain,caltrainIdentity),caltrainIdentity);
    const canonical=v=>JSON.stringify(v,(_key,value)=>value&&typeof value==='object'&&!Array.isArray(value)?Object.fromEntries(Object.entries(value).sort(([a],[b])=>a.localeCompare(b))):value);
    for(const test of fixtures.oracle){
      const result=await invoke(train,test.source,test.args);
      const actual=result.tag===0?{tag:0,value:result.value}:{tag:result.tag,kind:result.kind,message:result.message};
      if(canonical(actual)!==canonical(test.expected))throw new Error(`Caltrain parity ${test.source}: ${JSON.stringify({actual,expected:test.expected})}`);
    }
    train.dispose();
    return {guards:forms.length,caltrain:fixtures.oracle.length,store:true,isolated:true,refusals:true,async:true,storage:true};
  };
  const result=await call('Runtime.evaluate',{expression:`(${probe.toString()})(${JSON.stringify(fixtures)})`,returnByValue:true,awaitPromise:true});
  assert.equal(result.exceptionDetails,undefined,JSON.stringify(result.exceptionDetails));
  assert.deepEqual(result.result.value,{guards:25,caltrain:20,store:true,isolated:true,refusals:true,async:true,storage:true});
  console.log(JSON.stringify(result.result.value));
} finally {
  process.kill(-child.pid,'SIGKILL');await exited;server.closeAllConnections();await new Promise(r=>server.close(r));rmSync(profile,{recursive:true,force:true,maxRetries:3,retryDelay:100});
}
"#;
