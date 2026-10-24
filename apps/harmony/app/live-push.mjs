#!/usr/bin/env bun
/**
 * live-push — a message another account sends appears in an open room with no
 * poll: the change feed's long poll (`live`, app.ts) brings it while the
 * clock is frozen, so the 10 s `clock` task never runs.
 *
 * `bun scripts/memory-backend-test.mjs apps/harmony/app` runs it after the
 * contract tests, on the server and build it checked (a temporary store and
 * that origin alone), with EXACT_BACKEND_ORIGIN, EXACT_WEB_DIST and
 * EXACT_APP_DIR naming them; it is never run against a server whose data
 * matters, as it writes a message there.
 *
 * Ada's app opens #general and holds its long poll; then Grace, another
 * client (Snapback's own mutation route, as her persona), sends a message,
 * and `clock held live` waits in real time, the clock frozen, for the long
 * poll's reply, which must carry Grace's write (its `seq`), after which Ada's
 * room shows it. #general is marked read for Ada first, so opening it writes
 * no read mark whose reply could release the wait instead.
 */
import { spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { createInterface } from 'node:readline';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(fileURLToPath(new URL('../../..', import.meta.url)));
const GENERAL = '01HARMNY00000000000000000E';
const { EXACT_BACKEND_ORIGIN: origin, EXACT_WEB_DIST: dist, EXACT_APP_DIR: app } = process.env;
if (!origin || !dist || !app) throw new Error('live-push runs under scripts/memory-backend-test.mjs, which names its server and build');

const body = `Live from Grace ${randomUUID().slice(0, 8)}`;
// The room read before the send must not show it; the held clock (40 s, past
// Snapback's 25 s hold, so a send that lands before the wait starts is still
// read at the next reply) must settle; `follow` runs a commit later (`clock
// +5`, 5 ms of the 10 s tick's 10,000); then the room shows it.
const ops = ['clock data', 'tap room-general', 'clock settle', 'clock data', 'state latest.body', 'clock held live 40s', 'state live.seq', 'clock +5', 'clock data', 'state latest.body'];
const as = (persona) => ({ 'content-type': 'application/json', 'x-snapback-persona': persona });
const read = await fetch(`${origin}/m/markRead`, { method: 'POST', headers: as('ada'), body: JSON.stringify({ id: randomUUID(), args: { roomId: GENERAL } }) });
const marked = await read.json().catch(() => null);
if (!read.ok || marked?.state !== 'sent') throw new Error(`Ada's read mark answered ${read.status}: ${JSON.stringify(marked).slice(0, 300)}`);
const driver = spawn(process.execPath, ['scripts/agent.mjs', 'web', '--app', app, '--storage', 'live-push', '--epoch', 'now', ...ops],
  { cwd: ROOT, stdio: ['ignore', 'pipe', 'inherit'], env: process.env });
const exited = new Promise((ok) => driver.once('exit', (code) => ok(code ?? 1)));

const bodies = [];
let held = null, liveSeq = null, sentAt = 0, sent = null;
for await (const line of createInterface({ input: driver.stdout })) {
  console.log(line);
  if (/^clock .*held/.test(line) && bodies.length === 1 && !held) held = { line, ms: performance.now() - sentAt };
  if (/^\d+$/.test(line) && bodies.length === 1) liveSeq = Number(line);
  if (!line.startsWith('"')) continue;
  bodies.push(JSON.parse(line));
  if (bodies.length !== 1) continue;
  // The wait has begun by now (the driver sends its next op at once); the send lands during it.
  await new Promise((ok) => setTimeout(ok, 1000));
  sentAt = performance.now();
  const r = await fetch(`${origin}/m/sendMessage`, { method: 'POST', headers: as('grace'),
    body: JSON.stringify({ id: randomUUID(), args: { roomId: GENERAL, body } }) });
  sent = await r.json().catch(() => null);
  if (!r.ok || sent?.state !== 'sent') { driver.kill('SIGTERM'); throw new Error(`Grace's send answered ${r.status}: ${JSON.stringify(sent).slice(0, 300)}`); }
}
const code = await exited;

const fail = (why) => { console.error(`live-push: ${why}`); process.exit(1); };
if (code !== 0) fail(`the driver exited ${code}`);
if (bodies.length !== 2) fail(`the driver printed ${bodies.length} room reads, not 2`);
if (bodies[0] === body) fail('Ada\'s room showed the message before Grace sent it');
if (!held || /NOT settled/.test(held.line)) fail(`the long poll brought no reply: ${held?.line ?? 'no clock held line'}`);
if (!(liveSeq >= sent.seq)) fail(`the long poll's reply left live.seq at ${liveSeq}, before Grace's write (seq ${sent.seq})`);
if (bodies[1] !== body) fail(`Ada's room shows ${JSON.stringify(bodies[1])} after the long poll's reply, not Grace's ${JSON.stringify(body)}`);
console.error(`live-push: Grace's message reached Ada's open room in ${held.ms.toFixed(0)} ms, the clock frozen (no poll)`);
