// Harmony bakes for production (an App Store archive forces it, host/apple/build.mjs) without its test call:
// app.ts marks the test call's grants a development build's only (`development <grant>`), so a production bake
// carries no `device.calls` (so no `voip` or `audio` background mode, LLP 1133.033 §13 question 4) and no microphone,
// and the You tab's test-call row, shown only where the build holds the grant (`testCallBuild`), is gone. A
// development bake keeps both.
import { expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { parseSync } from 'rolldown/utils';
import { developmentGrants, staticFacts } from '../js/bake/src/typescript.mjs';
import { bakeOutput, resolveApp } from './app.mjs';

const ROOT = resolve(import.meta.dir, '..');
const APP = resolve(ROOT, 'apps/harmony/app');
const TEST_CALL = ['device.microphone purpose.microphone', 'device.calls purpose.calls', 'fs.read app:/cache/test-call', 'fs.write app:/cache/test-call'];

test("the build's copy of app.ts loses the test call's lines in production, and only the word otherwise", () => {
  const authored = readFileSync(resolve(APP, 'app.ts'), 'utf8');
  const declared = staticFacts(authored, parseSync).grants.split('\n');
  expect(declared.filter(line => line.startsWith('development ')).map(line => line.slice('development '.length))).toEqual(TEST_CALL);
  const copy = trust => developmentGrants(authored, parseSync, { EXACT_UPDATE_TRUST: trust });
  const grants = code => staticFacts(code, parseSync).grants.split('\n');
  expect(grants(copy('production'))).toEqual(declared.filter(line => !line.startsWith('development ')));
  expect(grants(copy('development'))).toEqual(declared.map(line => line.replace(/^development /, '')));
  // Nothing but the grants constant's value changes.
  const { start, end } = parseSync('app.ts', authored).program.body.map(s => s.declaration ?? s)
    .flatMap(s => s.declarations ?? []).find(d => d.id.name === 'grants').init;
  for (const trust of ['production', 'development']) {
    const code = copy(trust), tail = authored.length - end;
    expect([code.slice(0, start), code.slice(code.length - tail)]).toEqual([authored.slice(0, start), authored.slice(end)]);
  }
});

/** Harmony's web bake under `trust`: its grant ceiling, from the build's copy of app.ts (the grants the module reads
 * at run time too), and its compatibility id. */
function bake(trust) {
  const env = { ...process.env, EXACT_APP_DIR: APP, EXACT_UPDATE_TRUST: trust };
  const baked = spawnSync(process.execPath, [resolve(ROOT, 'host/web/build.mjs'), 'app-web', '--bake'], { cwd: ROOT, env, encoding: 'utf8' });
  expect({ trust, status: baked.status, error: baked.status ? (baked.stdout + baked.stderr).slice(-3000) : '' }).toEqual({ trust, status: 0, error: '' });
  const receipt = JSON.parse(readFileSync(resolve(bakeOutput(resolveApp(APP), env), 'web-wasm32-unknown-unknown.json'), 'utf8'));
  return { ceiling: receipt.inputs.grantCeiling.split('\n'), id: receipt.id };
}

test('a production bake of Harmony succeeds without the test call, and a development bake keeps it', () => {
  const production = bake('production'), development = bake('development');
  expect(production.ceiling.filter(line => TEST_CALL.includes(line))).toEqual([]);
  expect(development.ceiling.filter(line => TEST_CALL.includes(line))).toEqual(TEST_CALL);
  expect(development.ceiling.filter(line => !TEST_CALL.includes(line))).toEqual(production.ceiling);
  expect(production.id).not.toBe(development.id);
}, 1_800_000);

// Harmony's tests, on a production-trust build against a server of their own: every test passes but the one that
// asks for the test call, which finds the build doesn't hold the grant and the row isn't there.
test('a production build of Harmony shows no test call under You', () => {
  const run = spawnSync(process.execPath, [resolve(ROOT, 'scripts/memory-backend-test.mjs'), APP], {
    cwd: ROOT, env: { ...process.env, EXACT_UPDATE_TRUST: 'production' }, encoding: 'utf8' });
  const out = run.stdout + run.stderr, results = [...out.matchAll(/^test "(.*)": (ok|FAIL)$/gm)].map(([, name, result]) => [name, result]);
  const asks = 'a development build offers the test call under You';
  // Every test in the file ran, and only that one failed (the run's status says so too).
  const names = [...readFileSync(resolve(APP, 'app.test.contract'), 'utf8').matchAll(/^test "(.*)"$/gm)].map(([, name]) => name);
  expect(results).toEqual(names.map(name => [name, name === asks ? 'FAIL' : 'ok']));
  expect({ status: run.status, signal: run.signal }).toEqual({ status: 1, signal: null });
  expect(out).toContain(`${asks}: line `);
  expect(out).toMatch(/testCallBuild\.available is false, expected true/);
  expect(out).toMatch(/expected testId "test-call" present, it was absent/);
}, 1_800_000);
