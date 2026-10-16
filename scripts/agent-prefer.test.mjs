// `prefer`'s facts by their names (agent-prefer.mjs `preferGroups`): a fact answers to its near spellings
// (LLP 1116.007 D3.5: `prefer color-scheme dark`), and an unknown fact or value is still refused.
import { test, expect } from 'bun:test';
import { preferGroups, preferWeb } from './agent-prefer.mjs';
import { MalformedOperation } from './agent-keys.mjs';
import { pageReporter } from '../host/web/navigation.js';

test('a near spelling of a fact is that fact', () => {
  for (const name of ['prefers-color-scheme', 'color-scheme', 'colorScheme', 'prefersColorScheme', 'Color_Scheme']) {
    expect(preferGroups({ [name]: 'dark' }).media).toEqual({ 'prefers-color-scheme': 'dark' });
  }
  expect(preferGroups({ 'reduced-motion': 'reduce', contrast: 'more' }).media).toEqual({ 'prefers-reduced-motion': 'reduce', 'prefers-contrast': 'more' });
  expect(preferGroups({ canShare: 'false', rootFontSize: '20' }).page).toEqual({ 'can-share': false, 'root-font-size': 20 });
  expect(preferGroups({ displayMode: 'fullscreen' }).media).toEqual({ 'display-mode': 'fullscreen' });
  expect(preferGroups({ segments: '2x1', gap: '40' }).fold).toEqual({ cols: 2, rows: 1, gap: 40 });
});

test('an unknown fact or a value it does not take is refused naming what is expected', () => {
  for (const facts of [{ 'prefers-color-scheme': 'banana' }, { theme: 'dark' }, { 'has-focus': undefined }]) expect(() => preferGroups(facts)).toThrow(MalformedOperation);
  expect(() => preferGroups({ 'color-scheme': 'blue' })).toThrow(/^prefer: prefers-color-scheme blue: expected .*prefers-color-scheme dark\|light/);
  expect(() => preferGroups({ theme: 'dark' })).toThrow(/^prefer: theme dark: expected /);
  expect(() => preferGroups({ gap: '4' })).toThrow('prefer: gap needs segments');
});

test('Chrome refuses display-mode emulation instead of reporting an unapplied override', async () => {
  await expect(preferWeb({ media: { 'display-mode': 'fullscreen' }, page: {} }))
    .rejects.toThrow('Chrome prefer cannot emulate display-mode fullscreen');
  expect((await preferWeb({ media: { 'display-mode': 'fullscreen' }, page: {} }).catch(e => e)).malformed).toBeUndefined();
});

test('segments and gap are malformed before a display-mode refusal', async () => {
  const invalid = [
    ...['0x1', '1x0', '-1x1', '1.5x1', '1x1.5', `${'9'.repeat(400)}x1`].map(segments => ({ segments })),
    ...[-1, NaN, Infinity, 'banana'].map(gap => ({ segments: '2x1', gap })),
  ];
  for (const facts of invalid) {
    const error = await (async () => preferWeb(preferGroups({ 'display-mode': 'fullscreen', ...facts })))().catch(e => e);
    expect(error).toBeInstanceOf(MalformedOperation);
    expect(error.malformed).toBe(true);
  }
  expect(preferGroups({ segments: '1x1', gap: 0 }).fold).toEqual({ cols: 1, rows: 1, gap: 0 });
  expect(preferGroups({ segments: '2x3', gap: 0.5 }).fold).toEqual({ cols: 2, rows: 3, gap: 0.5 });
});

test('prefer mode is the app modes group (LLP 1121 D13): each pin, the scheme left alone', () => {
  const { media, mode } = preferGroups({ mode: 'dark-style=dim, density=compact' });
  expect(mode).toEqual({ 'dark-style': 'dim', density: 'compact' });
  expect(media).toEqual({});
  expect(() => preferGroups({ mode: 'dim' })).toThrow(/mode <axis>=<value>/);
  expect(() => preferGroups({ mode: 'dark-style:dim' })).toThrow(/mode <axis>=<value>/);
});

test('page preference refusals omit the malformed property', () => {
  const { prefer } = pageReporter(true);
  try { prefer({ 'keyboard-navigation': true }); throw new Error('expected a refusal'); }
  catch (error) { expect(error.message).toContain('not a page fact'); expect(Object.hasOwn(error, 'malformed')).toBe(false); }
  try { prefer({ banana: true }); throw new Error('expected malformed input'); }
  catch (error) { expect(error.malformed).toBe(true); }
});
