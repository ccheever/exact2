// Case bodies copied from comparison-full/harness/acceptance.mjs (2026-09-18).
// Transport-only changes: driver keys, process reload, and unsupported world pixels.
import {createCommand} from './command.mjs';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {resolve} from 'node:path';
const args = Object.fromEntries(process.argv.slice(2).map(a => {const i=a.indexOf('='); return [a.slice(0,i),a.slice(i+1)];}));
const engine = 'exact2-linux', task = args.task || 'baseline';
const out = resolve(args.out || 'game/tests/trials/evidence/baseline');
await mkdir(out,{recursive:true});
const level = JSON.parse(await readFile(new URL('./level.json',import.meta.url)));
if (task === 'extra') level.lanterns.push({id:'lantern-13',position:[-8,1.2,7]});
const started = performance.now(), errors=[], cases=[], trace=[];
const adapter = await createCommand({root:args.root});
let lastState, unsupported=[];
const check = (ok, message) => { if (!ok) throw new Error(message); };
const close = (a, b, e = .035) => Math.abs(a - b) <= e;
async function command(request) {
  const result = await adapter.command(request);
  if (result?.player) lastState = result;
  return result;
}
async function state() { return command({ op: 'state' }); }
async function ready() {
  // Bun/Playwright here treats an async waitForFunction predicate as truthy
  // before its Promise resolves. Await the adapter from the host instead.
  const deadline = performance.now() + 60000;
  while (performance.now() < deadline) {
    if ((await command({ op: 'ready' }))?.ready === true) return;
    await new Promise(resolve => setTimeout(resolve, 20));
  }
  throw new Error('Asset readiness timed out');
}
async function input(moveX = 0, moveZ = 0, jump = false, act = false) {
  return command({ op: 'input', moveX, moveZ, jump, act });
}
async function step(ticks) { return command({ op: 'step', ticks }); }
async function fresh() {
  await command({ op: 'reset' });
  await command({ op: 'start' });
  await input();
  await step(3);
  return state();
}
async function moveTo(x, z, tolerance = .18) {
  for (let i = 0; i < 180; i++) {
    const s = await state(), dx = x - s.player.x, dz = z - s.player.z;
    const distance = Math.hypot(dx, dz);
    if (distance <= tolerance) { await input(); return s; }
    const ticks = Math.max(1, Math.min(12, Math.floor(distance / .075)));
    await input(dx / distance, dz / distance);
    await step(ticks);
  }
  throw new Error(`Cannot navigate to ${x},${z}: ${JSON.stringify(lastState?.player)}`);
}
async function act() {
  await input(); await step(1);
  await input(0, 0, false, true); await step(1);
  await input(); return state();
}
async function test(name, fn) {
  const begin = performance.now();
  unsupported=[];
  try { await fn(); cases.push({name, status:unsupported.length ? 'unsupported' : 'pass', passed:!unsupported.length, mechanicsPassed:true, unsupported, milliseconds:performance.now()-begin}); }
  catch (e) { cases.push({ name, status:'fail', passed: false, error: String(e), state: lastState, milliseconds: performance.now() - begin }); }
  await writeFile(`${out}/acceptance.json`, JSON.stringify({ engine, cases, errors, trace }, null, 2));

}
async function screenshot(name) { const r=await adapter.screenshot(`${out}/${name}.png`); if(r.unsupported) unsupported.push(r.unsupported); }
try {

  await ready();
  await test('title, level and imported asset', async () => {
    const s = await state();
    check(s.phase === 'title', `Expected title, got ${s.phase}`);
    check(s.assetReady === true, 'Animated fox must be loaded');
    check(s.total === level.lanterns.length && s.lanterns.length === level.lanterns.length && s.count === 0, 'Expected unlit lantern roster');
    for (const expected of level.lanterns) {
      const got = s.lanterns.find(x => x.id === expected.id);
      check(got && ['x', 'y', 'z'].every((k, i) => close(got[k], expected.position[i])), `Level mismatch ${expected.id}`);
    }
    await screenshot('title');
  });
  await test('actual keyboard movement and active animation', async () => {
    const before = await fresh();
    // The HUD legitimately covers the canvas corner; focus through the playfield.
    await adapter.raw().tap('world');
    await adapter.raw().world('world').key_down('KeyW');
    await step(30);
    const after = await state();
    await adapter.raw().world('world').key_up('KeyW');
    check(after.player.z < before.player.z - 1.5, 'Browser keyboard must move real player');
    check(/walk|run/i.test(after.player.animation), `Moving clip: ${after.player.animation}`);
    await screenshot('moving');
  });
  await test('fixed movement, jump gravity and landing', async () => {
    const before = await fresh();
    await input(0, -1); await step(60); await input();
    const moved = await state();
    check(close(before.player.z - moved.player.z, 4.5, .4), 'One second movement should be 4.5 units');
    await input(0, 0, true); await step(15);
    const air = await state();
    check(air.player.y > .6 && !air.player.grounded, 'Jump must leave ground');
    await input(); await step(100);
    const landed = await state();
    check(landed.player.grounded && Math.abs(landed.player.y) < .15, 'Gravity must land player');
    check(landed.events.some(e => e.type === 'jump'), 'Jump journal entry');
  });
  await test('pause freezes clock and body', async () => {
    await fresh(); await input(1, 0);
    await command({ op: 'pause' }); const before = await state();
    await step(120); const after = await state();
    check(after.phase === 'paused' && before.ticks === after.ticks, 'Paused clock advanced');
    check(close(before.player.x, after.player.x, .001), 'Paused body moved');
    await command({ op: 'pause' });
    check((await state()).phase === 'playing', 'Resume failed');
  });
  await test('solid wall collision', async () => {
    await fresh(); await moveTo(-4, 12);
    await input(0, -1); await step(120); await input();
    const s = await state();
    check(s.player.z >= 7.7 && s.player.z < 9, `Passed through wall: z=${s.player.z}`);
  });
  await test('dynamic crate push and ledge collision', async () => {
    const before = await fresh(); await moveTo(4.8, 8);
    await input(1, 0); await step(65); await input(); await step(15);
    const s = await state();
    check(s.crate.x > before.crate.x + .25, `Crate did not move: ${s.crate.x}`);
    check(s.crate.x < 7.55, `Crate passed through ledge: ${s.crate.x}`);
    trace.push({ stage: 'crate-push', state: s });
    await screenshot('crate');
  });
  await test('save survives reload with world and clock restored', async () => {
    await fresh(); await moveTo(0, 7); await act();
    await moveTo(4.8, 8); await input(1, 0); await step(30); await input(); await step(15);
    const before = await state();
    check(before.count > 0, 'Must save meaningful progress');
    check((await command({ op: 'save' })).saved, 'Save did not persist');
    await adapter.reload(); 
    await ready(); await command({ op: 'load' });
    const after = await state();
    check(before.ticks === after.ticks && before.count === after.count, 'Save lost clock or lanterns');
    for (const entity of ['player', 'crate']) for (const key of ['x', 'y', 'z', 'vx', 'vy', 'vz']) {
      check(close(before[entity][key], after[entity][key], .05), `Save lost ${entity}.${key}`);
    }
    trace.push({ stage: 'persistent-save', before, after });
  });
  await test('complete lantern route and task-specific victory', async () => {
    await fresh(); await moveTo(-8, 12);
    for (const lantern of level.lanterns.slice(0, 11)) {
      await moveTo(lantern.position[0], lantern.position[2]);
      const s = await act();
      check(s.lanterns.find(x => x.id === lantern.id).lit, `Did not light ${lantern.id}`);
    }
    await moveTo(4.8, 8); await input(1, 0); await step(65); await input(); await step(15);
    let s = await state();
    // Move back from crate's face to jump onto its top, then land before second jump.
    await moveTo(s.crate.x - 1.5, s.crate.z);
    await input(1, 0, true); await step(20); await input(); await step(60);
    s = await state();
    check(s.player.y > .9 && s.player.grounded, `Did not land on crate: ${JSON.stringify(s.player)}`);
    await input(1, 0, true); await step(35); await input(); await step(60);
    s = await state();
    check(s.player.y > 2.2 && s.player.grounded, `Did not reach ledge: ${JSON.stringify(s.player)}`);
    await moveTo(10, 8); s = await act();
    check(s.count === 12 && s.remaining > 0, 'Original twelve must be collected before night');
    if (task === 'extra') {
      check(s.phase === 'playing', 'Twelve must no longer win');
      await input(0, 0, false, true); await step(1); await input();
      check((await state()).count === 12, 'Thirteenth lantern cannot be collected remotely');
      await moveTo(14, 8); await moveTo(14, 12); await moveTo(-8, 12); await moveTo(-8, 9.4);
      await input(0, -1, true); await step(27); await input(); await step(60);
      s = await state();
      check(s.player.y > 1.05 && s.player.grounded, 'New platform must be reachable and solid');
      await moveTo(-8, 7); s = await act();
      check(s.count === 13 && s.phase === 'won', 'All thirteen must win');
      await command({ op: 'save' }); await adapter.reload();
      
      await ready(); await command({ op: 'load' });
      check((await state()).count === 13, 'Reload must retain thirteenth lantern');
    } else if (task === 'home') {
      check(s.phase === 'playing', 'All twelve must await return home');
      await screenshot('return-home');
      await command({ op: 'save' }); await adapter.reload();
      
      await ready(); await command({ op: 'load' });
      s = await state();
      check(s.phase === 'playing' && s.count === 12, 'Return-home intermediate state must persist');
      await step(Math.ceil(s.remaining * 60) + 1);
      check((await state()).phase === 'lost', 'Timing out on the return must lose');
      await command({ op: 'load' });
      await moveTo(14, 8); await moveTo(14, 12); await moveTo(1.5, 12, .6);
      s = await state();
      check(s.phase === 'won' && Math.hypot(s.player.x, s.player.z - 12) <= 2.05, 'Must win on return to spawn');
    } else check(s.phase === 'won', 'All lanterns must win');
    trace.push({ stage: 'win', state: s }); await screenshot('won');
  });
  await test('night falls and restart works', async () => {
    await fresh(); await step(level.duration * 60);
    const s = await state();
    check(s.phase === 'lost' && s.remaining <= .02 && s.count === 0, 'No-action game must lose at180s');
    await screenshot('lost');
    await command({ op: 'start' });
    const restart = await state();
    check(restart.phase === 'playing' && restart.count === 0 && restart.ticks === 0, 'Restart must clear prior game');
  });
  await test('one batch equals many fixed ticks', async () => {
    await fresh(); await input(0, -1); await step(60); const one = await state();
    await fresh(); await input(0, -1);
    for (let i = 0; i < 60; i++) await step(1);
    const many = await state();
    check(one.ticks === many.ticks, 'Different tick count');
    for (const entity of ['player', 'crate']) for (const key of ['x', 'y', 'z', 'vx', 'vy', 'vz']) {
      check(close(one[entity][key], many[entity][key], .001), `Batch mismatch ${entity}.${key}`);
    }
  });
} finally {
  await adapter.close();
  const report = { instrumentVersion:adapter.version, engine, task, unavailable:[...adapter.unavailable],limitations:adapter.limitations, recordedAt: new Date().toISOString(), renderer:'Linux CPU UI only; world pixels unsupported',
    milliseconds: performance.now() - started, cases, errors, trace,
    passed: cases.length === 10 && cases.every(c => c.passed) && errors.length === 0 };
  await writeFile(`${out}/acceptance.json`, JSON.stringify(report, null, 2));
  if (!report.passed) process.exitCode = 1;
  console.log(JSON.stringify(report));
}
