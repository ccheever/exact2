#!/usr/bin/env bun
// The S0 smoke: one session, the existing eight operations, all failures reported.
import { mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { gzipSync } from 'node:zlib';
import { spawnSync } from 'node:child_process';
import { open, render } from '../../../scripts/agent.mjs';

const app = fileURLToPath(new URL('.', import.meta.url));
const root = resolve(app, '../../..');
process.env.EXACT_APP_DIR = app;
const host = process.argv[2] ?? 'web';
const scratch = resolve(process.env.EXACT_GREYBOX_PROOF ?? resolve(root, 'game/target/greybox-proof'));
const dist = resolve(process.env.EXACT_WEB_DIST ?? resolve(app, 'dist'));
Object.assign(process.env, {EXACT_WEB_DIST:dist, EXACT_UPDATE_TRUST:'development', CARGO_TARGET_DIR:process.env.CARGO_TARGET_DIR ?? resolve(root, 'game/target')});
mkdirSync(scratch, { recursive: true });
const started = performance.now(), failures = [], transcript = [], replies = [];
const say = (line) => { transcript.push(line); console.log(line); };
function check(ok, description, actual) {
  if (ok) return;
  const message = description + (actual === undefined ? '' : `: ${JSON.stringify(actual)}`);
  failures.push(message);
  process.exitCode = 1; // Mark failure immediately, but finish the independent checks.
  say(`FAIL ${message}`);
}
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const node = (tree, id) => tree?.nodes?.find(n => n.props?.testId === id);
const position = (state) => state?.entity?.components?.Transform?.position;
let session;
async function op(label, invoke) {
  try {
    const reply = await invoke();
    replies.push({ operation: label, reply });
    say(`${label}\n${render(label.split(' ')[0], reply)}`);
    return reply;
  } catch (error) {
    check(false, label, error.message);
    return null;
  }
}
try {
  if (!['web','macos'].includes(host)) throw new Error(`greybox proof unavailable on this host yet: ${host}`);
  const build = spawnSync('bun', [resolve(root, host === 'web' ? 'host/web/build.mjs' : 'host/apple/build.mjs')], {cwd:root, env:process.env, stdio:'inherit'});
  if (build.status !== 0) throw new Error('shared web build failed');
  session = await open({ host, app: 'greybox', size: [1280, 720], webDist: dist });
  const s = session;
  const screenshot = path => host === 'web' ? s.screenshot(path) : (say('SKIP macOS screenshot: screencapture has no permission in this session'), Promise.resolve({skipped:'screen capture permission'}));
  const title = await op('tree', () => s.tree());
  check(!!node(title, 'play') && title?.nodes?.some(n => n.props?.text === 'Grey box'), 'title and Play are the initial UI');
  check(!node(title, 'world') && (host !== 'web' || s.gpuMs() === null), 'no world or loaded GPU module on the title');
  await op('screenshot title', () => screenshot(resolve(scratch, 'greybox-title.png')));
  await op('tap play', () => s.tap('play'));
  const playing = await op('tree', () => s.tree());
  const canvas = node(playing, 'world');
  check(canvas?.world?.entities === 8 && canvas.world.tick === 0, 'canvas carries the setup world summary', canvas?.world);
  for (const id of ['hud-beacons', 'pause']) {
    check(node(playing, id)?.parent === canvas?.id, `${id} is a direct canvas child`, node(playing, id));
  }
  check(node(playing, 'hud-beacons')?.props?.text === 'Beacons 0 / 1', 'initial HUD text');
  const outline = await op('tree world', () => s.tree('world'));
  for (const name of ['ground', 'player', 'camera', 'sun', 'crate-1', 'crate-2', 'crate-3', 'beacon-1']) {
    check((outline?.entities ?? outline?.nodes ?? []).some(e => e.name === name), `world outline contains ${name}`);
  }
  const initial = await op('state', () => s.state());
  check(initial?.world?.[0]?.hash === '0x4a9f1ad15148813e', 'setup hash equals native golden', initial?.world?.[0]?.hash);
  const down = await op('type world key KeyW down', () => s.type('world', { key: 'KeyW', phase: 'down' }));
  check(down?.delivery === (host === 'web' ? 'platform' : 'recognized'), 'W reaches the real browser input path', down);
  await op('clock +1500', () => s.clock('+1500'));
  const player = await op('state world:player', () => s.state('world:player'));
  check(same(position(player), [0, 0.9, -5.733332]), 'W for 1500 ms equals the native pinned position', position(player));
  const forward = await op('state', () => s.state());
  // Commit 46fea5b added DirectionalLight.shadows and repinned the native test.
  // B1b's 0x4dcde63de7f70139 predates that schema change; do not undo the scene.
  const hash = forward?.world?.[0]?.hash;
  check(hash === '0x70c17d4a69834418', 'W for 1500 ms equals the current native hash', hash);
  check(forward?.world?.[0]?.tick === 90, '1500 ms advances exactly 90 ticks', forward?.world?.[0]?.tick);
  say(`PARITY current native=0x70c17d4a69834418 ${host}=${hash}; brief=0x4dcde63de7f70139 (superseded in 46fea5b)`);
  const layout = await op('layout world:player', () => s.layout('world:player'));
  const box = layout?.entity?.screen;
  check(box && [box.x, box.y, box.w, box.h].every(Number.isFinite)
    && box.w > 0 && box.h > 0 && box.x >= 0 && box.y >= 0
    && box.x + box.w <= 1280 && box.y + box.h <= 720, 'player has a viewport-space screen box', box);
  await op('type world key KeyW up', () => s.type('world', { key: 'KeyW', phase: 'up' }));
  if (box) {
    const pick = await op('layout world at player', () => s.layout('world', [box.x + box.w / 2, box.y + box.h / 2]));
    check(pick?.hit?.name === 'player', 'pick at the player box reaches player', pick);
    const tap = await op('tap world:player', () => s.tap('world:player'));
    check(tap?.delivery === 'platform', 'entity tap uses platform input', tap);
  }
  await op('type world key KeyW down', () => s.type('world', { key: 'KeyW', phase: 'down' }));
  await op('clock +100', () => s.clock('+100'));
  await op('type world key KeyW up', () => s.type('world', { key: 'KeyW', phase: 'up' }));
  await op('type world key KeyE', () => s.type('world', { key: 'KeyE' }));
  await op('clock +100', () => s.clock('+100'));
  const beacon = await op('state world:beacon-1', () => s.state('world:beacon-1'));
  check(beacon?.entity?.components?.Beacon?.lit === true, 'E lights beacon-1', beacon);
  const settled = await op('clock settle', () => s.clock('settle'));
  check(settled?.settled === true && settled?.world?.[0]?.quiescent === true, 'world settles after movement and beacon spring', settled);
  const hud = await op('tree', () => s.tree());
  check(node(hud, 'hud-beacons')?.props?.text === 'Beacons 1 / 1', 'typed surface record → HUD text', node(hud, 'hud-beacons'));
  await op('state', () => s.state());
  const logs = await op('logs', () => s.logs());
  check(logs?.world?.some(w => w.lines.some(line => line.includes('beacon-1 lit'))), 'world journal carries beacon-1 lit', logs?.world);
  check(!logs?.host?.some(line => /^(exception:|console\.error:|error:)/.test(line)), 'no browser/GPU errors', logs?.host);
  await op('tap pause', () => s.tap('pause'));
  const beforePause = await op('state world:player', () => s.state('world:player'));
  // A held movement input makes the pause assertion meaningful even after settle.
  await op('type world key KeyW down', () => s.type('world', { key: 'KeyW', phase: 'down' }));
  await op('clock +1000', () => s.clock('+1000'));
  const paused = await op('state world:player', () => s.state('world:player'));
  check(same(position(paused), position(beforePause)) && paused?.tick === beforePause?.tick, 'pause preserves player and tick with W held', paused);
  await op('type world key KeyW up', () => s.type('world', { key: 'KeyW', phase: 'up' }));
  await op('screenshot greybox-web.png', () => screenshot(resolve(scratch, 'greybox-web.png')));
  const pausedTree = await op('tree', () => s.tree());
  check(pausedTree?.nodes?.some(n => n.props?.text === 'Resume'), 'paused HUD offers Resume');
  const finalState = await op('state', () => s.state());
  check(finalState?.world?.[0]?.paused === true, 'Contract paused argument reaches world');
  const perf = finalState?.world?.[0]?.perf;
  if (host === 'web') {
  check(perf?.wallClock === true && perf.navigationToFirstContentfulPaintMs > 0
    && perf.inputToFirstFrameMs > 0 && perf.gpuMs > 0, 'first-pixel and first-world timings are in state.world.perf', perf);
  for (const name of ['/gpu-glue.js', '/gpu.js', '/gpu_bg.wasm']) {
    const resource = perf?.resources?.find(r => r.name === name);
    check(resource?.startMs > perf?.navigationToFirstContentfulPaintMs && resource?.startMs >= perf?.inputMs,
      `${name} fetched only after title paint and Play`, resource);
  }
  } else say('SKIP browser resource/paint timings on macOS: this host does not expose them');
  say(`TIMINGS ${JSON.stringify(perf)}`);
  writeFileSync(resolve(scratch, 'state.json'), JSON.stringify(finalState, null, 2) + '\n');
  // Key events from HUD descendants fall through, except the control's own keys.
  if (host === 'web') {
    await op('tap pause (resume, retain button focus)', () => s.tap('pause'));
    const beforeKeys = await s.state('world:player');
    await op('type pause key KeyW down', () => s.type('pause', {key:'KeyW',phase:'down'}));
    await op('clock +500', () => s.clock('+500'));
    await op('type pause key KeyW up', () => s.type('pause', {key:'KeyW',phase:'up'}));
    const afterKeys = await s.state('world:player');
    check(position(afterKeys)?.[2] < position(beforeKeys)?.[2], 'W bubbles from the focused Resume button and moves the world');
    await op('type pause key Space (activates Pause only)', () => s.type('pause', {key:'Space'}));
    check((await s.state()).world[0].paused, 'Space activates the focused button');
    await s.tap('pause'); await s.clock('+100');
    const afterSpace = await s.state('world:player');
    check(position(afterSpace)?.[1] === 0.9, 'button Space never queues a world jump', position(afterSpace));
    await s.tap('pause');
  } else say('SKIP web descendant-key bubbling: macOS uses native control dispatch');
  // Resume uses the same live argument without reconstructing the world.
  await op('tap pause (resume)', () => s.tap('pause'));
  await op('type world key KeyW down', () => s.type('world', { key: 'KeyW', phase: 'down' }));
  await op('clock +100', () => s.clock('+100'));
  const resumed = await op('state world:player', () => s.state('world:player'));
  check(position(resumed)?.[2] < position(paused)?.[2], 'Resume continues the existing world');
  await op('type world key KeyW up', () => s.type('world', { key: 'KeyW', phase: 'up' }));

  // D6: hold W, queue a jump without advancing a tick, then capture the whole sim.
  await session.close(); session = null;
  const worldFile = resolve(scratch, 'checkpoint.world');
  const originalFile = resolve(scratch, 'original.world'), restoredFile = resolve(scratch, 'restored.world');
  session = await open({host, app: 'greybox', size:[1280,720], webDist:dist});
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
  session = await open({host, app:'greybox', world:worldFile, size:[1280,720], webDist:dist});
  check(!node(await session.tree(), 'world'), 'save waits behind Play');
  await session.tap('play');
  const restored = (await session.state()).world[0];
  check(restored.restored === true && restored.tick === saved?.tick && restored.hash === saved?.hash,
    'new session restores before first render, with the same tick and hash', restored);
  const continued = await continueWorld();
  await session.screenshot(restoredFile, 'world', 'save');
  check(continued.world.restored === false && continued.world.hash === uninterrupted.world.hash
    && continued.world.tick === uninterrupted.world.tick && same(position(continued.player), position(uninterrupted.player)),
    'D6 two sessions continue to the same state, position, tick and hash', {expected:uninterrupted.world.hash, actual:continued.world.hash});
  check(readFileSync(originalFile).equals(readFileSync(restoredFile)), 'D6 entire Sim save is byte-identical, including held input, queue, clock, journal and publications');
} catch (error) {
  check(false, 'proof could not continue', error.stack ?? error.message);
} finally {
  try { if (session) await session.close(); } // The carrier kills its recorded Chrome group and waits.
  catch (error) { check(false, 'session cleanup', error.message); }
  const sizes = { 'app.wasm': { raw: 0, gzip: 0 }, 'gpu_bg.wasm': { raw: 0, gzip: 0 }, 'gpu.js': { raw: 0, gzip: 0 }, rest: { raw: 0, gzip: 0 } };
  function count(dir) {
    for (const name of readdirSync(dir)) {
      const path = resolve(dir, name);
      if (statSync(path).isDirectory()) count(path);
      else {
        const bytes = readFileSync(path), row = sizes[path.slice(dist.length + 1)] ?? sizes.rest;
        row.raw += bytes.length; row.gzip += gzipSync(bytes, { level: 9 }).length;
      }
    }
  }
  try { count(dist); say(`DIST bytes (gzip level 9, per file) ${JSON.stringify(sizes)}`); }
  catch (error) { check(false, 'dist sizes', error.message); }
  say(`PROOF ${failures.length ? 'FAIL' : 'PASS'}: ${failures.length} failures; wall ${((performance.now() - started) / 1000).toFixed(3)} s`);
  writeFileSync(resolve(scratch, `transcript-${host}.txt`), transcript.join('\n\n') + '\n');
  writeFileSync(resolve(scratch, `replies-${host}.json`), JSON.stringify(replies, null, 2) + '\n');
  say(`Artifacts: ${scratch}`);
}
// Match agent.mjs's CLI: after awaited Chrome shutdown, run the shared reader's exit hook.
process.exit(process.exitCode ?? 0);
