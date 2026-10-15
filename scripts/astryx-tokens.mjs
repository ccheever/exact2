// An Astryx theme's tokens, resolved (LLP 1121 D14, importer 4): the pinned
// theme package and its core resolved to light and dark values with
// `resolveThemeTokens()`, written as JSON for `contract tamber import astryx`.
// The packages are installed in a temporary directory, never in this repo.
//
//   bun scripts/astryx-tokens.mjs [--theme @astryxdesign/theme-neutral@0.7.0] [--out <file>]
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const argv = process.argv.slice(2);
const at = name => { const i = argv.indexOf(name); return i < 0 ? undefined : argv[i + 1]; };
const theme = at('--theme') ?? '@astryxdesign/theme-neutral@0.7.0';
const version = theme.slice(theme.lastIndexOf('@') + 1);
const name = theme.slice(0, theme.lastIndexOf('@'));
const dir = mkdtempSync(join(tmpdir(), 'exact-astryx-'));
try {
  writeFileSync(join(dir, 'package.json'), JSON.stringify({ name: 'astryx-tokens', private: true }));
  const added = spawnSync(process.execPath, ['add', '--ignore-scripts', theme, `@astryxdesign/core@${version}`], { cwd: dir, encoding: 'utf8' });
  if (added.status !== 0) throw new Error(`bun add: ${added.stderr || added.stdout}`);
  // The resolution runs where the packages are.
  writeFileSync(join(dir, 'resolve.mjs'), `
import { resolveThemeTokens } from '@astryxdesign/core/theme/tokens';
import * as pkg from ${JSON.stringify(name)};
const theme = Object.values(pkg).find(v => v && typeof v === 'object' && 'tokens' in v) ?? pkg.default;
const out = { source: ${JSON.stringify(theme)}, light: resolveThemeTokens(theme, { mode: 'light' }), dark: resolveThemeTokens(theme, { mode: 'dark' }) };
process.stdout.write(JSON.stringify(out, null, 2) + '\\n');
`);
  const resolved = spawnSync(process.execPath, ['resolve.mjs'], { cwd: dir, encoding: 'utf8', maxBuffer: 64 << 20 });
  if (resolved.status !== 0) throw new Error(`resolving: ${resolved.stderr}`);
  if (at('--out')) writeFileSync(at('--out'), resolved.stdout);
  else process.stdout.write(resolved.stdout);
} finally {
  rmSync(dir, { recursive: true, force: true });
}
