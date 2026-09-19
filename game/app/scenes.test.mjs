import {test, expect} from 'bun:test';
import {runInNewContext} from 'node:vm';
import {mkdtempSync, realpathSync, readFileSync, writeFileSync, rmSync, existsSync, statSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
import {bakeGameScene, sceneInputs, sceneSource} from './scenes.mjs';
import {gameDefaults, prepareGame} from './shells.mjs';

const game = resolve(import.meta.dir, '..');
const original = resolve(game, 'games/lanterns');
const sha = path => createHash('sha256').update(readFileSync(path)).digest('hex');

// This is a real native baker and real Rust Data constructors, in a scratch output.
// It never launches a server, GPU, native app or device.
test('content rebake changes initial data with no Rust compilation and never replaces output on error', () => {
  const dir = realpathSync(mkdtempSync(resolve(tmpdir(), 'exact-scene-build-')));
  try {
    prepareGame(original, gameDefaults(original).game);
    const app = {dir, workspace:resolve(original,'.shells'), target:resolve(process.env.CARGO_TARGET_DIR ?? resolve(game,'target'))};
    const input = JSON.parse(readFileSync(resolve(original, 'scene.json'), 'utf8'));
    for (const [name, path] of Object.entries(input.fragments)) input.fragments[name] = resolve(original, path);
    for (const asset of Object.values(input.assets)) asset.path = resolve(original, asset.path);
    const scene = resolve(dir, 'scene.json');
    writeFileSync(resolve(dir, 'app.json'), readFileSync(resolve(original, 'app.json')));
    writeFileSync(scene, JSON.stringify(input, null, 2));
    const first = bakeGameScene(app);
    const executable = resolve(app.target, 'debug/lanterns-scene');
    const before = {sha:sha(executable), mtime:statSync(executable).mtimeMs};
    const originalMap = sceneSource(app, first.digest, 'lantern-1');
    expect(originalMap.instances[0].file).toBe(scene);
    expect(sceneSource(app, 'wrong digest', 'lantern-1').unavailable).toContain('digest differs');
    expect(sceneInputs(app)).toContain(resolve(game, 'games/shared/solid.fragment.json'));

    input.entities.find(e => e.id === 'lantern-1').parameters.position[0] = -11;
    writeFileSync(scene, JSON.stringify(input, null, 2));
    expect(sceneSource(app, first.digest, 'lantern-1').unavailable).toContain('source changed');
    const start = performance.now();
    const second = bakeGameScene(app, {build:false});
    const elapsed = performance.now() - start;
    expect(second.digest).not.toBe(first.digest);
    expect(second.content).not.toBe(first.content);
    expect(second.compiled).toEqual([]);
    expect({sha:sha(executable), mtime:statSync(executable).mtimeMs}).toEqual(before);
    const cached = bakeGameScene(app);
    expect(cached.compiled).toEqual([]); // actual Cargo freshness, not merely a CLI label
    expect(cached.digest).toBe(second.digest);
    console.log(`scene-only sample n=1: ${elapsed.toFixed(2)} ms; native baker SHA unchanged ${before.sha}; Cargo compiled 0 targets`);

    const outputs = ['scene.contract', 'scene.binhex', 'scene.map.json'].map(file => resolve(dir,'.scene',file));
    const accepted = outputs.map(sha);
    input.entities[0].parameters.position[1] = 'wrong type';
    writeFileSync(scene, JSON.stringify(input, null, 2));
    expect(() => bakeGameScene(app, {build:false})).toThrow('/entities/0/parameters/position/1');
    expect(outputs.map(sha)).toEqual(accepted);
    input.entities[0].parameters.position[1] = -0.5;
    writeFileSync(scene, JSON.stringify(input, null, 2));
    bakeGameScene(app, {development:false, build:false});
    expect(existsSync(resolve(dir,'.scene/scene.map.json'))).toBe(false);
    expect(sceneSource(app, second.digest, 'lantern-1').unavailable).toContain('absent');
    expect(readFileSync(resolve(dir,'.scene/scene.contract'),'utf8')).not.toContain(dir);
  } finally { rmSync(dir, {recursive:true, force:true}); }
}, 60_000);

test('comparison adapter reads the current roster and retains the scene argument when loading', async () => {
  let bound, truncated = false;
  const world = {device:{}, tick:60, args:{scene:'baked-scene', started:true}, resources:{Session:{elapsed:60}}, published:{phase:'playing'}};
  const entities = {
    fox:{Mesh:{Asset:['fox.model']}, Animation:{clip:'Survey'}},
    player:{Transform:{position:[0,.65,12]}, CapsuleController:{velocity:[0,0,0]}},
    crate:{Transform:{position:[6,.6,8]}, Body:{velocity:[0,0,0]}},
  };
  const exact = {
    agent:async () => ({nodes:[{id:1,props:{testId:'world'}}]}),
    views:new Map([[1,{getBoundingClientRect:()=>({left:0,top:0,width:800,height:600})}]]),
    gpu:{
      agent:(_, q) => q.op === 'logs' ? {lines:[]} : q.entity === '*' ? {truncated,entities:[
        {name:'edited-target',components:{Lantern:{lit:true},Transform:{position:[99,2,3]}}},
      ]} : q.entity ? {entity:{components:entities[q.entity]}} : {world},
      destroy:()=>{}, surface:(_,name,values)=>{bound=values;},
    },
  };
  const context = {exact, location:{search:'?agent=1'}, URLSearchParams, performance, Uint8Array, atob,
    localStorage:{getItem:()=> 'AA=='}, setTimeout};
  runInNewContext(readFileSync(resolve(original,'assets/lanterns-adapter.mjs'),'utf8'), context);
  const state = await context.lanterns.command({op:'state'});
  expect(state.lanterns).toEqual([{id:'edited-target',x:99,y:2,z:3,lit:true}]);
  expect(state.total).toBe(1);
  await context.lanterns.command({op:'load'});
  expect(bound.length).toBe(8);
  expect(bound[7]).toBe('baked-scene');
  truncated = true;
  await expect(context.lanterns.command({op:'state'})).rejects.toThrow('truncated');
});


test('scene input discovery handles a cyclic edit and refuses oversized input before parsing', () => {
  const dir = realpathSync(mkdtempSync(resolve(tmpdir(), 'exact-scene-discovery-')));
  try {
    const scene = resolve(dir,'scene.json'), fragment = resolve(dir,'loop.json');
    writeFileSync(scene, JSON.stringify({fragments:{loop:'loop.json'}}));
    writeFileSync(fragment, JSON.stringify({fragments:{back:'scene.json'}}));
    expect(sceneInputs(dir)).toEqual([fragment,scene].sort());
    writeFileSync(fragment, Buffer.alloc(16*1024*1024+1,32));
    expect(() => sceneInputs(dir)).toThrow('byte limit');
    writeFileSync(fragment,'{}');
    expect(sceneInputs(dir)).toEqual([fragment,scene].sort());
  } finally { rmSync(dir,{recursive:true,force:true}); }
});
