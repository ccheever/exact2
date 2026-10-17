// The token names a native module reads (LLP 1121 D12), written from the
// app's design before the module compiles: `contract tamber gen swift` or
// `kotlin` into the app's `.exact/tamber/` (never committed), so a view names
// `.swatchRing` or `TamberColor.SWATCH_RING`, a param of its own `component`
// (D15) among them. An app with no design has none.
import { spawnSync } from 'node:child_process';
import { existsSync, rmSync } from 'node:fs';
import { resolve } from 'node:path';

const root = resolve(new URL('..', import.meta.url).pathname);

/** Write `app`'s names for `language` (`swift`, `kotlin`); throws when the
 * compiler refuses the design (two names that map alike, named). */
export function writeTamberNames(app, language) {
  // No design, no names: one left from a design since removed is not compiled.
  const file = resolve(app.dir, '.exact/tamber', language === 'swift' ? 'tamber.swift' : 'TamberTokens.kt');
  if (!app.manifest?.tamber && !existsSync(resolve(app.dir, 'app.tamber'))) return rmSync(file, { force: true });
  const r = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--bin', 'contract', '--manifest-path', resolve(root, 'Cargo.toml'), '--', 'tamber', 'gen', language, app.dir], { cwd: root, stdio: ['ignore', 'ignore', 'inherit'] });
  if (r.status !== 0) throw new Error(`contract tamber gen ${language} ${app.dir} failed (${r.status ?? r.error?.message})`);
}
