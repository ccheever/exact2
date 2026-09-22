#!/usr/bin/env bun
import { spawn } from 'node:child_process';
import { mkdtempSync, rmSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve, join, extname } from 'node:path';
const here = resolve(import.meta.dir);
const root = resolve(here, '..');
const started = Date.now(), checks = [], browsers = [];
const assert = (name, ok, detail = null) => { checks.push({ name, pass: !!ok, detail }); console.log(`${ok ? 'PASS' : 'FAIL'} ${name}${detail === null ? '' : ` ${JSON.stringify(detail)}`}`); };
const equal = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const delay = ms => new Promise(r => setTimeout(r, ms));
const server = Bun.serve({ port: process.argv.includes('--serve') ? 8086 : 0, hostname: '127.0.0.1', async fetch(req) {
  const path = resolve(root, '.' + decodeURIComponent(new URL(req.url).pathname));
  if (!path.startsWith(root + '/') || !(path.startsWith(here + '/') || path.startsWith(join(root, 'node_modules/three/') ))) return new Response('Forbidden', { status: 403 });
  const file = Bun.file(path);
  if (!(await file.exists())) return new Response('Missing', { status: 404 });
  return new Response(file, { headers: { 'Content-Type': { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.json': 'application/json' }[extname(path)] ?? 'application/octet-stream', 'Cache-Control': 'no-store' } });
}});
const url = `http://127.0.0.1:${server.port}/beacons/index.html`;
if (process.argv.includes('--serve')) { console.log(url); await new Promise(() => {}); }
async function launch(label) {
  const profile = mkdtempSync(join(here, '.chrome-'));
  const child = spawn('/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', [
    '--headless=new', '--no-sandbox', '--disable-crash-reporter', `--user-data-dir=${profile}`, '--remote-debugging-port=0', '--no-first-run',
    '--no-default-browser-check', '--disable-background-networking', '--disable-component-update',
    '--disable-sync', '--disable-background-timer-throttling', '--disable-renderer-backgrounding',
    '--window-size=1280,800', 'about:blank'], { stdio: ['ignore', 'ignore', 'pipe'] });
  let stderr = ''; child.stderr.on('data', d => { stderr += d; });
  const browser = { child, profile, label, errors: [] }; browsers.push(browser);
  console.log(`Chrome ${label} launched PID ${child.pid}`);
  let port;
  for (let n = 0; n < 150; n++) {
    try { port = readFileSync(join(profile, 'DevToolsActivePort'), 'utf8').split('\n')[0]; break; } catch {}
    if (child.exitCode !== null || child.signalCode) throw new Error(`Chrome exited code=${child.exitCode} signal=${child.signalCode}: ${stderr}`);
    await delay(100);
  }
  if (!port) throw new Error(`Chrome debug port timeout: ${stderr}`);
  const tabs = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
  const ws = new WebSocket(tabs.find(t => t.type === 'page').webSocketDebuggerUrl);
  browser.ws = ws;
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = rej; });
  let seq = 0; const pending = new Map();
  ws.onmessage = event => {
    const m = JSON.parse(event.data);
    if (m.id) { const p = pending.get(m.id); if (!p) return; pending.delete(m.id); clearTimeout(p.timer); m.error ? p.reject(new Error(JSON.stringify(m.error))) : p.resolve(m.result); }
    else if (m.method === 'Runtime.exceptionThrown') browser.errors.push(m.params.exceptionDetails);
  };
  browser.send = (method, params = {}) => new Promise((resolve, reject) => {
    const id = ++seq, timer = setTimeout(() => { pending.delete(id); reject(new Error(`CDP timeout: ${method}`)); }, 10000);
    pending.set(id, { resolve, reject, timer }); ws.send(JSON.stringify({ id, method, params }));
  });
  browser.eval = async expression => {
    const r = await browser.send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
    if (r.exceptionDetails) throw new Error(JSON.stringify(r.exceptionDetails));
    return r.result.value;
  };
  await browser.send('Runtime.enable');
  await browser.send('Page.enable');
  await browser.send('Emulation.setDeviceMetricsOverride', { width: 1280, height: 800, deviceScaleFactor: 1, mobile: false });
  await browser.send('Page.navigate', { url: `${url}?proof` });
  let ready = false;
  for (let n = 0; n < 150; n++) { ready = await browser.eval('!!window.beacons'); if (ready) break; await delay(100); }
  if (!ready) throw new Error(`Game load failed: ${JSON.stringify(browser.errors)}`);
  assert(`${label}: real three.js r186 and WebGL draw calls`, await browser.eval('beacons.revision === "186" && beacons.rendered().calls > 0'));
  return browser;
}
async function close(b) {
  b.ws?.close();
  if (b.child.exitCode === null && !b.child.signalCode) {
    const exit = new Promise(resolve => b.child.once('exit', resolve));
    b.child.kill('SIGTERM'); // Only the process object/PID recorded at launch.
    await Promise.race([exit, delay(2500)]);
    if (b.child.exitCode === null && !b.child.signalCode) { b.child.kill('SIGKILL'); await exit; }
  }
  rmSync(b.profile, { recursive: true, force: true });
}
async function click(b, id) {
  const rect = await b.eval(`(() => { const r=document.getElementById(${JSON.stringify(id)}).getBoundingClientRect(); return {x:r.x+r.width/2,y:r.y+r.height/2}; })()`);
  await b.send('Input.dispatchMouseEvent', { type: 'mousePressed', ...rect, button: 'left', clickCount: 1 });
  await b.send('Input.dispatchMouseEvent', { type: 'mouseReleased', ...rect, button: 'left', clickCount: 1 });
}
async function key(b, code, down) {
  const keys = { KeyW: ['w', 87], KeyA: ['a', 65], KeyS: ['s', 83], KeyD: ['d', 68], KeyE: ['e', 69], Space: [' ', 32], ArrowUp: ['ArrowUp', 38] };
  await b.send('Input.dispatchKeyEvent', { type: down ? 'keyDown' : 'keyUp', code, key: keys[code][0], windowsVirtualKeyCode: keys[code][1] });
}
const advance = (b, ticks) => b.eval(`beacons.advance(${ticks})`);
const state = b => b.eval('beacons.state()');
async function walk(b, x, z) {
  let held = [];
  for (let n = 0; n < 220; n++) {
    const s = await state(b), dx = x - s.player.x, dz = z - s.player.z;
    if (Math.hypot(dx, dz) < .45) break;
    const desired = [];
    if (Math.abs(dx) > .16) desired.push(dx > 0 ? 'KeyD' : 'KeyA');
    if (Math.abs(dz) > .16) desired.push(dz > 0 ? 'KeyS' : 'KeyW');
    for (const c of held) if (!desired.includes(c)) await key(b, c, false);
    for (const c of desired) if (!held.includes(c)) await key(b, c, true);
    held = desired; await advance(b, 12);
  }
  for (const c of held) await key(b, c, false);
  await advance(b, 90);
  const p = (await state(b)).player;
  assert(`${b.label}: walked within interaction radius (${x}, ${z})`, Math.hypot(x - p.x, z - p.z) <= 1.5, { x: p.x, z: p.z });
}
async function prefix(b) {
  assert(`${b.label}: title and accessible Play button`, await b.eval('!document.getElementById("title").hidden && document.getElementById("play").textContent === "Play" && document.activeElement.id === "play"'));
  await click(b, 'play'); await key(b, 'KeyW', true);
  const first = await advance(b, 180); await key(b, 'KeyW', false);
  // Independent closed-form sum of the specified exponential velocity recurrence.
  const q = Math.exp(-10 / 120), expected = -4 / 120 * (180 - q * (1 - q ** 180) / (1 - q));
  assert(`${b.label}: W for exactly 1.5 s, position within 1 mm`, first.tick === 180 && Math.abs(first.player.z - expected) <= .001 && first.player.x === 0 && first.player.y === 0, { expected: [0, 0, expected], actual: [first.player.x, first.player.y, first.player.z] });
  await walk(b, 8, 0);
  await key(b, 'KeyE', true); const early = await advance(b, 12); await key(b, 'KeyE', false);
  assert(`${b.label}: beacon count, DOM HUD and eased material at 0.1 s`, early.beacons.filter(x => x.lit).length === 1 && Math.abs(early.beacons[0].glow - .104) < 1e-12 && await b.eval('document.getElementById("count").textContent === "Beacons 1 / 3" && Math.abs(beacons.rendered().emissive[0] - .208) < 1e-12'), { glow: early.beacons[0].glow });
  await click(b, 'pause'); const frozen = await b.eval('beacons.save()'); await advance(b, 240);
  assert(`${b.label}: pause freezes every saved field through 2 s`, frozen === await b.eval('beacons.save()') && await b.eval('document.getElementById("pause").textContent === "Resume"'));
  await click(b, 'pause');
  await key(b, 'KeyD', true); await key(b, 'Space', true); await advance(b, 7);
  const save = await b.eval('beacons.save()');
  assert(`${b.label}: mid-run save is airborne, moving, easing and holding input`, await b.eval('beacons.state().player.y > 0 && beacons.state().beacons[0].glow < 1 && beacons.state().keys.includes("KeyD")'));
  return { first, save };
}
async function continuation(b) {
  await advance(b, 41); await key(b, 'Space', false); await key(b, 'Space', true);
  const before = (await state(b)).player.vy; await advance(b, 1);
  assert(`${b.label}: airborne Space does not double jump`, Math.abs((await state(b)).player.vy - (before - .1)) < 1e-12);
  await key(b, 'Space', false); await advance(b, 59); // 120 ticks since E, excluding paused time.
  assert(`${b.label}: glow fully up 1 s after E`, await b.eval('beacons.state().beacons[0].glow === 1 && beacons.rendered().emissive[0] === 2'));
  await key(b, 'KeyD', false); await advance(b, 90);
  return b.eval('beacons.save()');
}
try {
  console.log(`Proof started ${new Date(started).toISOString()}`);
  const a = await launch('A'); const pa = await prefix(a); const endA = await continuation(a);
  writeFileSync(join(here, 'save.json'), pa.save + '\n');
  const shot = await a.send('Page.captureScreenshot', { format: 'png' });
  writeFileSync(join(here, 'playing.png'), Buffer.from(shot.data, 'base64'));
  // The human screenshot is an artifact, not a pixel-perfect rendering assertion.
  await walk(a, -6, 7); await key(a, 'KeyE', true); await advance(a, 120); await key(a, 'KeyE', false);
  await walk(a, 3, -9); await key(a, 'KeyE', true); await advance(a, 120); await key(a, 'KeyE', false);
  assert('A: all three beacons produce accessible win UI', await a.eval('beacons.state().mode === "won" && !document.getElementById("win").hidden && document.getElementById("won").textContent === "All beacons lit"'));
  await click(a, 'again');
  assert('A: Play again resets the world and repeats seeded crates', await a.eval('beacons.state().tick === 0 && beacons.state().player.x === 0 && beacons.state().beacons.every(b => !b.lit)') && equal((await state(a)).crates, pa.first.crates));
  await key(a, 'Space', true); let peak = 0;
  for (let i = 0; i < 120; i++) peak = Math.max(peak, (await advance(a, 1)).player.y);
  await key(a, 'Space', false);
  assert('A: hop reaches 1.2 m and lands without repeating held Space', Math.abs(peak - 1.2) < .001 && (await state(a)).player.grounded, { peak });
  await key(a, 'ArrowUp', true); await advance(a, 180); await key(a, 'ArrowUp', false);
  assert('A: arrow movement matches W', Math.abs((await state(a)).player.z - pa.first.player.z) < 1e-12);
  assert('A: no browser runtime exceptions', a.errors.length === 0, a.errors);
  await close(a);
  const b = await launch('B'); const pb = await prefix(b); const endB = await continuation(b);
  assert('A/B: identical states after W, at save, and at continuation end', equal(pa.first, pb.first) && pa.save === pb.save && endA === endB);
  assert('B: no browser runtime exceptions', b.errors.length === 0, b.errors); await close(b);
  const c = await launch('C fresh restore');
  await c.eval(`beacons.restore(${JSON.stringify(readFileSync(join(here, 'save.json'), 'utf8').trim())})`);
  assert('C: disk save restores every field exactly in fresh Chrome process', pa.save === await c.eval('beacons.save()'));
  const endC = await continuation(c);
  assert('A/C: restored continuation is byte-identical to original', endA === endC);
  assert('C: no browser runtime exceptions', c.errors.length === 0, c.errors); await close(c);
  writeFileSync(join(here, 'proof-result.json'), JSON.stringify({ pass: checks.every(c => c.pass), started: new Date(started).toISOString(), finished: new Date().toISOString(), seconds: (Date.now() - started) / 1000, checks, endState: JSON.parse(endA), screenshot: 'playing.png', browserPids: browsers.map(b => b.child.pid) }, null, 2) + '\n');
  process.exitCode = checks.every(c => c.pass) ? 0 : 1;
} catch (e) {
  console.error(e.stack); writeFileSync(join(here, 'proof-error.txt'), `${new Date().toISOString()}\n${e.stack}\n`); process.exitCode = 1;
} finally { for (const b of browsers) await close(b); server.stop(true); }
