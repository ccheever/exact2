// The drive's launch facts (agent-launch.mjs `launchFacts`, LLP 1027.000.000 D3), and the browser it finds; the fixed
// defaults and overrides are host/web/agent.test.mjs's "launch setup" test.
import { test, expect } from 'bun:test';
import { chmodSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, resolve } from 'node:path';
import { chromium, launchFacts, launchEnvironment, parseFlags, launchLog, launchFailure, webPort } from './agent-launch.mjs';
import { timeReporter } from '../host/web/navigation.js';

// LLP 1107 D1: `--reader` is a flag of the drive, said to the Android carrier as the variable it reads
// (agent-android.mjs `androidBinary`), and never an op.
test('--reader names the Canvas reader to the Android carrier', () => {
  const before = process.env.EXACT_ANDROID_READER;
  try {
    delete process.env.EXACT_ANDROID_READER;
    expect(parseFlags(['android', '--app', 'shelf', 'tree']).flags.reader).toBeUndefined();
    expect(process.env.EXACT_ANDROID_READER).toBeUndefined();
    const { flags, rest } = parseFlags(['android', '--reader', '--app', 'shelf', 'tree']);
    expect(flags.reader).toBe(true);
    expect(rest).toEqual(['android', 'tree']);
    expect(process.env.EXACT_ANDROID_READER).toBe('1');
  } finally {
    if (before === undefined) delete process.env.EXACT_ANDROID_READER; else process.env.EXACT_ANDROID_READER = before;
  }
});

// App farm round 2: a drive against a live backend opts into the machine's clock, read once at launch; the host is
// told those milliseconds, so `state.time.epochAtZero` is the launch instant and `clock +N` moves the date from it.
test('--epoch now is the machine clock read once at launch, reported as that instant', () => {
  const {flags} = parseFlags(['web', '--epoch', 'now', 'state']);
  expect(flags.epoch).toBe('now');
  const before = Date.now(), facts = launchFacts(flags), after = Date.now();
  expect(facts.epoch).toBeGreaterThanOrEqual(before);
  expect(facts.epoch).toBeLessThanOrEqual(after);
  expect(launchEnvironment(facts).EXACT_AGENT_EPOCH).toBe(String(facts.epoch));
  // A relaunch from the environment the driver hands a host keeps the instant it read, not a later one.
  expect(launchFacts({env:launchEnvironment(facts)}).epoch).toBe(facts.epoch);
  const fromEnv = launchFacts({env:{EXACT_AGENT_EPOCH:'now'}}).epoch;
  expect(Math.abs(fromEnv - Date.now())).toBeLessThan(1000);
  const time = timeReporter(new URLSearchParams({agent:'1', ...facts}), new Proxy({}, {get() { throw new Error('agent read the platform'); }}));
  expect(time(0)).toEqual([facts.epoch, 0]);
  expect(time(60000)).toEqual([facts.epoch, 0]);
});

test('Windows browser discovery accepts an installed Chrome and explicit executable paths with spaces', () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact browser paths-'));
  try {
    const chrome = resolve(dir, 'Google/Chrome/Application/chrome.exe');
    mkdirSync(dirname(chrome), {recursive:true});
    writeFileSync(chrome, '');
    chmodSync(chrome, 0o755);
    expect(chromium({ProgramFiles:dir}, 'win32').executable).toBe(chrome);
    expect(chromium({CHROME:chrome}, 'win32').unavailable).toBe(null);
    expect(chromium({CHROME:resolve(dir, 'missing.exe')}, 'win32').unavailable).toBeTruthy();
  } finally { rmSync(dir, {recursive:true, force:true}); }
});
// App farm wave 1 (21 builds): a builder on a shared machine keeps every listener in its assigned range.
test('a web drive takes its port from --port, else EXACT_PORT, and refuses anything but a port', () => {
  expect(parseFlags(['web', '--port', '50821', 'tree']).flags.port).toBe('50821');
  expect(webPort('50821', {EXACT_PORT: '50822'})).toBe(50821);
  expect(webPort(undefined, {EXACT_PORT: '50822'})).toBe(50822);
  expect(webPort(undefined, {})).toBe(null);
  expect(() => webPort('99999', {})).toThrow('--port 99999: a port number');
  expect(() => webPort(undefined, {EXACT_PORT: 'x'})).toThrow('EXACT_PORT x: a port number');
});

// App farm 082: Chrome's fatal line came before a native stack the tail kept instead.
test("a Chrome that dies at launch is reported by its first fatal line, before the stack's tail", () => {
  const log = launchLog();
  log.add('[1:1:1011/000000.000:FATAL:process_singleton_posix.cc(313)] Socket path too long: /long/.org.chromium.Chromium.x/SingletonSocket');
  for (let i = 0; i < 20; i++) log.add(`  r${i}: 0000000000000006`);
  const message = launchFailure(new Error('the DevTools pipe closed'), log, {alive: false, chrome: '/c', env: {TMPDIR: '/long'}}).message;
  expect(message).toContain('FATAL:process_singleton_posix.cc(313)] Socket path too long');
  expect(message).toContain('r19: 0000000000000006');
  expect(message).toContain('give the drive a shorter TMPDIR');
  expect(launchFailure(new Error('closed'), launchLog(), {alive: false, chrome: '/c'}).message).toContain('(it printed nothing)\nSet CHROME');
});

// App farm wave 1 (four builds): `test web` against no `snapback4 dev` failed as data mismatches. The loopback origins
// app.ts fetches are asked first; a Snapback app's run is refused naming the origin, any other app's is told.
test('a web test run asks the backend its app fetches first', async () => {
  const { backendPreflight, loopbackOrigins, fetchOrigins } = await import('./agent-test.mjs');
  const { createServer } = await import('node:net');
  const { mkdtempSync, mkdirSync, rmSync, writeFileSync } = await import('node:fs');
  const { tmpdir } = await import('node:os');
  const { resolve } = await import('node:path');
  const dist = mkdtempSync(resolve(tmpdir(), 'exact-preflight-')), server = createServer().listen(0, '127.0.0.1');
  try {
    await new Promise((ok) => server.once('listening', ok));
    const up = `http://127.0.0.1:${server.address().port}`, down = 'http://127.0.0.1:1';
    mkdirSync(resolve(dist, '.gen'));
    const grants = (...lines) => writeFileSync(resolve(dist, '.gen/admission-data.js'), `export const tsGrantSet=createGrantSet(${JSON.stringify({ entries: lines.map((l, i) => [i, l, [], null]) })});`);
    grants(`net.fetch ${up}`, `net.fetch ${down}/api`, 'net.fetch https://example.com', 'sqlite.open app:/data');
    expect(loopbackOrigins(dist)).toEqual([up, down]);
    expect(fetchOrigins(dist)).toEqual([up, down, 'https://example.com']);
    const snapback = { manifest: { typescript: { sources: { snapback4: '../../exact2/snapback4/ts' } } } }, said = [];
    await expect(backendPreflight(snapback, dist)).rejects.toThrow(`no backend at ${down}: nothing listens there`);
    await backendPreflight({ manifest: {} }, dist, (line) => said.push(line));
    expect(said).toEqual([`note: no backend at ${down}: nothing listens there, and app.ts fetches it (a net.fetch grant); a test that loads from it fails`]);
    grants(`net.fetch ${up}`);
    await backendPreflight(snapback, dist);
    expect(loopbackOrigins(resolve(dist, 'none'))).toEqual([]);
  } finally { server.close(); rmSync(dist, { recursive: true, force: true }); }
});

// A mutation's refused reply is named with why, from either runner's line (sb4 farm wave 6, E3); a resource's is not.
test("a mutation whose reply was refused is named with the field its shape lacks", async () => {
  const { refusedReplies } = await import('./agent-test.mjs');
  const why = '`post` answered outside its shape: field `seq` is not in the shape';
  expect([...refusedReplies([
    `t=12 refused reply posted; wall 3 ms: ${why}`,
    `t=12 request 5 (posted) failed and is no longer pending: it ends unsent: ${why}`, // the web runtime
    'request 7 (saved) failed and is no longer pending: it is not retried: `save` answered outside its shape: field `id` is missing', // the runner
    'request 8 (board) failed and is no longer pending: it keeps its last value: E_OFFLINE', // a resource: `state.failed` says it
    { not: 'a line' },
  ])]).toEqual([['posted', why], ['saved', '`save` answered outside its shape: field `id` is missing']]);
  expect(refusedReplies(undefined).size).toBe(0);
});

// A named port a page just let go of is asked again for a moment; one held for good is refused.
test('a named port still held is asked again, then refused', async () => {
  const { listenOn } = await import('./agent-launch.mjs');
  const { createServer } = await import('node:http');
  const holder = createServer(), port = await listenOn(holder, 0);
  try {
    const next = createServer(), started = Date.now();
    setTimeout(() => holder.close(), 300);
    expect(await listenOn(next, port)).toBe(port);
    expect(Date.now() - started).toBeGreaterThanOrEqual(250);
    const third = createServer();
    await expect(listenOn(third, port, '127.0.0.1', 300)).rejects.toMatchObject({ code: 'EADDRINUSE' });
    // A port the OS handed out is in its ephemeral range, which a named port's refusal names where it can read it.
    const said = await listenOn(createServer(), port, '127.0.0.1', 0, true).catch((e) => e.message);
    expect(said).toContain(`port ${port} (--port or EXACT_PORT) is in use`);
    if (process.platform === 'linux') expect(said).toContain("in this machine's ephemeral range");
    next.close();
  } finally { holder.close(); }
});
