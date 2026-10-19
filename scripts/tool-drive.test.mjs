// A drive or verify of the design tool named straight to scripts/agent.mjs or
// scripts/verify.mjs (tool-drive.mjs): the checkout's own port, the stage's
// directory and dist, each --test file the stage's; any other app, or
// EXACT_TOOL_PORT=7740, as it is.
import { expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';
import { checkoutPort } from './port-lock.mjs';
import { appServer } from './verify.mjs';

const ROOT = resolve(new URL('..', import.meta.url).pathname);

/** toolDrive(verb, argv) in a fresh process under `env` (EXACT_TOOL_PORT and
 * EXACT_APP_DIR unset unless it names them): its argv and the variables it set. */
function drive(verb, argv, env = {}) {
  const base = { ...process.env };
  for (const k of ['EXACT_TOOL_PORT', 'EXACT_APP_DIR', 'EXACT_WEB_DIST']) delete base[k];
  const r = spawnSync(process.execPath, ['-e', `const { toolDrive } = await import(${JSON.stringify(resolve(ROOT, 'scripts/tool-drive.mjs'))}); const args = await toolDrive(${JSON.stringify(verb)}, ${JSON.stringify(argv)}); const e = process.env; console.log(JSON.stringify({ args, port: e.EXACT_TOOL_PORT ?? null, dir: e.EXACT_APP_DIR ?? null, dist: e.EXACT_WEB_DIST ?? null }))`], { cwd: ROOT, env: { ...base, ...env }, encoding: 'utf8' });
  if (r.status !== 0) throw new Error(r.stderr);
  return JSON.parse(r.stdout.trim().split('\n').at(-1));
}

test('a drive or verify naming the design tool runs on the checkout\'s own port, from its stage', () => {
  const port = checkoutPort(ROOT), stage = resolve(ROOT, `target/design-tool-test-${port}`);
  const agent = drive('agent', ['web', '--app', 'design-tool', '--test', 'apps/design-tool/app.test.contract', 'tree']);
  expect(agent).toEqual({ args: ['web', '--app', 'design-tool', '--test', resolve(stage, 'app.test.contract'), 'tree'], port: String(port), dir: stage, dist: resolve(ROOT, `target/design-tool-test-web-${port}`) });
  // By its path, and under verify.
  // (The path given way to the tool's name, which the stage answers: resolveApp takes a path over EXACT_APP_DIR.)
  const verify = drive('verify', ['web', '--app', 'apps/design-tool']);
  expect([verify.args, verify.port, verify.dir]).toEqual([['web', '--app', 'design-tool'], String(port), stage]);
  // A path names the tool over another app's EXACT_APP_DIR, as resolveApp reads them.
  expect(drive('agent', ['web', '--app', './apps/design-tool'], { EXACT_APP_DIR: resolve(ROOT, 'apps/caltrain') }).dir).toBe(stage);
  // The stage names its stub's server at that port, not 7740 (verify.mjs appServer reads app.ts's origin).
  expect(appServer(stage).port).toBe(port);
  // A port named is that port's stage.
  expect(drive('agent', ['web', '--app', 'design-tool'], { EXACT_TOOL_PORT: '17997' }).dir).toBe(resolve(ROOT, 'target/design-tool-test-17997'));
});

test('another app, a drive already in a stage, and EXACT_TOOL_PORT=7740 are left as they are', () => {
  const none = { port: null, dir: null, dist: null };
  expect(drive('agent', ['web', '--app', 'caltrain', 'tree'])).toEqual({ args: ['web', '--app', 'caltrain', 'tree'], ...none });
  expect(drive('agent', ['web', 'tree'])).toEqual({ args: ['web', 'tree'], ...none });
  // Through its exact.mjs (app-cli.mjs), EXACT_APP_DIR is already the stage.
  const staged = resolve(ROOT, 'target/design-tool-test-17996');
  expect(drive('agent', ['web', '--app', 'design-tool'], { EXACT_APP_DIR: staged, EXACT_TOOL_PORT: '17996' })).toEqual({ args: ['web', '--app', 'design-tool'], port: '17996', dir: staged, dist: null });
  // Help stages nothing.
  expect(drive('agent', ['web', '--app', 'design-tool', '--help'])).toEqual({ args: ['web', '--app', 'design-tool', '--help'], ...none });
  expect(drive('verify', ['web', '--app', 'design-tool'], { EXACT_TOOL_PORT: '7740' })).toEqual({ args: ['web', '--app', 'design-tool'], port: '7740', dir: null, dist: null });
});

test('agent.mjs and verify.mjs named the tool by its path or name stage it: what answers at its stub\'s port is that port\'s, never 7740', async () => {
  // Something already answering at the stage's port: each refuses before any
  // build, naming the URL it would have started the stub at.
  const port = 17994, held = Bun.serve({ port, hostname: '127.0.0.1', fetch: () => new Response('held') });
  try {
    const env = { ...process.env, EXACT_TOOL_PORT: String(port) };
    for (const k of ['EXACT_APP_DIR', 'EXACT_WEB_DIST']) delete env[k];
    const run = (args) => { const r = spawnSync(process.execPath, args, { cwd: ROOT, env, encoding: 'utf8', timeout: 120_000 }); return `${r.stdout}${r.stderr}`; };
    const said = [
      run(['scripts/verify.mjs', 'web', '--app', 'apps/design-tool']),
      run(['scripts/verify.mjs', 'web', '--app', 'design-tool']),
      run(['scripts/agent.mjs', 'web', '--app', './apps/design-tool', '--storage', 'tool-drive-test', '--reset-storage', 'tree']),
    ];
    for (const s of said) {
      expect(s).toContain(`127.0.0.1:${port}`);
      expect(s).not.toContain('127.0.0.1:7740');
    }
  } finally { held.stop(true); }
}, 300_000);
