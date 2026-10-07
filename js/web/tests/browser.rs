//! Real Chrome coverage of the private module realm, not a simulated DOM.
use std::path::Path;
use std::process::Command;
#[path = "support/caltrain.rs"]
mod caltrain;

#[test]
fn browser_fixture_serves_every_on_demand_host_module_without_chrome() {
    let result = Command::new("bun")
        .args(["--input-type=module", "-e", PROBE])
        .env("EXACT_MODULE_ROUTES_ONLY", "1")
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn browser_modules_guard_their_own_builtins_and_refuse_bad_candidates() {
    let chrome = std::env::var("CHROME")
        .unwrap_or_else(|_| "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".into());
    if !Path::new(&chrome).exists() {
        eprintln!("browser module sweep unavailable: set CHROME");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ec_pair =
        exact_data::crypto::generate_p256(&exact_runner::Store::new("", []), true).unwrap();
    let result = Command::new("bun")
        .args(["--input-type=module", "-e", PROBE])
        .env("CHROME", chrome)
        .env(
            "EXACT_PARITY",
            serde_json::to_string(&caltrain::oracle()).unwrap(),
        )
        .env("EXACT_DIGESTS", expected_digests())
        .env("EXACT_AGENT_STREAM", agent_stream())
        .env("EXACT_EC_JWK", ec_pair.private.to_jwk().unwrap().to_json())
        .env("EXACT_GRANT_SETS", grant_sets())
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
    check_ecdsa(&String::from_utf8_lossy(&result.stdout), &ec_pair);
}

/// LLP 1069.005 D1b: what Chrome's WebCrypto signed in each placement,
/// verified here by Rust's P-256 (the reference Hermes is held to as well):
/// a key Chrome generated exports a JWK Rust imports and verifies under;
/// Rust's key imported in Chrome signs what Rust verifies; a kept pair
/// signs after a new realm read it back from IndexedDB.
fn check_ecdsa(stdout: &str, rust: &exact_data::crypto::EcKeyPair) {
    use exact_data::crypto::{EcKey, Jwk};
    let line = stdout
        .lines()
        .find_map(|l| l.strip_prefix("ECDSA_RESULT "))
        .expect("the probe reports its ECDSA results");
    let all: serde_json::Value = serde_json::from_str(line).unwrap();
    let unhex = |h: &serde_json::Value| -> Vec<u8> {
        let h = h.as_str().unwrap();
        (0..h.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap())
            .collect()
    };
    let key = |v: &serde_json::Value| {
        EcKey::from_jwk(&Jwk::from_json(&v.to_string()).unwrap(), true).unwrap()
    };
    for placement in ["main", "worker"] {
        let r = &all[placement];
        let pair = &r["keypair"];
        assert_eq!(
            pair["shape"], "private true sign public verify P-256 [object CryptoKey] true",
            "{placement}"
        );
        let public = key(&pair["pub"]);
        assert_eq!(
            key(&pair["priv"]).public_key().to_jwk().unwrap(),
            public.to_jwk().unwrap()
        );
        assert!(
            public.verify(b"proof", &unhex(&pair["signature"])),
            "{placement}: Chrome's signature"
        );
        assert!(
            rust.public.verify(b"imported", &unhex(&r["imported"])),
            "{placement}: Rust's key in Chrome"
        );
        let kept = key(&r["kept"]["pub"]);
        assert_eq!(r["kept"]["extractable"], false);
        assert!(
            kept.verify(b"kept", &unhex(&r["kept"]["signature"])),
            "{placement}: kept"
        );
        assert!(
            kept.verify(b"kept", &unhex(&r["later"])),
            "{placement}: kept across realms"
        );
    }
}

/// The entropy fixture's `digests`, as `js/tests/it/entropy.rs` computes
/// them for Hermes (LLP 1069.005 D1): each SHA-2 size over the empty string,
/// `abc` and 1 MiB of `i % 251`, then SHA-256 of `abc` twice more.
fn expected_digests() -> String {
    use sha2::Digest;
    let large: Vec<u8> = (0..1usize << 20).map(|i| (i % 251) as u8).collect();
    let inputs: [&[u8]; 3] = [b"", b"abc", &large];
    let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let mut lines: Vec<String> = Vec::new();
    lines.extend(inputs.iter().map(|i| hex(&sha2::Sha256::digest(i))));
    lines.extend(inputs.iter().map(|i| hex(&sha2::Sha384::digest(i))));
    lines.extend(inputs.iter().map(|i| hex(&sha2::Sha512::digest(i))));
    lines.push(hex(&sha2::Sha256::digest(b"abc")));
    lines.push(hex(&sha2::Sha256::digest(b"abc")));
    lines.join("\n")
}

/// What a realm under the agent with seed 1 answers to `uuid`, `bytes(20)`,
/// `uuid` (LLP 1069.005 D2b): the Rust stream's first 52 bytes, as Hermes
/// draws them.
fn agent_stream() -> String {
    let mut stream = exact_data::crypto::AgentStream::new(1, "typescript");
    let mut bytes = [0u8; 52];
    stream.fill(&mut bytes);
    let middle: Vec<String> = bytes[16..36].iter().map(u8::to_string).collect();
    format!(
        "{} {} {}",
        exact_data::crypto::format_uuid(bytes[..16].try_into().unwrap()),
        middle.join(","),
        exact_data::crypto::format_uuid(bytes[36..].try_into().unwrap())
    )
}

fn grant_sets() -> String {
    let specs = [
        "",
        "secret.keep token",
        "net.fetch https://fixture.exact.test\n",
        "secret.keep dpop\n",
        "net.fetch https://api.castle.xyz\nsecret.keep castle.session\n",
        "fs.read app:/data\nfs.write app:/data\nsqlite.open app:/data/notes.db\nnet.fetch https://example.test\nsecret.keep session\n",
        "fs.read app:/\nfs.write app:/\nsqlite.open app:/data",
        "fs.read app:/data/move\nfs.write app:/data/move",
        "fs.read app:/data",
        "sqlite.open app:/data/notes.db",
        "sqlite.open app:/data/fieldnotes.db\nfs.read app:/data/backups\nfs.write app:/data/backups\nfs.read app:/tmp/picked\nsecret.keep fieldnotes.revision",
    ];
    let sets = specs
        .into_iter()
        .map(|spec| {
            (
                spec.to_string(),
                serde_json::from_str::<serde_json::Value>(&exact_runner::grants::normalized_json(
                    spec,
                ))
                .unwrap(),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    serde_json::Value::Object(sets).to_string()
}

const PROBE: &str = r#"
import { Cdp } from './scripts/agent.mjs';
import { webHostFiles } from './scripts/app.mjs';
import { spawn, execFileSync } from 'node:child_process';
import { createServer } from 'node:http';
import { readFileSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';
const routes = Object.fromEntries(['/','/startup/'].flatMap(prefix=>
  Object.entries(webHostFiles()).map(([name,source])=>[prefix+name,source])));
const hostPage=readFileSync('host/web/index.html','utf8');
const modulePage=hostPage.replace(/<script type="module" src="\.\/glue\.js"><\/script>/,'');
const startupStub=()=>{
  const memory=new WebAssembly.Memory({initial:2});
  const out=value=>{const bytes=new TextEncoder().encode(JSON.stringify(value));new Uint8Array(memory.buffer,65536,bytes.length).set(bytes);return bytes.length;};
  const rustOnly=new URL(location.href).searchParams.has('rust');
  globalThis.startup={dispatch:[],activations:0,activated:false,painted:false,storageRuns:0,release:null};
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
  if(rustOnly)batch.ops.push({op:'grants',lines:['fs.write app:/data']},{op:'storage',ticket:9,payload:'{"version":1,"op":"fs.mkdir","args":{"path":"app:/data/backup"}}',scope:null});
  WebAssembly.instantiateStreaming=async response=>{
    await response;
    if(new URL(location.href).searchParams.has('early'))queueMicrotask(()=>{globalThis.earlyReload=globalThis.exact.reload(new Uint8Array([1]));});
    // A real result carries its module; the glue reads the module's custom sections.
    return {module:await WebAssembly.compile(new Uint8Array([0,97,115,109,1,0,0,0])),instance:{exports:{memory,exact_out:()=>65536,exact_in:()=>0,
      exact_compat:()=>out({inputs:{app:'test.startup'}}),exact_logic:()=>out(rustOnly?null:{appId:'test.startup',grants:''}),
      ...(rustOnly?{}:{exact_module_artifact:()=>0}),
      exact_plan:()=>out([]),exact_plan_fonts:()=>out([]),exact_boot:()=>out(batch),exact_boot_plan:()=>out(batch),
      exact_agent:()=>out({nodes:[]}),
      exact_data_ready:()=>{startup.activations++;startup.activated=true;startup.painted=!!document.getElementById('exact-root').dataset.frameCallbackMs;return out({ops:[
        {op:'props',id:7,set:{disabled:'true'},clear:[]},
        {op:'props',id:8,set:{},clear:['disabled']}
      ]});},
      exact_fulfill:()=>out({ops:[]}),
      exact_dispatch:(id,kind,length)=>{startup.dispatch.push({id,kind,value:new TextDecoder().decode(new Uint8Array(memory.buffer,0,length))});return out({ops:[]});}
    }}};
  };
};
const startupPage=hostPage.replace('<script type="module" src="./glue.js"></script>',`<script>(${startupStub.toString()})()</script><script type="module" src="/startup/glue.js"></script>`);
const server = createServer((req,res)=>{
  const path=new URL(req.url,'http://fixture.invalid').pathname;
  // The dev protocol's SHA-256 for a page with no `crypto.subtle` (LLP 1069.005 D1).
  if(path==='/sha256'&&req.method==='POST'){
    const hash=createHash('sha256');req.on('data',chunk=>hash.update(chunk));
    req.on('end',()=>{res.setHeader('content-type','text/plain');res.end(hash.digest('hex'));});return;
  }
  if(path==='/startup/storage-request.js'){
    res.setHeader('content-type','text/javascript');
    res.end(`startup.storageBeforePaint=!document.getElementById('exact-root').dataset.frameCallbackMs;globalThis.exact.createStorageRequests=(app,grants)=>({run:async()=>{startup.storageRuns++;return new Uint8Array();},dispose(){}});`);return;
  }
  if(path==='/startup/module-glue.js'){
    res.setHeader('content-type','text/javascript');
    res.end(`globalThis.exact.moduleRuntime={baked:async()=>{await globalThis.startupGate;if(location.search.includes('fail'))throw new Error('controlled loader failure');return {};},prepare:async()=>({id:0,dispose(){}})};`);return;
  }
  if(!routes[path]&&!['/','/startup','/startup/app.wasm'].includes(path)){res.writeHead(404);res.end();return;}
  res.setHeader('content-type', path.endsWith('.wasm') ? 'application/wasm' : routes[path] ? 'text/javascript' : 'text/html');
  res.end(routes[path] ? readFileSync(routes[path]) : path==='/startup/app.wasm' ? '' : path==='/startup' ? startupPage : modulePage);
});
await new Promise(r=>server.listen(0,'127.0.0.1',r));
// @ref LLP 1043.000 §3 D7/D8 — exercise the actual fixture server without Chrome.
if(process.env.EXACT_MODULE_ROUTES_ONLY==='1'){
  try {
    const glue=readFileSync('host/web/glue.js','utf8');
    const requested=[...glue.matchAll(/loadAfterPaint\(['"]\.\/([^'"]+)['"]/g)].map(match=>match[1]);
    assert(requested.length>0,'negative control: the loader inventory cannot be empty');
    const problems=[];
    for(const name of new Set([...requested,...Object.keys(webHostFiles())]))for(const prefix of ['/','/startup/']){
      const path=prefix+name;
      const response=await fetch(`http://127.0.0.1:${server.address().port}${path}`);
      const body=Buffer.from(await response.arrayBuffer());
      if(response.status!==200||response.headers.get('content-type')!==(name.endsWith('.wasm')?'application/wasm':'text/javascript'))problems.push(`${path}: ${response.status} ${response.headers.get('content-type')}`);
      else if(!['/startup/module-glue.js','/startup/storage-request.js'].includes(path)&&!body.equals(readFileSync(webHostFiles()[name]??'host/web/'+name)))problems.push(`${path}: wrong module bytes`);
    }
    assert.deepEqual(problems,[],'every on-demand host module is served as JavaScript');
    assert.equal((await fetch(`http://127.0.0.1:${server.address().port}/startup/missing-glue.js`)).status,404,'missing modules never masquerade as HTML');
  } finally {server.closeAllConnections();await new Promise(resolve=>server.close(resolve));}
  process.exit(0);
}
const fixtures=Object.fromEntries(['inputs','castle','caltrain','ambient-init','storage','entropy','ecdsa'].map(name=>[name,execFileSync(process.execPath,['./node_modules/.bin/rolldown',`js/tests/fixtures/${name}.ts`,'--format','iife'],{encoding:'utf8',stdio:['ignore','pipe','pipe']})]));
fixtures.oracle=JSON.parse(process.env.EXACT_PARITY);
fixtures.digests=process.env.EXACT_DIGESTS;
fixtures.agentStream=process.env.EXACT_AGENT_STREAM;
fixtures.ecJwk=process.env.EXACT_EC_JWK;
fixtures.grantSets=JSON.parse(process.env.EXACT_GRANT_SETS);
const profile = mkdtempSync(resolve(tmpdir(),'exact-module-browser-'));
const child = spawn(process.env.CHROME, ['--headless=new','--no-sandbox','--remote-debugging-pipe','--no-first-run','--disable-background-networking','--host-resolver-rules=MAP lan.test 127.0.0.1',`--user-data-dir=${profile}`,'about:blank'],{detached:true,stdio:['ignore','ignore','ignore','pipe','pipe']});
const cdp = new Cdp(child.stdio[3],child.stdio[4]);
const exited = new Promise(r=>child.on('exit',()=>{cdp.fail('browser closed');r();}));
try {
  const { targetInfos } = await cdp.send('Target.getTargets');
  const page=targetInfos.find(t=>t.type==='page')??await cdp.send('Target.createTarget',{url:'about:blank'});
  const { sessionId } = await cdp.send('Target.attachToTarget',{targetId:page.targetId,flatten:true});
  const call = (method,params)=>cdp.send(method,params,sessionId);
  await call('Page.enable');
  await call('Page.navigate',{url:`http://127.0.0.1:${server.address().port}/`});
  const probe = async (fixtures) => {
    await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));
    globalThis.exact ??= {};
    const {prepare,call,run,baked} = await import('/module-glue.js');
    const {createGrantSet}=await import('/grant-admission.js');
    const grantSet=grants=>createGrantSet(fixtures.grantSets[grants]);
    const admit=value=>({...value,grantSet:grantSet(value.grants)});
    const checkpoint=async result=>{for(let i=0;result.continuation&&i<20;i++)result=await run(result.continuation);return result;};
    const oldDate=Date, oldNow=Date.now, oldRandom=Math.random;
    const guest=document.createElement('iframe');document.getElementById('exact-root').append(guest);
    const guestBox=guest.getBoundingClientRect(), pageHeight=document.documentElement.scrollHeight;
    if(guestBox.width!==300||guestBox.height!==150)throw new Error('guest iframe lost its 300x150 box');
    const guestDate=guest.contentWindow.Date;
    const identity=admit({appId:'test.browser.module',grants:'secret.keep token'});
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
    const embedded = await payload(source), memory = new WebAssembly.Memory({initial:1});
    const fetched = [], fetchBefore = globalThis.fetch;
    globalThis.fetch = (...args) => { fetched.push(String(args[0])); return fetchBefore(...args); };
    globalThis.exact.wasm = {memory, exact_out:()=>0, exact_module_artifact(index) {
      // Real exports reuse their buffer and may grow memory. Both must leave
      // the receipt copied by the previous call intact.
      if(index===1)memory.grow(1);
      const bytes=index===0?embedded.receipt:embedded.script;
      new Uint8Array(memory.buffer).set(bytes);return bytes.length;
    }};
    const loaded = await baked();
    delete globalThis.exact.wasm;globalThis.fetch = fetchBefore;
    if(fetched.length || new TextDecoder().decode(loaded.receipt)!==new TextDecoder().decode(embedded.receipt)
      || new TextDecoder().decode(loaded.script)!==source)throw new Error('baked module was refetched or its paired bytes changed');
    const module = await prepare(loaded,identity);
    const privateFrames=[...document.querySelectorAll('iframe')].filter(frame=>frame!==guest);
    if(privateFrames.length!==1||privateFrames.some(frame=>frame.getClientRects().length))throw new Error('private module iframe participates in layout');
    if(document.documentElement.scrollHeight!==pageHeight)throw new Error('private module grew document scroll height');
    const answer=(source,args=[])=>call({op:'answer',id:module.id,source,args,store:[['token','old']],grants:['token']});
    const results=[];
    for(const name of ['alias','random','constructor','intl'])results.push(await checkpoint(answer(name)));
    if(!results.every(r=>r.message?.includes('pass time or a random seed')))throw new Error(JSON.stringify(results));
    if((await checkpoint(answer('explicit',[0]))).value!==1970)throw new Error('explicit date arithmetic changed');
    const written=await checkpoint(answer('write'));
    if(written.value!=='old'||written.reads[0]!=='token'||written.writes[0][1]!=='next')throw new Error('store seam changed');
    if(!(await checkpoint(answer('refused'))).message?.includes('not granted'))throw new Error('store grant bypass');
    if((await checkpoint(answer('async'))).value!=='later')throw new Error('Promise resolution failed');
    if(Date!==oldDate||Date.now!==oldNow||Math.random!==oldRandom||Date.now()<=0||guest.contentWindow.Date!==guestDate||guest.contentWindow.Date.now()<=0)throw new Error('page/guest globals changed');
    const count=document.querySelectorAll('iframe').length;
    for(const bad of [await payload(source.replace('appId:\'test.browser.module\'','appId:\'another.app\'')), {...await payload(source),script:encode('corrupt')}]) {
      let refused=false;try{await prepare(bad,identity);}catch{refused=true;}
      if(!refused||document.querySelectorAll('iframe').length!==count)throw new Error('bad module leaked an environment');
    }
    module.dispose();
    if(!call({op:'answer',id:module.id,source:'explicit',args:[0]}).error)throw new Error('disposed environment is callable');

    const inputsIdentity=admit({appId:'test.explicit-inputs',grants:'net.fetch https://fixture.exact.test\n'});
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
    // LLP 1069.005 D2/D3, as js/tests/it/entropy.rs holds Hermes to: a draw is
    // a counted read, refused at initialization, and no realm has `subtle`.
    const entropyIdentity=admit({appId:'test.entropy',grants:'net.fetch https://fixture.exact.test\n'});
    for(const placement of ['main','worker']){
      const realm=await prepare(await payload(fixtures.entropy,entropyIdentity),{...entropyIdentity,placement});
      const ask=(source,args=[],outcome)=>{
        const started=call({id:realm.id,op:outcome?'resume':'answer',source,args,store:[],grants:[],outcome});
        if(placement==='worker')call({op:'dispatch',id:realm.id,token:started.continuation,store:[],grants:[]});
        return checkpoint(started);
      };
      for(const form of ['uuid','bytes'])if(!(await ask('atInit',[form])).value?.includes('unavailable during module initialization; call it inside an answer'))throw new Error(`${placement}: ${form} at initialization`);
      const uuid=await ask('uuid'), later=await ask('uuidLater');
      if(!/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(uuid.value)||uuid.entropy!==true)throw new Error(`${placement}: uuid ${JSON.stringify(uuid)}`);
      if(later.request?.url!=='https://fixture.exact.test/value'||later.entropy||(await ask('uuidLater',[],response())).entropy!==true)throw new Error(`${placement}: a draw after a fetch`);
      if((await ask('bytes',[4])).value.split(',').length!==4||(await ask('plain')).entropy!==false)throw new Error(`${placement}: bytes or a plain answer`);
      if((await ask('refusals')).value!=='QuotaExceededError/TypeMismatchError/TypeError')throw new Error(`${placement}: refusals`);
      if((await ask('globals')).value!=='object/function/function/[object Crypto]/getRandomValues,randomUUID,subtle/[object SubtleCrypto]')throw new Error(`${placement}: crypto's shape`);
      // D1: digests, the same bytes as Hermes; the rest of `subtle` refuses by name.
      if((await ask('atInit',['digest'])).value!=='ran: function')throw new Error(`${placement}: a digest at initialization`);
      const digested=await ask('digests');
      if(digested.value!==fixtures.digests||digested.entropy||digested.externalRead)throw new Error(`${placement}: digests ${JSON.stringify(digested).slice(0,400)}`);
      if((await ask('digestRefusals')).value!=='NotSupportedError/NotSupportedError/TypeError/TypeError/NotSupportedError/NotSupportedError')throw new Error(`${placement}: digest refusals`);
      if((await ask('digestLater')).request?.url!=='https://fixture.exact.test/value'||(await ask('digestLater',[],response())).value!=='ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad')throw new Error(`${placement}: a digest after a fetch`);
      realm.dispose();
    }
    // D2b: under the agent (a loopback page with `?agent`) a realm draws the
    // seed's repeatable stream, the bytes Hermes draws; outside it, the OS's.
    const pageUrl=location.href;
    // The seed is the launch URL's (its navigation entry, 18d0dec29), not the
    // route's: stand in that entry as well as the location.
    const entries=performance.getEntriesByType;
    const agentRun=async(query,placement)=>{
      history.replaceState(null,'',query);
      performance.getEntriesByType=type=>type==='navigation'?[{name:new URL(query,location.origin).href}]:entries.call(performance,type);
      try{
        const realm=await prepare(await payload(fixtures.entropy,entropyIdentity),{...entropyIdentity,placement});
        const ask=(source,args=[])=>{
          const started=call({id:realm.id,op:'answer',source,args,store:[],grants:[]});
          if(placement==='worker')call({op:'dispatch',id:realm.id,token:started.continuation,store:[],grants:[]});
          return checkpoint(started);
        };
        const first=await ask('uuid'), bytes=await ask('bytes',[20]), second=await ask('uuid');
        if(!first.entropy||!bytes.entropy)throw new Error(`${placement}: an agent draw is still a read`);
        realm.dispose();
        return [first.value,bytes.value,second.value].join(' ');
      } finally {history.replaceState(null,'',pageUrl);performance.getEntriesByType=entries;}
    };
    for(const placement of ['main','worker']){
      const seeded=await agentRun('/?agent=1&seed=1',placement);
      if(seeded!==fixtures.agentStream||await agentRun('/?agent=1&seed=1',placement)!==seeded)throw new Error(`${placement}: the agent's stream ${seeded}`);
      if((await agentRun('/?agent=1&seed=2',placement)).split(' ')[0]===seeded.split(' ')[0])throw new Error(`${placement}: another seed, the same stream`);
      const [a,b]=[await agentRun('/',placement),await agentRun('/',placement)];
      if(a===b||a.split(' ')[0]===seeded.split(' ')[0])throw new Error(`${placement}: outside the agent a draw is not the OS's`);
    }

    // LLP 1069.005 D1b: ECDSA P-256 in both placements, the same checks
    // Hermes meets; the signatures go back to Rust to verify (the harness).
    const ecIdentity=admit({appId:'test.ecdsa',grants:'secret.keep dpop\n'}), ecdsa={};
    for(const placement of ['main','worker']){
      const open=async()=>prepare(await payload(fixtures.ecdsa,ecIdentity),{...ecIdentity,placement});
      const asker=realm=>(source,args=[],store=[])=>{
        const started=call({id:realm.id,op:'answer',source,args,store,grants:['dpop']});
        if(placement==='worker')call({op:'dispatch',id:realm.id,token:started.continuation,store,grants:['dpop']});
        return checkpoint(started);
      };
      const realm=await open(), ask=asker(realm);
      const init=await ask('initKey');
      if(!String(init.value).includes('unavailable during module initialization'))throw new Error(`${placement}: a key at initialization ${JSON.stringify(init)}`);
      const pair=await ask('keypair');
      if(!pair.entropy||pair.tag!==0)throw new Error(`${placement}: keypair ${JSON.stringify(pair)}`);
      const imported=await ask('importSign',[fixtures.ecJwk]);
      const trip=await ask('roundTrip',[JSON.stringify({...JSON.parse(fixtures.ecJwk),d:undefined})]);
      if(trip.entropy||!trip.value?.startsWith('EC P-256 ')||!trip.value.endsWith(' true'))throw new Error(`${placement}: import and export are pure ${JSON.stringify(trip)}`);
      const refusals=(await ask('refusals')).value;
      if(refusals!=='NotSupportedError/NotSupportedError/SyntaxError/InvalidAccessError/NotSupportedError/NotSupportedError/InvalidAccessError/NotSupportedError/NotSupportedError')throw new Error(`${placement}: refusals ${refusals}`);
      const kept=await ask('keep');
      const handle=kept.writes?.find(w=>w[0]==='dpop')?.[1];
      if(!/^exact\.key:/.test(handle??''))throw new Error(`${placement}: keepKey keeps a handle, not a key ${JSON.stringify(kept.writes)}`);
      realm.dispose();
      // A new realm over the kept secret: the pair comes back from IndexedDB.
      const again=await open(), later=await asker(again)('kept',[],[['dpop',handle]]);
      again.dispose();
      ecdsa[placement]={keypair:JSON.parse(pair.value),imported:imported.value,kept:JSON.parse(kept.value),later:later.value};
    }
    globalThis.ecdsaResult=ecdsa;
    const castleIdentity=admit({appId:'xyz.castle.test',grants:'net.fetch https://api.castle.xyz\nsecret.keep castle.session\n'});
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
    // Two targets ask one source with equal arguments: each call keeps its
    // own continuation, whatever order the replies come in.
    const twins=async (realm,invokeTwin)=>{
      const starts=[await invokeTwin('Mutation(0)'),await invokeTwin('Mutation(1)')];
      if(starts.some(r=>r.request?.url!=='https://api.castle.xyz/me'))throw new Error(`twin starts: ${JSON.stringify(starts)}`);
      const second=await invokeTwin('Mutation(1)',response('{"username":"bob"}'));
      const first=await invokeTwin('Mutation(0)',response('{"username":"ada"}'));
      if(first.request?.url!=='https://api.castle.xyz/profile/ada'||second.request?.url!=='https://api.castle.xyz/profile/bob')throw new Error(`${realm}: twin calls crossed: ${JSON.stringify([first,second])}`);
      const done=[await invokeTwin('Mutation(0)',response('ada-profile')),await invokeTwin('Mutation(1)',response('bob-profile'))];
      if(done[0].value?.username!=='ada'||done[0].value.error!=='ada-profile'||done[1].value?.username!=='bob'||done[1].value.error!=='bob-profile')throw new Error(`${realm}: twin answers crossed: ${JSON.stringify(done)}`);
    };
    await twins('iframe',(target,outcome)=>checkpoint(call({id:castle.id,op:outcome?'resume':'answer',target,source:'profile',args:[],store,grants,outcome})));
    const castleWorkerIdentity={...castleIdentity,grants:' net.fetch https://api.castle.xyz\n\n  secret.keep castle.session  ',placement:'worker'};
    const castleWorker=await prepare(await payload(fixtures.castle,castleWorkerIdentity),castleWorkerIdentity);
    await twins('worker',async (target,outcome)=>{
      const started=call({id:castleWorker.id,op:outcome?'resume':'answer',target,source:'profile',args:[],store,grants,outcome});
      call({op:'dispatch',id:castleWorker.id,token:started.continuation,store,grants});
      return await checkpoint(await run(started.continuation));
    });
    // The runner let go of a call (newer arguments replaced its request):
    // the realm drops it and keeps the one still in flight (LLP 1016 D5).
    const forgets=async (realm,invokeCall)=>{
      const login=who=>['Mutation(0)',[who,'pw']];
      for(const who of ['ada','bob'])if(!(await invokeCall(...login(who))).request)throw new Error(`${realm}: ${who} did not fetch`);
      if(call({op:'forget',id:realm==='iframe'?castle.id:castleWorker.id,inFlight:[{target:'Mutation(0)',source:'login',args:['bob','pw']}]}).ok!==true)throw new Error(`${realm}: forget refused`);
      let dropped=false;try{await invokeCall(...login('ada'),response('{"data":{"loginV2":{"token":"t","username":"ada"}}}'));}catch(e){dropped=String(e?.message??e).includes('not in flight');}
      if(!dropped)throw new Error(`${realm}: a call the runner let go still resumed`);
      const kept=await invokeCall(...login('bob'),response('{"data":{"loginV2":{"token":"t","username":"bob"}}}'));
      if(kept.value?.username!=='bob')throw new Error(`${realm}: the call in flight was lost: ${JSON.stringify(kept)}`);
    };
    await forgets('iframe',(target,args,outcome)=>checkpoint(call({id:castle.id,op:outcome?'resume':'answer',target,source:'login',args,store,grants,outcome})));
    await forgets('worker',async (target,args,outcome)=>{
      const started=call({id:castleWorker.id,op:outcome?'resume':'answer',target,source:'login',args,store,grants,outcome});
      call({op:'dispatch',id:castleWorker.id,token:started.continuation,store,grants});
      return await checkpoint(await run(started.continuation));
    });
    castleWorker.dispose();
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
    for(const expected of ['1/1','2/2']){await ask('reused');const reused=await ask('reused',[],response('ok'));if(reused.value?.error!==expected)throw new Error(`abort listeners outlive their fetches: ${JSON.stringify(reused)}`);}
    if((await ask('logout')).writes[0][1]!==null)throw new Error('forget');
    castle.dispose();
    // A worker placement (LLP 1027.002 D2): the same module, prepared on a
    // dedicated Worker; the turn's snapshot arrives at dispatch, its writes
    // and reads come back in the reply, and disposal terminates the Worker.
    const workerIdentity={...identity,placement:'worker'};
    const placed=await prepare(await payload(source.replace("return 'ok';","if(source==='where')return typeof WorkerGlobalScope==='undefined'?'page':'worker';return 'ok';")),workerIdentity);
    if(placed.placement!=='worker'||placed.frame!==null)throw new Error('worker placement prepared an iframe');
    const started=call({op:'answer',id:placed.id,source:'where',args:[],store:[],grants:['token']});
    if(!started.continuation)throw new Error('worker answer was not a turn');
    if(call({op:'dispatch',id:placed.id,token:started.continuation,store:[['token','old']],grants:['token']}).ok!==true)throw new Error('dispatch refused');
    const where=await run(started.continuation);
    if(where.value!=='worker')throw new Error(`module ran on the ${JSON.stringify(where)}`);
    const turn=call({op:'answer',id:placed.id,source:'write',args:[],store:[['token','stale']],grants:['token']});
    call({op:'dispatch',id:placed.id,token:turn.continuation,store:[['token','committed']],grants:['token']});
    const wrote=await run(turn.continuation);
    if(wrote.value!=='committed'||wrote.reads[0]!=='token'||wrote.writes[0][1]!=='next')throw new Error(`worker turn read the answer-time store: ${JSON.stringify(wrote)}`);
    const guarded=await checkpoint(call({op:'answer',id:placed.id,source:'alias',args:[],store:[],grants:[]}));
    if(!guarded.message?.includes('pass time or a random seed'))throw new Error('worker realm lost the ambient guards');
    const discarded=call({op:'answer',id:placed.id,source:'write',args:[],store:[],grants:['token']});
    call({op:'discard',id:placed.id,token:discarded.continuation});
    let gone=false;try{await run(discarded.continuation);}catch{gone=true;}
    if(!gone)throw new Error('discarded turn still ran');
    const late=call({op:'answer',id:placed.id,source:'async',args:[],store:[],grants:[]});
    placed.dispose();
    let terminated=false;try{await run(late.continuation);}catch{terminated=true;}
    if(!terminated||!call({op:'answer',id:placed.id,source:'where',args:[]}).error)throw new Error('disposed worker realm is callable');
    const NativeWorker=globalThis.Worker;
    let workersCreated=0, workersTerminated=0;
    globalThis.Worker=class extends NativeWorker {
      constructor(...args){super(...args);workersCreated++;}
      terminate(){workersTerminated++;return super.terminate();}
    };
    const storageIdentity=admit({appId:'dev.exact.storage-test',grants:'fs.read app:/data\nfs.write app:/data\nsqlite.open app:/data/notes.db\nnet.fetch https://example.test\nsecret.keep session\n'});
    const beforeStorage=(await indexedDB.databases()).length;
    let storage=await prepare(await payload(fixtures.storage,storageIdentity),storageIdentity);
    if((await indexedDB.databases()).length!==beforeStorage)throw new Error('prepare opened storage');
    if(performance.getEntriesByType('resource').some(entry=>/\/storage-(fs|sqlite)\.js$/.test(entry.name)))throw new Error('unused storage adapters downloaded');
    const {createStorage}=await import('/storage.js');
    const canceled=createStorage(globalThis,storageIdentity,()=>({}),storageIdentity.appId);
    void canceled.capability.fs.writeFile('app:/data/canceled-load',new Uint8Array([1]));
    void canceled.capability.sqlite.open('app:/data/notes.db');
    await Promise.resolve(); // start the first adapter imports, then unload
    canceled.dispose();
    const storageCall=async(op,value='')=>{
      const result=await invoke(storage,'work',[op,value],[],['session']);
      if(result.tag!==0)throw new Error(`storage ${op}: ${JSON.stringify(result)}`);
      if(result.externalRead!==true||result.reads.length!==0)throw new Error('storage dependency must not invent a secret read');
      return result.value.text;
    };
    if(await storageCall('file','hello')!=='hello')throw new Error('file bytes');
    const {createFileSystem:inspectFiles}=await import('/storage-fs.js');
    const filesAfterCancel=inspectFiles(storageIdentity.appId,storageIdentity.grantSet);
    if((await filesAfterCancel.readdir('app:/data')).includes('canceled-load')||workersCreated!==0)throw new Error('disposed lazy storage started I/O');
    filesAfterCancel.dispose();
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
    const onWorker=await prepare(await payload(fixtures.storage,storageIdentity),{...storageIdentity,placement:'worker'});
    const workerInvoke=async(source,args,store=[],grants=[],outcome)=>{
      const started=call({id:onWorker.id,op:outcome?'resume':'answer',source,args,store:[],grants,outcome});
      if(!started.continuation)throw new Error('worker storage answer was not a turn');
      call({op:'dispatch',id:onWorker.id,token:started.continuation,store,grants});
      return await checkpoint(await run(started.continuation));
    };
    const fromWorker=await workerInvoke('work',['read',''],[],['session']);
    if(fromWorker.value?.text!=='hello'||fromWorker.externalRead!==true)throw new Error(`worker storage read: ${JSON.stringify(fromWorker)}`);
    if((await workerInvoke('work',['list',''],[],['session'])).value?.text!=='remember')throw new Error('worker SQLite read');
    const workerFetch=await workerInvoke('work',['fetch','w'],[],['session']);
    if(workerFetch.request?.url!=='https://example.test/w')throw new Error(`worker fetch yields to the page: ${JSON.stringify(workerFetch)}`);
    const workerResumed=await workerInvoke('work',['fetch','w'],[],['session'],response('reply'));
    if(workerResumed.value?.text!=='w:reply'||workerResumed.writes[0]?.[1]!=='w')throw new Error(`worker fetch resume: ${JSON.stringify(workerResumed)}`);
    onWorker.dispose();
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
    const fsSet=grantSet(fsGrants),fs=createFileSystem(fsApp,fsSet),sql=createSqlite(fsApp,fsSet);
    const refused=async promise=>{try{await promise;}catch(e){if(e.kind!=='Unavailable')throw e;return;}throw new Error('operation should refuse');};
    await fs.mkdir('app:/data/dir');
    await fs.writeFile('app:/data/dir/a',new Uint8Array([1,2]));
    await Promise.all(Array.from({length:8},()=>fs.appendFile('app:/data/dir/a',new Uint8Array([3]))));
    if((await fs.stat('app:/data/dir/a')).size!==10)throw new Error('concurrent append lost bytes');
    const [ownedA,ownedB]=await Promise.all([fs.readFile('app:/data/dir/a'),fs.readFile('app:/data/dir/a')]);
    new Uint8Array(ownedA)[0]=99;
    if(new Uint8Array(ownedB)[0]!==1||new Uint8Array(await fs.readFile('app:/data/dir/a'))[0]!==1)throw new Error('read buffers must own independent bytes');
    structuredClone(ownedA,{transfer:[ownedA]});
    if(new Uint8Array(await fs.readFile('app:/data/dir/a'))[0]!==1)throw new Error('detached read must not detach stored data');
    const errors=async(promise,code)=>{try{await promise;}catch(error){if(error.code===code)return;throw error;}throw new Error('expected '+code);};
    await errors(fs.readFile('app:/data/absent'),'ENOENT');
    await errors(fs.stat('app:/data/absent'),'ENOENT');
    await errors(fs.realpath('app:/data/absent'),'ENOENT');
    await errors(fs.readFile('app:/data/dir'),'EISDIR');
    await errors(fs.readdir('app:/data/dir/a'),'ENOTDIR');
    await errors(fs.readdir('app:/data/absent'),'ENOENT');
    for(const method of ['writeFile','atomicWriteFile','appendFile']) {
      await errors(fs[method]('app:/data/dir',new Uint8Array([9])),'EISDIR');
      await errors(fs[method]('app:/data/absent/a',new Uint8Array([9])),'ENOENT');
      await errors(fs[method]('app:/data/dir/a/child',new Uint8Array([9])),'ENOTDIR');
      const buffer=new Uint8Array([0,4,5,0]);
      const saved=fs[method]('app:/cache/snapshot',buffer.subarray(1,3));
      buffer.fill(99);structuredClone(buffer.buffer,{transfer:[buffer.buffer]});
      await saved;
      if([...new Uint8Array(await fs.readFile('app:/cache/snapshot'))].join()!=='4,5')throw new Error(method+' must snapshot input before waiting');
      await fs.rm('app:/cache/snapshot');
    }
    await fs.atomicWriteFile('app:/cache/target',new Uint8Array([6]));
    await errors(fs.copyFile('app:/data/absent','app:/cache/target'),'ENOENT');
    await errors(fs.copyFile('app:/data/dir','app:/cache/target'),'EISDIR');
    await errors(fs.copyFile('app:/data/dir/a','app:/data/dir'),'EISDIR');
    await errors(fs.copyFile('app:/data/dir/a','app:/data/absent/a'),'ENOENT');
    await refused(fs.copyFile('app:/data/dir/a','app:/data/dir/a'));
    if(new Uint8Array(await fs.readFile('app:/cache/target'))[0]!==6)throw new Error('failed copy changed destination');
    if((await fs.stat('app:/data/dir/a')).size!==10)throw new Error('failed mutation changed source');
    await fs.copyFile('app:/data/dir/a','app:/cache/target');
    await fs.atomicWriteFile('app:/cache/target',new Uint8Array([8]));
    if((await fs.stat('app:/cache/target')).size!==1||new Uint8Array(await fs.readFile('app:/data/dir/a'))[0]!==1)throw new Error('overwrite must replace independent contents');
    await fs.writeFile('app:/cache/unrelated-large',new Uint8Array(1024*1024));
    let loadedBytes=0;
    const originalReads=new Map(['get','getAll'].map(method=>[method,IDBObjectStore.prototype[method]]));
    for(const [method,original] of originalReads)IDBObjectStore.prototype[method]=function(...args){
      const request=original.apply(this,args);
      if(this.transaction.db.name===`exact-storage:${encodeURIComponent(fsApp)}`)request.addEventListener('success',()=>{
        const values=Array.isArray(request.result)?request.result:[request.result];
        for(const value of values)loadedBytes+=value?.contents?.byteLength||0;
      });
      return request;
    };
    try {
      await fs.readFile('app:/data/dir/a');await fs.stat('app:/data/dir/a');
      await fs.realpath('app:/data/dir/a');await fs.readdir('app:/data/dir');
      await fs.writeFile('app:/cache/target',new Uint8Array([6]));
      await fs.atomicWriteFile('app:/cache/target',new Uint8Array([7]));
      await fs.appendFile('app:/cache/target',new Uint8Array([8]));
      await fs.copyFile('app:/data/dir/a','app:/cache/target');
      await fs.mkdir('app:/cache/created/nested');
      await fs.rename('app:/cache/created','app:/cache/renamed');
      await fs.rm('app:/cache/renamed');
      await fs.rm('app:/cache/unrelated-large');
    } finally {for(const [method,original] of originalReads)IDBObjectStore.prototype[method]=original;}
    if(loadedBytes>=1024*1024)throw new Error('filesystem operation loaded unrelated or removed file contents');
    await fs.rm('app:/cache/unrelated-large');
    await fs.mkdir('app:/data/ops/from/nested');await fs.mkdir('app:/data/ops/from0');
    await fs.mkdir('app:/data/ops/to');await fs.mkdir('app:/data');
    for(const path of ['app:/data/ops/from/\ufffftail','app:/data/ops/from/nested/🌿','app:/data/ops/from!'])await fs.writeFile(path,new Uint8Array([12]));
    await errors(fs.mkdir('app:/data/ops/from!/child'),'ENOTDIR');
    await errors(fs.rm('app:/data/ops/from!/child'),'ENOTDIR');
    await errors(fs.rm('app:/data/absent/child'),'ENOENT');
    await errors(fs.rename('app:/data/absent','app:/data/ops/to'),'ENOENT');
    await errors(fs.rename('app:/data/ops/from','app:/data/absent/to'),'ENOENT');
    await refused(fs.rename('app:/data/ops/from','app:/data/ops/from/nested/moved'));
    await refused(fs.rename('app:/data/ops/from!','app:/data/ops/to'));
    await errors(fs.rename('app:/data/ops/from','app:/data/ops'),'ENOTEMPTY');
    await fs.writeFile('app:/data/ops/to/occupied',new Uint8Array([13]));
    await errors(fs.rename('app:/data/ops/from','app:/data/ops/to'),'ENOTEMPTY');
    await fs.rm('app:/data/ops/to/occupied');
    const originalPut=IDBObjectStore.prototype.put;let puts=0;
    IDBObjectStore.prototype.put=function(...args){
      if(this.transaction.db.name===`exact-storage:${encodeURIComponent(fsApp)}`&&++puts===2)throw new Error('injected move failure');
      return originalPut.apply(this,args);
    };
    try {
      try{await fs.rename('app:/data/ops/from','app:/data/ops/to');throw new Error('move should fail');}
      catch(error){if(error.message!=='injected move failure')throw error;}
    } finally {IDBObjectStore.prototype.put=originalPut;}
    if((await fs.readdir('app:/data/ops/to')).length||new Uint8Array(await fs.readFile('app:/data/ops/from/nested/🌿'))[0]!==12)throw new Error('failed move must roll back removed and inserted records');
    await fs.rename('app:/data/ops/from','app:/data/ops/from');
    await fs.rename('app:/data/ops/from','app:/data/ops/to');
    if((await fs.readdir('app:/data/ops')).join()!=='from!,from0,to'||new Uint8Array(await fs.readFile('app:/data/ops/to/\ufffftail'))[0]!==12)throw new Error('move must preserve Unicode descendants and adjacent siblings');
    await fs.writeFile('app:/data/ops/replaced',new Uint8Array([99]));
    await fs.rename('app:/data/ops/from!','app:/data/ops/replaced');
    if(new Uint8Array(await fs.readFile('app:/data/ops/replaced'))[0]!==12)throw new Error('file move must replace destination');
    await fs.rm('app:/data/ops/to');
    if((await fs.readdir('app:/data/ops')).join()!=='from0,replaced')throw new Error('recursive removal must preserve adjacent siblings');
    await fs.rm('app:/data/ops');
    await fs.mkdir('app:/data/list/nested');await fs.mkdir('app:/data/list0');
    for(const path of ['app:/data/list/\ufffftail','app:/data/list/🌿','app:/data/list/nested/hidden','app:/data/list0/sibling'])await fs.writeFile(path,new Uint8Array([4]));
    if((await fs.readdir('app:/data/list')).join()!==['nested','🌿','\ufffftail'].sort().join())throw new Error('directory keys must include Unicode direct children only');
    if((await fs.stat('app:/data/list')).isDirectory!==true)throw new Error('directory metadata');
    await fs.rm('app:/data/list');await fs.rm('app:/data/list0');
    await fs.copyFile('app:/data/dir/a','app:/cache/copy');
    await fs.rename('app:/data/dir','app:/data/moved');
    if((await fs.readdir('app:/data/moved')).join()!=='a'||await fs.realpath('app:/data//moved/a/')!=='app:/data/moved/a')throw new Error('directory rename/canonical path');
    for(const path of ['app:/data/../cache/escape','app:/database/escape','/tmp/escape','app:/data/./escape'])await refused(fs.writeFile(path,new Uint8Array([1])));
    const narrow=createFileSystem(fsApp,grantSet('fs.read app:/data/move\nfs.write app:/data/move'));
    await refused(narrow.readFile('app:/data/moved/a'));narrow.dispose();
    await fs.rm('app:/data/moved');await fs.rm('app:/data/missing');
    if((await fs.readdir('app:/data')).length)throw new Error('recursive removal');
    const {createFileStore}=await import('/storage-fs.js');
    const ownedStore=createFileStore(fsApp);
    const ownedPath='app:/cache/owned';
    const ownedBytes=new Uint8Array([3,4,5]).buffer;
    const ownedWrite=ownedStore.atomicWriteOwnedFile(ownedPath,ownedBytes);
    if(ownedBytes.byteLength!==0)throw new Error('private write must take ownership before yielding');
    await ownedWrite;
    if([...new Uint8Array(await fs.readFile(ownedPath))].join()!=='3,4,5')throw new Error('owned write lost bytes');
    const failedBytes=new Uint8Array([9]).buffer;
    const oldOwnedPut=IDBObjectStore.prototype.put;
    IDBObjectStore.prototype.put=function(value,...args){
      if(value.path===ownedPath)throw new Error('injected owned write failure');
      return oldOwnedPut.call(this,value,...args);
    };
    try {
      try {await ownedStore.atomicWriteOwnedFile(ownedPath,failedBytes);throw new Error('owned write should fail');}
      catch(error){if(error.message!=='injected owned write failure')throw error;}
    } finally {IDBObjectStore.prototype.put=oldOwnedPut;ownedStore.close();}
    if(failedBytes.byteLength!==0||[...new Uint8Array(await fs.readFile(ownedPath))].join()!=='3,4,5')throw new Error('failed owned write must leave durable bytes unchanged');
    if('atomicWriteOwnedFile' in fs)throw new Error('private transfer exposed through filesystem capability');
    const db=await sql.open('app:/data/live.db');
    await db.execute('CREATE TABLE t (value INTEGER)');
    await db.execute('INSERT INTO t VALUES (?)',[42n]);
    await refused(fs.atomicWriteFile('app:/data/live.db',new Uint8Array([0])));
    await refused(fs.rm('app:/data'));
    await fs.writeFile('app:/cache/unrelated',new Uint8Array([1]));
    const secondSql=createSqlite(fsApp,fsSet);await refused(secondSql.open('app:/data/live.db'));secondSql.dispose();
    await db.close();
    await fs.copyFile('app:/data/live.db','app:/data/copied.db');
    if(!new TextDecoder().decode(await fs.readFile('app:/data/copied.db')).startsWith('SQLite format 3'))throw new Error('database is not a SQLite file');
    const copied=await sql.open('app:/data/copied.db');
    if((await copied.query('SELECT value FROM t')).rows[0][0]!==42n)throw new Error('copied database contents');
    const prepared=await copied.prepare('SELECT value FROM t');await copied.close();
    await refused(prepared.query());
    const reloadApp='test.browser.sqlite-overlap';
    let current=createSqlite(reloadApp,fsSet);
    const coldStart=performance.now();
    let live=await current.open('app:/data/reload.db');
    const coldMs=performance.now()-coldStart;
    await live.execute('CREATE TABLE t (value INTEGER)');
    await live.execute('INSERT INTO t VALUES (?)',[91n]);
    let oldStatement=await live.prepare('SELECT value FROM t');
    const warmMs=[], expectedWorkers=workersCreated;
    for(let i=0;i<5;i++){
      const next=createSqlite(reloadApp,fsSet);
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
    const caltrainIdentity=admit({appId:'com.exact.caltrain',grants:''});
    const train=await prepare(await payload(fixtures.caltrain,caltrainIdentity),caltrainIdentity);
    const canonical=v=>JSON.stringify(v,(_key,value)=>value&&typeof value==='object'&&!Array.isArray(value)?Object.fromEntries(Object.entries(value).sort(([a],[b])=>a.localeCompare(b))):value);
    for(const test of fixtures.oracle){
      const result=await invoke(train,test.source,test.args);
      const actual=result.tag===0?{tag:0,value:result.value}:{tag:result.tag,kind:result.kind,message:result.message};
      if(canonical(actual)!==canonical(test.expected))throw new Error(`Caltrain parity ${test.source}: ${JSON.stringify({actual,expected:test.expected})}`);
    }
    train.dispose();
    return {ecdsa:globalThis.ecdsaResult,guards:forms.length,caltrain:fixtures.oracle.length,store:true,isolated:true,refusals:true,async:true,storage:true,sqliteReload:{coldMs,warmMs}};
  };
  const result=await call('Runtime.evaluate',{expression:`(${probe.toString()})(${JSON.stringify(fixtures)})`,returnByValue:true,awaitPromise:true});
  assert.equal(result.exceptionDetails,undefined,JSON.stringify(result.exceptionDetails));
  console.log(JSON.stringify(result.result.value.sqliteReload));delete result.result.value.sqliteReload;
  console.log('ECDSA_RESULT '+JSON.stringify(result.result.value.ecdsa));delete result.result.value.ecdsa;
  assert.deepEqual(result.result.value,{guards:25,caltrain:20,store:true,isolated:true,refusals:true,async:true,storage:true});
  await call('Page.reload');
  const persisted=await call('Runtime.evaluate',{expression:`(async()=>{
    await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));
    const {createFileSystem}=await import('/storage-fs.js');
    const {createSqlite}=await import('/storage-sqlite.js');
    const {createGrantSet}=await import('/grant-admission.js');
    const grantSets=${process.env.EXACT_GRANT_SETS};
    const fs=createFileSystem('dev.exact.storage-test',createGrantSet(grantSets['fs.read app:/data']));
    const sql=createSqlite('dev.exact.storage-test',createGrantSet(grantSets['sqlite.open app:/data/notes.db']));
    const bytes=new TextDecoder().decode(await fs.readFile('app:/data/note'));
    const db=await sql.open('app:/data/notes.db');
    const rows=await db.query('SELECT body FROM notes');await db.close();sql.dispose();fs.dispose();
    return bytes==='hello'&&rows.rows[0][0]==='remember';
  })()`,returnByValue:true,awaitPromise:true});
  assert.equal(persisted.exceptionDetails,undefined,JSON.stringify(persisted.exceptionDetails));
  assert.equal(persisted.result.value,true,'storage survives full page reload');
  // LLP 1069.005 D1 on a LAN dev page: plain HTTP to a name that is not
  // loopback is no secure context, so neither realm has `crypto.subtle`.
  // SHA-256 is the dev protocol's (here the fixture server's); SHA-384
  // refuses by name; `randomUUID` is formed from `getRandomValues`.
  await call('Page.navigate',{url:`http://lan.test:${server.address().port}/?agent=1&seed=1`});
  const lanProbe=async(fixture,seeded,grantSetValue)=>{
    await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));
    if(isSecureContext||globalThis.crypto.subtle)throw new Error('lan.test is a secure context');
    const encode=s=>new TextEncoder().encode(s);
    const sha=async bytes=>(await fetch('/sha256',{method:'POST',body:bytes})).text();
    globalThis.exact??={};globalThis.exact.moduleDigest=sha;
    const {prepare,call,run}=await import('/module-glue.js');
    const {createGrantSet}=await import('/grant-admission.js');
    const identity={appId:'test.entropy',grants:'net.fetch https://fixture.exact.test\n',grantSet:createGrantSet(grantSetValue)};
    const script=encode(fixture);
    const payload={script,receipt:encode(JSON.stringify({version:1,abi:1,...identity,module:{sha256:'a'.repeat(64)},web:{file:'app.js',bytes:script.length,sha256:await sha(script)}}))};
    const seen=[];
    for(const placement of ['main','worker']){
      const realm=await prepare(payload,{...identity,placement});
      const ask=async source=>{
        let result=call({id:realm.id,op:'answer',source,args:[],store:[],grants:[]});
        if(placement==='worker')call({op:'dispatch',id:realm.id,token:result.continuation,store:[],grants:[]});
        for(let i=0;result.continuation&&i<20;i++)result=await run(result.continuation);
        return result;
      };
      // `?agent` on a page that is not loopback draws no repeatable stream.
      const id=(await ask('uuid')).value;
      seen.push(`${placement} ${(await ask('lan')).value} ${/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(id)} ${id!==seeded}`);
      realm.dispose();
    }
    return seen;
  };
  const lan=await call('Runtime.evaluate',{expression:`(${lanProbe.toString()})(${JSON.stringify(fixtures.entropy)},${JSON.stringify(fixtures.agentStream.split(" ")[0])},${JSON.stringify(fixtures.grantSets['net.fetch https://fixture.exact.test\n'])})`,returnByValue:true,awaitPromise:true});
  assert.equal(lan.exceptionDetails,undefined,JSON.stringify(lan.exceptionDetails));
  const abc='ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad';
  assert.deepEqual(lan.result.value,[`main ${abc}/NotSupportedError true true`,`worker ${abc}/NotSupportedError true true`],'a LAN page digests SHA-256 through the dev protocol and has no agent stream');
  console.log(JSON.stringify({lan:lan.result.value}));
  console.log(JSON.stringify(result.result.value));
  const evaluate=async expression=>{
    const result=await call('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});
    assert.equal(result.exceptionDetails,undefined,JSON.stringify(result.exceptionDetails));
    return result.result.value;
  };
  const frames='await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))';
  // HTML's `change` commits a text field on blur (LLP 1069.001): type, then leave.
  const type=async text=>{await call('Input.insertText',{text});await evaluate(`document.getElementById('editor').blur()`);};
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
    await type('early');
    await blockedRange();
    assert.deepEqual(await evaluate(`({dispatch:startup.dispatch,value:document.getElementById('editor').value})`),{dispatch:[],value:'baked'},'unready app neither dispatches nor edits baked input');
    await evaluate('startup.release()');
    assert.equal(await evaluate(`(async()=>{for(let i=0;i<120;i++){${frames};const root=document.getElementById('exact-root');if(root.dataset.${fails?'error':'moduleReady'})return true;}return false;})()`),true,'module release settles readiness');
    if(fails){
      await scroll();
      await click('action');await click('editor');await type('failed');
      await blockedRange();
      assert.deepEqual(await evaluate(`({active:startup.activated,dispatch:startup.dispatch,value:document.getElementById('editor').value})`),{active:false,dispatch:[],value:'baked'},'failed module remains gated without disabling scrolling');
    }else{
      await click('action');await click('editor');await type('ready');
      const active=await evaluate(`({dispatch:startup.dispatch,value:document.getElementById('editor').value,disabled:document.getElementById('disabled').disabled})`);
      assert.equal(active.dispatch.some(event=>event.id===2&&event.kind===0),true,'ready button dispatches');
      assert.equal(active.dispatch.some(event=>event.id===3&&event.kind===1),true,'ready input dispatches edits');
      assert.equal(active.value.includes('ready'),true);assert.equal(active.disabled,true,'authored disabled state survives activation');
      assert.deepEqual(await evaluate(`['disabled-on-activation','enabled-on-activation'].map(id=>document.getElementById(id).disabled)`),[true,false],'activation prop changes override originally authored disabled state');
      // The range is controlled (LLP 1069.001): an edit reaches the app, and the
      // control shows the app's value, which this stub never changes.
      const rangeEdits=`startup.dispatch.filter(event=>event.id===6&&event.kind===1).map(event=>event.value)`;
      await rangeKeyboard();
      assert.deepEqual(await evaluate(rangeEdits),['51'],'activated range accepts native keyboard editing');
      assert.equal(await evaluate(`document.getElementById('range').value`),'50','the range shows the value the app holds');
      await click('range');
      const edits=await evaluate(rangeEdits);
      assert.equal(edits.length,2,'keyboard and pointer range edits both dispatch');
      assert.equal(Number(edits[1])<50,true,'activated range accepts native pointer editing');
      await click('enabled-on-activation');
      assert.equal(await evaluate(`startup.dispatch.some(event=>event.id===8&&event.kind===0)`),true,'activation can enable an originally disabled button');
      const count=await evaluate('startup.dispatch.length');await click('disabled');await click('disabled-on-activation');
      assert.equal(await evaluate('startup.dispatch.length'),count,'authored disabled button stays noninteractive');
    }
  }
  console.log('startup: private iframe layout, pre-activation scroll, input gating, successful and failed activation');
  await call('Page.navigate',{url:`http://127.0.0.1:${server.address().port}/startup?rust=1`});
  assert.equal(await evaluate(`(async()=>{for(let i=0;i<120;i++){${frames};if(globalThis.startup?.activated)return true;}return false;})()`),true,'Rust-only deferred source activates without module metadata');
  assert.equal(await evaluate('startup.painted'),true,'Rust-only activation follows a rendering opportunity');
  assert.equal(await evaluate('!!globalThis.exact.moduleRuntime'),false,'Rust-only activation loads no TS executor');
  await evaluate('globalThis.exact.ready');
  await click('action');
  assert.equal(await evaluate('startup.dispatch.some(event=>event.id===2&&event.kind===0)'),true,'Rust-only actions become ready');
  await evaluate('globalThis.exact.reload(new Uint8Array([1]))');
  assert.equal(await evaluate('startup.activations'),2,'Rust-only Contract reload activates its fresh source');
  assert.equal(await evaluate(`(async()=>{for(let i=0;i<120;i++){${frames};if(startup.storageRuns)return true;}return false;})()`),true,'raw Rust storage reaches the host service');
  assert.equal(await evaluate('startup.storageBeforePaint'),false,'raw initial storage waits for the first-paint barrier');
  await call('Page.navigate',{url:`http://127.0.0.1:${server.address().port}/startup?rust=1&early=1`});
  await evaluate(`(async()=>{for(let i=0;i<120;i++){${frames};if(globalThis.earlyReload){await earlyReload;return;}}throw new Error('early reload did not run');})()`);
  assert.deepEqual(await evaluate('({painted:startup.painted,activations:startup.activations})'),{painted:true,activations:2},'immediate Contract reload follows first-paint activation');
  console.log('startup: Rust-only deferred activation after paint without JavaScript module');
} finally {
  process.kill(-child.pid,'SIGKILL');await exited;server.closeAllConnections();await new Promise(r=>server.close(r));rmSync(profile,{recursive:true,force:true,maxRetries:3,retryDelay:100});
}
"#;

#[test]
fn browser_portable_storage_shares_fieldnotes_data_and_enforces_scope() {
    let chrome = std::env::var("CHROME")
        .unwrap_or_else(|_| "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".into());
    if !Path::new(&chrome).exists() {
        eprintln!("browser storage sweep unavailable: set CHROME");
        return;
    }
    let result = Command::new("bun")
        .args(["--input-type=module", "-e", PROTOCOL_PROBE])
        .env("CHROME", chrome)
        .env("EXACT_GRANT_SETS", grant_sets())
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
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

const PROTOCOL_PROBE: &str = r#"
import { Cdp } from './scripts/agent.mjs';
import { webHostFiles } from './scripts/app.mjs';
import { spawn, execFileSync } from 'node:child_process';
import { createServer } from 'node:http';
import { readFileSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import assert from 'node:assert/strict';
const routes=Object.fromEntries(Object.entries(webHostFiles()).map(([name,source])=>['/'+name,source]));
const app=execFileSync(process.execPath,['./node_modules/.bin/rolldown','apps/fieldnotes/app.ts','--format','iife','--name','fieldnotes'],{encoding:'utf8',stdio:['ignore','pipe','pipe']})+'\nglobalThis.exact={...fieldnotes,abi:1};';
const server=createServer((req,res)=>{
  res.setHeader('content-type',req.url.endsWith('.wasm')?'application/wasm':routes[req.url]?'text/javascript':'text/html');
  res.end(routes[req.url]?readFileSync(routes[req.url]):'<main id="exact-root"></main>');
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const profile=mkdtempSync(resolve(tmpdir(),'exact-storage-protocol-'));
const child=spawn(process.env.CHROME,['--headless=new','--no-sandbox','--remote-debugging-pipe','--no-first-run','--disable-background-networking',`--user-data-dir=${profile}`,'about:blank'],{detached:true,stdio:['ignore','ignore','ignore','pipe','pipe']});
const cdp=new Cdp(child.stdio[3],child.stdio[4]);
const exited=new Promise(resolve=>child.on('exit',()=>{cdp.fail('browser closed');resolve();}));
try {
  const {targetInfos}=await cdp.send('Target.getTargets');
  const page=targetInfos.find(t=>t.type==='page')??await cdp.send('Target.createTarget',{url:'about:blank'});
  const {sessionId}=await cdp.send('Target.attachToTarget',{targetId:page.targetId,flatten:true});
  const send=(method,params)=>cdp.send(method,params,sessionId);
  await send('Page.enable');
  await send('Page.navigate',{url:`http://127.0.0.1:${server.address().port}/`});
  const probe=async(app,reloaded,grantSets)=>{
    await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));
    globalThis.exact??={};
    const {prepare,call,run}=await import('/module-glue.js');
    const {createStorageRequests}=await import('/storage-request.js');
    const {createGrantSet,scopedGrantSet}=await import('/grant-admission.js');
    const encoder=new TextEncoder(),decoder=new TextDecoder();
    const identity={appId:'com.exact.fieldnotes',grants:'sqlite.open app:/data/fieldnotes.db\nfs.read app:/data/backups\nfs.write app:/data/backups\nfs.read app:/tmp/picked\nsecret.keep fieldnotes.revision'};
    identity.grantSet=createGrantSet(grantSets[identity.grants]);
    const script=encoder.encode(app);
    const sha256=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',script)),b=>b.toString(16).padStart(2,'0')).join('');
    const receipt=encoder.encode(JSON.stringify({version:1,abi:1,...identity,module:{sha256:'a'.repeat(64)},web:{file:'app.js',bytes:script.length,sha256}}));
    let realm=await prepare({script,receipt},identity);
    const snapshot=new Map(JSON.parse(sessionStorage.getItem('fieldnotes-store')||'[]'));
    const priorRevision=Number(snapshot.get('fieldnotes.revision')||0);
    const ask=async(source,args=[])=>{
      let result=call({op:'answer',id:realm.id,source,args,store:[...snapshot],grants:['fieldnotes.revision']});
      for(let i=0;result.continuation&&i<200;i++)result=await run(result.continuation);
      if(result.tag!==0)throw new Error(source+': '+JSON.stringify(result));
      for(const [key,value] of result.writes||[]){if(value===null)snapshot.delete(key);else snapshot.set(key,value);}
      sessionStorage.setItem('fieldnotes-store',JSON.stringify([...snapshot]));
      if(source!=='library'&&source!=='openNote'&&(result.writes?.length!==1||result.value.revision!==Number(snapshot.get('fieldnotes.revision'))))throw new Error('mutation revision must settle once');
      return result.value;
    };
    const check=(condition,message)=>{if(!condition)throw new Error(message);};
    const service=createStorageRequests(identity.appId,identity.grantSet);
    const request=async(op,args,scope)=>JSON.parse(decoder.decode(await service.run(JSON.stringify({version:1,op,args}),scope==null?identity.grantSet:scopedGrantSet(identity.grantSet,scope))));
    const command=(kind,sql,params=[])=>({kind,sql,params});
    const db='app:/data/fieldnotes.db',path='app:/data/backups/fieldnotes.json';
    if(reloaded){
      const read=await ask('readBackup');check(read.revision===priorRevision+1,'revision survives full page reload');check(!read.failed&&read.backupText.includes('Protocol edited this note'),'TS reload reads portable backup');
      const restored=await ask('restoreNotes',['']);check(!restored.failed,'TS restores portable backup after page reload');
      const library=await ask('library',['',0,0]);check(library.notes[0].title==='Protocol edited this note'&&library.total===1,'TS reload sees same SQLite IDs/data');
      const large='\\'.repeat(20000);
      const inserted=await request('sqlite.transaction',{path:db,commands:Array.from({length:105},()=>command('execute','INSERT INTO notes (title,body,pinned) VALUES (?,?,?)',['Large note',large,{integer:'0'}]))});
      check(!inserted.error,'large notebook fixture');
      const oversized=await ask('backupNotes');
      check(oversized.failed&&oversized.message==='This backup exceeds 4 MB. Split or remove large notes before backing up.','bounded TypeScript backup preserves its size error');
      check((await ask('readBackup')).backupText===read.backupText,'oversized backup retains previous file');
      check(!(await ask('restoreNotes',[''])).failed,'restore remains available after oversized backup');
      service.dispose();realm.dispose();return {reloaded:true};
    }
    const saved=await ask('saveNote',['','Café 🌿','京都\nBinary-safe backups\u2028line separator\u2029paragraph separator',true,1]);check(!saved.failed,'TS creates notebook');
    const {createFileStore}=await import('/storage-fs.js');
    const files=createFileStore(identity.appId);
    const beforeBackup=(await files.stat(db)).modifiedMs;
    const expected=await ask('backupNotes');check(!expected.failed,'TS original backup');
    check((await files.stat(db)).modifiedMs===beforeBackup,'TypeScript backup does not rewrite an initialized database');
    files.close();
    // These are the same value-only operations emitted by fieldnotes-data;
    // the native fixture separately executes the Rust source through ABI2.
    const rows=await request('sqlite',{path:db,commands:[command('query','SELECT id,title,body,pinned FROM notes ORDER BY pinned DESC,id DESC')]});
    check(!rows.error,'portable SQL opens TS database: '+JSON.stringify(rows));
    const notes=rows[0].rows.map(r=>({id:r[0].integer,title:r[1],body:r[2],pinned:r[3].integer==='1'}));
    const text=JSON.stringify({version:1,notes},null,2);check(text===expected.backupText,'portable backup preserves exact TypeScript format');
    check(!(await request('fs.mkdir',{path:'app:/data/backups'}))?.error,'portable mkdir');
    check(!(await request('fs.atomicWriteFile',{path,text}))?.error,'portable writes backup');
    check((await ask('readBackup')).backupText===text,'TS reads portable UTF8 backup');
    const beforeReplacement=await ask('readBackup');
    realm.dispose();realm=await prepare({script,receipt},identity);
    const afterReplacement=await ask('readBackup');
    check(afterReplacement.message===beforeReplacement.message&&afterReplacement.revision===beforeReplacement.revision+1,'identical result messages after source replacement still advance shared revision');
    const typed=await request('sqlite',{path:db,commands:[command('query','SELECT ?,?,?,?,?',[{integer:'9223372036854775807'},{integer:'-9223372036854775808'},1.25,{bytes:[0,255]},null])]});
    check(JSON.stringify(typed[0].rows[0])===JSON.stringify([{integer:'9223372036854775807'},{integer:'-9223372036854775808'},1.25,{bytes:[0,255]},null]),'SQLite integer/blob/real/null survive protocol');
    const numericTypes=await request('sqlite',{path:db,commands:[command('query','SELECT typeof(?),typeof(?)',[1,0.5])]});
    check(JSON.stringify(numericTypes[0].rows)===JSON.stringify([['integer','real']]),'ordinary safe integral numbers bind as SQLite integers');
    const unsafeNumber=await request('sqlite',{path:db,commands:[command('query','SELECT ?',[9007199254740992])]});
    check(typeof unsafeNumber.error==='string','unsafe integral numbers require explicit int64 representation');
    const edited=await request('sqlite.transaction',{path:db,commands:[command('execute','UPDATE notes SET title=? WHERE id=?',['Protocol edited this note',{integer:saved.id}])]});
    check(!edited.error,'portable updates same note');
    const refused=await request('sqlite.transaction',{path:db,commands:[command('execute','DELETE FROM notes'),command('execute','INSERT INTO missing_table VALUES (?)',[1])]});
    check(typeof refused.error==='string','invalid transaction refuses');
    const library=await ask('library',['',0,0]);check(library.total===1&&library.notes[0].id===saved.id&&library.notes[0].title==='Protocol edited this note','failed portable transaction rolls back and TS sees preceding update');
    const narrow=await request('fs.atomicWriteFile',{path:'app:/data/backups/refused',text:'deny'},'sqlite.open app:/data/fieldnotes.db');
    check(typeof narrow.error==='string','narrow source cannot borrow sibling filesystem grant');
    const widened=await request('fs.mkdir',{path:'app:/cache/no'},'fs.write app:/');
    check(widened.error?.includes('exceeds'),'scope cannot exceed admitted grants');
    // A page opened under the agent ('?agent', no scratch store named) gets no
    // storage; the store is chosen when the service is made, not per request,
    // from the URL the page was opened at (its navigation entry, 18d0dec29), so
    // a router's replaceState cannot change it: stand in that entry.
    const entries=performance.getEntriesByType;
    performance.getEntriesByType=type=>type==='navigation'?[{name:location.origin+'/?agent=1'}]:entries.call(performance,type);
    const agent=createStorageRequests(identity.appId,identity.grantSet);performance.getEntriesByType=entries;
    check(JSON.parse(decoder.decode(await agent.run(JSON.stringify({version:1,op:'fs.readFile',args:{path}})))).error?.includes('unavailable in agent mode'),'agent mode withholds portable storage');agent.dispose();
    check(typeof (await request('fs.atomicWriteFile',{path:'app:/data/backups/../escape',text:'deny'})).error==='string','traversal refused');
    const binary='app:/data/backups/binary';await request('fs.atomicWriteFile',{path:binary,bytes:[0,255]});
    check((await request('fs.readFile',{path:binary})).base64==='AP8=','file byte representation exact');
    // Concurrent library/detail callers share a realm. Browser realms hold
    // each whole storage-backed turn through connection close.
    for(const placement of ['main','worker']) {
      realm.dispose();realm=await prepare({script,receipt},{...identity,placement});
      const [list,opened]=await Promise.all([ask('library',['',0,0]),ask('openNote',[saved.id,1])]);
      check(list.ready&&opened.ready&&opened.body.includes('Binary-safe backups'),'concurrent library/detail reads settle on '+placement);
    }
    const updated={version:1,notes:await Promise.all(library.notes.map(async ({id})=>{
      const note=await ask('openNote',[id,1]);check(note.ready,'read full body for portable backup');
      return {id,title:note.title,body:note.body,pinned:note.pinned};
    }))};
    await request('fs.atomicWriteFile',{path,text:JSON.stringify(updated,null,2)});
    await ask('deleteNote',[saved.id]);check((await ask('library',['',0,0])).total===0,'TS deletes before reload restore');
    const pending=service.run(JSON.stringify({version:1,op:'sqlite',args:{path:db,commands:[command('query','SELECT 1')]}}));
    service.dispose();check(typeof JSON.parse(decoder.decode(await pending)).error==='string','disposal refuses outstanding operation');
    check(typeof (await request('fs.readFile',{path})).error==='string','disposed service refuses future calls');
    realm.dispose();return {shared:true,types:true,rollback:true,scope:true,disposal:true};
  };
  for(const reloaded of [false,true]){
    if(reloaded)await send('Page.reload');
    const result=await send('Runtime.evaluate',{expression:`(${probe.toString()})(${JSON.stringify(app)},${reloaded},${process.env.EXACT_GRANT_SETS})`,returnByValue:true,awaitPromise:true});
    assert.equal(result.exceptionDetails,undefined,JSON.stringify(result.exceptionDetails));
    assert.deepEqual(result.result.value,reloaded?{reloaded:true}:{shared:true,types:true,rollback:true,scope:true,disposal:true});
    console.log(JSON.stringify(result.result.value));
  }
} finally {
  process.kill(-child.pid,'SIGKILL');await exited;server.closeAllConnections();await new Promise(resolve=>server.close(resolve));rmSync(profile,{recursive:true,force:true,maxRetries:3,retryDelay:100});
}
"#;
