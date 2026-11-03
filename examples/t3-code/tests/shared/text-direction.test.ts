// T3 Code 5d3d78ab93 apps/web/src/textDirection.test.ts (MIT, see LICENSE-T3).
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
    ['tied sentences, by the whole text\'s words', 'Done. تم إصلاح المشكلة في الملف الرئيسي', 'rtl'],
    ['mostly Arabic sentences beside one long English one', 'هذا مثال على paragraph يحتوي على نص عربي و English words معًا. Arabic text should flow right-to-left while English words like JavaScript, React, and API should remain left-to-right within the same line. الأرقام 1234567890 ورموز مثل @username و #hashtag و 100$ تظهر في الاتجاه الصحيح. هذه جملة لاختبار RTL rendering في المتصفح أو المحرر.', 'rtl'],
    ['mostly English sentences beside one Arabic one', 'Run the migration first. ثم شغّل الاختبارات كلها. Then deploy to staging.', 'ltr'],
    ['Arabic with inline abbreviations', 'هذه جملة عربية تتحدث عن e.g. React i.e. TypeScript JavaScript', 'rtl'],
    ['an abbreviation at a sentence boundary', 'This long English sentence describes the technology sector in the U.S. هذه جملة عربية. وهذه جملة عربية أخرى.', 'rtl'],
  ] as const) {
    test(`reads ${name}`, () => expect(textDirection(text)).toBe(direction));
  }
  test('leaves text without letters to its surroundings', () => {
    expect(textDirection('42 — 7 ✓')).toBeUndefined();
  });
});
