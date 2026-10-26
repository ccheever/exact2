// A build's swap leaves nothing for `git add -A` to take (serve.mjs replaceBuild, host/web/generations.mjs): beside
// every web dist that sits in this tree, the kept builds by generation, their leases, the reaped list, the lock, a
// stage and `.previous` are git-ignored; and two builds swapped in a sample app's dist leave its `git status` clean.
import { expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { replaceBuild } from '../host/web/serve.mjs';
import { generationOf, leaseGeneration } from '../host/web/generations.mjs';
import { newerThan } from './agent-launch.mjs';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
const SIBLINGS = ['.gen/0123456789abcdef0123456789abcdef/app.js', '.leases/0123456789abcdef0123456789abcdef/1-1', '.gen.reaped', '.lock', '.previous/app.js', '.stage-abc123/app.js'];
// Every web dist that sits in this checkout (an outside app's is its own target/web-dist).
const DISTS = ['host/web/dist', 'apps/harmony/app/dist', 'game/games/forest/dist', 'game/games/garden/dist', 'game/games/rivals/dist', 'game/games/beacons/dist', 'apps/caltrain/target/web-dist'];

test('beside every in-tree web dist, the swap\'s siblings are git-ignored', () => {
  const paths = DISTS.flatMap((d) => SIBLINGS.map((s) => d + s));
  const r = spawnSync('git', ['check-ignore', '--no-index', '--stdin'], { cwd: ROOT, input: paths.join('\n'), encoding: 'utf8' });
  const ignored = new Set(r.stdout.split('\n').filter(Boolean));
  expect(paths.filter((p) => !ignored.has(p))).toEqual([]);
});

test('two builds swapped in a sample app\'s dist, one leased, leave its git status clean', () => {
  const root = mkdtempSync(join(tmpdir(), 'exact-dist-siblings-')), app = join(root, 'app');
  try {
    const git = (...args) => spawnSync('git', ['-c', 'user.email=t@example.test', '-c', 'user.name=t', ...args], { cwd: app, encoding: 'utf8' });
    mkdirSync(app);
    copyFileSync(resolve(ROOT, 'apps/harmony/app/.gitignore'), join(app, '.gitignore')); // an app whose dist sits beside its sources
    writeFileSync(join(app, 'app.contract'), 'component App\n  view\n    text "x"\n');
    git('init', '-q'); git('add', '-A'); git('commit', '-q', '-m', 'app');
    const build = (k) => { const s = join(root, `stage-${k}`); mkdirSync(s); writeFileSync(join(s, 'app.js'), `build ${k}`);
      writeFileSync(join(s, '.exact-build.json'), JSON.stringify({ exactBuild: 1, target: 'js', files: [{ name: 'app.js', sha256: String(k) }] })); return s; };
    const dist = join(app, 'dist');
    replaceBuild(build(1), dist);
    const release = leaseGeneration(dist, generationOf(dist));
    replaceBuild(build(2), dist);
    writeFileSync(`${dist}.gen.reaped`, '0123456789abcdef0123456789abcdef\n');
    expect(git('status', '--porcelain').stdout).toBe('');
    release();
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('a dist\'s swap siblings are no build input: a later build\'s reap leaves a drive\'s build fresh', () => {
  const root = mkdtempSync(join(tmpdir(), 'exact-dist-inputs-'));
  try {
    for (const name of ['dist.gen.reaped', 'dist.lock', 'dist.stage-abc123', 'web-dist.gen.reaped', 'serve.mjs', 'dist.ts', 'web-dist.json']) writeFileSync(join(root, name), 'x');
    // A source that only starts with the word is an input.
    expect(newerThan(0, [{ shallow: root }]).map((p) => p.slice(p.lastIndexOf('/') + 1)).sort()).toEqual(['dist.ts', 'serve.mjs', 'web-dist.json']);
  } finally { rmSync(root, { recursive: true, force: true }); }
});
