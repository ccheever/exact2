// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import { expect, test } from 'bun:test';
import { mobileLayoutFacts } from './root-presentation';
import { mobileComposerHairline } from './composer-command-presentation';

test('attached display scale follows native facts and source hairline rounding', () => {
  for (const [scale, points] of [[1, 1], [2, 0.5], [3, 1 / 3], [4, 0.5]]) {
    const facts = mobileLayoutFacts(JSON.stringify({ displayScale: scale }));
    expect(facts.displayScale).toBe(scale);
    expect(mobileComposerHairline(facts.displayScale)).toBe(points);
  }
});
test('pre-native and malformed display facts retain a finite bake fallback', () => {
  for (const value of [undefined, null, '3', 0, -1, true]) {
    expect(mobileLayoutFacts(JSON.stringify({ displayScale: value })).displayScale).toBe(1);
  }
  for (const raw of ['', '{', '{"displayScale":1e999}']) {
    expect(mobileLayoutFacts(raw).displayScale).toBe(1);
  }
  expect(mobileComposerHairline(NaN)).toBe(1);
});
