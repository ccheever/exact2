// The async lane's skip rule for the semantics checks (scripts/async-inputs.mjs).
// No Cargo, Lean or Bun process runs here: inputs are temporary files.
import { afterAll, expect, test } from 'bun:test';
import { mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Inputs, SCHEMA, Store, UNAVAILABLE, changedSince, checkEnv, decide, entry, fingerprint, outcomeOf } from './async-inputs.mjs';
import { checks, semanticsChecks } from './async.mjs';

const dir = mkdtempSync(join(tmpdir(), 'async-inputs-'));
afterAll(() => rmSync(dir, { recursive: true, force: true }));
const fp = (groups, command = ['sh', ['-c', 'true']], env = {}) => fingerprint(...command, env, groups);

test('a path is absent, a directory, a link or a file by its content', () => {
  const root = mkdtempSync(join(dir, 'entry-'));
  expect(entry(join(root, 'missing'))).toBe('absent');
  mkdirSync(join(root, 'strings'));
  expect(entry(join(root, 'strings'))).toBe('dir');
  writeFileSync(join(root, 'a'), 'one');
  const one = entry(join(root, 'a'));
  writeFileSync(join(root, 'a'), 'two');
  expect(entry(join(root, 'a'))).not.toBe(one);
  symlinkSync('a', join(root, 'link'));
  expect(entry(join(root, 'link'))).toBe('link a');
});

test('a walk sees hidden files and empty directories, and skips what it is told to', () => {
  const root = mkdtempSync(join(dir, 'walk-'));
  const walk = (_, r, skip) => new Inputs(root, {}).lines([r], skip);
  mkdirSync(join(root, 'app/.shells'), { recursive: true });
  writeFileSync(join(root, 'app/.shells/app.json'), '{}');
  const before = walk(root, 'app');
  expect(before.some(l => l.startsWith('app/.shells/app.json\tfile '))).toBe(true);
  // An empty strings/ refuses the app where an absent one does not (contract/cli/src/strings.rs).
  mkdirSync(join(root, 'app/strings'));
  expect(walk(root, 'app')).not.toEqual(before);
  mkdirSync(join(root, 'app/.lake'));
  writeFileSync(join(root, 'app/.lake/build'), 'x');
  expect(walk(root, 'app', (_, name) => name === '.lake')).toEqual(walk(root, 'app', (_, name) => name === '.lake' || name === 'strings').concat('app/strings\tdir').sort());
});

test('a fingerprint moves with any input group, the command or the environment', () => {
  const base = fp({ binary: ['difftest\tfile 1'], programs: ['a\tfile 2'] });
  expect(fp({ binary: ['difftest\tfile 1'], programs: ['a\tfile 2'] }).digest).toBe(base.digest);
  expect(fp({ binary: ['difftest\tfile 1'], programs: ['a\tfile 3'] }).digest).not.toBe(base.digest);
  expect(fp({ binary: ['difftest\tfile 1'], programs: ['a\tfile 2'] }, ['sh', ['-c', 'false']]).digest).not.toBe(base.digest);
  expect(fp({ binary: ['difftest\tfile 1'], programs: ['a\tfile 2'] }, undefined, { DIFFTEST_JOBS: '0' }).digest).not.toBe(base.digest);
});

test('the environment keeps only what the checks may read', () => {
  const env = checkEnv({ HOME: '/h', PATH: '/p', DIFFTEST_JOBS: '0', EXACT_AGENT: '1', EXACT_JS_GPU_SURFACES: '1', BUN: '/b' }, { DIFFTEST_CACHE: '1' });
  expect(env).toEqual({ HOME: '/h', PATH: '/p', BUN: '/b', HERMES_LEAN_SYS_OFFLINE: '1', DIFFTEST_CACHE: '1' });
});

test('a check is skipped only while its fingerprint\'s last outcome is a pass', () => {
  const store = new Store(mkdtempSync(join(dir, 'store-')));
  const a = fp({ g: ['a'] }), b = fp({ g: ['b'] });
  expect(store.passed('difftest', a)).toBe(null);
  store.record('difftest', a, 'pass', { sha: 'one' });
  expect(store.passed('difftest', a)?.sha).toBe('one');
  expect(store.passed('difftest', b)).toBe(null);
  expect(store.passed('lean', a)).toBe(null);
  // A failure under the same inputs (a flake, or a tool that changed outside them) withdraws the pass.
  store.record('difftest', a, 'fail', { sha: 'two' });
  expect(store.passed('difftest', a)).toBe(null);
  store.record('difftest', a, 'pass', { sha: 'three' });
  expect(store.passed('difftest', a)?.sha).toBe('three');
  // What did not finish, or could not run, records nothing.
  store.record('difftest', b, 'unavailable');
  store.record('difftest', b, undefined);
  expect(store.read('difftest', `${b.digest}.json`)).toBe(null);
  expect(store.read('difftest', 'last-pass.json')?.schema).toBe(SCHEMA);
});

test('the store keeps its newest records', () => {
  const store = new Store(mkdtempSync(join(dir, 'prune-')), 3);
  const fps = [1, 2, 3, 4, 5].map(i => fp({ g: [String(i)] }));
  for (const f of fps) store.record('lean', f, 'pass');
  expect(fps.filter(f => store.read('lean', `${f.digest}.json`)).length).toBe(3);
  expect(store.passed('lean', fps[4])).not.toBe(null);
});

test('what changed is named against the last pass', () => {
  const store = new Store(mkdtempSync(join(dir, 'changed-')));
  const a = fp({ binary: ['1'], programs: ['1'] });
  expect(changedSince(store, 'difftest', a)).toEqual(['(no pass recorded)']);
  store.record('difftest', a, 'pass');
  expect(changedSince(store, 'difftest', fp({ binary: ['2'], programs: ['1'] }))).toEqual(['binary']);
});

test('a gated check is skipped on a pass under its inputs, and runs when the gate cannot tell', async () => {
  const store = new Store(mkdtempSync(join(dir, 'decide-')));
  const opts = { env: {}, inputs: async () => ({ g: ['a'] }) };
  const first = await decide(store, null, 'lean', 'sh', [], opts);
  expect(first.changed).toEqual(['(no pass recorded)']);
  store.record('lean', first.fp, 'pass', { sha: 'one' });
  expect((await decide(store, null, 'lean', 'sh', [], opts)).passed?.sha).toBe('one');
  expect((await decide(store, null, 'lean', 'sh', [], { ...opts, inputs: async () => ({ g: ['b'] }) })).changed).toEqual(['g']);
  const failed = await decide(store, null, 'lean', 'sh', [], { ...opts, inputs: async () => { throw new Error('no graph'); } });
  expect(failed).toEqual({ error: 'no graph' });
});

test('the semantics run as four gated checks, and only a finished run is an outcome', () => {
  const steps = checks('new', 'old', { lane: [], apple: false, glue: [] });
  expect(steps.filter(step => step[3]).map(([name]) => name)).toEqual(['lean', 'vm-extract', 'difftest', 'difftest-js']);
  const byName = Object.fromEntries(semanticsChecks({ HOME: '/h', PATH: '/p', EXACT_AGENT: '1' }).map(s => [s[0], s]));
  expect(byName.difftest[3].env).toEqual({ HOME: '/h', PATH: '/h/.elan/bin:/p', HERMES_LEAN_SYS_OFFLINE: '1', DIFFTEST_CACHE: '1' });
  expect(byName['difftest-js'][2][1]).toContain('--js-only');
  expect(byName.difftest[2][1]).not.toContain('--js');
  expect(outcomeOf(0, false)).toBe('pass');
  expect(outcomeOf(UNAVAILABLE, false)).toBe('unavailable');
  expect(outcomeOf(1, false)).toBe('fail');
  expect(outcomeOf(0, true)).toBe('fail');
});
