// The reference's text boxes are browser textareas and inputs (T3 Code 1e2ecbd975, MIT; see
// LICENSE-T3), which never rewrite what was typed. On macOS a Contract textarea or input keeps the
// typed bytes only with `autocorrect="off"` (exact2 #111: no spelling correction, smart quotes or
// dashes, no text replacement), so every editable textarea and text input in the clone carries it.
// An input's correction includes AppKit's automatic capitalization: a right-click that selected a
// typed word in Settings' search field accepted it ("theme" became "Theme", the selection collapsed
// and the menu's Cut cut nothing; realinput-1010g RG-2).
import { describe, test, expect } from 'bun:test';
import { readdirSync } from 'node:fs';

const dir = new URL('./', import.meta.url);

/** Each `<tag>` element line of a contract file, with its line number. */
async function elements(tag: 'textarea' | 'input'): Promise<{ at: string; line: string }[]> {
  const found: { at: string; line: string }[] = [];
  const opens = new RegExp(`^\\s*${tag}\\s`);
  for (const name of readdirSync(dir).filter(file => file.endsWith('.contract')).sort()) {
    const lines = (await Bun.file(new URL(name, dir)).text()).split('\n');
    lines.forEach((line, index) => { if (opens.test(line)) found.push({ at: `${name}:${index + 1}`, line }); });
  }
  return found;
}
const textareas = () => elements('textarea');
/** An input that takes no typed text (AppKit draws a control, not a field editor). */
const typesNoText = (line: string) => /\btype="(checkbox|radio|range|color|date|time|file|hidden)"/.test(line);

describe('typed text is kept as typed', () => {
  test('every editable textarea has autocorrect="off"', async () => {
    const all = await textareas();
    expect(all.length).toBeGreaterThan(5);
    const missing = all.filter(({ line }) => !/\breadonly=true\b/.test(line) && !/\bautocorrect="off"/.test(line)).map(({ at }) => at);
    expect(missing).toEqual([]);
  });

  test('every text input has autocorrect="off" (realinput-1010g RG-2)', async () => {
    const all = (await elements('input')).filter(({ line }) => !typesNoText(line));
    expect(all.length).toBeGreaterThan(90);
    const missing = all.filter(({ line }) => !/\bautocorrect="off"/.test(line)).map(({ at }) => at);
    expect(missing).toEqual([]);
    const search = all.find(({ line }) => line.includes('id="settings-search"'));
    expect(search?.line).toContain('autocorrect="off"');
  });

  test('the composer, the Files editor and the commit message are among them', async () => {
    const ids = (await textareas()).map(({ line }) => /\b(?:id|testId)="([^"]+)"/.exec(line)?.[1]).filter(Boolean);
    expect(ids).toEqual(expect.arrayContaining(['composer', 'file-editor', 'git-commit-message']));
  });
});
