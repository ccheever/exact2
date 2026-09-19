#!/usr/bin/env bun
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { proof } from '../../proof.mjs';
import { audioProof } from '../../bench/probes/audio.mjs';

await proof(import.meta, async ({pin, pinSave, open, check, equal, out, host, say}) => {
const node = (tree, id) => tree?.nodes?.find(n => n.props?.testId === id);
  let session = await open();
  const s = session;
  const screenshot = path => host === 'web' || host === 'ios' ? s.screenshot(path) : (say(host === 'linux' ? 'SKIP headless screenshot: gameplay and HUD use tree/state' : 'SKIP macOS screenshot: screencapture has no permission in this session'), Promise.resolve({skipped:host === 'linux' ? 'headless proof' : 'screen capture permission'}));
  const title = await s.tree();
  check('Play is initially focused and named by its text', node(title, 'play')?.focused === true && node(title, 'play')?.accessibleName === 'Play');
  check('state focus agrees with tree', (await s.state()).focus.logical === node(title, 'play').id);
  check('title and Play are the initial UI', !!node(title, 'play') && title?.nodes?.some(n => n.props?.text === 'Grey box'));
  check('no world or loaded GPU module on the title', !node(title, 'world') && (host !== 'web' || s.gpuMs() === null));
  await screenshot(resolve(out, 'greybox-title.png'));
  await s.tap('play');
  const playing = await s.tree();
  check('HUD is polite and Pause is named by text', node(playing, 'hud-beacons')?.props.accessibilityLive === 'polite' && node(playing, 'pause')?.accessibleName === 'Pause');
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
  pin(0, initial.world[0]);
  const down = await s.world('world').key_down('KeyW');
  check('W reaches the real browser input path', down?.delivery === (host === 'web' ? 'platform' : 'recognized'), down);
  await s.world('world').run(1500);
  const player = await s.world('world').get('player', 'Mesh');
  const position = await s.world('world').global_position('player');
  check('player has the exact authored capsule', equal(player,
    {Capsule:{height:1.8, radius:0.4}}), player);
  check('W for 1500 ms equals the native pinned position', equal(position, [0, 0.9, -5.3666644]), position);
  const forward = await s.state();
  pin(90, forward.world[0]);
  check('1500 ms advances exactly 90 ticks', forward?.world?.[0]?.tick === 90, forward?.world?.[0]?.tick);
  const layout = await s.layout('world:player');
  const box = layout?.entity?.screen;
  check('player has a viewport-space screen box', box && [box.x, box.y, box.w, box.h].every(Number.isFinite)
    && box.w > 0 && box.h > 0 && box.x >= 0 && box.y >= 0
    && box.x + box.w <= 1280 && box.y + box.h <= 720, box);
  await s.world('world').key_up('KeyW');
  if (host === 'linux') {
    check('headless world reports device false', initial.world[0].device === false);
    check('headless CPU pick answers without a device', 'hit' in (await s.layout('world', [1,1])));
    check('headless occlusion is explicitly unavailable', layout.entity.visible.occluded.unavailable === true);
    check('headless canvas capture is explicitly unavailable', (await s.screenshot(resolve(out,'unavailable.png'),'world')).unavailable === true);

  }
  if (box) {
    const pick = await s.layout('world', [box.x + box.w / 2, box.y + box.h / 2]);
    check('pick at the player box reaches player', pick?.hit?.name === 'player', pick);
    if (host !== 'linux') {
      const tap = await s.tap('world:player');
      check('entity tap uses the host input carrier', tap?.delivery === (host === 'ios' ? 'recognized' : 'platform'), tap);
    }
  }
  await s.world('world').hold('KeyW', 100);
  await s.world('world').tap('KeyE');
  await s.world('world').run(100);
  const beacon = await s.world('world').get('beacon-1', 'Beacon');
  check('E lights beacon-1', beacon?.lit === true, beacon);
  const chimeAudio = (await s.state()).world[0].audio;
  check('state.world.audio carries the live beacon chime', chimeAudio?.voices?.some(voice => voice.sound === 'chime' && voice.at === 'beacon-1'));
  const settled = await s.world('world').settle();
  check('world settles after movement and beacon spring', settled === true, settled);
  const hud = await s.tree();
  check('typed surface record → HUD text', node(hud, 'hud-beacons')?.props?.text === 'Beacons 1 / 1', node(hud, 'hud-beacons'));
  await s.state();
  const logs = await s.logs();
  check('world journal carries beacon-1 lit', logs?.world?.some(w => w.lines.some(line => line.includes('beacon-1 lit'))), logs?.world);
  for (const sound of ['sfx footstep at player ', 'sfx chime at beacon-1 ', 'loop wind on ']) {
    check(`world journal carries ${sound.trim()}`, logs?.world?.some(w => w.lines.some(line => line.includes(sound))));
  }
  const audio = (await s.state()).world[0].audio;
  check('state.world.audio reports camera wind and finite voices', Array.isArray(audio?.voices)
    && audio?.sources?.some(source => source.sound === 'wind' && source.entity === 'camera' && source.playing));
  check('no browser/GPU errors', !logs?.host?.some(line => /^(exception:|console\.error:|error:)/.test(line)), logs?.host);
  await s.tap('pause');
  const beforePause = await s.world('world').snapshot();
  // A held movement input makes the pause assertion meaningful even after settle.
  await s.world('world').key_down('KeyW');
  await s.world('world').run(2000);
  const paused = await s.world('world').snapshot();
  check('pause + 2 seconds preserves the simulation snapshot with W held', equal(paused, beforePause));
  await s.world('world').key_up('KeyW');
  await screenshot(resolve(out, 'greybox-web.png'));
  const pausedTree = await s.tree();
  check('Resume accessible name follows its text', node(pausedTree, 'pause')?.accessibleName === 'Resume');
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
  } else say(`SKIP browser resource/paint timings on ${host}: this host does not expose them`);
  say(`TIMINGS ${JSON.stringify(perf)}`);
  writeFileSync(resolve(out, 'state.json'), JSON.stringify(finalState, null, 2) + '\n');
  // Key events from HUD descendants fall through, except the control's own keys.
  {
    await s.tap('pause');
    const beforeKeys = await s.world('world').global_position('player');
    await s.type('pause', {key:'KeyW',phase:'down'});
    await s.world('world').run(500);
    await s.type('pause', {key:'KeyW',phase:'up'});
    const afterKeys = await s.world('world').global_position('player');
    check('W bubbles from the focused Resume button and moves the world', afterKeys?.[2] < beforeKeys?.[2]);
    await s.world('world').key_down('Space');
    await s.tap('pause');
    await s.type('pause', {key:'Space',phase:'up'});
    check('hold Space then click Pause releases the world key', !(await s.state()).world[0].input.held.includes('Space'));
    await s.tap('pause');
    await s.type('pause', {key:'Space'});
    check('Space activates the focused button', (await s.state()).world[0].paused);
    await s.tap('pause'); await s.world('world').run(100);
    const afterSpace = await s.world('world').global_position('player');
    check('button Space never queues a world jump', afterSpace?.[1] === 0.9, afterSpace);
    await s.tap('pause');
  }
  // Resume uses the same live argument without reconstructing the world.
  await s.tap('pause');
  await s.world('world').key_down('KeyW');
  await s.world('world').run(100);
  const resumed = await s.world('world').global_position('player');
  check('Resume continues the existing world', resumed?.[2] < paused.entities.find(e => e.name === 'player').components.Transform.position[2]);
  await s.world('world').key_up('KeyW');

  // D6: hold W, queue a jump without advancing a tick, then capture the whole sim.
  await session.close(); session = null;
  const worldFile = resolve(out, 'checkpoint.world');
  const originalFile = resolve(out, 'original.world'), restoredFile = resolve(out, 'restored.world');
  session = await open();
  await session.tap('play'); await session.clock(0);
  await session.world('world').key_down('KeyW'); await session.world('world').run(500);
  await session.world('world').key_down('Space');
  const checkpointState = await session.world('world').snapshot();
  const saved = await session.world('world').save(worldFile);
  const continueWorld = async () => {
    await session.world('world').run(500);
    const jumped = await session.world('world').global_position('player');
    check('saved queued jump executes after capture', jumped?.[1] > 0.9, jumped);
    await session.world('world').key_up('Space');
    await session.world('world').run(1000);
    await session.world('world').key_up('KeyW');
    return session.world('world').snapshot();
  };
  const uninterrupted = await continueWorld();
  await session.world('world').save(originalFile);
  pinSave("continuation", originalFile);
  await session.close(); session = null; say('CLOSED original browser/process before restoring');
  session = await open({world:worldFile});
  check('save waits behind Play', !node(await session.tree(), 'world'));
  await session.tap('play');
  const restored = (await session.state()).world[0];
  check('new session restores before first render, with the same tick and hash', restored.restored === true && restored.tick === saved?.tick && restored.hash === saved?.hash, restored);
  check('restore preserves the simulation snapshot', equal(checkpointState, await session.world('world').snapshot()));
  const continued = await continueWorld();
  await session.world('world').save(restoredFile);
  check('D6 two sessions continue to the same simulation snapshot', equal(continued, uninterrupted));
  check('D6 entire Sim save is byte-identical, including held input, queue, clock, journal and publications', readFileSync(originalFile).equals(readFileSync(restoredFile)));

  await session.close(); session = null;
  const refusedFile = resolve(out, 'refused.world');
  writeFileSync(refusedFile, 'invalid simulation save');
  session = await open({world:refusedFile});
  let refusal;
  try { await session.tap('play'); } catch (error) { refusal = error.message; }
  check('the operation creating the canvas reports its restore refusal', refusal?.includes('restore refused'), refusal);
  const fresh = (await session.state()).world[0];
  check('refusal belongs to canvas state and leaves a fresh world', fresh.tick === 0 && fresh.restoreError?.includes('restore refused'), fresh);
  await session.world('world').run(100);
  check('unrelated operations keep working after a refused restore', (await session.state()).world[0].tick === 6);
  const refusedLogs = await session.logs();
  check('restore refusal is in the canvas journal', refusedLogs.world?.some(w=>w.lines.some(line=>line.includes('restore refused'))), refusedLogs.world);

  if (host === 'web' && process.env.EXACT_AUDIO_PROBE === '1') {
    await session.close(); session = null;
    await audioProof({out, check, say});
  }
});
