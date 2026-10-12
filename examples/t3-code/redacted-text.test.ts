// RedactedSensitiveText's placeholder (no reference test exists; RedactedSensitiveText.tsx:6-25).
import { describe, expect, test } from 'bun:test';
import { redactedPlaceholder, redactedValue } from './redacted-text';

const ALPHABET = /^[abcdefghjkmnpqrstuvwxyz23456789@.\-_]*$/;

describe('redactedPlaceholder', () => {
  test('keeps the value\'s shape: length and @ . - _ in place, other characters from the alphabet', () => {
    const value = 'first.last-name_1@example.co.uk';
    const placeholder = redactedPlaceholder(value);
    expect(placeholder).toHaveLength(value.length);
    expect(placeholder).toMatch(ALPHABET);
    for (const [index, char] of Array.from(value).entries()) {
      if ('@.-_'.includes(char)) expect(placeholder[index]).toBe(char);
      else expect('@.-_'.includes(placeholder[index]!)).toBe(false);
    }
    expect(placeholder).not.toContain('first');
    expect(placeholder).not.toContain('example');
  });
  test('is deterministic, and differs between values', () => {
    expect(redactedPlaceholder('dev@example.com')).toBe(redactedPlaceholder('dev@example.com'));
    expect(redactedPlaceholder('dev@example.com')).not.toBe(redactedPlaceholder('dex@example.com'));
  });
  test('matches the reference algorithm for a known value', () => {
    // Computed by running RedactedSensitiveText.tsx:6-25 itself at 1e2ecbd975.
    expect(redactedPlaceholder('developer@example.com')).toBe('yvs4jrutu@gyetdne.2j3');
    expect(redactedPlaceholder('dev@example.com')).toBe('bcz@x7f9r39.des');
    expect(redactedPlaceholder('secret-account')).toBe('5pjj3h-kw99kza');
    expect(redactedPlaceholder('')).toBe('');
  });
  test('counts code points as Array.from does', () => {
    expect(redactedPlaceholder('😀@x')).toHaveLength(3);
  });
});

describe('redactedValue', () => {
  test('trims, and has nothing to draw without a value', () => {
    expect(redactedValue('  dev@example.com ')).toEqual({ value: 'dev@example.com', placeholder: redactedPlaceholder('dev@example.com') });
    expect(redactedValue('   ')).toEqual({ value: '', placeholder: '' });
    expect(redactedValue(undefined)).toEqual({ value: '', placeholder: '' });
  });
});
