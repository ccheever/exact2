#!/usr/bin/env node
// @ref LLP 1086 D12 — pin changes qualify the authored examples, including reloads.
import assert from 'node:assert/strict';
import {spawn, spawnSync} from 'node:child_process';
import {existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync} from 'node:fs';
import {createServer} from 'node:net';
import {dirname, join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {checkRepository, digest, receiptPath, testedFiles} from './check-snapback-readme.mjs';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const read=name=>readFileSync(join(root,name),'utf8');
const filesOf=['snapback/schema.q','app.contract','app.ts','app.test.contract'];

export function extractExamples(readme, recipe) {
  const sections=[...readme.matchAll(/^### (A complete[^\n]+)\n([\s\S]*?)(?=^### |^## |$(?![\s\S]))/gm)];
  const examples=sections.map(([,name,text])=>({name,online:name!=='A complete app',blocks:[...text.matchAll(/```(?:quarry|contract(?:-test)?|ts)?\n([\s\S]*?)\n```/g)].map(m=>m[1])}));
  const online=examples.find(e=>e.name==='A complete online app');
  for(const example of examples) {
    if(example.name==='A complete online password app' && example.blocks.length===3)
      example.blocks.unshift(online.blocks[0].replace('use personas alice, bob','use identity password'));
  }
  for(const [,name,text] of recipe.matchAll(/^### ([^\n]+)\n([\s\S]*?)(?=^### |^## |$(?![\s\S]))/gm)) {
    const blocks=[...text.matchAll(/```(?:quarry|contract(?:-test)?|ts)\n([\s\S]*?)\n```/g)].map(m=>m[1]);
    if(blocks.length) examples.push({name:`Auth recipe: ${name}`,online:true,blocks});
  }
  assert.ok(examples.length>=4,'README and auth recipe must contain every complete example (three guestbooks and mixed auth)');
  for(const example of examples) {
    assert.equal(example.blocks.length,4,`${example.name}: expected schema, Contract, TypeScript and test fences`);
    assert.ok(/^test /m.test(example.blocks[3]),`${example.name}: no tests`);
  }
  return examples;
}
export function refreshClaims(text, version, date) {
  return text.replace(/tested as written against\s+\d+\.\d+\.\d+\s*\(\d{4}-\d{2}-\d{2}\)/g,`tested as written against ${version} (${date})`);
}
export function commitFiles(changes, {read=readFileSync,write=writeFileSync,remove=rmSync}={}) {
  const before=new Map([...changes].map(([path])=>[path,existsSync(path)?read(path):null]));
  try {for(const [path,content] of changes) write(path,content);}
  catch(error) {
    for(const [path,content] of before) {if(content===null) remove(path,{force:true});else write(path,content);}
    throw error;
  }
}
function run(command,args,cwd,env) {
  const result=spawnSync(command,args,{cwd,env,encoding:'utf8',timeout:600_000,maxBuffer:16*1024*1024});
  if(result.status!==0) throw new Error(`${command} ${args.join(' ')} failed (${result.error?.message ?? result.status}):\n${result.stdout ?? ''}\n${result.stderr ?? ''}`);
  return result;
}
async function reservePort() {
  const socket=createServer(); await new Promise((ok,fail)=>{socket.once('error',fail);socket.listen(0,'127.0.0.1',ok);});
  const port=socket.address().port;await new Promise(ok=>socket.close(ok));return port;
}
async function startServer(binary,port,cwd,env) {
  const server=spawn(binary,['dev','--port',String(port),'--no-watch'],{cwd,env,stdio:['ignore','pipe','pipe']});
  try {
    await new Promise((ok,fail)=>{
      let log='';const timer=setTimeout(()=>fail(new Error(`Snapback backend did not start: ${log}`)),30_000);
      const finish=error=>{clearTimeout(timer);error?fail(error):ok();};
      const read=bytes=>{log+=bytes;if(/SNAPBACK4_READY|^http:\/\/127\.0\.0\.1:\d+\s*$/m.test(log))finish();};
      server.stdout.on('data',read);server.stderr.on('data',read);
      server.once('error',finish);server.once('exit',code=>finish(new Error(`backend exited ${code}: ${log}`)));
    });
    return server;
  } catch(error) {if(server.exitCode===null)server.kill('SIGTERM');throw error;}
}
async function stopServer(server) {
  if(!server || server.exitCode!==null)return;
  await new Promise(ok=>{server.once('exit',ok);server.kill('SIGTERM');});
}
async function executeExample(example,base,version,registry,env,bun) {
  const project=mkdtempSync(join(base,'example-')), app=join(project,'app');let server;
  try {
    writeFileSync(join(project,'package.json'),JSON.stringify({private:true,type:'module',dependencies:{snapback4:version}}));
    run(bun,['install','--registry',registry],project,env);
    const binary=join(project,`node_modules/snapback4-${process.platform}-${process.arch}/snapback4`);
    const localEnv={...env,SNAPBACK4_BIN:binary};
    for(const name of ['snapback4',`snapback4-${process.platform}-${process.arch}`])
      assert.equal(JSON.parse(readFileSync(join(project,'node_modules',name,'package.json'),'utf8')).version,version);
    mkdirSync(join(project,'snapback'),{recursive:true});
    writeFileSync(join(project,filesOf[0]),example.blocks[0]+'\n');
    const port=await reservePort();
    const exact=(args,cwd=app)=>run(bun,[cwd===app?'exact.mjs':join(root,'scripts/exact.mjs'),...args],cwd,localEnv);
    exact(['new',app,'--snapback',...(example.online?['--online']:[]),'--port',String(port)],project);
    for(const [i,name] of filesOf.slice(1).entries())writeFileSync(join(app,name),example.blocks[i+1]+'\n');
    const manifest=JSON.parse(readFileSync(join(app,'app.json'),'utf8'));
    const appId=/export const appId\s*=\s*['"]([^'"]+)['"]/.exec(example.blocks[2]);
    assert.ok(appId,`${example.name}: app.ts needs its documented appId`);
    manifest.app.id=appId[1];writeFileSync(join(app,'app.json'),JSON.stringify(manifest,null,2)+'\n');
    exact(['snapback','--port',String(port)]);
    exact(['contract','types','app.contract','-o','app.contract.d.ts']);
    server=await startServer(binary,port,project,localEnv);
    const tests=[...example.blocks[3].matchAll(/^test /gm)].length;
    for(let round=0;round<2;round++) {
      const result=exact(['test','web','--backend',`http://127.0.0.1:${port}`]);
      assert.match(result.stdout,new RegExp(`\\b${tests} passed, 0 failed\\b`));
    }
    for(const [i,name] of filesOf.slice(1).entries())assert.equal(readFileSync(join(app,name),'utf8'),example.blocks[i+1]+'\n',`${example.name}: authored ${name} changed`);
    exact(['contract','types','--check','app.contract']);
    console.log(`${example.name}: ${tests} tests passed twice as written (${version})`);
    return {name:example.name,tests,runs:2};
  } finally {await stopServer(server);rmSync(project,{recursive:true,force:true});}
}
export async function retest({version,registry='https://registry.npmjs.org/',revision}) {
  if(!/^\d+\.\d+\.\d+$/.test(version ?? ''))throw new Error('--version requires a complete version');
  const registryUrl=new URL(registry);if(!['http:','https:'].includes(registryUrl.protocol))throw new Error('--registry must be an HTTP(S) URL');
  const bun=process.env.BUN ?? 'bun';
  const env={...process.env,CARGO_INCREMENTAL:'0',CARGO_PROFILE_DEV_DEBUG:'0',CARGO_PROFILE_TEST_DEBUG:'0'};
  const metadata=JSON.parse(run('npm',['view',`snapback4@${version}`,'version','gitHead','dist.integrity','--json','--registry',registry],root,env).stdout);
  assert.equal(typeof metadata==='string'?metadata:metadata.version,version,'registry answered another version');
  const pkg=JSON.parse(read('package.json')), previous=pkg.devDependencies.snapback4;
  const cargo=read('snapback4/Cargo.toml'), oldRevision=/snapback4-core = .*rev = "([a-f0-9]{40})"/.exec(cargo)?.[1];
  revision ??= metadata.gitHead ?? (version===previous?oldRevision:null);
  if(!/^[a-f0-9]{40}$/.test(revision ?? ''))throw new Error('Registry has no source gitHead: pass --revision <immutable published source SHA> to move the Cargo pin');
  const initial=Object.fromEntries(testedFiles.map(name=>[name,read(name)]));
  const examples=extractExamples(initial['snapback4/README.md'],initial['docs/recipes/snapback-auth.md']);
  const protectedNames=['package.json','bun.lock','snapback4/Cargo.toml','snapback4/Cargo.lock'];
  const snapshots=new Map(protectedNames.map(name=>[join(root,name),readFileSync(join(root,name))]));
  mkdirSync(join(root,'target'),{recursive:true});const base=mkdtempSync(join(root,'target/readme-retest-'));
  let complete=false;
  const restore=()=>{if(!complete)for(const [path,bytes] of snapshots){if(bytes===null)rmSync(path,{force:true});else writeFileSync(path,bytes);}};
  const interrupted=()=>{restore();process.exit(130);};process.once('SIGINT',interrupted);process.once('SIGTERM',interrupted);
  try {
    // Candidate pins are rolled back on every failed test. Claims and receipt are
    // untouched until all authored tests have completed against this candidate.
    if(version!==previous || revision!==oldRevision) {
      writeFileSync(join(root,'package.json'),read('package.json').replace(/("snapback4"\s*:\s*")[^"]+(")/,(_match,prefix,suffix)=>prefix+version+suffix));
      writeFileSync(join(root,'snapback4/Cargo.toml'),cargo.replaceAll(previous,version).replaceAll(oldRevision,revision));
      run('cargo',['update','--workspace','--manifest-path',join(root,'snapback4/Cargo.toml')],root,env);
    }
    // Reinstall even on a same-version rerun: the previous attempt may have
    // installed a candidate, or used a different registry.
    run(bun,['install','--registry',registry],root,env);
    const results=[];const failures=[];
    for(const example of examples) {
      try {results.push(await executeExample(example,base,version,registry,env,bun));}
      catch(error){failures.push(`${example.name}: ${error.message}`);console.error(failures.at(-1));}
    }
    if(failures.length)throw new Error(`${failures.length} README example(s) failed; pins and claims were restored`);
    for(const name of testedFiles.filter(name=>!protectedNames.includes(name)))
      if(read(name)!==initial[name])throw new Error(`${name} changed during the run; retest the current text`);
    const date=new Date().toISOString().slice(0,10),changes=new Map();
    for(const name of ['snapback4/README.md','docs/recipes/snapback-auth.md']) {
      const text=refreshClaims(read(name).replaceAll(previous,version),version,date);
      changes.set(join(root,name),text);
    }
    for(const name of ['AGENTS.md','README.md','docs/reference.md','docs/start-here.md'])
      changes.set(join(root,name),read(name).replaceAll(previous,version).replaceAll(oldRevision,revision));
    const tested=Object.fromEntries(testedFiles.map(name=>[name,changes.get(join(root,name)) ?? read(name)]));
    const receipt={format:1,version,revision,registry:registryUrl.href,date,examples:results,
      files:Object.fromEntries(testedFiles.map(name=>[name,digest(tested[name])]))};
    changes.set(join(root,receiptPath),JSON.stringify(receipt,null,2)+'\n');
    // Validate the full receipt before publishing any claim.
    const {checkReceipt,checkReadmeVersion}=await import('./check-snapback-readme.mjs');
    checkReadmeVersion(tested['snapback4/README.md'],version);checkReceipt(receipt,version,tested);
    for(const [path] of changes) if(!snapshots.has(path))snapshots.set(path,existsSync(path)?readFileSync(path):null);
    commitFiles(changes);checkRepository(root);complete=true;
    console.log(`Snapback pin and ${results.length} tested-as-written examples refreshed: ${version} (${date})`);
    return receipt;
  } finally {restore();process.removeListener('SIGINT',interrupted);process.removeListener('SIGTERM',interrupted);rmSync(base,{recursive:true,force:true});}
}
if(process.argv[1] && resolve(process.argv[1])===fileURLToPath(import.meta.url)) {
  try {
    const options={},args=process.argv.slice(2);
    if(args.length===1 && ['--help','-h'].includes(args[0])) {
      console.log('usage: node scripts/retest-snapback-readme.mjs --version <v> [--registry <url>] [--revision <published-source-sha>]\nRuns every complete README/auth-recipe app twice. A failure restores pins and leaves claims unchanged.\nThe registry gitHead supplies the Cargo source; --revision is needed when that metadata is absent. No commit is made.');
    } else {
      for(let i=0;i<args.length;i+=2) {const name=args[i].slice(2);if(!['version','registry','revision'].includes(name)||!args[i+1]||options[name])throw new Error('Invalid arguments; use --help');options[name]=args[i+1];}
      await retest(options);
    }
  } catch(error) {console.error(error.message);process.exitCode=1;}
}
