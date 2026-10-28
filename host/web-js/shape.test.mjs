// A lone UTF-16 surrogate is no string on the web, as on native hosts (LLP 1146 D6).
import { expect, test } from 'bun:test';
import { conforms, loneSurrogate } from './shape.js';

test('a string with a lone surrogate does not conform, and is said', () => {
  const cut = '😀 reply'.slice(0, 1); // half an emoji, as `.slice(0, n)` through one leaves it
  expect(conforms('a 😀', 's')).toBe(true);
  expect(conforms(cut, 's')).toBe(false);
  expect(conforms(['ok', cut], '[s')).toBe(false);
  expect(conforms([cut], '{s}')).toBe(false);
  expect(loneSurrogate([['ok'], ['x', cut]])).toBe(true);
  expect(loneSurrogate(['a 😀', 3, null])).toBe(false);
});
