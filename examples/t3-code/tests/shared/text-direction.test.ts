// T3 Code 7c3b3b3605ef apps/web/src/textDirection.test.ts (MIT, see LICENSE-T3).
import { describe, expect, test } from 'bun:test';
import { textDirection } from '../../src/shared/text-direction';

describe('textDirection', () => {
  for (const [name, text, direction] of [
    ['Arabic', 'اكتب دالة تجمع الأرقام', 'rtl'],
    ['Hebrew', 'תקן את הבאג', 'rtl'],
    ['English', 'fix the failing test', 'ltr'],
    ['Arabic that opens with an English term', 'StrictMode: يعيد الـ render مرتين في التطوير', 'rtl'],
    ['English quoting one Arabic word', 'translate the label مرحبا for the login screen', 'ltr'],
    ['a tie, by its first word', 'React مكتبة', 'ltr'],
    ['a tie that opens in Arabic', 'مكتبة React', 'rtl'],
    ['Arabic around numbers and punctuation', 'عدّل السطر 42، ثم شغّل الاختبارات (3)', 'rtl'],
  ] as const) {
    test(`reads ${name}`, () => expect(textDirection(text)).toBe(direction));
  }
  test('leaves text without letters to its surroundings', () => {
    expect(textDirection('42 — 7 ✓')).toBeUndefined();
  });
});
