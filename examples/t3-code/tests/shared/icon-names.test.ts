// Every name a source draws with Icon (icons.contract) has geometry in its table: Icon draws an empty
// path for a name iconGeometry does not list, so a missing name is a button with no glyph.
import { describe, expect, test } from 'bun:test';
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { APP } from '../support/app-files';
import { ICON } from '../../tools/generate/icon-tables.mjs';

const names = new Set((ICON as (string | [string, unknown])[]).map(entry => (typeof entry === 'string' ? entry : entry[0])));

/** Each `Icon(name=…)` in the app's Contract sources: a literal name, or the literal results of a conditional. */
function iconUses(): { file: string; name: string }[] {
  const uses: { file: string; name: string }[] = [];
  const src = join(APP, 'src');
  for (const file of readdirSync(src, { recursive: true }) as string[]) {
    if (!file.endsWith('.contract')) continue;
    const text = readFileSync(join(src, file), 'utf8');
    for (const match of text.matchAll(/(?<![A-Za-z])Icon\(name=("[^"]*"|\((?:[^()]|\([^()]*\))*\))/g)) {
      const expr = match[1]!;
      const literals = expr.startsWith('"') ? [expr] : [...expr.matchAll(/[?:]\s*("[^"]*")/g)].map(m => m[1]!);
      for (const literal of literals) uses.push({ file, name: JSON.parse(literal) });
    }
  }
  return uses;
}

describe('Icon names', () => {
  test('the sweep finds the literal and conditional names', () => {
    const found = new Set(iconUses().map(use => use.name));
    for (const name of ['x', 'chevron', 'right', 'minimize', 'maximize', 'minus', 'plus', 'play']) expect(found.has(name)).toBeTrue();
  });
  test('every name has geometry in the Icon table', () => {
    expect(iconUses().filter(use => !names.has(use.name))).toEqual([]);
  });
});
