// The app's files by name, for the tests that read sources (a component's actions, a view's attributes)
// or sweep them. Every file name under src/ and tests/ is unique, so a test names `composer.contract`
// and appPath finds src/composer/composer.contract; a name with a slash is a path from the app's root
// (`modules/apple/T3Composer.swift`).
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { basename, dirname, join, resolve } from 'node:path';

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

/** Members join their component at the listing, with paths relative to the listing file. */
export function appText(file: string): string {
  const path = appPath(file);
  let owner = '';
  return readFileSync(path, 'utf8').split('\n').flatMap(line => {
    const component = /^component (\w+)$/.exec(line);
    if (component) owner = component[1]!;
    const listing = /^  members "([^"]+)"(?:\s*\/\/.*)?$/.exec(line);
    if (!listing) return [line];
    const memberPath = resolve(dirname(path), listing[1]!);
    const source = readFileSync(memberPath, 'utf8').split('\n');
    const from = source.indexOf(`members ${owner}`);
    if (!owner || from < 0) throw new Error(`${memberPath}: no members ${owner}`);
    const to = source.findIndex((text, i) => i > from && /^\S/.test(text) && !text.startsWith('//'));
    const body = source.slice(from + 1, to < 0 ? undefined : to);
    if (to < 0 && body.at(-1) === '') body.pop();
    return body;
  }).join('\n');
}
