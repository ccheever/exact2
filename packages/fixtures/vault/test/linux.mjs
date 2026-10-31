// @fixture/vault on the Linux host, headless (LLP 1154 §3 item 4, as far as
// the shim goes): the fixture app made outside the repo, its generated Linux
// crate built with the package's shim on the data module's native slot, and
// driven.
//
// - The bake wrote the factory into module.rs, and the generated shell
//   attached it: the shim's own answer is shown.
// - `open` of a path the app's grants do not cover is refused, naming the
//   grant that would admit it; of a granted one, it is admitted.
// - While the shim holds the file, the app's own `storage.sqlite.open` of
//   it is refused; after `close` it is admitted, and while the app holds it
//   the shim's `open` is refused.
//
// usage: bun packages/fixtures/vault/test/linux.mjs [<app dir>]
//   (an app already made by fixture.mjs is reused, and built again)
import { existsSync, readdirSync, readFileSync, realpathSync, rmSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { checker, makeFixture, run } from './fixture.mjs';

const given = process.argv[2] && resolve(process.argv[2]);
const app = given && existsSync(given) ? given : makeFixture(...(given ? [given] : []));
const { check, failed } = checker();
run('bun', ['exact.mjs', 'linux'], app);

// The bake's module.rs for the Linux crate, wherever Cargo put its OUT_DIR; the bake writes the shim's canonical path.
const modules = [];
const walk = (dir, depth) => {
  for (const entry of (() => { try { return readdirSync(dir, { withFileTypes: true }); } catch { return []; } })()) {
    const at = join(dir, entry.name);
    if (entry.isDirectory() && depth > 0) walk(at, depth - 1);
    else if (entry.name === 'module.rs' && at.includes('-linux-') && at.endsWith(join('out', 'module.rs'))) modules.push(at);
  }
};
walk(join(app, 'target'), 5);
const module = modules.map(file => readFileSync(file, 'utf8')).find(text => text.includes('exact_native')) ?? '';
check('bake', 'module.rs joins the shim and writes the factory', module.includes(`#[path = "${realpathSync(resolve(import.meta.dir, '../native/lib.rs'))}"]`) && module.includes('Box::new(exact_native_shim::Vault::new(APP))'), modules.join(', ') || 'no module.rs found');

const drive = (...args) => run('bun', ['exact.mjs', 'agent', 'linux', '--reset-storage', '--json', ...args], app).trim().split('\n').map(l => JSON.parse(l));
const last = (tree) => (tree.nodes ?? []).find(n => n.props?.testId === 'last')?.props?.text ?? '';
const steps = [
  ['ping', 'the shim answers, through the factory the shell attached', t => t.startsWith('vault 7f3a for ')],
  ['open-ungranted', 'an ungranted path is refused, naming the grant', t => t.includes('app:/data/other.sqlite: no grant covers it; grant `sqlite.open app:/data/other.sqlite`')],
  ['open', 'a granted path is admitted at its real path, made new', t => t === 'opened vault.sqlite (new)'],
  ['own-open', 'while the shim holds it, the app\'s own open is refused', t => t.startsWith('refused: ') && t.includes('vault.sqlite is reserved for another SQLite library')],
  ['close', 'the shim lets it go', t => t === 'closed'],
  ['own-open', 'after close the app\'s own open is admitted', t => t === 'own opened'],
  ['open', 'while the app holds it, the shim\'s open is refused', t => t.includes('app:/data/vault.sqlite is open in the app\'s own storage')],
  ['own-close', 'the app lets it go', t => t === 'own closed'],
  ['open', 'then the shim\'s open is admitted, and the file was there', t => t === 'opened vault.sqlite (kept)'],
];
const out = drive('clock settle', ...steps.flatMap(([id]) => [`tap ${id}`, 'clock settle', 'tree']));
steps.forEach(([id, name, ok], i) => {
  const text = last(out[1 + i * 3 + 2]);
  check('linux', `${id}: ${name}`, ok(text), text);
});
if (!given) rmSync(dirname(app), { recursive: true, force: true });
process.exit(failed() ? 1 : 0);
