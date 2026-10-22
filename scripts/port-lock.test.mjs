import { expect, test } from 'bun:test';
import { spawn } from 'node:child_process';
import { holderOf, takePort } from './port-lock.mjs';

// A port nothing here serves: only its turn is used.
const PORT = 55001;
const LOCK = JSON.stringify(new URL('./port-lock.mjs', import.meta.url).pathname);
// Another process, run without blocking this one (which answers who holds the turn).
const run = (code, env = {}) => new Promise((done) => {
  const child = spawn(process.execPath, ['-e', code], { env: { ...process.env, ...env } });
  let stdout = '', stderr = '';
  child.stdout.on('data', (d) => { stdout += d; });
  child.stderr.on('data', (d) => { stderr += d; });
  child.on('exit', () => done({ stdout, stderr }));
});

test('runs take a port in turn: a second waits and says for whom, a holder killed frees it, and the wait runs out naming it', async () => {
  const give = await takePort(PORT, { label: 'the first', say: () => {} });
  expect(await holderOf(PORT)).toMatchObject({ label: 'the first', pid: process.pid });
  // Another process waits, and its wait runs out naming the holder.
  const other = await run(`const { takePort } = await import(${LOCK}); try { await takePort(${PORT}, { waitMs: 1200 }); } catch (e) { console.log(e.message); }`, { EXACT_PORT_TURNS: '' });
  expect(other.stderr).toMatch(/^port 55001: waiting for the first \(pid \d+/);
  expect(other.stdout).toMatch(/still taken by the first \(pid \d+/);
  // A run this one starts has the turn already.
  const child = await run(`const { takePort } = await import(${LOCK}); await takePort(${PORT}, { waitMs: 300 }); console.log('had it');`);
  expect(child.stdout.trim()).toBe('had it');
  give();
  give();
  expect(await holderOf(PORT)).toBe(null);
  // A holder killed before it gave the turn back frees it.
  const killed = spawn(process.execPath, ['-e', `const { takePort } = await import(${LOCK}); await takePort(${PORT}); console.log('taken'); setInterval(() => {}, 1000);`], { env: { ...process.env, EXACT_PORT_TURNS: '' } });
  await new Promise((ok) => killed.stdout.on('data', ok));
  killed.kill('SIGKILL');
  await new Promise((ok) => killed.on('exit', ok));
  (await takePort(PORT, { waitMs: 2000, say: () => {} }))();
});

test('claimPort: two runs at once each get a port of their own, past one something listens on, and give it back', async () => {
  const { claimPort } = await import('./port-lock.mjs');
  const net = await import('node:net');
  // A start whose own port something already listens on (a tool run by hand): it is passed over.
  const busy = net.createServer();
  await new Promise((ok) => busy.listen({ host: '127.0.0.1', port: 0 }, ok));
  const start = 17741 + Math.floor(Math.random() * 150);
  const squatter = net.createServer();
  const squatted = await new Promise((ok) => { squatter.once('error', () => ok(false)); squatter.listen({ host: '127.0.0.1', port: start, exclusive: true }, () => ok(true)); });
  busy.close();
  const before = process.env.EXACT_PORT_TURNS;
  try {
    const a = await claimPort(start, { label: 'run a' });
    // A second claim in another process sees a's turn held and takes the next.
    const b = await run(`const { claimPort } = await import(${LOCK}); console.log((await claimPort(${a.port}, { label: 'run b' })).port);`, { EXACT_PORT_TURNS: '' });
    const bPort = Number(b.stdout.trim());
    expect(bPort, b.stderr).toBeGreaterThan(0);
    if (squatted) expect(a.port).not.toBe(start);
    expect(bPort).not.toBe(a.port);
    expect(process.env.EXACT_PORT_TURNS?.split(',')).toContain(String(a.port));
    a.give();
    expect((process.env.EXACT_PORT_TURNS ?? '').split(',')).not.toContain(String(a.port));
  } finally {
    squatter.close();
    if (before === undefined) delete process.env.EXACT_PORT_TURNS; else process.env.EXACT_PORT_TURNS = before;
  }
});

test('claimPort: a port named in except is never taken (the contract tests\' stub on the checkout port)', async () => {
  const { claimPort } = await import('./port-lock.mjs');
  const before = process.env.EXACT_PORT_TURNS;
  const start = 17741 + Math.floor(Math.random() * 150);
  try {
    const got = await claimPort(start, { label: 'run c', except: [start, start + 1] });
    expect(got.port).not.toBe(start);
    expect(got.port).not.toBe(start + 1);
    got.give();
  } finally {
    if (before === undefined) delete process.env.EXACT_PORT_TURNS; else process.env.EXACT_PORT_TURNS = before;
  }
});
