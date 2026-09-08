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
import { readFileSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import assert from 'node:assert/strict';
const routes = {'/module-glue.js':'host/web/module-glue.js','/module-prelude.js':'js/src/prelude.js'};
for(const name of ['storage.js','storage-fs.js','storage-sqlite.js','storage-worker.js'])routes['/'+name]='host/web/'+name;
routes['/sqlite3.mjs']='node_modules/@sqlite.org/sqlite-wasm/dist/index.mjs';
routes['/sqlite3.wasm']='node_modules/@sqlite.org/sqlite-wasm/dist/sqlite3.wasm';
const fixtures=Object.fromEntries(['inputs','castle','caltrain','ambient-init','storage'].map(name=>[name,execFileSync('./node_modules/.bin/rolldown',[`js/tests/fixtures/${name}.ts`,'--format','iife'],{encoding:'utf8',stdio:['ignore','pipe','pipe']})]));
fixtures.oracle=JSON.parse(process.env.EXACT_PARITY);
const server = createServer((req,res)=>{
  res.setHeader('content-type', req.url.endsWith('.wasm') ? 'application/wasm' : routes[req.url] ? 'text/javascript' : 'text/html');
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
    return {guards:forms.length,caltrain:fixtures.oracle.length,store:true,isolated:true,refusals:true,async:true,storage:true,sqliteReload:{coldMs,warmMs}};
  };
  const result=await call('Runtime.evaluate',{expression:`(${probe.toString()})(${JSON.stringify(fixtures)})`,returnByValue:true,awaitPromise:true});
  assert.equal(result.exceptionDetails,undefined,JSON.stringify(result.exceptionDetails));
  console.log(JSON.stringify(result.result.value.sqliteReload));delete result.result.value.sqliteReload;
  assert.deepEqual(result.result.value,{guards:25,caltrain:20,store:true,isolated:true,refusals:true,async:true,storage:true});
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
} finally {
  process.kill(-child.pid,'SIGKILL');await exited;server.closeAllConnections();await new Promise(r=>server.close(r));rmSync(profile,{recursive:true,force:true,maxRetries:3,retryDelay:100});
}
"#;
