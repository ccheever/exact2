// What the M5 suites (linux.mjs, apple.mjs; LLP 1154 §3 item 4, §6 M5) share: the package built with its library
// (library.mjs), the fixture app depending on it, an app depending on a copy that declares a wrong hash, and the
// drive every host answers the same.
//
// The fixture app is an app `exact new` made outside the repo, its view replaced by `fixture/app.contract`,
// depending on the built package by `file:` and installed.
import { cpSync, existsSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { checker, run } from '../../tally/test/fixture.mjs';
import { ANSWER, buildPackage } from './library.mjs';

export { ANSWER, checker, run };
export const here = import.meta.dir, root = resolve(here, '../../../..');
export const LIBRARY_CACHE = resolve(homedir(), '.cache/exact/package-libraries');

/** The suite's directory: `given` (reused, its app built again) or a new temporary one. */
export const workDir = (given) => given ? resolve(given) : realpathSync(mkdtempSync(join(tmpdir(), 'exact-vault-')));

/** An app `exact new` made at `app`, its view the fixture's, depending on the package at `pkg`; made once. */
export function makeApp(app, pkg) {
  if (!existsSync(join(app, 'app.json'))) {
    run('bun', [resolve(root, 'scripts/exact.mjs'), 'new', app], root);
    for (const f of ['app.contract', 'app.ts']) cpSync(join(here, 'fixture', f), join(app, f));
    rmSync(join(app, 'app.test.contract'), { force: true });
    const id = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8')).app.id;
    writeFileSync(join(app, 'app.ts'), readFileSync(join(app, 'app.ts'), 'utf8').replace('__APP_ID__', id));
  }
  const manifest = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
  manifest.dependencies = { ...manifest.dependencies, '@fixture/vault': `file:${pkg}` };
  writeFileSync(join(app, 'package.json'), JSON.stringify(manifest, null, 2));
  run('bun', ['install'], app);
  return app;
}

/** The package and its library built under `dir`, the fixture app, and an app whose copy of the package declares
 * each library with a SHA-256 its bytes do not have (`wrong`, that copy's `native.library`). */
export function makeFixture(dir) {
  const { pkg, library } = buildPackage(join(dir, 'build'));
  const wrong = join(dir, 'wrong-hash');
  rmSync(wrong, { recursive: true, force: true });
  cpSync(pkg, wrong, { recursive: true });
  const declaration = JSON.parse(readFileSync(join(wrong, 'exact.json'), 'utf8'));
  for (const entry of Object.values(declaration.native.library)) entry.sha256 = entry.sha256.replace(/.$/, c => c === '0' ? '1' : '0');
  writeFileSync(join(wrong, 'exact.json'), JSON.stringify(declaration, null, 2));
  return { pkg, library, wrong: declaration.native.library, app: makeApp(join(dir, 'vaultfixture'), pkg), wrongApp: makeApp(join(dir, 'wrongfixture'), wrong) };
}

/** `cmd` in `cwd` that must fail: its stderr and stdout. */
export function refused(cmd, args, cwd, env = process.env) {
  try { run(cmd, args, cwd, env); return null; } catch (e) { return e.message; }
}

/** The refusal a build of `wrongApp` for `target` prints: the library's actual hash and the one `wrong` declares. */
export const wrongHash = (library, wrong, target) => `@fixture/vault: its ${target} library ${library[target].url} has SHA-256 ${library[target].sha256}, and its exact.json says ${wrong[target].sha256}; nothing was linked`;

/** Forget the fetched library: what is left to load is the copy the product carries. */
export const forget = (library) => { for (const { sha256 } of Object.values(library)) rmSync(join(LIBRARY_CACHE, sha256), { recursive: true, force: true }); };
/** Whether a build fetched any of `library` again. */
export const fetched = (library) => Object.values(library).some(({ sha256 }) => existsSync(join(LIBRARY_CACHE, sha256)));

/** The drive, and what each press answers, on every host. */
export const STEPS = [
  ['answer', 'the library answers, through the shim the bake joined and the factory the shell attached', t => t.startsWith(`${ANSWER} for `)],
  ['open-ungranted', 'an ungranted path is refused, naming the grant', t => t.includes('app:/data/other.sqlite: no grant covers it; grant `sqlite.open app:/data/other.sqlite`')],
  ['open', 'a granted path is opened by the library\'s SQLite, made new, and holds the row it wrote', t => t === 'opened vault.sqlite (new): 1 rows'],
  ['own-open', 'while the library holds it, the app\'s own open is refused', t => t.startsWith('refused: ') && t.includes('vault.sqlite is reserved for another SQLite library')],
  ['close', 'the library lets it go', t => t === 'closed'],
  ['own-open', 'after close the app\'s own open is admitted', t => t === 'own opened'],
  ['open', 'while the app holds it, the library\'s open is refused', t => t.includes('app:/data/vault.sqlite is open in the app\'s own storage')],
  ['own-close', 'the app lets it go', t => t === 'own closed'],
  ['open', 'then the library opens it again, kept, with both rows', t => t === 'opened vault.sqlite (kept): 2 rows'],
  ['close', 'and lets it go', t => t === 'closed'],
];

/** Drive `host` through STEPS from an empty store (`flags` before the ops), each answer checked. */
export function drive(check, host, app, flags = [], env = process.env) {
  const out = run('bun', ['exact.mjs', 'agent', host, '--reset-storage', '--json', ...flags, 'clock settle', ...STEPS.flatMap(([id]) => [`tap ${id}`, 'clock settle', 'tree'])], app, env)
    .trim().split('\n').map(l => JSON.parse(l));
  const last = (tree) => (tree?.nodes ?? []).find(n => n.props?.testId === 'last')?.props?.text ?? '';
  STEPS.forEach(([id, name, ok], i) => {
    const text = last(out[1 + i * 3 + 2]);
    check(host, `${id}: ${name}`, ok(text), text);
  });
}
