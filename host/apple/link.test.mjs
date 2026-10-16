import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { appleComposition, assertCapabilityFrameworks, loadedDylibs } from './link.mjs';

test('production links what the plan uses, the manifest names and the grants open; development all but the photo picker and sign-in sheet, which it links as production would (LLP 1047.001.000 E, F)', () => {
  const graph = { uses: ['markdown', 'picker'] };
  assert.deepEqual(appleComposition(graph, true), { link: 'markdown,picker', products: ['ExactMarkdown'] });
  assert.deepEqual(appleComposition({ uses: [] }, true), { link: 'none', products: [] });
  assert.deepEqual(appleComposition(graph, true, [], true).products, ['ExactMarkdown', 'ExactAuthSession']);
  assert.deepEqual(appleComposition(graph, false).products, ['ExactGroupedLists', 'ExactMarkdown', 'ExactSurfaces', 'ExactDrag']);
  assert.deepEqual(appleComposition({ uses: ['picker', 'media_picker'] }, false, [], true).products,
    ['ExactGroupedLists', 'ExactMarkdown', 'ExactSurfaces', 'ExactDrag', 'ExactMediaPicker', 'ExactAuthSession']);
  assert.deepEqual(appleComposition(graph, false, ['auth_session']).products.at(-1), 'ExactAuthSession');
  assert.deepEqual(appleComposition({ uses: ['numerals'] }, false).products.at(-1), 'ExactNumerals');
  assert.ok(!appleComposition(graph, false).products.includes('ExactNumerals'));
  assert.equal(appleComposition(undefined, false).link, 'all');
});

test('the load commands are read from the file, each slice of a universal one; a binary with no capability framework passes the check', () => {
  assert.ok(loadedDylibs('/bin/ls').includes('/usr/lib/libSystem.B.dylib'));
  assert.doesNotThrow(() => assertCapabilityFrameworks('/bin/ls', []));
});
