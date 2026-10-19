// scripts/help-parity.mjs, which caps.mjs runs before every push: a flag a script parses and its help does not
// name, and a help row naming a flag no script parses, are each found, in a copy of the scripts it reads.
import { expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { helpParity } from './help-parity.mjs';

const ROOT = resolve(import.meta.dir, '..');
const READ = ['scripts/exact.mjs', 'scripts/app-cli.mjs', 'scripts/tamber-pairing.mjs', 'host/apple/build.mjs', 'host/apple/devices.mjs', 'host/apple/link.mjs', 'host/apple/bundle.mjs', 'host/web/dev.mjs', 'scripts/install-page.mjs', 'scripts/tamber-remote.mjs', 'host/web/build.mjs', 'scripts/agent-launch.mjs', 'scripts/agent-widget.mjs', 'scripts/verify.mjs', 'scripts/build-linux.mjs', 'scripts/agent-android.mjs', 'scripts/feedback.mjs'];

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
  const caps = spawnSync(process.execPath, [resolve(ROOT, 'scripts/caps.mjs')], { cwd: ROOT, encoding: 'utf8' });
  expect(caps.stdout).toContain("helps: each verb's flags against its script's, 0 disagreeing");
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
});
