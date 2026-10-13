import { test, expect } from 'bun:test';
import { x_querySelectorAll } from '../../web-js/dataset.js';

test('a data-word read includes literal and bound attributes in tree order', () => {
  const previous = globalThis.document;
  const previousStyle = globalThis.getComputedStyle;
  const root = { parentElement: null };
  const node = (id, ds) => ({ id, $ds: ds, parentElement: root, closest: () => null });
  const save = node('save');
  const row0 = node('row-0', ['paint']);
  const row2 = node('row-2', ['paint']);
  root.querySelectorAll = () => [save, row0, row2];
  globalThis.document = { getElementById: () => root };
  globalThis.getComputedStyle = () => ({ display: 'block' });
  try {
    expect(x_querySelectorAll('[data-paint~="accent"]')).toEqual(['save', 'row-0', 'row-2']);
  } finally {
    if (previous === undefined) delete globalThis.document;
    else globalThis.document = previous;
    if (previousStyle === undefined) delete globalThis.getComputedStyle;
    else globalThis.getComputedStyle = previousStyle;
  }
});
