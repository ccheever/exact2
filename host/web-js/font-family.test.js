import { expect, test } from 'bun:test';
import { fontFamily } from './font-family.js';

test('runtime family lists preserve installed names and portable generic fallback', () => {
  expect(fontFamily('"Comma, Family", ui-monospace, Georgia')).toBe('"Comma, Family",ui-monospace,monospace, Georgia');
  expect(fontFamily("'ui-serif', ui-serif")).toBe("'ui-serif',ui-serif,serif");
  expect(fontFamily('Arial, sans-serif')).toBe('Arial, sans-serif');
});

test('CSS-wide keywords follow the kernel: initial is its family 0, revert inherits', () => {
  expect(fontFamily('initial')).toBe('system-ui');
  expect(fontFamily(' INITIAL ')).toBe('system-ui');
  expect(fontFamily('revert')).toBe('unset');
  expect(fontFamily('Revert-Layer')).toBe('unset');
  expect(fontFamily('inherit')).toBe('inherit');
  expect(fontFamily("'initial'")).toBe("'initial'");
});
