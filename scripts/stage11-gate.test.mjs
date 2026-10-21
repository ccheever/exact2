// scripts/stage11-gate.mjs: the scale table's sources, read as the kernel's
// build reads them, a cargo run that ran something, the command line, and
// the release identity's judge.
import { expect, test } from 'bun:test';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { citations, lines, parse, ranOk, releaseIdentity } from './stage11-gate.mjs';

const ROOT = new URL('..', import.meta.url).pathname;
const schema = JSON.parse(readFileSync(`${ROOT}kernel/tables/schema.json`, 'utf8'));
const measured = readFileSync(`${ROOT}docs/scale-measured.md`, 'utf8');

test('every scale cell the table has is a cited form', () => {
  expect(citations(schema, measured)).toEqual([]);
});

test('a bare number, an unknown metric, an inexact binding or a terminal cell of no number is no source', () => {
  const row = (name, ...cells) => ({ ...schema, scale: [[name, ...cells]] });
  const refs = ['=grouped-row-gap', '=grouped-row-gap', '=grouped-row-gap'];
  expect(citations(row('space.xs', 4, ...refs, { cells: 0 }), measured)).toEqual(['space.xs column 1: 4 names no source']);
  expect(citations(row('space.xs', '=no-such', ...refs, { cells: 0 }), measured)[0]).toContain('names no metrics row');
  expect(citations(row('space.xs', { px: 7, measured: 'ios-button-small-inset-top' }, ...refs, { cells: 0 }), measured)[0]).toContain('binds no `space.xs = 7`');
  expect(citations(row('space.xs', { px: 5, measured: 'nowhere' }, ...refs, { cells: 0 }), measured)[0]).toContain('has no entry');
  // 5 is not 5.6, and the step is the name's own.
  expect(citations(row('radius.sm', { px: 5, measured: 'ios-cornerstyle-small-44' }, ...refs, 0), measured)[0]).toContain('binds no `radius.sm = 5`');
  expect(citations(row('space.xs', ...refs, '=grouped-row-gap', { cells: 'one' }), measured)[0]).toContain('no number of cells');
  expect(citations(row('space.xs', 'full', ...refs, { cells: 0 }), measured)[0]).toContain('full outside radius.full');
  // A stretch (stage 13 D5): a metric with its why and its entry, or refused.
  expect(citations(row('space.xs', { stretch: '=grouped-label-inset', why: 'a test' }, ...refs, { cells: 0 }), measured)).toEqual([]);
  expect(citations(row('space.xs', { stretch: '=grouped-label-inset' }, ...refs, { cells: 0 }), measured)[0]).toContain('no stretch');
  expect(citations(row('space.xs', { stretch: '=grouped-row-gap', why: 'a test' }, ...refs, { cells: 0 }), measured)[0]).toContain('no entry under Role stretches');
  expect(citations(row('space.xs', { stretch: '=grouped-label-inset', why: 'two\nlines' }, ...refs, { cells: 0 }), measured)[0]).toContain('no stretch');
  // The page's line ends are its own (Astra's round 2): CRLF reads alike.
  expect(citations(row('space.xs', { stretch: '=grouped-label-inset', why: 'a test' }, ...refs, { cells: 0 }), measured.replaceAll('\n', '\r\n'))).toEqual([]);
  // An entry outside Role stretches is none.
  const elsewhere = measured.replace('## Role stretches\n', '## Moved\n');
  expect(citations(row('space.xs', { stretch: '=grouped-label-inset', why: 'a test' }, ...refs, { cells: 0 }), elsewhere)[0]).toContain('no entry under Role stretches');
});

test('a cargo run passes only when it ran a test and none failed', () => {
  expect(ranOk('test result: ok. 1 passed; 0 failed; 0 ignored')).toBe(true);
  expect(ranOk('test result: ok. 0 passed; 0 failed; 0 ignored')).toBe(false);
  expect(ranOk('test result: FAILED. 3 passed; 1 failed;')).toBe(false);
  expect(ranOk('error: could not compile')).toBe(false);
});

test('the gate\'s lines, and its command line', () => {
  const ids = lines({ hosts: [], app: 'unsaid', only: [] }).map(([id]) => id);
  expect(ids).toEqual(['base-values', 'citations', 'packages-restyle', 'packages-base', 'package-full-name', 'package-one-name', 'package-no-dials', 'package-when-axis', 'package-no-base', 'server-rows', 'server-writes', 'private', 'live', 'tool', 'clone', 'no-regressions']);
  expect(parse(['--host', 'linux', '--only', 'citations'])).toMatchObject({ hosts: ['linux'], only: ['citations'], app: 'unsaid' });
  expect(parse(['--host'])).toBe('--host needs a value');
  expect(parse(['--bogus', 'x'])).toBe('unknown argument --bogus');
  expect(parse(['--host', 'tv'])).toContain('--host tv: one of');
  expect(parse(['--only', 'nothing'])).toContain('--only nothing: one of');
});

test('release identity: a plan moves only as named', () => {
  const file = rows => { const f = join(mkdtempSync(join(tmpdir(), 'rel11-')), 'r.json'); writeFileSync(f, JSON.stringify(rows)); return f; };
  const b1 = ['LLP 1121 stage 11 B1: the base scale table'], d = ['LLP 1121 stage 11 D: base names on the wire'];
  const digest = { sizes: [10, 10], changed: 8, range: [17, 24] };
  expect(releaseIdentity(null).status).toBe('WAITS');
  const named = [{ run: 'a..b', commits: b1, source: 'apps/hello/app.contract', status: 'DIFFERS', bytes: digest },
    { run: 'c..d', commits: ['Design tool, stage 11 D'], source: 'apps/design-tool/app.contract', status: 'DIFFERS', bytes: { sizes: [1, 2] } },
    { run: 'c..d', commits: d, source: 'apps/shelf/app.contract', status: 'identical' }];
  expect(releaseIdentity(file(named)).status).toBe('PASS');
  // The digest's bytes in a run that is not B1's, a byte past the header, a
  // size moved, another app, or a build failing on one side: each fails.
  for (const row of [{ ...named[0], commits: d }, { ...named[0], bytes: { ...digest, range: [17, 25] } }, { ...named[0], bytes: { ...digest, sizes: [10, 11] } },
    { ...named[1], source: 'apps/shelf/app.contract' }, { run: 'a..b', commits: b1, source: 'apps/hello/app.contract', status: 'build failed' }]) {
    expect(releaseIdentity(file([row])).status).toBe('FAIL');
  }
});
