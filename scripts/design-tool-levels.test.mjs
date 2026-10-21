// The component panel's words for a param's winning level (apps/design-tool/components.ts `LEVEL_WORDS`), one for
// each raw level the protocol lists on `resolve`'s tuples (docs/tamber-protocol.md, stage 12 B2-levels), and none
// more (review2's condition on stage 12 D's tool plan).
import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { LEVEL_WORDS, levelWords } from '../apps/design-tool/components.ts';

test('each raw level the protocol lists has its words, one to one', () => {
  const doc = readFileSync(new URL('../docs/tamber-protocol.md', import.meta.url), 'utf8').replace(/\s+/g, ' ');
  const listed = /Each of `resolve`'s tuples adds `level` and `name`, each `\{light, dark\}`.*? A level is one of (.*?) \(the param's own default/.exec(doc)?.[1];
  expect(listed).toBeTruthy();
  const levels = [...listed.matchAll(/`([a-z-]+)`/g)].map(m => m[1]);
  expect(levels.sort()).toEqual(Object.keys(LEVEL_WORDS).sort());
  expect(new Set(Object.values(LEVEL_WORDS)).size).toBe(levels.length);
  // Each level's words, as the panel says them, and its name handling.
  expect(LEVEL_WORDS).toEqual({ 'app-param': 'set for this app', 'app-token': "the app's token", package: "the package's token", base: "Exact's base", role: "the platform's role", default: "the component's default" });
  const self = 'p.contract#Card.inset';
  expect([
    levelWords('app-param', self, self), levelWords('app-param', 'p.contract#Card.gap', self),
    levelWords('app-token', 'accent.fill', self), levelWords('package', 'cards.fill', self),
    levelWords('base', 'space.md', self), levelWords('role', '-exact-label', self),
    levelWords('default', self, self), levelWords('default', 'p.contract#Card.gap', self),
  ]).toEqual([
    'set for this app', 'set for this app, on Card.gap',
    "the app's token accent.fill", "the package's token cards.fill",
    "Exact's base space.md", "the platform's role -exact-label",
    "the component's default", "the component's default, on Card.gap",
  ]);
});
