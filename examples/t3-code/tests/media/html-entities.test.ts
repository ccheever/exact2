import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { LEGACY, NAMED } from '../../src/media/html-entities';
import { decode } from '../../src/media/media-html';

// The table is markdown-parse's (entities.rs), whose string pieces are valid JavaScript strings as written.
const rust = readFileSync(join(import.meta.dir, '../../../../apps/markdown/parse/src/entities.rs'), 'utf8');
const pieces = (name: string) => {
  const body = new RegExp(`const ${name}: &str = concat!\\(\\n([\\s\\S]*?)\\n\\);`).exec(rust)?.[1] ?? '';
  return body.split('\n').map(line => new Function(`return ${line.trim().replace(/,$/, '')}`)() as string).join('');
};

test("the reference table is markdown-parse's", () => {
  expect(NAMED).toBe(pieces('NAMED'));
  expect(LEGACY).toBe(pieces('LEGACY'));
});

test('references decode as an HTML tokenizer decodes an attribute value', () => {
  expect(decode('javascript&colon;x')).toBe('javascript:x');
  expect(decode('&copy; &amp &notit; &not;x &amp=1 &ampy')).toBe('\u00a9 & &notit; \u00acx &amp=1 &ampy');
  expect(decode('&#0;&#x80;&#X1F600&#xD800;&#x110000;&#;')).toBe('\ufffd\u20ac\u{1f600}\ufffd\ufffd&#;');
  expect(decode('&NewLine;&Tab;&fjlig;&AMP;')).toBe('\n\tfj&');
});
