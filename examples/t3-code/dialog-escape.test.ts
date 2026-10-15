// Escape over stacked popups. The Mac host gives a key to the first `aria-keyshortcuts` button it finds by node id
// (host/apple ShortcutsMac.swift `nodes()`), so a popup mounted earlier takes Escape from a dialog mounted over it unless
// it gives up its shortcut while that dialog shows. Base UI closes only the topmost popup. Read from the Contract
// sources; the keys are driven on macOS.
import { describe, expect, test } from 'bun:test';

const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
const line = (text: string, needle: string) => text.split('\n').find(candidate => candidate.includes(needle)) ?? '';

describe('the thread details card gives Escape to the dialogs it opens', () => {
  test('a git dialog or the Merge confirmation keeps Escape from the card\'s backdrop', async () => {
    const card = await source('shell-details.contract');
    expect(line(card, 'testId="thread-details-backdrop"')).toContain('aria-keyshortcuts=(details.git.dialog != "" or details.prRows.merge.open ? "" : "Escape")');
    // The dialogs' own Escape buttons.
    expect(line(await source('r4-git.contract'), 'testId=`${testId}-backdrop`')).toContain('aria-keyshortcuts="Escape"');
    expect(line(await source('r6-pr.contract'), 'testId="pr-merge-cancel"')).toContain('aria-keyshortcuts="Escape"');
  });
});
