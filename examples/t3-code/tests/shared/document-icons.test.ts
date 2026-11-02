// pierre-icons.ts at 7c3b3b3605ef (MIT, see LICENSE-T3): PDFs, word processor, spreadsheet, presentation and EPUB
// files draw their kind's coloured Lucide icon (DOCUMENT_ICON_COLORS), ahead of Pierre's generic ones.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { fileIconToken } from '../../src/shared/timeline-files';
import { appPath } from '../support/app-files';

describe('document icons', () => {
  test('each document extension names its kind', () => {
    expect(['report.pdf', 'letter.doc', 'notes.docx', 'draft.odt', 'readme.rtf', 'q3.xls', 'q3.xlsx', 'q3.ods', 'data.csv', 'data.tsv', 'deck.ppt', 'deck.pptx', 'deck.odp', 'book.epub'].map(fileIconToken))
      .toEqual(['pdf', 'word', 'word', 'word', 'word', 'spreadsheet', 'spreadsheet', 'spreadsheet', 'spreadsheet', 'spreadsheet', 'slides', 'slides', 'slides', 'epub']);
  });
  test('the icon component draws them in their tints', () => {
    const icon = readFileSync(appPath('src/shared/timeline-files.contract'), 'utf8');
    for (const tint of ['light-dark(#d52c36, #ff6762)', 'light-dark(#1a85d4, #69b1ff)', 'light-dark(#199f43, #5ecc71)', 'light-dark(#d47628, #ffa359)', 'light-dark(#693acf, #9d6afb)']) expect(icon).toContain(tint);
    expect(icon).toContain('"spreadsheet"');
  });
});
