// An Astryx theme's tokens, resolved (LLP 1121 D14, importer 4): the pinned
// theme package and its core resolved to light and dark values with
// `resolveThemeTokens()`, written as JSON for `contract tamber import astryx`.
// The packages are installed in a temporary directory, never in this repo.
//
//   bun scripts/astryx-tokens.mjs [--theme @astryxdesign/theme-neutral@0.7.0] [--out <file>]
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const argv = process.argv.slice(2);
const at = name => { const i = argv.indexOf(name); return i < 0 ? undefined : argv[i + 1]; };
const theme = at('--theme') ?? '@astryxdesign/theme-neutral@0.7.0';
const version = theme.slice(theme.lastIndexOf('@') + 1);
const name = theme.slice(0, theme.lastIndexOf('@'));
// Pinned: an exact version, never a range or a tag.
if (!/^\d+\.\d+\.\d+(-[\w.]+)?$/.test(version)) throw new Error(`--theme pins an exact version (name@1.2.3), not ${JSON.stringify(theme)}`);
const dir = mkdtempSync(join(tmpdir(), 'exact-astryx-'));
try {
  writeFileSync(join(dir, 'package.json'), JSON.stringify({ name: 'astryx-tokens', private: true }));
  // The theme first; its core at the exact version the theme asks for.
  const add = pkgs => {
    const r = spawnSync(process.execPath, ['add', '--ignore-scripts', ...pkgs], { cwd: dir, encoding: 'utf8' });
    if (r.status !== 0) throw new Error(`bun add: ${r.stderr || r.stdout}`);
  };
  add([theme]);
  const manifest = JSON.parse(readFileSync(join(dir, 'node_modules', name, 'package.json'), 'utf8'));
  const wanted = manifest.peerDependencies?.['@astryxdesign/core'] ?? manifest.dependencies?.['@astryxdesign/core'];
  if (!/^\d+\.\d+\.\d+(-[\w.]+)?$/.test(wanted ?? '')) throw new Error(`${theme} names no exact @astryxdesign/core (it asks for ${JSON.stringify(wanted)})`);
  add([`@astryxdesign/core@${wanted}`]);
  // The resolution runs where the packages are.
  writeFileSync(join(dir, 'resolve.mjs'), `
import { resolveThemeTokens } from '@astryxdesign/core/theme/tokens';
import * as pkg from ${JSON.stringify(name)};
// Exactly one exported theme: none would resolve core's defaults.
const themes = Object.values(pkg).filter(v => v && typeof v === 'object' && 'tokens' in v);
if (themes.length !== 1) throw new Error(\`${name} exports \${themes.length} themes; one is needed\`);
const [theme] = themes;
const out = { source: ${JSON.stringify(theme)}, core: '@astryxdesign/core@${wanted}', light: resolveThemeTokens(theme, { mode: 'light' }), dark: resolveThemeTokens(theme, { mode: 'dark' }) };
process.stdout.write(JSON.stringify(out, null, 2) + '\\n');
`);
  const resolved = spawnSync(process.execPath, ['resolve.mjs'], { cwd: dir, encoding: 'utf8', maxBuffer: 64 << 20 });
  if (resolved.status !== 0) throw new Error(`resolving: ${resolved.stderr}`);
  if (at('--out')) writeFileSync(at('--out'), resolved.stdout);
  else process.stdout.write(resolved.stdout);
} finally {
  rmSync(dir, { recursive: true, force: true });
}
