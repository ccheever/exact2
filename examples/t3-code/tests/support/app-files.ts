// The app's files by name, for the tests that read sources (a component's actions, a view's attributes)
// or sweep them. Every file name under src/ and tests/ is unique, so a test names `composer.contract`
// and appPath finds src/composer/composer.contract; a name with a slash is a path from the app's root
// (`modules/apple/T3Composer.swift`).
import { readdirSync, statSync } from 'node:fs';
import { basename, join } from 'node:path';

export const APP = join(import.meta.dir, '..', '..');

let index: Map<string, string[]> | null = null;

function files(): Map<string, string[]> {
  if (index) return index;
  const found = new Map<string, string[]>();
  const add = (path: string) => found.set(basename(path), [...(found.get(basename(path)) ?? []), path]);
  for (const name of readdirSync(APP)) if (statSync(join(APP, name)).isFile()) add(join(APP, name));
  for (const dir of ['src', 'tests']) {
    for (const name of readdirSync(join(APP, dir), { recursive: true }) as string[]) {
      if (statSync(join(APP, dir, name)).isFile()) add(join(APP, dir, name));
    }
  }
  return index = found;
}

/** The absolute path of the app file `name`: a file name, or a path from the app's root. */
export function appPath(name: string): string {
  if (name.includes('/')) return join(APP, name);
  const paths = files().get(name) ?? [];
  if (paths.length !== 1) throw new Error(`${name}: ${paths.length ? `${paths.length} app files have this name` : 'no such app file'}`);
  return paths[0]!;
}

/** The names of the app's files: the root's, and every file under src/ and tests/. */
export function appFiles(): string[] {
  return [...files().keys()];
}
