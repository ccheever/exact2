// The conformance fixture (LLP 1133.001 D7), as every platform's suite makes
// it: an app `exact new` made outside the repo, its files replaced by
// `fixture/`'s, depending on this package by `file:` and installed.
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

export const here = import.meta.dir, pkg = resolve(here, '..'), root = resolve(pkg, '../..');

export const run = (cmd, args, cwd, env = process.env) => {
  const r = spawnSync(cmd, args, { cwd, env, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(' ')}: ${r.stdout}\n${r.stderr}`);
  return r.stdout;
};

/** The fixture app's directory and its bundle id; `model` is the package's `package.json` spec, `dependencies` the app's others. */
export function makeFixture({ model = `file:${pkg}`, dependencies = {} } = {}) {
  const dir = mkdtempSync(join(tmpdir(), 'exact-language-model-')), app = join(dir, 'languagemodelfixture');
  run('bun', [resolve(root, 'scripts/exact.mjs'), 'new', app], root);
  for (const f of ['app.contract', 'app.ts', 'strings']) cpSync(join(here, 'fixture', f), join(app, f), { recursive: true });
  rmSync(join(app, 'app.test.contract'), { force: true });
  const id = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8')).app.id;
  writeFileSync(join(app, 'app.ts'), readFileSync(join(app, 'app.ts'), 'utf8').replace('__APP_ID__', id));
  const manifest = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
  manifest.dependencies = { ...manifest.dependencies, '@exact/language-model': model, ...dependencies };
  writeFileSync(join(app, 'package.json'), JSON.stringify(manifest, null, 2));
  run('bun', ['install'], app);
  return { app, id };
}

/** The fixture's drives under the agent, the same on every host (§10's drives 7 and 8 on the device's runtime): each
 * drive's name, operations and what its output must show. */
export function agentDrives() {
  const script = v => `prefer extension language-model.responses=${JSON.stringify(v)}`;
  return [
    ['a constrained prompt answers the shape it names; a tool runs the app\'s handler',
      [script([{ value: { title: 'Rust lifetimes' } }, { toolCalls: [{ name: 'findNotes', arguments: { query: 'lifetimes' } }], text: 'You noted it.' }]),
        'tap create', 'clock data', 'tap make-title', 'clock data', 'tree title', 'tap stream', 'clock data', 'clock +1000', 'clock data', 'tree', 'logs'],
      ['[title] "Rust lifetimes"', '[tool] "findNotes done [{\\"title\\":\\"Borrowing\\"', '[reply] "You noted it."', 'findNotes for response-', 'tool:model:response-']],
    ['drive 8: a handler prompting its own session fails at once, and one that would download is refused',
      ['tap prepare', 'clock data', 'prefer extension language-model.languageModelAvailability={"availability":"downloadable"}',
        script([{ toolCalls: [{ name: 'promptOwn', arguments: {} }, { name: 'startDownload', arguments: {} }], text: 'done' }]),
        'tap reenter', 'clock data', 'tree reentered', 'tree availability', 'logs'],
      ['[reentered] "promptOwn InvalidStateError; startDownload NotAllowedError"', '[availability] "downloadable', 'person-required (tool origin)']],
    ['drive 8: a prompt whose mutation is sent again is cancelled at the implementation',
      [script([{ text: 'A held reply', hold: true }]), 'tap create', 'clock data', 'tap stream', 'clock data', 'tap make-title', 'tap make-title', 'clock data', 'logs'],
      ['cancel language-model.prompt request', 'cancelled: its call (request']],
    ['data code\'s constraint is a schema object, its answer JSON text', ['tap ask', 'clock data', 'tree asked'], ['[asked] "{\\"title\\":\\"\\"}"']],
  ];
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
