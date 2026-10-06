// The reference's text boxes are browser textareas (T3 Code 1e2ecbd975, MIT; see LICENSE-T3),
// which never rewrite what was typed. On macOS a Contract textarea keeps the typed bytes only
// with `autocorrect="off"` (exact2 #111: no smart quotes or dashes, no text replacement), so
// every editable textarea in the clone carries it.
import { describe, test, expect } from 'bun:test';
import { readdirSync } from 'node:fs';

const dir = new URL('./', import.meta.url);

/** Each `textarea` element line of a contract file, with its line number. */
async function textareas(): Promise<{ at: string; line: string }[]> {
  const found: { at: string; line: string }[] = [];
  for (const name of readdirSync(dir).filter(file => file.endsWith('.contract')).sort()) {
    const lines = (await Bun.file(new URL(name, dir)).text()).split('\n');
    lines.forEach((line, index) => { if (/^\s*textarea\s/.test(line)) found.push({ at: `${name}:${index + 1}`, line }); });
  }
  return found;
}

describe('typed text is kept as typed', () => {
  test('every editable textarea has autocorrect="off"', async () => {
    const all = await textareas();
    expect(all.length).toBeGreaterThan(5);
    const missing = all.filter(({ line }) => !/\breadonly=true\b/.test(line) && !/\bautocorrect="off"/.test(line)).map(({ at }) => at);
    expect(missing).toEqual([]);
  });

  test('the composer, the Files editor and the commit message are among them', async () => {
    const ids = (await textareas()).map(({ line }) => /\b(?:id|testId)="([^"]+)"/.exec(line)?.[1]).filter(Boolean);
    expect(ids).toEqual(expect.arrayContaining(['composer', 'file-editor', 'git-commit-message']));
  });
});
