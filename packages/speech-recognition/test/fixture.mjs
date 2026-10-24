// The conformance fixture (LLP 1133.001 D7), as every platform's suite makes
// it: an app `exact new` made outside the repo, its files replaced by
// `fixture/`'s, depending on this package and @exact/media-capture by `file:`
// and installed; and the known spoken file the suites recognise.
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

export const here = import.meta.dir, pkg = resolve(here, '..'), root = resolve(pkg, '../..');
export const capture = resolve(root, 'packages/media-capture');

export const run = (cmd, args, cwd, env = process.env) => {
  const r = spawnSync(cmd, args, { cwd, env, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(' ')}: ${r.stdout}\n${r.stderr}`);
  return r.stdout;
};

/** What the spoken file says. */
export const PHRASE = 'pick up milk and eggs';

/** The spoken file: macOS's `say` speaking PHRASE as 16-bit 16 kHz mono WAV, a second of silence after it, made
 * once into the cache (a recognizer finalises a phrase after silence). */
export function spokenFile() {
  const dir = join(homedir(), '.cache/exact/speech'), file = join(dir, 'pick-up-milk.wav');
  if (existsSync(file)) return file;
  mkdirSync(dir, { recursive: true });
  run('say', ['-o', file, '--data-format=LEI16@16000', `${PHRASE}. [[slnc 1500]]`], dir);
  return file;
}

/** The fixture app's directory and its bundle id. `recognition` and `mediaCapture` are the two dependencies
 * (`file:` the packages here by default). */
export function makeFixture({ recognition = `file:${pkg}`, mediaCapture = `file:${capture}` } = {}) {
  const dir = mkdtempSync(join(tmpdir(), 'exact-speech-recognition-')), app = join(dir, 'recogfixture');
  run('bun', [resolve(root, 'scripts/exact.mjs'), 'new', app], root);
  for (const f of ['app.contract', 'app.ts', 'strings']) cpSync(join(here, 'fixture', f), join(app, f), { recursive: true });
  rmSync(join(app, 'app.test.contract'), { force: true });
  const id = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8')).app.id;
  writeFileSync(join(app, 'app.ts'), readFileSync(join(app, 'app.ts'), 'utf8').replace('__APP_ID__', id));
  const manifest = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
  manifest.dependencies = { ...manifest.dependencies, '@exact/media-capture': mediaCapture, '@exact/speech-recognition': recognition };
  writeFileSync(join(app, 'package.json'), JSON.stringify(manifest, null, 2));
  run('bun', ['install'], app);
  return { app, id };
}

/** A suite's PASS or FAIL line; `failed()` counts the FAILs. */
export function checker() {
  let failures = 0;
  const check = (where, name, ok, detail) => {
    if (!ok) failures++;
    console.log(`${ok ? 'PASS' : 'FAIL'} ${where} ${name}${ok ? '' : `: ${detail}`}`);
  };
  return { check, failed: () => failures };
}
