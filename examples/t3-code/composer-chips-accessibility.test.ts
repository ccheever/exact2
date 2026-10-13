// composer-chips-accessibility (G1-8, G5-D1): a terminal excerpt's details and the context window popover's role, against
// T3 Code 1e2ecbd975 (MIT, LICENSE-T3). TerminalContextInlineChip.tsx: while the excerpt holds text its chip is
// ContextChipPopover's trigger ("Terminal excerpt, <label>. Show details"), whose details are the terminal's name, "Line N" or
// "Lines a–b", and the captured output ("Captured terminal output"). ContextWindowMeter.tsx: the meter is a PopoverTrigger
// and its details a PopoverPopup, a dialog with no name of its own. The native side (the chips' elements, the read text, the
// glyph and the press) is macos/tests/composer/chipaccessibility.swift.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import type { Obj } from './domain';
import { chipPopover, NO_CHIP, terminalPopover } from './composer-chip-popover';

const here = import.meta.dir;
const read = (file: string) => readFileSync(join(here, file), 'utf8');
const press = (extra: Obj = {}): Obj => ({ seq: 7, open: true, surface: 'composer', owner: 'draft:t1', kind: 'terminal', name: 'Terminal 2 lines 1-738',
  label: 'Terminal 2 lines 1-738', context: 'terminal_abc', frame: [300, 600, 160, 19.5], ...extra });
const record = (extra: Obj = {}): Obj => ({ version: 1, kind: 'terminal', contextId: 'terminal_abc', label: 'Terminal 2 lines 1-738', terminalId: 'term-2',
  terminalLabel: 'Terminal 2', lineStart: 1, lineEnd: 738, text: '$ ls\na.txt\nb.txt', ...extra });

describe('a terminal excerpt\'s details (G1-8)', () => {
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

  test('the window draws it in the chip popover layer with the reference\'s parts', () => {
    const contract = read('composer-chip-popover.contract');
    const branch = contract.slice(contract.indexOf('when chip.kind == "terminal"'), contract.indexOf('      else\n'));
    expect(branch).toContain('aria-label="Captured terminal output"');
    expect(branch).toContain('d="M12 19h8M4 17l6-6-6-6"'); // lucide TerminalIcon
    expect(branch).toContain('text chip.lines');
    expect(branch).toContain('max-height="20rem"'); // max-h-80
    // The layer's dialog is named by the title (PopoverTitle sr-only): "Terminal excerpt, <label>".
    expect(contract).toContain('role="dialog" aria-label=chip.title');
    // The window shows it for every press but a citation's (whose comment is its own layer).
    expect(read('app-window.contract')).toContain('when chipShown and chip.kind != "citation"');
  });

  test('the native chip names the press "terminal" with its context id, and draws the terminal glyph', () => {
    const press = read('modules/apple/T3ComposerChipPress.swift');
    expect(press).toContain('value["kind"] = "terminal"');
    expect(press).toContain('styler.popoverChip(at: point)');
    const icons = read('modules/apple/T3ComposerIcons.swift');
    expect(icons).toContain('case "terminal": return "terminal"');
    expect(icons).toContain('"terminal": ("<path d=\\"M12 19h8\\"/><path d=\\"m4 17 6-6-6-6\\"/>", 2)');
  });
});

describe('the context window popover is a dialog (G5-D1)', () => {
  const contract = read('composer-controls.contract');
  test('ComposerMenus serves the meter\'s details as an unnamed dialog, the other menus as before', () => {
    const line = contract.split('\n').find(row => row.includes('column id="composer-menu"')) ?? '';
    expect(line).toContain('role=(which == "context" ? "dialog" : which == "runtime" or which == "branch" or wideWorkspace ? "listbox" : "menu")');
    expect(line).toContain('aria-label=(which == "context" ? "" : which == "more"');
  });
  test('the meter is a PopoverTrigger: it says it opens a dialog', () => {
    const meter = contract.split('\n').find(row => row.includes('testId="context-window-meter"')) ?? '';
    expect(meter).toContain('aria-haspopup="dialog"');
  });
});

describe('a terminal reference with no record is an unavailable context (G1-8)', () => {
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
