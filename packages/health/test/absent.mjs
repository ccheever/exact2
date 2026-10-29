// The targets with none of the three (LLP 1133.026 D21, §15 stage 5): the Health conformance fixture built for tvOS
// (an Apple TV simulator), macOS or Linux and driven under the agent. Each status reads `available` false; a Health
// read and request are refused `NotSupportedError`; nothing is asked.
//
// usage: bun packages/health/test/absent.mjs <tvos|macos|linux> [--sim <udid>] [--no-install]
//   (`tvos --no-install` builds and signs the Apple TV bundle and drives nothing, for a Mac with no tvOS runtime)
import { rmSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { checker, makeFixture, root, run, texts } from './fixture.mjs';

const argv = process.argv.slice(2);
const host = ['tvos', 'macos', 'linux'].find((h) => argv.includes(h));
if (!host) throw new Error('usage: bun packages/health/test/absent.mjs <tvos|macos|linux> [--sim <udid>]');
const sim = argv.includes('--sim') ? argv[argv.indexOf('--sim') + 1] : null;
const compileOnly = host === 'tvos' && argv.includes('--no-install');
const { check, failed } = checker();
const { app } = makeFixture(), name = app.split('/').at(-1);
const env = { ...process.env, EXACT_APP_DIR: app, EXACT_IDENTITY: process.env.EXACT_IDENTITY ?? '-' };
if (host === 'linux') run('bun', ['exact.mjs', 'linux'], app);
else run('bun', [resolve(root, 'host/apple/build.mjs'), ...(host === 'tvos' ? ['--tvos', ...(compileOnly ? ['--no-install'] : sim ? ['--sim', sim] : [])] : []), `${name}-apple`], root, env);
if (compileOnly) {
  rmSync(dirname(app), { recursive: true, force: true });
  console.log('the tvOS bundle built; nothing driven');
  process.exit(0);
}

process.env.EXACT_APP_DIR = app;
if (sim) process.env.EXACT_SIM = sim;
const { open } = await import(resolve(root, 'scripts/agent.mjs'));
const s = await open({ host, app: host === 'linux' ? app : name, storage: `absent-${Date.now().toString(36)}` });
try {
  await s.clock('settle');
  let t = texts(await s.tree());
  for (const id of ['health-status', 'steps-status', 'height-status']) check(host, `${id} reads absent`, t[id] === 'false unknown', t[id]);
  check(host, 'a Health read is refused', t.days === 'error NotSupportedError' && t['workouts-error'] === 'NotSupportedError', `${t.days} | ${t['workouts-error']}`);
  await s.tap('ask');
  await s.clock('settle');
  t = texts(await s.tree());
  const pending = (await s.state()).pending ?? [];
  check(host, 'the request asks nothing', !pending.length && t.asked === '', `${t.asked} ${JSON.stringify(pending)}`);
} finally {
  await s.close();
  rmSync(dirname(app), { recursive: true, force: true });
}
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
