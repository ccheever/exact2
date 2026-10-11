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
