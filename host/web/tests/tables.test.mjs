import {test, expect} from 'bun:test';
import {readFileSync, writeFileSync, mkdirSync, mkdtempSync, rmSync} from 'node:fs';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
import {spawnSync} from 'node:child_process';
import {tableOperation, watchTables} from '../../../game/app/tables.mjs';
import {filesystem} from '../../../scripts/filesystem.mjs';
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
  const start = source.indexOf('const tableGenerations = new Map();'), end = source.indexOf('// The resident compiler', start);
  const make = new Function('createHash','canonicalBytes','webEnvelope','resolve','filesystem',`
    const typescript=false,portableRust=false,currentModule=null,currentRust=null,app={},dist='unused';
    const epoch='a'.repeat(32),program='program',seq=2;let retentionToken=null;
    let current={files:new Map([['app.plan',Buffer.from('plan')],['assets/exact-tables.json',Buffer.from('old')],['assets/other',Buffer.from('sibling')]]),envelope:{assets:[{name:'assets/exact-tables.json',sha256:'old'},{name:'assets/other',sha256:'sibling'}]}};
    const classifyGeneration=()=>[],announcePlan=()=>{},keepMap=()=>{},console={log(){}};
    const retainDevGeneration=()=>{throw Error('table publication used the disk helper');};
    ${source.slice(start,end)}
    return {publish(bytes){captureGeneration(true,new Map([['assets/exact-tables.json',bytes]]));return current;}};
  `);
  const api = make(createHash, x=>Buffer.from(JSON.stringify(x)), (_,bytes,assets)=>({plan:{sha256:hash(bytes),bytes:bytes.length},assets}),resolve,filesystem);
  let result;
  expect(tableOperation(() => { result = api.publish(next); }).processes).toBe(0);
  expect(result.files.get('app.plan')).toEqual(plan);
  expect(result.files.get('assets/exact-tables.json')).toEqual(next);
  expect(result.files.get('assets/other').toString()).toBe('sibling');
  expect(result.envelope.assets[0].sha256).toBe(hash(next));
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
    const published=[],errors=[],timings=[];
    watcher=await watchTables({app,dist,publish:(name,bytes)=>published.push(JSON.parse(bytes)),refused:e=>errors.push(e),reported:t=>timings.push(t)});
    writeFileSync(resolve(app,'second.json'),'{"speed":8}'); module(['first.json','second.json']);
    await watcher.reload();
    expect(Object.keys(published.at(-1))).toEqual(['first.json','second.json']);
    expect(timings.every(t=>t.processes===0)).toBe(true); expect(errors).toEqual([]);
    writeFileSync(resolve(app,'second.json'),'{}'); watcher.flush();
    expect(errors.at(-1)).toContain('second.json.speed'); expect(published).toHaveLength(2);
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
