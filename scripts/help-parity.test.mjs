// scripts/help-parity.mjs, which caps.mjs runs before every push: a flag a script parses and its help does not
// name, and a help row naming a flag no script parses, are each found, in a copy of the scripts it reads.
import { expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { helpParity } from './help-parity.mjs';

const ROOT = resolve(import.meta.dir, '..');
const READ = ['scripts/exact.mjs', 'scripts/app-cli.mjs', 'scripts/tamber-pairing.mjs', 'host/apple/build.mjs', 'host/apple/devices.mjs', 'host/apple/link.mjs', 'host/apple/bundle.mjs', 'host/web/dev.mjs', 'scripts/install-page.mjs', 'scripts/tamber-remote.mjs', 'scripts/tamber-relay.mjs', 'host/web/build.mjs', 'scripts/agent-launch.mjs', 'scripts/agent-widget.mjs', 'scripts/verify.mjs', 'scripts/build-linux.mjs', 'scripts/agent-android.mjs', 'scripts/feedback.mjs'];

/** helpParity over a copy of the scripts it reads, `edit` applied to the copy first. */
function over(edit) {
  const dir = mkdtempSync(join(tmpdir(), 'help-parity-'));
  try {
    for (const f of READ) cpSync(resolve(ROOT, f), join(dir, f), { recursive: true });
    edit((f, change) => writeFileSync(join(dir, f), change(readFileSync(join(dir, f), 'utf8'))));
    return helpParity(dir);
  } finally { rmSync(dir, { recursive: true, force: true }); }
}

test('the helps agree with their scripts here, and caps says it checked them', () => {
  expect(helpParity(ROOT)).toEqual([]);
  const caps = spawnSync(process.execPath, [resolve(ROOT, 'scripts/caps.mjs')], { cwd: ROOT, encoding: 'utf8', env: { ...process.env, EXACT_CAPS_WASM: '0' } }); // the wasm32 check is wasm-check.test's
  expect(caps.status).toBe(0);
  expect(caps.stdout).toContain("helps: each verb's flags against its script's, its rows and lengths, 0 problems");
});

test('a flag with no help row, and a row whose flag no script parses, are each said', () => {
  const unsaid = over((edit) => edit('scripts/verify.mjs', (s) => `${s}\nconst frob = args.includes('--frobnicate');\n`));
  expect(unsaid.map((p) => p.code)).toEqual(['help:unsaid']);
  expect(unsaid[0].message).toBe('verify parses --frobnicate, which its help does not name.');
  // agent's and test's --bind read gone (as when a fix on another row hid one): both helps now name a flag nothing parses.
  const stale = over((edit) => {
    for (const f of ['scripts/agent-launch.mjs', 'scripts/agent-widget.mjs']) edit(f, (s) => s.replaceAll("'--bind'", "'--bound'"));
  });
  expect(stale.filter((p) => p.code === 'help:stale').map((p) => p.message)).toEqual([
    "agent's help names --bind, which its script does not parse.", "test's help names --bind, which its script does not parse.",
  ]);
  // A slice it reads gone is said, not taken for agreement.
  expect(over((edit) => edit('scripts/tamber-pairing.mjs', (s) => s.replace('export function pairTtlOf(', 'function ttl(')))[0].code).toBe('help:sources');
  // So is a verb's slice of exact.mjs gone (docs parses no flags: an empty slice would agree with its help).
  const moved = over((edit) => edit('scripts/exact.mjs', (s) => s.replace('export function docs(', 'export function guides(')));
  expect(moved.map((p) => p.code)).toEqual(['help:sources']);
  expect(moved[0].message).toContain('export function docs(');
});

const RULES = `# Rules
- **6 blocking checks, 65s total.** **[check]**
- **15 documents in the working set.** **[check]**
- **10 documents in the foundation.** **[check]**
- **This file: 700 words.** **[check]**
- **1,500 lines per source file.** **[check]**
## Time budgets
| | |
|---|---|
| Cold start to interactive | 100ms |
## The six checks
\`build\` · \`test\` · \`lint\` · \`caps\` · \`boot\` · \`workspace\`
`;

/** caps.mjs's exit and output in a throwaway repository whose scripts/help-parity.mjs is `parity`. */
function caps(parity) {
  const dir = mkdtempSync(join(tmpdir(), 'help-caps-'));
  try {
    for (const [f, body] of [['rules/RULES.md', RULES], ...(parity === null ? [] : [['scripts/help-parity.mjs', parity]])]) { mkdirSync(join(dir, f, '..'), { recursive: true }); writeFileSync(join(dir, f), body); }
    spawnSync('git', ['init', '-q', '-b', 'main'], { cwd: dir });
    const r = spawnSync(process.execPath, [resolve(ROOT, 'scripts/caps.mjs')], { cwd: dir, encoding: 'utf8' });
    return { code: r.status, out: `${r.stdout}${r.stderr}` };
  } finally { rmSync(dir, { recursive: true, force: true }); }
}

test('caps fails on every problem the helps have, and fails closed when their check cannot run', () => {
  const two = caps("export const helpParity = () => [{ code: 'help:unsaid', message: 'verify parses --frobnicate, which its help does not name.', detail: 'Add a row.' }, { code: 'help:long', message: 'web --help is 1900 bytes, over 1800.' }];\n");
  expect(two.code).toBe(1);
  expect(two.out).toContain('verify parses --frobnicate, which its help does not name.');
  expect(two.out).toContain('web --help is 1900 bytes, over 1800.');
  expect(two.out).toContain('2 violations.');
  const broken = caps("export const helpParity = () => { throw new Error('no HELP'); };\n");
  expect(broken.code).toBe(1);
  expect(broken.out).toContain('help:parity');
  expect(broken.out).toContain('no HELP');
  // A checkout without the file (another repository caps runs in) checks no helps, and passes.
  expect(caps(null).out).not.toContain('helps:');
});
