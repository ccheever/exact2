// A wasm development build carries its plan's source map beside app.plan, as
// the JS target's does, so the agent driving that dist names each node's
// source (LLP 1012.001.000 D6): the bake's map, for exactly these plan bytes.
// The slow lane (EXACT_SLOW_TESTS=1): a wasm build compiles the app's crate.
import {describe, expect, test} from 'bun:test';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {existsSync, mkdtempSync, readFileSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {developmentBuildEnv} from '../../scripts/app.mjs';

const SLOW = process.env.EXACT_SLOW_TESTS === '1';
const root = resolve(import.meta.dir, '../..');

describe.skipIf(!SLOW)('a wasm development build', () => {
  test("writes its plan's source map, the digest of its app.plan", () => {
    const dir = mkdtempSync(resolve(tmpdir(), 'exact-wasm-plan-map-'));
    try {
      const dist = resolve(dir, 'dist'), app = resolve(root, 'apps/library-gate');
      const env = {...process.env, ...developmentBuildEnv(), EXACT_APP_DIR: app, EXACT_WEB_DIST: dist, EXACT_UPDATE_TRUST: 'development'};
      const built = spawnSync(process.execPath, ['host/web/build.mjs', 'library-gate', '--wasm'], {cwd: root, env, encoding: 'utf8', timeout: 1_800_000});
      expect(built.status, `${built.stdout}\n${built.stderr}`.slice(-4000)).toBe(0);
      expect(existsSync(resolve(dist, 'app.wasm'))).toBe(true);
      const plan = createHash('sha256').update(readFileSync(resolve(dist, 'app.plan'))).digest('hex');
      const map = JSON.parse(readFileSync(resolve(dist, 'app.plan.map.json'), 'utf8'));
      expect(map.digest).toBe(plan);
      expect(map.nodes.some(n => n?.component)).toBe(true);
    } finally {
      rmSync(dir, {recursive: true, force: true});
    }
  }, 1_900_000);
});
