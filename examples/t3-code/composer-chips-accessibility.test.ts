// A terminal excerpt's details and the context window popover's role, against
// T3 Code 1e2ecbd975 (MIT, LICENSE-T3). TerminalContextInlineChip.tsx: while the excerpt holds text its chip is
// ContextChipPopover's trigger ("Terminal excerpt, <label>. Show details"), whose details are the terminal's name, "Line N" or
// "Lines a–b", and the captured output ("Captured terminal output"). ContextWindowMeter.tsx: the meter is a PopoverTrigger
// and its details a PopoverPopup, a dialog with no name of its own. The native side (the chips' elements, the read text, the
// glyph and the press) is macos/tests/composer/chipaccessibility.swift. The rendered app popovers are covered by
// macos/src/composer_accessibility_tests.rs.
import { describe, expect, test } from 'bun:test';
import type { Obj } from './domain';
import { chipPopover, NO_CHIP, terminalPopover } from './composer-chip-popover';

const press = (extra: Obj = {}): Obj => ({ seq: 7, open: true, surface: 'composer', owner: 'draft:t1', kind: 'terminal', name: 'Terminal 2 lines 1-738',
  label: 'Terminal 2 lines 1-738', context: 'terminal_abc', frame: [300, 600, 160, 19.5], ...extra });
const record = (extra: Obj = {}): Obj => ({ version: 1, kind: 'terminal', contextId: 'terminal_abc', label: 'Terminal 2 lines 1-738', terminalId: 'term-2',
  terminalLabel: 'Terminal 2', lineStart: 1, lineEnd: 738, text: '$ ls\na.txt\nb.txt', ...extra });

describe('a terminal excerpt\'s details', () => {
  test('the terminal\'s name, its lines and the captured output, at the chip', () => {
    expect(chipPopover(press(), [], undefined, { terminal_abc: record() })).toEqual({ ...NO_CHIP, seq: 7, open: true, surface: 'composer', kind: 'terminal',
      label: 'Terminal 2', title: 'Terminal excerpt, Terminal 2 lines 1-738', description: '$ ls\na.txt\nb.txt', lines: 'Lines 1–738', rows: 3,
      x: 300, y: 600, width: 160, height: 19.5 });
    // One line reads "Line N" (TerminalContextInlineChip: `lineStart === lineEnd`).
    expect(terminalPopover(press(), record({ lineStart: 4, lineEnd: 4 })).lines).toBe('Line 4');
  });

  test('no record, another record, or an expired excerpt (no text) shows nothing but keeps the sequence', () => {
    expect(chipPopover(press(), [], undefined, {})).toEqual({ ...NO_CHIP, seq: 7 });
    expect(chipPopover(press(), [], undefined, { terminal_abc: record({ kind: 'file' }) }).open).toBe(false);
    expect(chipPopover(press(), [], undefined, { terminal_abc: record({ text: '' }) }).open).toBe(false);
  });
});

describe('a terminal reference with no record is an unavailable context', () => {
  test('only a reference the draft holds a record for draws as a terminal excerpt', async () => {
    const { chipContexts } = await import('./composer-editor');
    const text = '[Terminal 2 lines 1-738](t3-context://v1/terminal/terminal_abc) [Terminal 1 lines 1-2](t3-context://v1/terminal/terminal_lane1) now ';
    const client = { local: { terminalContexts: { terminal_abc: record() } }, draft: text } as unknown as Parameters<typeof chipContexts>[0];
    const contexts = chipContexts(client, text);
    expect(contexts['terminal/terminal_abc']).toBe('terminal\t\t$ ls\na.txt\nb.txt');
    // composerContextPresentation's fallback (UnresolvedContextChip): no entry, so the native chip is "Unavailable context".
    expect(contexts['terminal/terminal_lane1']).toBeUndefined();
  });
});


test('native pull request contexts keep the state and full title', async () => {
  const { chipContexts, rememberReviewCommentRecord } = await import('./composer-editor');
  const text = '[#12](t3-context://v1/review-comment/pr12)';
  const client = { local: {}, draft: text } as unknown as Parameters<typeof chipContexts>[0];
  for (const [state, isDraft, kind] of [['open', false, 'open'], ['open', true, 'draft'], ['merged', false, 'merged'], ['closed', false, 'closed']] as const) {
    rememberReviewCommentRecord(client, { contextId: 'pr12', pullRequest: { state, isDraft, title: 'Fix tabs\tand Unicode 한글' } });
    expect(chipContexts(client, text)['review-comment/pr12']).toBe(`pr-${kind}\t\tFix tabs\tand Unicode 한글`);
  }
});
