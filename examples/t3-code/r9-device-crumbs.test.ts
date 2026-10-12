import { describe, expect, test } from 'bun:test';
import { crumbsOffset, noteFirstRead, settleCrumbs, sourceGutter, SOURCE_NUMBER_INSET } from './r9-device-crumbs';
import { crumbsMask } from './r7-polish-crumbs';

// Lane r9-device: HEAD oracle at 840 × 620, wt / docs / guide / intro.md: the trail is 203pt in a
// 162pt viewport. A first open settles at scrollLeft 5 (its word-wrap toggle arrived after the
// scroll), a re-open at 41.
describe('Files breadcrumbs settle where the reference scroll left them', () => {
  const at = { anchors: { crumbs: [0, 203], 'crumbs-clip': [12, 162] } };
  test('a file read for the first time settles 36pt short of the end; a cached one at the end', () => {
    expect(crumbsOffset(at, true, true)).toBe(5);
    expect(crumbsOffset(at, false, true)).toBe(-1);
    expect(crumbsOffset(at, true, false)).toBe(-1);
    expect(crumbsOffset({ anchors: { crumbs: [0, 180], 'crumbs-clip': [12, 162] } }, true, true)).toBe(0);
    expect(crumbsOffset({ anchors: { crumbs: [0, 162], 'crumbs-clip': [12, 162] } }, true, true)).toBe(-1);
    expect(crumbsOffset({}, true, true)).toBe(-1);
  });
  test('the settle is fixed when the file becomes active', () => {
    const owner = {};
    noteFirstRead(owner, 'docs/guide/intro.md');
    expect(settleCrumbs(owner, 'docs/guide/intro.md')).toBe(true);
    expect(settleCrumbs(owner, 'docs/guide/intro.md')).toBe(true);
    expect(settleCrumbs(owner, 'README.md')).toBe(false);
    expect(settleCrumbs(owner, 'docs/guide/intro.md')).toBe(false);
    noteFirstRead(owner, 'long.txt');
    expect(settleCrumbs(owner, '')).toBe(false);
    expect(settleCrumbs(owner, 'long.txt')).toBe(true);
  });
  test('a trail scrolled short of both ends fades both edges over min(24, overflow)', () => {
    expect(crumbsMask({ anchors: { crumbs: [-5, 203], 'crumbs-clip': [12, 162] } }))
      .toBe('linear-gradient(to right, #00000000 0%, #000000 3.1%, #000000 85.2%, #00000000 100%)');
    expect(crumbsMask({ anchors: { crumbs: [0, 203], 'crumbs-clip': [12, 162] } }))
      .toBe('linear-gradient(to right, #000000 0%, #000000 0%, #000000 85.2%, #00000000 100%)');
    expect(crumbsMask({ anchors: { crumbs: [-41, 203], 'crumbs-clip': [12, 162] } })).toBe('linear-gradient(to right, #00000000 0%, #000000 14.8%)');
  });
});

describe('Source line-number column', () => {
  test('matches the oracle column for 1, 2, 3 and 4 digit files', () => {
    expect([1, 11, 120, 1200].map(sourceGutter)).toEqual([33.31, 41.13, 48.96, 56.79]);
    expect(SOURCE_NUMBER_INSET).toBe(9.83);
  });
});
