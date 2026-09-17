#!/usr/bin/env bun
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { proof } from '../../proof.mjs';

await proof(import.meta, async ({open, check, equal, out, host, say}) => {
const node = (tree, id) => tree?.nodes?.find(n => n.props?.testId === id);
const position = state => state?.entity?.components?.Transform?.position;
  let session = await open();
  const s = session;
  const screenshot = path => host === 'web' ? s.screenshot(path) : (say('SKIP macOS screenshot: screencapture has no permission in this session'), Promise.resolve({skipped:'screen capture permission'}));
  const title = await s.tree();
  check('title and Play are the initial UI', !!node(title, 'play') && title?.nodes?.some(n => n.props?.text === 'Grey box'));
  check('no world or loaded GPU module on the title', !node(title, 'world') && (host !== 'web' || s.gpuMs() === null));
  await screenshot(resolve(out, 'greybox-title.png'));
  await s.tap('play');
  const playing = await s.tree();
  const canvas = node(playing, 'world');
  check('canvas carries the setup world summary', canvas?.world?.entities === 8 && canvas.world.tick === 0, canvas?.world);
  for (const id of ['hud-beacons', 'pause']) {
    check(`${id} is a direct canvas child`, node(playing, id)?.parent === canvas?.id, node(playing, id));
  }
  check('initial HUD text', node(playing, 'hud-beacons')?.props?.text === 'Beacons 0 / 1');
  const outline = await s.tree('world');
  for (const name of ['ground', 'player', 'camera', 'sun', 'crate-1', 'crate-2', 'crate-3', 'beacon-1']) {
    check(`world outline contains ${name}`, (outline?.entities ?? outline?.nodes ?? []).some(e => e.name === name));
  }
  const initial = await s.state();
  check('setup hash equals native golden', initial?.world?.[0]?.hash === '0x6b4d864d2da4c316', initial?.world?.[0]?.hash);
  const down = await s.type('world', { key: 'KeyW', phase: 'down' });
  check('W reaches the real browser input path', down?.delivery === (host === 'web' ? 'platform' : 'recognized'), down);
  await s.clock('+1500');
  const player = await s.state('world:player');
  check('W for 1500 ms equals the native pinned position', equal(position(player), [0, 0.9, -5.7333384]), position(player));
  const forward = await s.state();
  const hash = forward?.world?.[0]?.hash;
  check('W for 1500 ms equals the current native hash', hash === '0xe361b9c0055bede6', hash);
  check('1500 ms advances exactly 90 ticks', forward?.world?.[0]?.tick === 90, forward?.world?.[0]?.tick);
  const layout = await s.layout('world:player');
  const box = layout?.entity?.screen;
  check('player has a viewport-space screen box', box && [box.x, box.y, box.w, box.h].every(Number.isFinite)
    && box.w > 0 && box.h > 0 && box.x >= 0 && box.y >= 0
    && box.x + box.w <= 1280 && box.y + box.h <= 720, box);
  await s.type('world', { key: 'KeyW', phase: 'up' });
  if (box) {
    const pick = await s.layout('world', [box.x + box.w / 2, box.y + box.h / 2]);
    check('pick at the player box reaches player', pick?.hit?.name === 'player', pick);
    const tap = await s.tap('world:player');
    check('entity tap uses platform input', tap?.delivery === 'platform', tap);
  }
  await s.type('world', {key:'KeyW', for:100});
  await s.type('world', { key: 'KeyE' });
  await s.clock('+100');
  const beacon = await s.state('world:beacon-1');
  check('E lights beacon-1', beacon?.entity?.components?.Beacon?.lit === true, beacon);
  const settled = await s.clock('settle');
  check('world settles after movement and beacon spring', settled?.settled === true && settled?.world?.[0]?.quiescent === true, settled);
  const hud = await s.tree();
  check('typed surface record → HUD text', node(hud, 'hud-beacons')?.props?.text === 'Beacons 1 / 1', node(hud, 'hud-beacons'));
  await s.state();
  const logs = await s.logs();
  check('world journal carries beacon-1 lit', logs?.world?.some(w => w.lines.some(line => line.includes('beacon-1 lit'))), logs?.world);
  check('no browser/GPU errors', !logs?.host?.some(line => /^(exception:|console\.error:|error:)/.test(line)), logs?.host);
  await s.tap('pause');
  const beforePause = await s.state('world:player');
  // A held movement input makes the pause assertion meaningful even after settle.
  await s.type('world', { key: 'KeyW', phase: 'down' });
  await s.clock('+1000');
  const paused = await s.state('world:player');
  check('pause preserves player and tick with W held', equal(position(paused), position(beforePause)) && paused?.tick === beforePause?.tick, paused);
  await s.type('world', { key: 'KeyW', phase: 'up' });
  await screenshot(resolve(out, 'greybox-web.png'));
  const pausedTree = await s.tree();
  check('paused HUD offers Resume', pausedTree?.nodes?.some(n => n.props?.text === 'Resume'));
  const finalState = await s.state();
  check('Contract paused argument reaches world', finalState?.world?.[0]?.paused === true);
  const perf = finalState?.world?.[0]?.perf;
  if (host === 'web') {
  check('first-pixel and first-world timings are in state.world.perf', perf?.wallClock === true && perf.navigationToFirstContentfulPaintMs > 0
    && perf.inputToFirstFrameMs > 0 && perf.gpuMs > 0, perf);
  for (const name of ['/gpu-glue.js', '/gpu.js', '/gpu_bg.wasm']) {
    const resource = perf?.resources?.find(r => r.name === name);
    check(`${name} fetched only after title paint and Play`, resource?.startMs > perf?.navigationToFirstContentfulPaintMs && resource?.startMs >= perf?.inputMs, resource);
  }
  } else say('SKIP browser resource/paint timings on macOS: this host does not expose them');
  say(`TIMINGS ${JSON.stringify(perf)}`);
  writeFileSync(resolve(out, 'state.json'), JSON.stringify(finalState, null, 2) + '\n');
  // Key events from HUD descendants fall through, except the control's own keys.
  if (host === 'web') {
    await s.tap('pause');
    const beforeKeys = await s.state('world:player');
    await s.type('pause', {key:'KeyW',phase:'down'});
    await s.clock('+500');
    await s.type('pause', {key:'KeyW',phase:'up'});
    const afterKeys = await s.state('world:player');
    check('W bubbles from the focused Resume button and moves the world', position(afterKeys)?.[2] < position(beforeKeys)?.[2]);
    await s.type('pause', {key:'Space'});
    check('Space activates the focused button', (await s.state()).world[0].paused);
    await s.tap('pause'); await s.clock('+100');
    const afterSpace = await s.state('world:player');
    check('button Space never queues a world jump', position(afterSpace)?.[1] === 0.9, position(afterSpace));
    await s.tap('pause');
  } else say('SKIP web descendant-key bubbling: macOS uses native control dispatch');
  // Resume uses the same live argument without reconstructing the world.
  await s.tap('pause');
  await s.type('world', { key: 'KeyW', phase: 'down' });
  await s.clock('+100');
  const resumed = await s.state('world:player');
  check('Resume continues the existing world', position(resumed)?.[2] < position(paused)?.[2]);
  await s.type('world', { key: 'KeyW', phase: 'up' });

  // D6: hold W, queue a jump without advancing a tick, then capture the whole sim.
  await session.close(); session = null;
  const worldFile = resolve(out, 'checkpoint.world');
  const originalFile = resolve(out, 'original.world'), restoredFile = resolve(out, 'restored.world');
  session = await open();
  await session.tap('play'); await session.clock(0);
  await session.type('world', {key:'KeyW',phase:'down'}); await session.clock('+500');
  await session.type('world', {key:'Space',phase:'down'});
  const saved = await session.screenshot(worldFile, 'world', 'save');
  const continueWorld = async () => {
    await session.clock('+500');
    const jumped = await session.state('world:player');
    check('saved queued jump executes after capture', position(jumped)?.[1] > 0.9, position(jumped));
    await session.type('world', {key:'Space',phase:'up'});
    await session.clock('+1000');
    await session.type('world', {key:'KeyW',phase:'up'});
    return {world:(await session.state()).world[0], player:await session.state('world:player')};
  };
  const uninterrupted = await continueWorld();
  await session.screenshot(originalFile, 'world', 'save');
  await session.close(); session = null; say('CLOSED original browser/process before restoring');
  session = await open({world:worldFile});
  check('save waits behind Play', !node(await session.tree(), 'world'));
  await session.tap('play');
  const restored = (await session.state()).world[0];
  check('new session restores before first render, with the same tick and hash', restored.restored === true && restored.tick === saved?.tick && restored.hash === saved?.hash, restored);
  const continued = await continueWorld();
  await session.screenshot(restoredFile, 'world', 'save');
  check('D6 two sessions continue to the same state, position, tick and hash', continued.world.restored === false && continued.world.hash === uninterrupted.world.hash
    && continued.world.tick === uninterrupted.world.tick && equal(position(continued.player), position(uninterrupted.player)), {expected:uninterrupted.world.hash, actual:continued.world.hash});
  check('D6 entire Sim save is byte-identical, including held input, queue, clock, journal and publications', readFileSync(originalFile).equals(readFileSync(restoredFile)));

});
