// The JS dev loop's word on a live proposal (dev-inline.mjs): a token its pages write inline that moves makes it a
// rebuild, and so does what can't be known yet (theme2's ruling of 2026-10-23 10:01), never live.
import { expect, test } from 'bun:test';
import { inlineWhy } from './dev-inline.mjs';

const token = (name, value, inline = false) => ({ name, value, slices: [], inline });
const pages = { tokens: [token('ink', '#000000'), token('weight.title', '600', true)] };
const ask = (texts, { ready = true, read = async () => pages, from = pages } = {}) => {
  let reads = 0;
  const why = inlineWhy({ texts, file: 'app.tamber', ready: () => ready, read: async t => { reads++; return read(t); }, pages: () => from, same: (a, b) => a.value === b.value });
  return why.then(w => ({ why: w, reads }));
};

test('a proposal that moves no inline token applies in place; one that moves one is a rebuild, said which', async () => {
  expect(await ask({ 'app.tamber': 'x' }, { read: async () => ({ tokens: [token('ink', '#111111'), token('weight.title', '600', true)] }) })).toEqual({ why: null, reads: 1 });
  expect((await ask({ 'app.tamber': 'x' }, { read: async () => ({ tokens: [token('ink', '#000000'), token('weight.title', '700', true)] }) })).why).toBe('`weight.title` is written inline on this page (a font weight, an SVG attribute)');
  // Not the design's file: nothing to check.
  expect(await ask({ 'app.contract': 'x' })).toEqual({ why: null, reads: 0 });
});

test('what cannot be known yet is a rebuild, said why: no reader built (and none read), a read that fails, no design the pages run, a design of other tokens', async () => {
  expect(await ask({ 'app.tamber': 'x' }, { ready: false })).toEqual({ why: 'the design reader is not built yet, so what these pages write inline is not yet known: rebuild assumed', reads: 0 });
  expect((await ask({ 'app.tamber': 'x' }, { read: async () => ({ error: 'unexpected token' }) })).why).toBe('the design reader could not read it (unexpected token): rebuild assumed');
  expect((await ask({ 'app.tamber': 'x' }, { from: null })).why).toBe('the design these pages run is not read yet: rebuild assumed');
  expect((await ask({ 'app.tamber': 'x' }, { read: async () => ({ tokens: [token('ink', '#000000')] }) })).why).toBe('it changes the design\'s tokens, which no page previews in place');
});
