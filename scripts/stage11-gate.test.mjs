// scripts/stage11-gate.mjs: the scale table's sources, read mechanically (a
// cited form or none), and the command line.
import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { citations, lines, parse } from './stage11-gate.mjs';

const ROOT = new URL('..', import.meta.url).pathname;
const schema = JSON.parse(readFileSync(`${ROOT}kernel/tables/schema.json`, 'utf8'));
const measured = readFileSync(`${ROOT}docs/scale-measured.md`, 'utf8');

test('every scale cell the table has is a cited form', () => {
  expect(citations(schema, measured)).toEqual([]);
});

test('a bare number, an unknown metric or an unbound entry is no source', () => {
  const row = (...cells) => ({ ...schema, scale: [['space.xs', ...cells]] });
  expect(citations(row(4, '=grouped-row-gap', '=grouped-row-gap', '=grouped-row-gap', { cells: 0 }), measured)).toEqual(['space.xs column 1: 4 names no source']);
  expect(citations(row('=no-such', '=grouped-row-gap', '=grouped-row-gap', '=grouped-row-gap', { cells: 0 }), measured)[0]).toContain('names no metrics row');
  expect(citations(row({ px: 7, measured: 'ios-button-small-inset-top' }, '=grouped-row-gap', '=grouped-row-gap', '=grouped-row-gap', { cells: 0 }), measured)[0]).toContain('binds no space.xs = 7');
  expect(citations(row({ px: 5, measured: 'nowhere' }, '=grouped-row-gap', '=grouped-row-gap', '=grouped-row-gap', { cells: 0 }), measured)[0]).toContain('has no entry');
  // `full` only for radius.full; a terminal radius of 0 is its form.
  expect(citations(row('full', '=grouped-row-gap', '=grouped-row-gap', '=grouped-row-gap', { cells: 0 }), measured)[0]).toContain('full outside radius.full');
});

test('the gate\'s lines, and its command line', () => {
  const ids = lines({ hosts: [], app: 'unsaid', only: [] }).map(([id]) => id);
  expect(ids).toEqual(['base-values', 'citations', 'packages-restyle', 'packages-base', 'package-full-name', 'package-one-name', 'package-no-dials', 'package-when-axis', 'package-no-base', 'server-rows', 'server-writes', 'live', 'tool', 'clone', 'no-regressions']);
  expect(parse(['--host', 'linux', '--only', 'citations'])).toMatchObject({ hosts: ['linux'], only: ['citations'], app: 'unsaid' });
  expect(parse(['--host'])).toBe('--host needs a value');
  expect(parse(['--bogus', 'x'])).toBe('unknown argument --bogus');
});
