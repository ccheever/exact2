// The conformance fixture (LLP 1133.001 D7, LLP 1133.006 §9 stage 1 item 7):
// an app `exact new` made outside the repo, its files replaced by
// `fixture/`'s, depending on this package by `file:` and installed.
import { cpSync, existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

export const here = import.meta.dir, pkg = resolve(here, '..'), root = resolve(pkg, '../..');

export const run = (cmd, args, cwd, env = process.env) => {
  const r = spawnSync(cmd, args, { cwd, env, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], maxBuffer: 64 << 20 });
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(' ')}: ${r.stdout}\n${r.stderr}`);
  return r.stdout;
};

/** A fixture `EXACT_CLIPBOARD_FIXTURE` names is refreshed and kept, so its native build is paid once while iterating. */
const kept = process.env.EXACT_CLIPBOARD_FIXTURE;

/** The fixture app's directory and its bundle id. */
export function makeFixture() {
  const reuse = kept && existsSync(join(kept, 'app.json'));
  const app = reuse ? resolve(kept) : kept ? resolve(kept) : join(mkdtempSync(join(tmpdir(), 'exact-clipboard-')), 'clipfixture');
  if (!reuse) run('bun', [resolve(root, 'scripts/exact.mjs'), 'new', app], root);
  for (const f of ['app.contract', 'app.ts', 'strings']) cpSync(join(here, 'fixture', f), join(app, f), { recursive: true });
  rmSync(join(app, 'app.test.contract'), { force: true });
  const id = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8')).app.id;
  writeFileSync(join(app, 'app.ts'), readFileSync(join(app, 'app.ts'), 'utf8').replace('__APP_ID__', id));
  const manifest = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
  manifest.dependencies = { ...manifest.dependencies, '@exact/clipboard': `file:${pkg}` };
  writeFileSync(join(app, 'package.json'), JSON.stringify(manifest, null, 2));
  run('bun', ['install'], app);
  return { app, id };
}

/** Remove the fixture, unless it is the one `EXACT_CLIPBOARD_FIXTURE` keeps. */
export const discard = app => { if (resolve(app) !== resolve(kept ?? '')) rmSync(dirname(app), { recursive: true, force: true }); };

/** The text a `tree` shows for each node of `testId`, in order. */
export const shown = (out, testId) => [...out.matchAll(new RegExp(`\\[${testId}\\] "((?:[^"\\\\]|\\\\.)*)"`, 'g'))].map(m => m[1]);

/** Each button the fixture has, what it answers, and what the write leaves in `lastWrite` (null: nothing new). */
export const KINDS = [
  ['png', 'ok png', { types: ['image/png'], image: { width: 8, height: 8 } }],
  ['png-bytes', 'ok png-bytes', { types: ['image/png'], image: { width: 8, height: 8 } }],
  ['apng', 'ok apng', { types: ['image/png'], image: { width: 8, height: 8 } }],
  ['text', 'ok text', { types: ['text/plain'], text: 'Copied from the fixture' }],
  ['both', 'ok both', { types: ['text/plain', 'image/png'], text: 'https://example.com/red.png', image: { width: 8, height: 8 } }],
  ['bad', 'DataError: clipboard.write: DataError: not a PNG', null],
  ['large', 'DataError: clipboard.write: DataError: too large: 9000×9000 is over 64,000,000 pixels', null],
  ['big-bytes', 'DataError: clipboard.write: image/png is 2097153 bytes, over 2097152: pass an app:/ path', null],
  ['race', 'AbortError: clipboard.write: a later call superseded it | B ok', { types: ['text/plain'], text: 'B' }],
  ['overflow', 'DataError: clipboard.write: DataError: not a PNG', null],
  ['long-text', 'DataError: clipboard.write: text/plain is 1048577 bytes, over 1048576', null],
  ['refused-first', 'A ok | DataError: clipboard.write: image/png is 2097153 bytes, over 2097152: pass an app:/ path', { types: ['text/plain'], text: 'A' }],
  ['empty', 'ok empty', null],
  ['two', 'NotAllowedError: clipboard.write: NotAllowedError: Support for multiple ClipboardItems is not implemented.', null],
  ['jpeg', 'NotAllowedError: clipboard.write: NotAllowedError: Type image/jpeg not supported on write.', null],
  ['nested', 'TypeError: ClipboardItem: an empty dictionary', null],
];

/** A suite's PASS or FAIL line; `failed()` counts the FAILs. */
export function checker() {
  let failures = 0;
  const check = (where, name, ok, detail) => {
    if (!ok) failures++;
    console.log(`${ok ? 'PASS' : 'FAIL'} ${where} ${name}${ok ? '' : `: ${detail}`}`);
  };
  return { check, failed: () => failures };
}
