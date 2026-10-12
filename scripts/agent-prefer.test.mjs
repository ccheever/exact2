// `prefer`'s facts by their names (agent-prefer.mjs `preferGroups`): a fact answers to its near spellings
// (LLP 1116.007 D3.5: `prefer color-scheme dark`), and an unknown fact or value is still refused.
import { test, expect } from 'bun:test';
import { preferGroups } from './agent-prefer.mjs';

test('a near spelling of a fact is that fact', () => {
  for (const name of ['prefers-color-scheme', 'color-scheme', 'colorScheme', 'prefersColorScheme', 'Color_Scheme']) {
    expect(preferGroups({ [name]: 'dark' }).media).toEqual({ 'prefers-color-scheme': 'dark' });
  }
  expect(preferGroups({ 'reduced-motion': 'reduce', contrast: 'more' }).media).toEqual({ 'prefers-reduced-motion': 'reduce', 'prefers-contrast': 'more' });
  expect(preferGroups({ canShare: 'false', rootFontSize: '20' }).page).toEqual({ 'can-share': false, 'root-font-size': 20 });
  expect(preferGroups({ segments: '2x1', gap: '40' }).fold).toEqual({ cols: 2, rows: 1, gap: 40 });
});

test('an unknown fact or a value it does not take is refused naming what is expected', () => {
  expect(() => preferGroups({ 'color-scheme': 'blue' })).toThrow(/^prefer: prefers-color-scheme blue: expected .*prefers-color-scheme dark\|light/);
  expect(() => preferGroups({ theme: 'dark' })).toThrow(/^prefer: theme dark: expected /);
  expect(() => preferGroups({ gap: '4' })).toThrow('prefer: gap needs segments');
});
