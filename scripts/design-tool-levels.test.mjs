// The component panel's words for a param's winning level (apps/design-tool/components.ts `LEVEL_WORDS`), one for
// each raw level the protocol lists on `resolve`'s tuples (docs/tamber-protocol.md, stage 12 B2-levels), and none
// more (review2's condition on stage 12 D's tool plan).
import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { LEVEL_WORDS, levelWords } from '../apps/design-tool/components.ts';

test('each raw level the protocol lists has its words, one to one', () => {
  const doc = readFileSync(new URL('../docs/tamber-protocol.md', import.meta.url), 'utf8').replace(/\s+/g, ' ');
  const listed = /Each of `resolve`'s tuples adds `level`, one of (.*?) \(the param's own default/.exec(doc)?.[1];
  expect(listed).toBeTruthy();
  const levels = [...listed.matchAll(/`([a-z-]+)`/g)].map(m => m[1]);
  expect(levels.sort()).toEqual(Object.keys(LEVEL_WORDS).sort());
  expect(new Set(Object.values(LEVEL_WORDS)).size).toBe(levels.length);
  expect([levelWords('app-param', 'x#C.p'), levelWords('package', 'cards.fill'), levelWords('default', '')]).toEqual(['set for this app', "the package's token cards.fill", "the component's default"]);
});
