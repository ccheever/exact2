#!/usr/bin/env bun
// Command-line proof of the real canvas app and saves resumed in a new browser.
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { gzipSync } from 'node:zlib';
import { spawnSync } from 'node:child_process';
import { open } from '../../../scripts/agent.mjs';
const app = new URL('.', import.meta.url).pathname;
const out = resolve(app, 'artifacts');
const root = resolve(app, '../../..');
mkdirSync(out, { recursive: true });
mkdirSync(resolve(app, 'target/tmp'), { recursive: true });
Object.assign(process.env, {
  EXACT_APP_DIR: app, EXACT_UPDATE_TRUST: 'development',
  EXACT_WEB_DIST: resolve(app, 'dist'), CARGO_TARGET_DIR: resolve(app, 'target'), TMPDIR: resolve(app, 'target/tmp'),
});
const started = performance.now(), failures = [], transcript = [], replies = [];
const say = line => { console.log(line); transcript.push(line); };
const check = (ok, label, value) => {
  say(`${ok ? 'PASS' : 'FAIL'} ${label}${value === undefined ? '' : ': ' + JSON.stringify(value)}`);
  if (!ok) failures.push(label);
};
const equal = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const node = (t, id) => t.nodes.find(n => n.props?.testId === id);
const names = ['ground', 'player', 'camera', 'sun', ...Array.from({length:6}, (_, i) => `crate-${i+1}`), 'beacon-1', 'beacon-2', 'beacon-3'];
let session;
const position = state => state?.entity?.components?.Transform?.position;
async function op(label, fn) { const reply = await fn(); replies.push({operation:label,reply}); say(`${label}: ${JSON.stringify(reply)}`); return reply; }
async function run(number) {
  let s;
  const trace = [];
  try {
    s = await open({ host: 'web', app: 'beacons', size: [1280,720], webDist: resolve(app, 'dist') });
    async function op(method, ...args) {
      const reply = await s[method](...args);
      replies.push({run:number, method, args, reply});
      return reply;
    }
    const entity = async name => (await op('state', `world:${name}`)).entity.components;
    const pos = async () => (await entity('player')).Transform.position;
    async function snapshot(label) {
      const state = await op('state');
      const world = state.world[0];
      const entities = {};
      for (const name of names) entities[name] = await entity(name);
      const snap = {label, hash:world.hash, tick:world.tick, entities};
      trace.push(snap);
      return snap;
    }
    const key = (code, phase) => op('type', 'world', {key:code, ...(phase ? {phase} : {})});
    async function hold(code, ms) {
      await key(code, 'down'); await op('clock', `+${ms}`); await key(code, 'up');
    }
    async function walk(x, z) {
      for (const [axis, target, positive, negative] of [[0,x,'KeyD','KeyA'],[2,z,'KeyS','KeyW']]) {
        const p = await pos(), delta = target-p[axis];
        if (Math.abs(delta) > 0.04) {
          await hold(delta > 0 ? positive : negative, Math.round(Math.abs(delta)*15)*1000/60);
          await op('clock', 'settle');
        }
      }
      const p = await pos();
      check(Math.hypot(p[0]-x,p[2]-z)<0.15, `run ${number}: walked to (${x}, ${z})`, p);
    }
    let tree = await op('tree');
    check(!!node(tree,'play') && tree.nodes.some(n=>n.props?.text==='Beacons'), `run ${number}: title + Play`);
    await op('tap','play');
    tree = await op('tree');
    check(node(tree,'hud-beacons')?.props.text==='Beacons 0 / 3', `run ${number}: initial HUD`);
    const delivery = await key('KeyW','down');
    check(delivery.delivery==='platform', `run ${number}: real browser key path`);
    await op('clock','+1500');
    const p = await pos();
    check(Math.hypot(p[0],p[1]-0.9,p[2]+5.733332)<0.001,
      `run ${number}: exactly 1500 ms → (0, 0.9, -5.733332), tolerance 1 mm`,p);
    const forward = await snapshot('W1500');
    check(forward.tick===90, `run ${number}: 90 fixed ticks`,forward.hash);
    check(forward.hash==='0x3d4bf2881f139dc7', `run ${number}: native/web simulation hash parity`,forward.hash);
    check(equal(forward.entities.player.Mesh.Capsule,{radius:0.4,height:1.8}), `run ${number}: capsule dimensions`);
    check(forward.entities.ground.Mesh.Plane.size===40, `run ${number}: 40 m ground`);
    for (const [i,expected] of [[1,[8,1,0]],[2,[-6,1,7]],[3,[3,1,-9]]]) {
      const beacon = forward.entities[`beacon-${i}`];
      check(equal(beacon.Transform.position,expected) && equal(beacon.Transform.scale,[1,1,1])
        && !!beacon.Mesh.Sphere, `run ${number}: beacon ${i} position and default 0.5 m radius`);
    }
    await key('KeyW','up'); await op('clock','settle');
    await walk(8,0);
    await key('KeyE'); await op('clock','+100');
    const beacon = (await entity('beacon-1')).Beacon;
    tree = await op('tree');
    check(beacon.lit && beacon.glow>0 && beacon.glow<1, `run ${number}: glow partway after 100 ms`,beacon);
    check(node(tree,'hud-beacons')?.props.text==='Beacons 1 / 3', `run ${number}: lit count reaches real HUD`);
    await op('clock','+900');
    check((await entity('beacon-1')).Beacon.glow===1, `run ${number}: glow fully up at 1 second`);
    await key('KeyW','down'); await op('clock','+100');
    await op('tap','pause');
    const before = await snapshot('paused');
    await op('clock','+2000');
    const after = await snapshot('paused+2000');
    check(before.hash===after.hash && before.tick===after.tick && equal(before.entities,after.entities),
      `run ${number}: pause freezes entire world with movement held`);
    tree = await op('tree');
    check(tree.nodes.some(n=>n.props?.text==='Resume'), `run ${number}: Resume label`);
    await key('KeyW','up'); await op('tap','pause'); await op('clock','settle');
    if (number===1) await op('screenshot',resolve(out,'beacons-playing.png'));
    await walk(-6,7); await key('KeyE'); await op('clock','+1000');
    await walk(3,-9); await key('KeyE'); await op('clock','+1000');
    tree = await op('tree');
    check(node(tree,'hud-beacons')?.props.text==='Beacons 3 / 3' && !!node(tree,'play-again')
      && tree.nodes.some(n=>n.props?.text==='All beacons lit'), `run ${number}: completion UI`);
    await snapshot('complete');
    await op('tap','play-again');
    check(equal(await pos(),[0,0.9,0]), `run ${number}: Play again resets position`);
    tree = await op('tree');
    check(node(tree,'hud-beacons')?.props.text==='Beacons 0 / 3' && !node(tree,'victory'), `run ${number}: restart resets HUD`);
    await snapshot('restarted');
    const logs = await op('logs');
    check(!logs.host?.some(l=>/^(exception:|console\.error:|error:)/.test(l)), `run ${number}: no page/GPU errors (Chrome profile diagnostics recorded)`, logs.host);
    const state = await op('state');
    say(`TIMINGS run ${number} ${JSON.stringify(state.world[0].perf)}`);
    return trace;
  } finally { if (s) await s.close(); }
}
try {
  say(`START ${new Date().toISOString()}`);
  const build = spawnSync('bun', [resolve(root, 'host/web/build.mjs')], {cwd:root, env:process.env, stdio:'inherit'});
  if (build.status !== 0) throw new Error('shared web build failed');
  const wasm = readFileSync(resolve(app, 'dist/app.wasm'));
  say(`APP bytes ${JSON.stringify({raw:wasm.length,gzip:gzipSync(wasm,{level:9}).length})}`);
  const a = await run(1), b = await run(2);
  check(equal(a,b),'two fresh browser runs have identical full entity states, ticks and hashes');
  writeFileSync(resolve(out,'runs.json'),JSON.stringify({a,b},null,2)+'\n');

  // D6: hold W, queue a jump without advancing a tick, then capture the whole sim.
  if (session) await session.close(); session = null;
  const worldFile = resolve(out, 'checkpoint.world');
  const originalFile = resolve(out, 'original.world'), restoredFile = resolve(out, 'restored.world');
  session = await open({host:'web', app:'beacons', size:[1280,720], webDist:resolve(app,'dist')});
  await session.tap('play'); await session.clock(0);
  await session.type('world', {key:'KeyW',phase:'down'}); await session.clock('+500');
  await session.type('world', {key:'Space',phase:'down'});
  const saved = await op('screenshot checkpoint.world world save', () => session.screenshot(worldFile, 'world', 'save'));
  const continueWorld = async () => {
    await session.clock('+500');
    const jumped = await session.state('world:player');
    check(position(jumped)?.[1] > 0.9, 'saved queued jump executes after capture', position(jumped));
    await session.type('world', {key:'Space',phase:'up'});
    await session.clock('+1000');
    await session.type('world', {key:'KeyW',phase:'up'});
    return {world:(await session.state()).world[0], player:await session.state('world:player')};
  };
  const uninterrupted = await continueWorld();
  await session.screenshot(originalFile, 'world', 'save');
  await session.close(); session = null; say('CLOSED original browser/process before restoring');
  session = await open({host:'web', app:'beacons', world:worldFile, size:[1280,720], webDist:resolve(app,'dist')});
  check(!node(await session.tree(), 'world'), 'save waits behind Play');
  await session.tap('play');
  const restored = (await session.state()).world[0];
  check(restored.restored === true && restored.tick === saved?.tick && restored.hash === saved?.hash,
    'new session restores before first render, with the same tick and hash', restored);
  const continued = await continueWorld();
  await session.screenshot(restoredFile, 'world', 'save');
  check(continued.world.restored === false && continued.world.hash === uninterrupted.world.hash
    && continued.world.tick === uninterrupted.world.tick && equal(position(continued.player), position(uninterrupted.player)),
    'D6 two sessions continue to the same state, position, tick and hash', {expected:uninterrupted.world.hash, actual:continued.world.hash});
  check(readFileSync(originalFile).equals(readFileSync(restoredFile)), 'D6 entire Sim save is byte-identical, including held input, queue, clock, journal and publications');

} catch (e) { check(false,'proof interrupted',e.stack ?? String(e)); }
finally {
  if (session) await session.close();
  say(`PROOF ${failures.length ? 'FAIL' : 'PASS'} ${new Date().toISOString()} — ${failures.length} failures, ${((performance.now()-started)/1000).toFixed(3)} s`);
  writeFileSync(resolve(out,'proof.txt'),transcript.join('\n')+'\n');
  writeFileSync(resolve(out,'replies.json'),JSON.stringify(replies,null,2)+'\n');
}
process.exit(failures.length ? 1 : 0);
