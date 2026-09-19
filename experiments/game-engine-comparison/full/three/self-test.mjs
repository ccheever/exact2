import { chromium } from '@playwright/test';
import { strict as assert } from 'node:assert';
import { mkdir } from 'node:fs/promises';

const baseURL = process.env.BASE_URL || 'http://127.0.0.1:4173';
const executablePath = process.env.CHROMIUM || '/home/ccheever/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome';
const browser = await chromium.launch({ headless: true, executablePath, args: ['--disable-gpu-sandbox'] });
const context = await browser.newContext({ viewport: { width: 1280, height: 800 } });
const page = await context.newPage();
const errors = [];
page.on('pageerror', (error) => errors.push(String(error)));
page.on('console', (message) => { if (message.type() === 'error') errors.push(message.text()); });

const command = (request) => page.evaluate((value) => window.lanterns.command(value), request);
const input = async (moveX = 0, moveZ = 0, jump = false, act = false) => command({ op: 'input', moveX, moveZ, jump, act });
const moveTo = async (x, z, tolerance = .62, limit = 900) => {
  for (let used = 0; used < limit;) {
    const state = await command({ op: 'state' });
    const dx = x - state.player.x;
    const dz = z - state.player.z;
    const distance = Math.hypot(dx, dz);
    if (distance <= tolerance) { await input(); return state; }
    await input(dx / distance, dz / distance);
    const ticks = Math.max(1, Math.min(30, Math.floor((distance - tolerance) * 60 / 4.5)));
    await command({ op: 'step', ticks });
    used += ticks;
  }
  const state = await command({ op: 'state' });
  throw new Error(`Could not walk to ${x},${z}; at ${state.player.x},${state.player.y},${state.player.z}`);
};
const lightHere = async () => {
  await input(0, 0, false, true);
  const state = await command({ op: 'step', ticks: 1 });
  await input();
  return state;
};

try {
  await page.goto(`${baseURL}/?agent=1`, { waitUntil: 'domcontentloaded' });
  assert.deepEqual(await command({ op: 'ready' }), { ready: true, engine: 'Three.js', version: '0.186.0' });
  let state = await command({ op: 'state' });
  assert.equal(state.phase, 'title');
  assert.equal(state.assetReady, true);
  await mkdir('screenshots', { recursive: true });
  await page.screenshot({ path: 'screenshots/title.png' });

  state = await command({ op: 'start' });
  await input(1, 0);
  state = await command({ op: 'step', ticks: 60 });
  assert.equal(state.ticks, 60);
  assert.ok(state.player.x > 4 && state.player.x < 5, `world-relative movement x=${state.player.x}`);
  state = await command({ op: 'pause' });
  assert.equal(state.phase, 'paused');
  const pausedTicks = state.ticks;
  state = await command({ op: 'step', ticks: 60 });
  assert.equal(state.ticks, pausedTicks);
  await command({ op: 'pause' });
  const saved = await command({ op: 'save' });
  assert.deepEqual(saved, { saved: true });
  const beforeReload = await command({ op: 'state' });
  await page.reload({ waitUntil: 'domcontentloaded' });
  await command({ op: 'ready' });
  state = await command({ op: 'load' });
  assert.equal(state.phase, beforeReload.phase);
  assert.equal(state.ticks, beforeReload.ticks);
  assert.ok(Math.abs(state.player.x - beforeReload.player.x) < .001);
  assert.equal(state.player.vx, 4.5);
  await page.waitForTimeout(350);
  await page.screenshot({ path: 'screenshots/gameplay.png' });

  await command({ op: 'start' });
  const groundRoute = [[0,7],[0,0],[5,2],[12,0],[12,-8],[6,-12],[0,-10],[-6,-12],[-12,-8],[-10,2],[-12,10]];
  for (let index = 0; index < groundRoute.length; index += 1) {
    await moveTo(...groundRoute[index]);
    state = await lightHere();
    assert.equal(state.count, index + 1, `lantern route index ${index}`);
  }

  for (const waypoint of [[-7,12],[-2,12],[4.85,8]]) await moveTo(...waypoint, .18);
  for (let index = 0; index < 120; index += 1) {
    state = await command({ op: 'state' });
    if (state.crate.x >= 7.0) break;
    await input(1, 0);
    state = await command({ op: 'step', ticks: 2 });
  }
  await input();
  assert.ok(state.crate.x >= 6.95, `crate pushed to x=${state.crate.x}`);

  await input(1, 0, true, false);
  await command({ op: 'step', ticks: 1 });
  await input(1, 0, false, false);
  let landedOnCrate = false;
  for (let index = 0; index < 120; index += 1) {
    state = await command({ op: 'step', ticks: 1 });
    if (state.player.y > 1.08 && Math.abs(state.player.x - state.crate.x) < .55) await input();
    if (state.player.grounded && state.player.y > .9 && state.player.y < 1.6) { landedOnCrate = true; break; }
  }
  assert.ok(landedOnCrate, `landed on crate: player=${JSON.stringify(state.player)} crate=${JSON.stringify(state.crate)}`);

  await input(1, 0, true, false);
  await command({ op: 'step', ticks: 1 });
  await input(1, 0, false, false);
  let landedOnLedge = false;
  for (let index = 0; index < 150; index += 1) {
    state = await command({ op: 'step', ticks: 1 });
    if (state.player.x > 9.15) await input();
    if (state.player.grounded && state.player.y > 2.15) { landedOnLedge = true; break; }
  }
  assert.ok(landedOnLedge, `landed on ledge: ${JSON.stringify(state.player)}`);
  await moveTo(10, 8, .5);
  state = await lightHere();
  assert.equal(state.phase, 'won');
  assert.equal(state.count, 12);
  assert.ok(state.events.some((event) => event.type === 'win'));
  await page.waitForTimeout(400);
  await page.screenshot({ path: 'screenshots/win.png' });
  assert.deepEqual(errors, []);
  console.log(JSON.stringify({ ok: true, ready: true, reloadSave: true, groundLanterns: 11, crateX: state.crate.x, finalPlayerY: state.player.y, wonAtTicks: state.ticks, errors }, null, 2));
} finally {
  await browser.close();
}
