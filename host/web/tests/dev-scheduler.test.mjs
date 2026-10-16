import {test} from 'bun:test';
import assert from 'node:assert/strict';
import {readFileSync, writeFileSync, mkdirSync, unlinkSync, mkdtempSync, openSync, ftruncateSync, closeSync, rmSync, existsSync, readdirSync, watch, renameSync, watchFile, unwatchFile, statSync, lstatSync, readlinkSync, utimesSync, symlinkSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {open, Cdp} from '../../../scripts/agent.mjs';
import {compilerPaths, gpuModules, pendingBuildInputs} from '../../../scripts/app.mjs';
import {createHash} from 'node:crypto';
import {runInNewContext} from 'node:vm';
import {spawn} from 'node:child_process';
import {createServer} from 'node:http';
import {watchStaticTrees} from '../serve.mjs';
import {chromium} from '../../../scripts/agent-launch.mjs';
import {need} from '../../../scripts/coverage.mjs';
const {executable:chrome,unavailable}=chromium();
// A test that edits an app's files and writes them back also puts their
// times back, so a later test's build-freshness check is not tripped.
const keepTimes = paths => { const kept = paths.map(p => [p, statSync(p)]); return () => { for (const [p, st] of kept) utimesSync(p, st.atime, st.mtime); }; };
const source = readFileSync(process.env.E2B_DEV_SOURCE || new URL('../dev.mjs', import.meta.url), 'utf8');

test('the JS dev loop carries state by built logic revision and refreshes host facts', async () => {
  // RealWorld has routes and text fields but no GPU surface. Keep this proof
  // independent of an optional renderer: a failure then identifies the dev
  // server, page reload, or CDP navigation instead of a wedged GPU adapter.
  const appDir=resolve(new URL('../../../apps/realworld',import.meta.url).pathname),contract=resolve(appDir,'app.contract'),logic=resolve(appDir,'app.ts'),helper=resolve(appDir,'dev-reload-helper.ts');
  const original=readFileSync(contract,'utf8'),originalLogic=readFileSync(logic,'utf8');
  const restoreTimes=keepTimes([contract,logic]);
  const privateDist=mkdtempSync(join(tmpdir(),'exact-dev-carry-'));
  const withProbe=text=>text
    .replace('component RealWorld\n','shape DevReloadAnswer\n  value: string\nshape DevReloadRow\n  id: string\nshape DevReloadPage\n  visibilityState: string\n  onLine: bool\n  canShare: bool\n\ncomponent RealWorld\n  state reloadProbe = "fresh"\n  state rowProbe = "row fresh"\n')
    .replace('  resource follows = followings() as shape Follow[]\n','  resource follows = followings() as shape Follow[]\n  resource devReloadAnswer = devReloadAnswer() as shape DevReloadAnswer\n  resource devReloadRows = devReloadRows() as shape DevReloadRow[]\n  resource devReloadPage = exactPage() as shape DevReloadPage\n')
    .replace('  action go(url: string)\n','  action setReloadProbe(value)\n    reloadProbe = value\n  action setRowProbe(value)\n    rowProbe = value\n  action go(url: string)\n')
    .replace('      head title="Conduit" description="A place to share your knowledge."\n','      head title="Conduit" description="A place to share your knowledge."\n      input value=reloadProbe input=setReloadProbe testId="reload-probe" aria-label="Reload probe"\n      text devReloadAnswer.value testId="reload-answer"\n      text `${devReloadPage.onLine}` testId="reload-page-online"\n      list virtualized=true height=80 width="100%" overflow-y="scroll" testId="reload-list"\n        for row in devReloadRows key=row.id\n          column width="100%"\n            input value=rowProbe input=setRowProbe testId=`row-probe-${row.id}` aria-label="Row reload probe"\n');
  const withNumberProbe=text=>text
    .replace('component RealWorld\n','component RealWorld\n  state reloadProbe = 7\n')
    .replace('      head title="Conduit" description="A place to share your knowledge."\n','      head title="Conduit" description="A place to share your knowledge."\n      text `${reloadProbe}` testId="reload-probe"\n');
  const withLogic=text=>text
    .replace(/^import type .* from '\.\/app\.contract\.d\.ts';\n/m,line=>`${line}import { devReloadValue } from './dev-reload-helper';\n`)
    .replace('const sources: Sources = {\n',"const sources: Sources = {\n  devReloadAnswer: () => ({ value: devReloadValue() }),\n  devReloadRows: () => [{ id: 'one' }],\n");
  const edited=text=>text.replace('          text "conduit"\n','          text "conduit carried"\n');
  const listener=createServer();await new Promise((ok,fail)=>{listener.once('error',fail);listener.listen(0,'127.0.0.1',ok)});const port=listener.address().port;await new Promise(ok=>listener.close(ok));
  let dev,drive,lines='',ready;
  const brief=value=>JSON.stringify(value,(key,item)=>typeof item==='string'&&item.length>200?`${item.slice(0,200)}… (${item.length} chars)`:item);
  const waitFor=async (read,accept,ms=30000)=>{const end=Date.now()+ms;let last;while(Date.now()<end){try{last=await read();if(accept(last))return last}catch{}await new Promise(r=>setTimeout(r,20))}throw new Error(`dev reload did not become observable: ${brief(last)}\n${lines.slice(-3000)}`)};
  try {
    // A fresh value per answer tells a carried answer from a refetched one; data sources have no Date.now() (web-js/ts-fetch.js).
    writeFileSync(helper,"export const devReloadValue = () => 'before:' + crypto.getRandomValues(new Uint32Array(2)).join('.');\n");
    writeFileSync(logic,withLogic(originalLogic));
    writeFileSync(contract,withProbe(original));
    dev=spawn(process.execPath,[resolve(new URL('../../web/dev.mjs',import.meta.url).pathname),'--app','realworld','--port',String(port)],{cwd:resolve(new URL('../../..',import.meta.url).pathname),env:{...process.env,EXACT_WEB_DIST:privateDist},stdio:['ignore','pipe','pipe']});
    ready=new Promise((ok,fail)=>{const take=d=>{lines+=d;for(const line of lines.split('\n'))if(line==='plan ready')return ok()};dev.stdout.on('data',take);dev.stderr.on('data',take);dev.once('error',fail);dev.once('exit',code=>fail(new Error(`dev loop exited ${code}: ${lines.slice(-2000)}`)))});
    await Promise.race([ready,new Promise((_,fail)=>setTimeout(()=>fail(new Error(`dev loop did not start: ${lines.slice(-2000)}`)),120000))]);
    drive=await open({host:'web',app:'realworld',url:`http://127.0.0.1:${port}/`});
    await drive.tap('nav-register');
    assert.ok((await drive.tree()).nodes.some(n=>n.props?.testId==='page-register'),'the route changed before the reload');
    const stack=(await drive.state()).navigation.stack;
    await drive.carrier.evaluate(`(()=>{const el=document.querySelector('[data-testid="reload-probe"]');el.value='x'.repeat(6*1024*1024);el.dispatchEvent(new Event('input',{bubbles:true}));return el.value.length})()`);
    assert.equal(await drive.carrier.evaluate(`document.querySelector('[data-testid="reload-probe"]').value.length`),6*1024*1024);
    await drive.clock('settle');
    await waitFor(()=>drive.tree(),tree=>tree.nodes.some(n=>n.props?.testId==='row-probe-one'));
    await drive.type('row-probe-one','focused row');
    await drive.prefer({online:'false'});
    await drive.clock('+123');
    const before=await drive.state();
    assert.equal(before.resources.devReloadPage.onLine,false);
    assert.match(before.resources.devReloadAnswer.value,/^before:/);
    writeFileSync(contract,edited(withProbe(original)));
    await waitFor(()=>Promise.resolve(lines),text=>text.includes('edit → plan ready'));
    let carried;try{carried=await waitFor(()=>drive.tree(),t=>t.nodes.some(n=>n.props?.text==='conduit carried'))}catch(error){error.message+=`\npage: ${JSON.stringify(drive.carrier.hostLines)}`;throw error}
    const state=await drive.state(),probe=carried.nodes.find(n=>n.props?.testId==='row-probe-one');
    assert.equal(await drive.carrier.evaluate(`document.querySelector('[data-testid="reload-probe"]').value.length`),6*1024*1024,'a checkpoint larger than sessionStorage survives');
    assert.equal(state.slots.rowProbe,'focused row');
    assert.equal(state.clock,123);
    assert.ok(carried.nodes.some(n=>n.props?.testId==='page-register'),'the route stack survives the reload');
    assert.deepEqual(state.navigation.stack,stack,'the complete route stack survives');
    assert.equal(state.focus.logical,probe.id);
    assert.deepEqual(state.resources.devReloadAnswer,before.resources.devReloadAnswer,'an ordinary settled answer is carried when emitted logic is identical');
    assert.equal(state.resources.devReloadPage.onLine,true,'exactPage is read from the new document instead of the checkpoint');
    writeFileSync(helper,"export const devReloadValue = () => 'after:' + crypto.getRandomValues(new Uint32Array(2)).join('.');\n");
    const changedLogic=await waitFor(()=>drive.state(),s=>s.resources.devReloadAnswer?.value?.startsWith('after:'));
    assert.match(changedLogic.resources.devReloadAnswer.value,/^after:/,'an imported-helper edit changes the emitted logic digest and drops stale answers');
    // The number probe declares no devReload* sources, so the logic goes back too: its build type-checks app.ts against the contract.
    writeFileSync(contract,edited(withNumberProbe(original)));
    writeFileSync(logic,originalLogic);
    await waitFor(()=>drive.state(),s=>s.slots.reloadProbe===7);
    writeFileSync(contract,edited(original));
    await waitFor(()=>drive.state(),s=>!('reloadProbe' in s.slots));
    writeFileSync(contract,edited(withProbe(original)));
    writeFileSync(logic,withLogic(originalLogic));
    const fresh=await waitFor(()=>drive.state(),s=>s.slots.reloadProbe==='fresh');
    assert.equal(fresh.slots.reloadProbe,'fresh','a slot absent from the intervening plan is not resurrected');
  } finally {
    writeFileSync(contract,original);
    writeFileSync(logic,originalLogic);
    restoreTimes();
    rmSync(privateDist,{recursive:true,force:true});
    if(existsSync(helper))unlinkSync(helper);
    await drive?.close();
    if(dev?.exitCode===null){dev.kill('SIGTERM');await new Promise(ok=>{const t=setTimeout(ok,2000);dev.once('exit',()=>{clearTimeout(t);ok()})})}
  }
},180_000);
test('the Caltrain JS dev loop keeps its station search across an app edit', async () => {
  const contract=resolve(new URL('../../../apps/caltrain/app.contract',import.meta.url).pathname),original=readFileSync(contract,'utf8');
  const restoreTimes=keepTimes([contract]);
  const privateDist=mkdtempSync(join(tmpdir(),'exact-dev-carry-'));
  // The brand's text, wherever the layout puts it; an edit that no longer applies fails here, not as a reload that never shows.
  const edited=original.replace(/(\n\s*text )"Caltrain"( [^\n]*testId="brand")/,'$1"Caltrain reloaded"$2');
  assert.notEqual(edited, original, 'the brand edit no longer applies to apps/caltrain/app.contract');
  const listener=createServer();await new Promise((ok,fail)=>{listener.once('error',fail);listener.listen(0,'127.0.0.1',ok)});const port=listener.address().port;await new Promise(ok=>listener.close(ok));
  let dev,drive,lines='';
  const waitFor=async (read,accept,ms=30000)=>{const end=Date.now()+ms;let last;while(Date.now()<end){try{last=await read();if(accept(last))return last}catch{}await new Promise(r=>setTimeout(r,20))}throw new Error(`Caltrain reload did not become observable: ${JSON.stringify(last)}\n${lines.slice(-3000)}`)};
  try {
    dev=spawn(process.execPath,[resolve(new URL('../../web/dev.mjs',import.meta.url).pathname),'--app','caltrain','--port',String(port)],{cwd:resolve(new URL('../../..',import.meta.url).pathname),env:{...process.env,EXACT_WEB_DIST:privateDist},stdio:['ignore','pipe','pipe']});
    const ready=new Promise((ok,fail)=>{const take=d=>{lines+=d;for(const line of lines.split('\n'))if(line==='plan ready')return ok()};dev.stdout.on('data',take);dev.stderr.on('data',take);dev.once('error',fail);dev.once('exit',code=>fail(new Error(`dev loop exited ${code}: ${lines.slice(-2000)}`)))});
    await Promise.race([ready,new Promise((_,fail)=>setTimeout(()=>fail(new Error(`dev loop did not start: ${lines.slice(-2000)}`)),120000))]);
    drive=await open({host:'web',app:'caltrain',url:`http://127.0.0.1:${port}/`});
    await drive.tap('change-station');
    await drive.type('station-search','Palo');
    const before=await drive.state();
    assert.equal(before.slots.screen,'stations');assert.equal(before.slots.query,'Palo');
    writeFileSync(contract,edited);
    const tree=await waitFor(()=>drive.tree(),t=>t.nodes.some(n=>n.props?.testId==='brand'&&n.props?.text==='Caltrain reloaded'));
    const state=await drive.state(),search=tree.nodes.find(n=>n.props?.testId==='station-search');
    assert.equal(state.slots.screen,'stations');assert.equal(state.slots.query,'Palo');assert.equal(state.focus.logical,search.id);
  } finally {
    writeFileSync(contract,original);
    restoreTimes();
    rmSync(privateDist,{recursive:true,force:true});
    await drive?.close();
    if(dev?.exitCode===null){dev.kill('SIGTERM');await new Promise(ok=>{const t=setTimeout(ok,2000);dev.once('exit',()=>{clearTimeout(t);ok()})})}
  }
},180_000);
need('chrome', unavailable)('a page whose first build failed reloads into the fixed build', async () => {
  // The first build's error page has no runtime (`globalThis.exact` never
  // exists), so its reload must not wait for one. Driven over a raw DevTools
  // pipe: agent.mjs's open() waits for a runtime this page never has.
  const contract=resolve(new URL('../../../apps/realworld/app.contract',import.meta.url).pathname),original=readFileSync(contract,'utf8');
  const restoreTimes=keepTimes([contract]);
  const listener=createServer();await new Promise((ok,fail)=>{listener.once('error',fail);listener.listen(0,'127.0.0.1',ok)});const port=listener.address().port;await new Promise(ok=>listener.close(ok));
  const profile=mkdtempSync(join(tmpdir(),'exact-dev-error-')),dist=mkdtempSync(join(tmpdir(),'exact-dev-error-dist-'));
  let dev,browser,lines='';
  const waitFor=async (read,accept,ms=60000)=>{const end=Date.now()+ms;let last;while(Date.now()<end){try{last=await read();if(accept(last))return last}catch{}await new Promise(r=>setTimeout(r,50))}throw new Error(`not observed: ${JSON.stringify(last)}\n${lines.slice(-3000)}`)};
  try {
    writeFileSync(contract,original+'\nthis is not Contract (\n');
    dev=spawn(process.execPath,[resolve(new URL('../../web/dev.mjs',import.meta.url).pathname),'--app','realworld','--port',String(port)],{cwd:resolve(new URL('../../..',import.meta.url).pathname),env:{...process.env,EXACT_WEB_DIST:dist},stdio:['ignore','pipe','pipe']});
    dev.stdout.on('data',d=>{lines+=d});dev.stderr.on('data',d=>{lines+=d});
    const url=`http://127.0.0.1:${port}/`;
    await waitFor(()=>fetch(url).then(r=>r.status),status=>status===200,180000);
    browser=spawn(chrome,['--headless=new','--remote-debugging-pipe','--no-sandbox',`--user-data-dir=${profile}`,'--no-first-run','about:blank'],{stdio:['ignore','ignore','ignore','pipe','pipe']});
    const cdp=new Cdp(browser.stdio[3],browser.stdio[4]);
    const page=await waitFor(()=>cdp.send('Target.getTargets'),r=>r.targetInfos.some(t=>t.type==='page'),20000);
    const {sessionId}=await cdp.send('Target.attachToTarget',{targetId:page.targetInfos.find(t=>t.type==='page').targetId,flatten:true});
    const evaluate=expression=>cdp.send('Runtime.evaluate',{expression,returnByValue:true},sessionId).then(r=>r.result.value);
    await cdp.send('Page.navigate',{url},sessionId);
    await waitFor(()=>evaluate("document.readyState"),s=>s==='complete',20000);
    assert.equal(await evaluate("typeof globalThis.exact"),'undefined','the first build failed, so the page has no runtime');
    writeFileSync(contract,original);
    await waitFor(()=>evaluate("typeof globalThis.exact==='object'&&!!document.getElementById('exact-root')"),up=>up===true,180000);
  } finally {
    writeFileSync(contract,original);
    restoreTimes();
    if(browser?.exitCode===null){browser.kill('SIGKILL');await new Promise(ok=>{const t=setTimeout(ok,2000);browser.once('exit',()=>{clearTimeout(t);ok()})})}
    if(dev?.exitCode===null){dev.kill('SIGTERM');await new Promise(ok=>{const t=setTimeout(ok,2000);dev.once('exit',()=>{clearTimeout(t);ok()})})}
    rmSync(profile,{recursive:true,force:true});
    rmSync(dist,{recursive:true,force:true});
  }
},420_000);
test('static watcher follows immediate creation, in-place edits, and directory replacement', async () => {
  const dir=mkdtempSync(join(tmpdir(),'exact-static-edits-'));
  const assets=join(dir,'assets'),outside=join(dir,'outside'),changes=[];
  let notify=()=>{};
  const watcher=watchStaticTrees(dir,[[assets,'assets']],change=>{changes.push(change);notify();});
  // Wait on the watcher's own reports, not a wall clock: the test's timeout is the hang bound.
  const until=async predicate=>{while(!predicate())await new Promise(r=>{notify=r;});};
  const tree=()=>changes.some(c=>c.tree&&c.root===assets&&c.name==='assets');
  const leaf=name=>changes.some(c=>!c.tree&&c.relative===name&&c.name===`assets/${name}`);
  try {
    mkdirSync(join(assets,'nested'),{recursive:true});
    const file=join(assets,'nested/live.txt');writeFileSync(file,'first');
    await until(tree);changes.length=0;
    const before=statSync(file);
    writeFileSync(file,'other');utimesSync(file,before.atime,before.mtime);
    await until(()=>leaf('nested/live.txt'));changes.length=0;
    writeFileSync(join(dir,'replacement'),'third');renameSync(join(dir,'replacement'),file);
    await until(tree);changes.length=0;
    mkdirSync(join(assets,'new'));writeFileSync(join(assets,'new/leaf.txt'),'new');
    await until(tree);changes.length=0;
    writeFileSync(join(assets,'new/leaf.txt'),'changed');
    await until(()=>leaf('new/leaf.txt'));changes.length=0;
    mkdirSync(outside);writeFileSync(join(outside,'secret.txt'),'outside');
    symlinkSync(outside,join(assets,'link'));await until(tree);changes.length=0;
    writeFileSync(join(outside,'secret.txt'),'changed outside');
    await new Promise(r=>setTimeout(r,250));
    assert.deepEqual(changes,[],'no watcher traverses the linked directory');
    rmSync(assets,{recursive:true});await until(tree);changes.length=0;
    mkdirSync(assets);writeFileSync(join(assets,'again.txt'),'again');
    await until(tree);changes.length=0;
    writeFileSync(join(assets,'again.txt'),'changed again');
    await until(()=>leaf('again.txt'));
    watcher.close();changes.length=0;
    writeFileSync(join(assets,'again.txt'),'after');
    await new Promise(r=>setTimeout(r,250));assert.deepEqual(changes,[]);
  } finally {watcher.close();rmSync(dir,{recursive:true,force:true});}
},30_000); // a hang bound; every wait above is on the watcher's reports
test('dev edits clear only their own errors while unrelated failures stay visible', async () => {
  let stream, overlay, envelope, plan, seq=0, swapErrors=[], reloaded=()=>{};
  const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
  const context={TextEncoder,TextDecoder,URL,URLSearchParams,AbortController,setTimeout,clearTimeout,performance,
    console:{error(){}},location:{origin:'http://localhost',href:'http://localhost/',search:''},
    EventSource:class {constructor(){stream=this;}},
    document:{body:{appendChild(el){overlay=el;return el;}},createElement(){return {remove(){overlay=null;}};},querySelector(){return null;}},
    navigator:{sendBeacon(){}},requestAnimationFrame:callback=>callback(),crypto,
    matchMedia:()=>({matches:false,addEventListener(){}}), // the build channel's facts (LLP 1122 D4)
    localStorage:{getItem:()=>'1',setItem(){}}, // the "Open in native" link was put away; the overlay is all this page shows
    fetch:async url=>new Response(url.endsWith('app.plan')?plan:JSON.stringify(envelope)),
    exact:{ready:Promise.resolve(),compat:{inputs:{app:'fixture'}},reloadGeneration:async()=>{reloaded();return true;},
      gpu:{swap:async()=>({ms:0,errors:swapErrors})}},
  };
  runInNewContext(readFileSync(new URL('../dev.js',import.meta.url),'utf8'),context);
  const send=message=>stream.onmessage({data:JSON.stringify(message)});
  const settled=async()=>{for(let i=0;i<10;i++)await new Promise(setImmediate);};
  const text=()=>overlay?.textContent??'';
  const edited=async()=>{
    plan=Uint8Array.of(++seq);
    const card={bytes:plan.length,sha256:hash(plan)};
    const dev={epoch:'a'.repeat(32),seq,generation:hash(JSON.stringify({assets:[],plan:card}))};
    envelope={exact:1,app:{id:'fixture'},dev,assets:[],plan:{...card,url:'/app.plan'}};
    // The page fetches and hashes the generation before the host takes it, and crypto.subtle's digests
    // resolve on no fixed turn: wait for the host's reload, then the turns in which the page commits it.
    const reload=new Promise(resolve=>{reloaded=resolve;});
    send({...dev,envelope:'/exact.json'});await reload;await settled();
  };
  send({error:'Rust: unknown speed',source:'gpu'});
  await edited();
  assert.match(text(),/Rust: unknown speed/,'a valid plan does not fix Rust');
  send({error:'Contract: unknown argument'});
  assert.match(text(),/Rust: unknown speed/);
  assert.match(text(),/Contract: unknown argument/);
  send({gpu:1});await settled();
  assert.match(text(),/Contract: unknown argument/,'a valid GPU module does not fix Contract');
  assert.doesNotMatch(text(),/Rust: unknown speed/);
  await edited();assert.equal(text(),'');
  send({error:'first error',source:'gpu'});send({error:'corrected error',source:'gpu'});
  assert.doesNotMatch(text(),/first error/);assert.match(text(),/corrected error/);
  overlay.onclick();assert.equal(text(),'');
  context.exact.devError('runtime refusal');
  swapErrors=['carry refusal'];send({gpu:2});await settled();
  assert.match(text(),/runtime refusal/);assert.match(text(),/carry refusal/);
  swapErrors=[];send({gpu:3});await settled();
  assert.match(text(),/runtime refusal/);assert.doesNotMatch(text(),/carry refusal/);
  await edited();assert.equal(text(),'');
});
test('dev startup rebuilds changed Rust even when the app identity still matches', async () => {
  const dir=mkdtempSync(join(tmpdir(),'exact-dev-startup-'));
  try {
    const input=join(dir,'lib.rs'), graphPath=join(dir,'bake.json');
    const original='fn speed() -> u32 { 4 }';
    writeFileSync(input,original);
    const build={version:1,trust:'development',binary:{
      inputs:[{name:'logic/lib.rs',path:input,sha256:createHash('sha256').update(original).digest('hex')}],
      missing:[],directories:[],configuration:{flags:{EXACT_WEB_LINK:'all'}},
    }};
    writeFileSync(graphPath,JSON.stringify(build));
    let identity=true;
    const start=source.indexOf('async function currentWebBuild()');
    const end=source.indexOf('\nif (!await currentWebBuild())',start);
    const current=new Function('builtAppMatches','dist','app','graphPath','readFileSync','pendingBuildInputs',
      'const producersOnly=false;'+source.slice(start,end)+';return currentWebBuild;')
      (async()=>identity,dir,{},graphPath,readFileSync,pendingBuildInputs);
    assert.equal(await current(),true);
    writeFileSync(join(dir,'notes.txt'),'unrelated edit');
    assert.equal(await current(),true);
    writeFileSync(input,'fn speed() -> u32 { 8 }');
    assert.equal(await current(),false);
    writeFileSync(input,original);
    assert.equal(await current(),true);
    identity=false;
    assert.equal(await current(),false);
    identity=true;
    writeFileSync(graphPath,JSON.stringify({...build,trust:'production'}));
    assert.equal(await current(),false);
    // A build that links only its plan's use-set is not the dev loop's (LLP 1047 D7).
    writeFileSync(graphPath,JSON.stringify({...build,binary:{...build.binary,configuration:{flags:{}}}}));
    assert.equal(await current(),false);
    writeFileSync(graphPath,'{');
    assert.equal(await current(),false);
    unlinkSync(graphPath);
    assert.equal(await current(),false);
  } finally {rmSync(dir,{recursive:true,force:true});}
});
test('generated game arguments use the resident plan compiler without a host rebuild', () => {
  const dir=mkdtempSync(join(tmpdir(),'exact-dev-declaration-'));
  try {
    const shell=join(dir,'.shells'), logic=join(dir,'logic');
    mkdirSync(shell); mkdirSync(logic);
    const declaration=join(shell,'surfaces.json'), rust=join(logic,'lib.rs'), manifest=join(shell,'Cargo.toml');
    const swift=join(logic,'main.swift'),contract=join(dir,'app.contract'),missing=join(dir,'missing/config.json');
    const app={dir,manifest:{game:{}}}, callbacks=new Map();
    const start=source.indexOf('const watched=new Map();'), end=source.indexOf('\nwatchCompilerInputs();',start);
    const changed=new Function('app','paths','watch','resolve','existsSync','watchFile','unwatchFile',`
      const source=resolve(app.dir,'app.contract'), skipped=/(^|\\/)(target|dist)(\\/|$)/;
      const builtReceipts=[{binary:{inputs:paths.map(path=>({path})),directories:[{path:resolve(app.dir,'.shells')}],missing:[resolve(app.dir,'missing/config.json')]}}];
      const contractGraph=()=>({files:new Set()});
      const rustInputFiles=new Set(),gpuInputs=new Set(),appInputs=new Set(),failedInputs=new Set(),assetTrees=[];
      const typescript=false,portableRust=false,rebuildOn={rust:'save'},changed=new Set();
      const console={log(){},error(error){throw new Error(error);}},gpuOnly=()=>false;
      const clearTimeout=()=>{},setTimeout=()=>0,rebuild=()=>{};let timer;
      ${source.slice(start,end)}
      watchCompilerInputs();return changed;
    `)(app,[declaration,rust,manifest,swift,contract],(dir,callback)=>{callbacks.set(dir,callback);return {};},resolve,existsSync,
      (path,options,listener)=>{callbacks.set(path,()=>listener({mtimeNs:1n},{mtimeNs:0n}));},()=>{});
    callbacks.get(declaration)();
    callbacks.get(contract)();
    assert.deepEqual([...changed],[]);
    callbacks.get(rust)();
    callbacks.get(manifest)();
    assert.deepEqual([...changed],[rust,manifest]);
    changed.clear();delete app.manifest.game;
    callbacks.get(declaration)();
    assert.deepEqual([...changed],[declaration],'ordinary hosts retain declared compiler inputs');
    changed.clear();callbacks.get(shell)('change','Cargo.toml');callbacks.get(logic)('change','main.swift');
    assert.deepEqual([...changed],[],'known files have one notification owner');
    callbacks.get(shell)('rename','new-input.json');callbacks.get(logic)('rename','extra.swift');callbacks.get(missing)();
    assert.deepEqual([...changed],[join(shell,'new-input.json'),join(logic,'extra.swift'),missing],'declared trees, Swift discovery, and missing inputs remain visible');
  } finally {rmSync(dir,{recursive:true,force:true});}
});
test('GPU edit profile recognizes both authored tables and generated inline TOML', () => {
  const body=source.slice(source.indexOf('async function produceGpu(files)'));
  const line=body.split('\n').find(line=>line.trimStart().startsWith('const profile ='));
  const profile=text=>new Function('readFileSync','resolve','app','Bun',`${line};return profile;`)
    (()=>text,resolve,{workspace:'.'},Bun);
  assert.equal(profile('[profile.gpu-dev]\nopt-level=1\n'),'gpu-dev');
  assert.equal(profile('[profile]\n"gpu-dev" = { inherits="dev", opt-level=1 }\n'),'gpu-dev');
  assert.equal(profile('[profile.web]\nopt-level="z"\n'),'web');
});
test('game edits omit bake-only sources but retain runtime, code generation and cross-package includes', () => {
  const dir=mkdtempSync(join(tmpdir(),'exact-game-dev-'));
  try {
    const path=(pkg,file='lib.rs')=>join(dir,pkg,file), target=join(dir,'target');
    const pkg=(id)=>({id,name:id,manifest_path:path(id,'Cargo.toml'),targets:[{kind:['lib'],src_path:path(id)}]});
    const dep=(pkg,kind=null)=>({pkg,dep_kinds:[{kind}]});
    const metadata={packages:['app-web','core','bake','game','codegen'].map(pkg),resolve:{nodes:[
      {id:'app-web',deps:[dep('core'),dep('bake','build')]},
      {id:'core',deps:[dep('codegen','build')]},{id:'bake',deps:[dep('game')]},
      {id:'game',deps:[]},{id:'codegen',deps:[]},
    ]}};
    const app={manifest:{game:{}},target,workspace:dir,crate:()=> 'app-web'};
    const deps=join(target,'wasm32-unknown-unknown/web/deps');mkdirSync(deps,{recursive:true});
    for (const id of ['app-web','core','codegen']) writeFileSync(join(deps,id+'.d'),
      `unit: ${path(id)}${id==='app-web' ? ' '+path('game','included.rs') : ''}\n`);
    const build=join(target,'web/build/core-123abc');mkdirSync(build,{recursive:true});
    writeFileSync(join(build,'output'),`cargo:rerun-if-changed=../game/schema.rs\n`);
    const start=source.indexOf('function gameRuntimeInputs('), end=source.indexOf('\nreadGpuInputs();',start);
    const select=new Function('app','buildEnv','spawnSync','cargoReproducibilityFlags','compilerPaths','resolve','existsSync','readdirSync','readFileSync',
      source.slice(start,end)+';return gameRuntimeInputs;');
    const run=spawn=>select(app,{},spawn,()=>[],compilerPaths,resolve,existsSync,readdirSync,readFileSync);
    const inputs=new Set([path('app-web'),path('core'),path('game'),path('game','included.rs'),path('game','schema.rs'),path('codegen'),path('unknown')]);
    const selectInputs=run(()=>({status:0,stdout:JSON.stringify(metadata)}));
    assert.deepEqual(selectInputs(inputs),new Set([...inputs].filter(p=>p!==path('game'))));
    unlinkSync(join(deps,'core.d'));
    assert.equal(selectInputs(inputs),inputs,'missing compiler evidence retains the full rebuild');
    assert.throws(()=>run(()=>({status:1,stderr:'metadata unavailable'}))(inputs),/metadata unavailable/);
    app.manifest={};
    assert.equal(run(()=>{throw new Error('non-game metadata must not be queried');})(inputs),inputs);
  } finally {rmSync(dir,{recursive:true,force:true});}
});
function scheduler() {
  const legacy = !source.includes('function drainBuilds()');
  const functions = legacy ? source.slice(source.indexOf('function produceRust()'), source.indexOf('  if (!rustChild)')) + 'calls.push(["rust"]);rustActive=true;}'
    : source.slice(source.indexOf('function produceRust()'), source.indexOf('function produceRustNow()'));
  return new Function(`
    let building=false, rustActive=false, buildPending=false, rustPending=false, rustDirty=false, again=false, changed=new Set(), calls=[];
    let lastFailed=new Set(), failedInputs=new Set(), failing=false;
    const gpuOnly = files => files.length && files.every(f=>f==='gpu.rs');
    const produceGpu = files => { building=true; calls.push(['gpu',files]); };
    const rebuildNow = files => { building=true; calls.push(['full',files]); if (failing) lastFailed=new Set(files); };
    const produceRustNow = () => { rustActive=true; calls.push(['rust']); };
    ${legacy ? 'function rebuild(){if(building){again=true;return;}const files=[...changed];changed.clear();if(gpuOnly(files))produceGpu(files);else rebuildNow(files);}function drainBuilds(){rebuild();}' : ''}
    ${functions}
    return {calls, rust:produceRust, change(file){changed.add(file);buildPending=true;drainBuilds();},
      set failing(value){failing=value;}, get retrying(){return lastFailed.size>0;},
      finish(){building=false;rustActive=false;${legacy ? 'if(again||changed.size){again=false;rebuild();}' : 'drainBuilds();'}}};
  `)();
}
test('a Rust request during GPU build stays Rust, with no empty full rebuild', () => {
  const s=scheduler(); s.change('gpu.rs'); s.rust();
  assert.equal(s.calls.length,1); s.finish();
  assert.deepEqual(s.calls,[['gpu',['gpu.rs']],['rust']]); s.finish(); assert.equal(s.calls.length,2);
});
test('GPU and full builds wait for the active resident Rust producer', () => {
  const s=scheduler(); s.rust(); s.change('gpu.rs'); assert.equal(s.calls.length,1);
  s.finish(); assert.equal(s.calls[1][0],'gpu'); s.change('app.rs'); s.finish();
  assert.deepEqual(s.calls[2],['full',['app.rs']]);
});
test('a failed build\'s files ride with the next build, whatever it touches', () => {
  const s=scheduler(); s.failing=true; s.change('app.rs'); s.finish();
  assert.deepEqual(s.calls,[['full',['app.rs']]]); assert.equal(s.retrying,true);
  s.failing=false; s.change('gpu.rs'); s.finish();
  assert.deepEqual(s.calls[1],['full',['app.rs','gpu.rs']],'a GPU-only save after a failed app build rebuilds the wasm');
  assert.equal(s.retrying,false); s.change('gpu.rs'); s.finish();
  assert.deepEqual(s.calls[2],['gpu',['gpu.rs']],'a success clears the failed set');
});
test('compiler cleanup kills the recorded GPU cargo group too', () => {
  const cleanup=source.slice(source.indexOf('const killCompiler = () => {'),source.indexOf('\nstartCompiler();'));
  const killed=[];
  new Function('process', `const unasked=()=>{};let gpuBuildChild={pid:123},manualTypescript,rustSourceWatch,rustOutputWatch,rustHeartbeat,rustActive,rustChild,moduleWatch,moduleTimer,moduleRun=0,dev,moduleStage; ${cleanup}; killCompiler();`)
    ({kill:(...args)=>killed.push(args)});
  assert.deepEqual(killed,[[-123,'SIGKILL']]);
});
test('failed Cargo metadata refuses startup before choosing a dev binary', () => {
  const body=source.slice(source.indexOf('  const metadata = spawnSync(\'cargo\''),source.indexOf('  const me = dev;'));
  let spawned=false;
  assert.throws(()=>new Function('spawnSync','spawn','cargoReproducibilityFlags',`const app={workspace:'.',crate:()=> 'fixture-web'},buildEnv={},root='.',source='',plan='';let dev;${body}`)
    (()=>({status:1,stderr:'metadata fixture failure'}),()=>{spawned=true;},()=>[]),/cargo metadata failed.*metadata fixture failure/);
  assert.equal(spawned,false);
});
test('world file size is refused before reading or launching either host', async () => {
  const dir=mkdtempSync(join(tmpdir(),'exact-world-cap-')), path=join(dir,'oversized.world');
  const fd=openSync(path,'w'); ftruncateSync(fd,256*1024*1024+1); closeSync(fd);
  try { for (const host of ['web','macos']) await assert.rejects(open({host,world:path}), /world carrier exceeds 256 MiB limit/); }
  finally { rmSync(dir,{recursive:true,force:true}); }
});
test('GPU reloads refresh their includes while retaining the current app input graph', () => {
  const dir=mkdtempSync(join(tmpdir(),'exact-dev-input-owner-'));
  try {
    const target=join(dir,'target'), workspace=join(dir,'.shells');
    const app={target,workspace,crate:kind=>'fixture-'+kind,manifest:{}}, reads=[];
    const input=(kind,profile,paths)=>{
      const file=join(target,'wasm32-unknown-unknown',profile,'fixture_'+kind+'.d');
      mkdirSync(resolve(file,'..'),{recursive:true});writeFileSync(file,`unit: ${paths.map(p=>join(dir,p)).join(' ')}\n`);
    };
    input('web','web',['host.rs','shared.rs']);input('gpu','web',['game.rs','shared.rs']);
    input('gpu','gpu-dev',['game.rs','shared.rs','new-include.rs']);
    let classifications=0;
    const start=source.indexOf('function readGpuInputs('), end=source.indexOf('\nfunction gameRuntimeInputs(',start);
    const api=new Function('app','resolve','existsSync','compilerPaths','readFileSync','gameRuntimeInputs','gpuModules',`
      let appInputs=new Set(),gpuInputs=new Set();
      ${source.slice(start,end)}
      const gpuOnly=files=>files.length>0&&appInputs.size>0&&files.every(p=>p.endsWith('.rs')&&gpuInputs.has(p)&&!appInputs.has(p));
      return {refresh:readGpuInputs,gpuOnly,app:()=>[...appInputs]};
    `)(app,resolve,existsSync,compilerPaths,(path,...args)=>{reads.push(path);return readFileSync(path,...args);},inputs=>{classifications++;return inputs;},gpuModules);
    api.refresh();const hostInputs=api.app();reads.length=0;
    api.refresh('gpu-dev');
    assert.deepEqual(api.app(),hostInputs);
    assert.equal(api.gpuOnly([join(dir,'new-include.rs')]),true);
    for(const name of ['host.rs','shared.rs','unknown.rs','Cargo.toml'])assert.equal(api.gpuOnly([join(dir,name)]),false,name);
    assert.equal(classifications,1,'unchanged host inputs need no new metadata classification');
    assert.equal(reads.length,1,'GPU-only reload reads only its completed compiler input file');
    input('web','web',['new-host.rs','new-include.rs']);
    api.refresh();
    assert.deepEqual(api.app(),[join(dir,'new-host.rs'),join(dir,'new-include.rs')]);
    assert.equal(classifications,2,'a completed full build refreshes the host inputs');
    api.refresh('gpu-dev');assert.equal(api.gpuOnly([join(dir,'new-include.rs')]),false);
    api.refresh('web');assert.equal(classifications,3,'the web-profile fallback remains conservative');
  } finally {rmSync(dir,{recursive:true,force:true});}
});

test('declared source files report in-place edits after inclusion and replacement', () => {
  // The dev loop polls each declared file's metadata (`watchFile`) and debounces a build (`setTimeout`). The test
  // stats the real files itself and runs the debounce at once, so what it checks holds at any machine speed
  const dir=mkdtempSync(join(tmpdir(),'exact-source-watch-'));
  let api;
  try {
    const src=join(dir,'logic'), main=join(src,'lib.rs'), extra=join(src,'extra.rs');
    mkdirSync(src);mkdirSync(join(dir,'target'));writeFileSync(join(dir,'app.json'),'{}');
    writeFileSync(main,'one');writeFileSync(extra,'unused');
    const polled=new Map(), timers=new Map();
    const stats=path=>statSync(path,{bigint:true,throwIfNoEntry:false})??{dev:0n,ino:0n,size:0n,mtimeNs:0n,ctimeNs:0n};
    const fakeWatchFile=(path,_options,listener)=>polled.set(path,{listener,previous:stats(path)});
    const fakeUnwatchFile=path=>polled.delete(path);
    let nextTimer=0;
    const fakeSetTimeout=fn=>{timers.set(++nextTimer,fn);return nextTimer;}, fakeClearTimeout=id=>timers.delete(id);
    // Poll every watched file, then run the pending build timers
    const settle=()=>{
      for(const [path,record] of polled){const previous=record.previous;record.previous=stats(path);record.listener(record.previous,previous);}
      for(const [id,fn] of [...timers]){timers.delete(id);fn();}
    };
    const start=source.indexOf('const watched=new Map();'),end=source.indexOf('\nwatchCompilerInputs();',start);
    api=new Function('app','main','watch','resolve','existsSync','watchFile','unwatchFile','setTimeout','clearTimeout',`
      const source=resolve(app.dir,'app.contract'),skipped=/(^|\\/)(target|dist)(\\/|$)/;
      const builtReceipts=[{binary:{inputs:[{path:main}],directories:[],missing:[]}}];
      const contractGraph=()=>({files:new Set()});
      const rustInputFiles=new Set(),gpuInputs=new Set(),appInputs=new Set(),failedInputs=new Set(),assetTrees=[];
      const typescript=false,portableRust=false,rebuildOn={rust:'save'},changed=new Set();
      const console={log(){},error(message){throw Error(message);}},gpuOnly=()=>true;
      const builds=[],rebuild=()=>{builds.push([...changed]);changed.clear();};let timer;
      ${source.slice(start,end)}
      watchCompilerInputs();
      return {changed,builds,add(path){gpuInputs.add(path);watchCompilerInputs();},remove(path){gpuInputs.delete(path);watchCompilerInputs();},
        has:path=>watched.has(path),close(){for(const record of watched.values())record.close();}};
    `)({dir,manifest:{game:{}}},main,watch,resolve,existsSync,fakeWatchFile,fakeUnwatchFile,fakeSetTimeout,fakeClearTimeout);
    const edit=(path,text,atomic=false)=>{
      const count=api.builds.length;
      if(text===null)unlinkSync(path);
      else if(atomic){const temp=path+'.new';writeFileSync(temp,text);renameSync(temp,path);}
      else writeFileSync(path,text);
      settle();
      assert.deepEqual(api.builds.slice(count),[[path]],`one save of ${path} requests one build of it`);
    };
    api.add(extra);
    edit(extra,'included');
    edit(extra,'replaced',true);
    edit(extra,'edited again');
    edit(extra,null);
    edit(extra,'recreated',true);
    edit(extra,'edited after recreation');
    api.remove(extra);assert.equal(api.has(extra),false,'obsolete file watches close');
    const count=api.builds.length;writeFileSync(extra,'no longer included');
    writeFileSync(join(dir,'target/output.rs'),'output');
    writeFileSync(join(dir,'notes.txt'),'unrelated');settle();
    assert.deepEqual([...api.changed],[],'output and unrelated files do not rebuild');
    assert.equal(api.builds.length,count,'ignored edits do not build');
  } finally {api?.close();rmSync(dir,{recursive:true,force:true});}
},60_000); // a hang bound: six one-second quiet windows plus waits on the builds themselves

// E2: the local backend selection is a bake input even though .exact is ignored.
test('the JS dev loop rebuilds backend selection creation, edits and removal',async()=>{
  const {createApp}=await import('../../../game/new.mjs');
  const {fetchOrigins}=await import('../../../scripts/agent-test.mjs');
  const root=mkdtempSync(resolve(import.meta.dir,'../../../target/dev-backend-')),app=join(root,'app');
  let child,log='';
  const until=async(origin)=>{const end=Date.now()+60000;while(Date.now()<end){try{if(fetchOrigins(join(app,'target/web-dist')).includes(origin))return;}catch{}await new Promise(r=>setTimeout(r,25));}throw new Error(`backend ${origin} not served:\n${log.slice(-4000)}`);};
  try {
    createApp(app);
    writeFileSync(join(app,'app.contract'),'component App\n  resource chosen = chosen() as shape string else empty()\n  view\n    text chosen\n');
    const appId=JSON.parse(readFileSync(join(app,'app.json'),'utf8')).app.id;
    const authored="import type {Answer} from './app.contract.d.ts';\nexport const appId="+JSON.stringify(appId)+";const origin='http://127.0.0.1:4400';export const grants=`net.fetch ${origin}`;export const answer:Answer=()=>origin;\n";
    writeFileSync(join(app,'app.ts'),authored);
    const env={...process.env,EXACT_APP_DIR:app};delete env.EXACT_BACKEND_ORIGIN;
    child=spawn(process.execPath,[resolve(import.meta.dir,'../dev.mjs'),'--app','app'],{env,stdio:['ignore','pipe','pipe']});
    for(const stream of [child.stdout,child.stderr])stream.on('data',b=>{log+=b.toString();});
    await until('http://127.0.0.1:4400');
    mkdirSync(join(app,'.exact'),{recursive:true});const selection=join(app,'.exact/backend-origin');
    writeFileSync(selection,'http://127.0.0.1:4412');await until('http://127.0.0.1:4412');
    writeFileSync(selection,'http://127.0.0.1:4413');await until('http://127.0.0.1:4413');
    rmSync(selection);await until('http://127.0.0.1:4400');
    assert.equal(readFileSync(join(app,'app.ts'),'utf8'),authored);
  }finally{
    if(child?.exitCode===null){const ended=new Promise(r=>child.once('exit',r));child.kill('SIGTERM');await ended;}
    rmSync(root,{recursive:true,force:true});
  }
},240000);

test('the resident module watcher follows backend selection without other local state',async()=>{
  const dir=mkdtempSync(join(tmpdir(),'exact-resident-backend-'));let watcher,changes=0;
  const begin=source.indexOf('function watchModuleSources('),end=source.indexOf('\nlet moduleWatch',begin);
  const make=new Function('lstatSync','readdirSync','resolve','readlinkSync','statSync','watch',source.slice(begin,end)+';return watchModuleSources;')(lstatSync,readdirSync,resolve,readlinkSync,statSync,watch);
  const backendInput=new Function('return '+source.match(/const backendInput = (.*);/)[1])();
  const until=async()=>{const n=changes,end=Date.now()+3000;while(changes===n&&Date.now()<end)await new Promise(r=>setTimeout(r,10));assert.ok(changes>n,'the resident producer was notified');};
  try{
    watcher=make(dir,name=>name.startsWith('.')&&!backendInput(name),error=>{assert.equal(error,null);changes++;});
    mkdirSync(join(dir,'.exact'));const selection=join(dir,'.exact/backend-origin');
    writeFileSync(selection,'http://127.0.0.1:4412');await until();
    writeFileSync(selection,'http://127.0.0.1:4413');await until();
    rmSync(selection);await until();
    const before=changes;writeFileSync(join(dir,'.exact/other'),'not an input');
    await new Promise(r=>setTimeout(r,250));assert.equal(changes,before);
  }finally{watcher?.close();rmSync(dir,{recursive:true,force:true});}
},15000);
