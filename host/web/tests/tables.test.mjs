import {test, expect} from 'bun:test';
import {readFileSync, writeFileSync, existsSync, mkdirSync, mkdtempSync, rmSync} from 'node:fs';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
import {spawn, spawnSync} from 'node:child_process';
import {loadavg} from 'node:os';
import {tableOperation, watchTables} from '../../../game/app/tables.mjs';
import {filesystem} from '../../../scripts/filesystem.mjs';
import {devGenerationDigest} from '../serve.mjs';
import {pendingBuildInputs} from '../../../scripts/app.mjs';
const source = readFileSync(new URL('../dev.mjs', import.meta.url), 'utf8');

test('table publication counts the whole operation and refuses imported subprocess helpers', () => {
  let published = false;
  const timing = tableOperation(() => { published = true; });
  expect(published).toBe(true); expect(timing.processes).toBe(0);
  expect(() => tableOperation(() => spawnSync('cargo', ['--version']))).toThrow('prohibited subprocess');
  expect(() => tableOperation(() => filesystem({op:'get',root:'.',path:'Cargo.toml'}))).toThrow('prohibited subprocess');
  expect(spawnSync(process.execPath, ['--version']).status).toBe(0);
});

test('a table generation publishes with zero spawns and preserves the admitted plan and siblings', () => {
  const plan = Buffer.from('plan'), before = Buffer.from('old'), next = Buffer.from('new');
  const hash = b => createHash('sha256').update(b).digest('hex');
  const start = source.indexOf('const tableGenerations = new Map();'), end = source.indexOf('\nlet dev = null;', start);
  const make = new Function('createHash','canonicalBytes','webEnvelope','resolve','filesystem','devGenerationDigest',`
    const typescript=true,portableRust=true,currentModule=new Map([['app.plan',Buffer.from('unpublished')],['plans/pending.plan',Buffer.from('pending')]]),currentRust=null,app={},dist='unused';
    const epoch='a'.repeat(32),program='program',seq=2;let retentionToken=null;
    let current={files:new Map([['app.plan',Buffer.from('plan')],['assets/exact-tables.json',Buffer.from('old')],['assets/other',Buffer.from('sibling')],['plans/admitted.plan',Buffer.from('admitted')]]),envelope:{assets:[{name:'assets/exact-tables.json',sha256:'old'},{name:'assets/other',sha256:'sibling'},{name:'plans/admitted.plan',sha256:'admitted'}]}};
    const classifyGeneration=()=>[],announcePlan=()=>{},keepMap=()=>{},console={log(){}};
    const retainDevGeneration=()=>{throw Error('table publication used the disk helper');};
    ${source.slice(start,end)}
    return {publish(bytes){captureGeneration(true,new Map([['assets/exact-tables.json',bytes]]));return current;}};
  `);
  const api = make(createHash, x=>Buffer.from(JSON.stringify(x)), (_,bytes,assets)=>({plan:{sha256:hash(bytes),bytes:bytes.length},assets}),resolve,filesystem,devGenerationDigest);
  let result;
  expect(tableOperation(() => { result = api.publish(next); }).processes).toBe(0);
  expect(result.files.get('app.plan')).toEqual(plan);
  expect(result.files.get('assets/exact-tables.json')).toEqual(next);
  expect(result.files.get('assets/other').toString()).toBe('sibling');
  expect(result.envelope.assets[0].sha256).toBe(hash(next));
  expect(result.files.get('plans/admitted.plan').toString()).toBe('admitted');
  expect(result.files.has('plans/pending.plan')).toBe(false);
  expect(result.envelope.module).toBeUndefined();
});

test('primary declaration reload validates and publishes current files without a second edit', async () => {
  const root = mkdtempSync(resolve(import.meta.dir, '../../../target/table-reload-'));
  let watcher;
  try {
    const app=resolve(root,'app'),dist=resolve(root,'dist');
    for(const dir of [app,dist])mkdirSync(resolve(dir,'assets'),{recursive:true});
    const module = names => {
      writeFileSync(resolve(dist,'gpu_bg.wasm'), new Uint8Array());
      writeFileSync(resolve(dist,'gpu.js'), `export function initSync(){};export function exact_game_tables(text){if(!text)return ${JSON.stringify(JSON.stringify(names))};const rows=JSON.parse(text);for(const name of ${JSON.stringify(names)})if(!JSON.parse(rows[name]).speed)throw Error(name+'.speed');return text;}`);
    };
    writeFileSync(resolve(app,'first.json'),'{"speed":4}'); module(['first.json']);
    const published=[],errors=[],attempts=[],timings=[];
    watcher=await watchTables({app,dist,publish:(name,bytes)=>published.push(JSON.parse(bytes)),refused:(e,text)=>{errors.push(e);attempts.push(text);},reported:t=>timings.push(t)});
    writeFileSync(resolve(app,'second.json'),'{"speed":8}'); module(['first.json','second.json']);
    await watcher.reload();
    expect(Object.keys(published.at(-1))).toEqual(['first.json','second.json']);
    expect(timings.every(t=>t.processes===0)).toBe(true); expect(errors).toEqual([]);
    writeFileSync(resolve(app,'second.json'),'{}'); watcher.flush();
    expect(errors.at(-1)).toContain('second.json.speed'); expect(published).toHaveLength(2);
    expect(JSON.parse(attempts.at(-1))['second.json']).toBe('{}');
    expect(readFileSync(resolve(dist,'assets/exact-tables.json'),'utf8')).toContain('speed');
  } finally {watcher?.close();rmSync(root,{recursive:true,force:true});}
});

test('unchanged JS and Rust modules allow live table delivery; changed modules require a generation', () => {
  const browser = readFileSync(new URL('../dev.js',import.meta.url),'utf8');
  const sameModule = new Function(browser.slice(browser.indexOf('function sameModule('),browser.indexOf('async function readBounded'))+'return sameModule;')();
  const bytes = n=>new Uint8Array([n]);
  const module={receipt:bytes(1),script:bytes(2)},rust={receipt:bytes(3),module:bytes(4)};
  const predicate = new Function('candidate','activeGeneration','sameModule',`const names=candidate.surfaceAssets;return ${browser.match(/const liveAssets = ([\s\S]*?);\n/)[1]};`);
  const active={module,rust,planSha256:'plan',assets:new Map([['assets/exact-tables.json',{sha256:'old'}]])};
  const candidate={...active,surfaceAssets:['assets/exact-tables.json'],assets:new Map([['assets/exact-tables.json',{sha256:'new'}]]),module:{...module},rust:{...rust}};
  expect(predicate(candidate,active,sameModule)).toBe(true);
  candidate.rust={...rust,module:bytes(5)};expect(predicate(candidate,active,sameModule)).toBe(false);
  candidate.rust=rust;candidate.module={...module,script:bytes(6)};expect(predicate(candidate,active,sameModule)).toBe(false);
});

test('web proof profile selection is retained under production trust', () => {
  const build=readFileSync(new URL('../build.mjs',import.meta.url),'utf8');
  // The selection alone: through `const gpuProfile`'s line (what follows warms the build's environment).
  const start=build.indexOf('const proofProfile ='),end=build.indexOf('\n',build.indexOf('const gpuProfile =',start));
  const select=new Function('game','buildEnv','process',build.slice(start,end)+'return gpuProfile;');
  expect(select(true,{EXACT_UPDATE_TRUST:'production'},{env:{EXACT_GAME_PROOF_PROFILE:'gpu-dev'}})).toBe('gpu-dev');
  expect(select(true,{EXACT_UPDATE_TRUST:'production'},{env:{EXACT_GAME_PROOF_PROFILE:'release'}})).toBe('web');
});

test('real game dev journals rejected edits and reloads tables after a gpu-dev primary build', async () => {
  const {open} = await import('../../../scripts/agent.mjs');
  const app=resolve(import.meta.dir,'../../../game/games/beacons');
  const table=resolve(app,'movement.json'),rust=resolve(app,'logic/src/lib.rs');
  const originals=[table,rust].map(path=>({path,text:readFileSync(path,'utf8')}));
  const oldApp=process.env.EXACT_APP_DIR;
  const env={...process.env,PATH:`${process.env.HOME}/.cargo/bin:${process.env.PATH}`,EXACT_APP_DIR:app};delete env.EXACT_GAME_PROOF_PROFILE;
  let child,drive,log='';
  const until=async(read,ms=60000,step='dev condition')=>{const end=Date.now()+ms;while(Date.now()<end){if(child?.exitCode!==null&&child?.exitCode!==undefined)throw Error('dev exited '+child.exitCode+':\n'+log);const value=await read();if(value)return value;await Bun.sleep(50);}throw Error(step+' missing after '+ms/1000+' s:\n'+log.slice(-4000));};
  try {
    process.env.EXACT_APP_DIR=app;
    child=spawn(process.execPath,[resolve(import.meta.dir,'../../../game/dev.mjs'),'beacons'],{env,stdio:['ignore','pipe','pipe']});
    for(const stream of [child.stdout,child.stderr])stream.on('data',bytes=>{log+=bytes;});
    await until(()=>log.includes('plan ready'),600000);
    const url=await until(()=>log.match(/http:\/\/127\.0\.0\.1:\d+/)?.[0]);expect(url).toBeTruthy();
    drive=await open({host:'web',app:'beacons',url:url+'/',size:[1280,720]});await drive.tap('play');
    let world=drive.world('world');
    const valid=speed=>JSON.stringify({speed:[[0,speed],[1,speed]],jump:1.2});
    const editToObserved=async(speed,edits,label)=>{
      const load=loadavg()[0],started=performance.now();
      writeFileSync(table,valid(speed));
      await until(async()=>(await world.snapshot()).tableEdits===edits);
      await world.run(17);
      expect((await world.get('player','Character')).speed).toBe(speed);
      const milliseconds=performance.now()-started;
      console.log(`table edit-to-observed ${label}: ${milliseconds.toFixed(1)} ms; load ${load.toFixed(2)} -> ${loadavg()[0].toFixed(2)}`);
      if(milliseconds>=1000)throw Error(`table edit-to-observed exceeded 1000 ms: ${milliseconds.toFixed(1)} ms`);
    };
    await editToObserved(8,1,'warm');
    for(const [text,path] of [
      ['{"speed":"wrong","jump":1.2}','movement.json.Movement.speed'],
      ['{"speed":[[0,8],[1,8]]}','movement.json.Movement.jump'],
      ['{"speed":[[0],[1,8]],"jump":1.2}','movement.json.Movement.speed.0'],
    ]) {
      await drive.logs();writeFileSync(table,text);
      await until(async()=>JSON.stringify((await drive.logs()).world ?? []).includes(path));
      expect((await world.snapshot()).tableEdits).toBe(1);
      await world.run(17);expect((await world.get('player','Character')).speed).toBe(8);
    }
    // Delay actual table transport, keeping the same file-to-gameplay oracle.
    await drive.carrier.evaluate(`globalThis.tableFetch=globalThis.fetch;globalThis.delayedTables=0;
      globalThis.fetch=async(...args)=>{
        if(String(args[0]).includes('exact-tables.json')){globalThis.delayedTables++;await new Promise(ok=>setTimeout(ok,1100));}
        return globalThis.tableFetch(...args);
      };`);
    try {
      await expect(editToObserved(10,2,'delayed')).rejects.toThrow('table edit-to-observed exceeded 1000 ms');
      expect(await drive.carrier.evaluate('globalThis.delayedTables')).toBeGreaterThan(0);
    } finally {
      await drive.carrier.evaluate('globalThis.fetch=globalThis.tableFetch;delete globalThis.tableFetch;delete globalThis.delayedTables');
    }
    writeFileSync(table,originals[0].text);await until(async()=>(await world.snapshot()).tableEdits===3);
    const offset=log.length;
    writeFileSync(rust,originals[1].text+'\n// Exercise the primary development rebuild.\n');
    await until(()=>log.slice(offset).includes('rust: rebuilt'),600000);
    const receipt=JSON.parse(readFileSync(resolve(app,'dist/bake.json'),'utf8'));
    expect(receipt.products.some(p=>p.path.endsWith('/gpu-dev/beacons_gpu.wasm'))).toBe(true);
    // The rebuild restarts its resident producer; wait for its first generation. A game with data of its own bakes
    // through its web shell's dev bin, which the rebuild relinks in release (about 40 s at load 10, more loaded): a hang
    // guard sized for a loaded machine, not a deadline (theme2's 05:50 ruling; QUEUE: a dev profile for -dev bins).
    await until(async()=>(await fetch(url+'/exact.json')).ok,300000,'the rebuilt producer\'s first generation (exact.json)');
    await drive.close();
    drive=await open({host:'web',app:'beacons',url:url+'/',size:[1280,720]});
    await drive.tap('play');world=drive.world('world');
    // Removing tuning.reload() leaves the watcher closed after the primary build.
    await editToObserved(9,1,'after primary rebuild');
    expect(log).toMatch(/table: validated and published in .*subprocesses 0/);
  } finally {
    await drive?.close();
    if(child?.exitCode===null){const ended=new Promise(ok=>child.once('exit',ok));child.kill('SIGTERM');await ended;}
    for(const {path,text} of originals)writeFileSync(path,text);
    // Proofs may reuse a build receipt, so restore the generated tables through its validator too.
    const restored=await watchTables({app,dist:resolve(app,'dist'),publish:()=>false,refused:error=>{throw new Error(error);}});
    restored.close();
    if(oldApp===undefined)delete process.env.EXACT_APP_DIR;else process.env.EXACT_APP_DIR=oldApp;
  }
}, 1_500_000);

test('no-table game carries its world across Rust edits after cold start and profile switches', async () => {
  const {createGame} = await import('../../../game/new.mjs');
  const {open} = await import('../../../scripts/agent.mjs');
  // The dev watcher excludes target/; generate the authored sources beside the games.
  const root=mkdtempSync(resolve(import.meta.dir,'../../../game/games/dev-carry-'));
  const app=resolve(root,'carry-game'),rust=resolve(app,'logic/src/lib.rs');
  const oldApp=process.env.EXACT_APP_DIR;
  const env={...process.env,PATH:`${process.env.HOME}/.cargo/bin:${process.env.PATH}`,EXACT_APP_DIR:app};
  delete env.EXACT_GAME_PROOF_PROFILE;delete env.CARGO_TARGET_DIR;
  let child,drive,log='',passed=false;
  const until=async(read,ms=60000)=>{const end=Date.now()+ms;while(Date.now()<end){if(child?.exitCode!==null&&child?.exitCode!==undefined)throw Error('dev exited '+child.exitCode+':\n'+log);const value=await read();if(value)return value;await Bun.sleep(50);}throw Error('dev condition missing:\n'+log.slice(-4000));};
  const stop=async()=>{
    await drive?.close();drive=null;
    if(child?.exitCode===null){const ended=new Promise(ok=>child.once('exit',ok));child.kill('SIGTERM');await ended;}
  };
  try {
    createGame(app);
    expect(existsSync(resolve(app,'target'))).toBe(false);
    process.env.EXACT_APP_DIR=app;
    let previousSpeed;
    for (const [profile,speed] of [['gpu-dev',7],['web',8],['gpu-dev',9]]) {
      const dependency=resolve(app,`target/wasm32-unknown-unknown/${profile}/carry_game_gpu.d`);
      // Leave the previous receipt and artifacts intact, but remove the requested
      // profile's dependency file so a cache mismatch cannot classify the edit.
      rmSync(dependency,{force:true});expect(existsSync(dependency)).toBe(false);
      if(profile==='web')env.EXACT_GAME_PROOF_PROFILE='release';else delete env.EXACT_GAME_PROOF_PROFILE;
      const start=log.length;
      child=spawn(process.execPath,[resolve(import.meta.dir,'../../../game/dev.mjs'),app],{env,stdio:['ignore','pipe','pipe']});
      for(const stream of [child.stdout,child.stderr])stream.on('data',bytes=>{log+=bytes;});
      await until(()=>log.slice(start).includes('plan ready'),1_200_000);
      expect(existsSync(dependency)).toBe(true);
      const receipt=JSON.parse(readFileSync(resolve(app,'dist/bake.json'),'utf8'));
      expect(receipt.products.some(p=>p.path.endsWith(`/${profile}/carry_game_gpu.wasm`))).toBe(true);
      if(previousSpeed===undefined)expect(existsSync(resolve(app,'target/wasm32-unknown-unknown/web/carry_game_gpu.d'))).toBe(false);
      const url=await until(()=>log.slice(start).match(/http:\/\/127\.0\.0\.1:\d+/)?.[0]);expect(url).toBeTruthy();
      drive=await open({host:'web',app:'carry-game',url:url+'/',size:[1280,720]});
      await drive.tap('play');const world=drive.world('world');
      await world.hold('KeyW',500);
      const before=await world.snapshot(),position=await world.local_position('player');
      expect(before.tick).toBeGreaterThan(0);expect(position[2]).toBeLessThan(0);
      // Keep the driver clock, but accept the ordinary dev client's automatic GPU swaps.
      await drive.carrier.evaluate("const url=new URL(location.href);url.searchParams.delete('agent');history.replaceState(null,'',url)");
      const source=readFileSync(rust,'utf8'),offset=log.length;
      writeFileSync(rust,previousSpeed===undefined
        ? source.replace('        w.character("player")', `        w.require_mut::<Character>("player").speed = ${speed}.0;\n        w.character("player")`)
        : source.replace(`.speed = ${previousSpeed}.0;`,`.speed = ${speed}.0;`));
      await until(()=>log.slice(offset).includes('· swapped in'),600000);
      expect(log.slice(offset)).not.toContain('rust: rebuilt');
      const after=await world.snapshot();
      expect(after.tick).toBe(before.tick);expect(after.hash).toBe(before.hash);
      expect(await world.local_position('player')).toEqual(position);
      await world.run(9);expect((await world.get('player','Character')).speed).toBe(speed);
      previousSpeed=speed;
      await stop();
      // A side edit does not update the primary build receipt. Rebuild the primary
      // in this session's profile before switching, to isolate profile reuse from freshness.
      const built=spawnSync(process.execPath,[resolve(import.meta.dir,'../build.mjs'),'--wasm'],{
        env:{...env,EXACT_WEB_DIST:resolve(app,'dist'),EXACT_WEB_LINK:'all',EXACT_UPDATE_TRUST:'development',EXACT_GAME_PROOF_PROFILE:profile==='web'?'release':'gpu-dev'},encoding:'utf8'});
      log+=built.stdout+built.stderr;expect(built.status).toBe(0);
      expect(pendingBuildInputs(JSON.parse(readFileSync(resolve(app,'dist/bake.json'),'utf8')))).toEqual([]);
    }
    passed=true;
  } finally {
    await stop();
    writeFileSync(resolve(root,'dev.log'),log);
    if(oldApp===undefined)delete process.env.EXACT_APP_DIR;else process.env.EXACT_APP_DIR=oldApp;
    if(passed)rmSync(root,{recursive:true,force:true});else console.error(`dev carry evidence: ${root}`);
  }
}, 3_000_000);
