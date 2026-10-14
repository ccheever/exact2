// adopt-main-fixes-r11: the workarounds main's fixes ended stay retired. Each read the clone's sources; the behaviour is
// proven by the AppKit tests named below and the record's agent drives (tasks/closed/20261012-adopt-main-fixes-r11.md).
import { describe, expect, test } from 'bun:test';
import { readdirSync } from 'node:fs';

const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();

describe('RT-1: what #327 retires', () => {
  test('X51: the colour popover takes no press or retainFocus of its own (a press inside a popover stops at its top layer)', async () => {
    const picker = await source('theme-color-picker.contract');
    const popover = picker.split('\n').find(line => line.includes('testId=`theme-color-${row.id}-popover`'))!;
    // Painted (LLP 1115.002 D7.3's authored fallback row); KW-6: align="end" with Base UI's flips, no app-computed side.
    expect(popover).toContain('column id=`theme-color-${row.id}` popover="auto" position-try-fallbacks="flip-block, flip-inline, flip-block flip-inline" key=popKey tabindex=-1 position="absolute" position-area="bottom span-left" margin-top="0.625rem" ');
    expect(popover).not.toMatch(/\bpress=|\bretainFocus=/);
    expect(picker).not.toContain('popPress');
  });

  test('X50: the module keeps its files where its context says (named roots under --storage), no TMPDIR re-rooting', async () => {
    const dir = new URL('./modules/apple/', import.meta.url);
    const swift = await Promise.all(readdirSync(dir).filter(name => name.endsWith('.swift')).map(name => Bun.file(new URL(name, dir)).text()));
    expect(swift.filter(text => text.includes('T3Storage'))).toEqual([]);
    const module = await source('modules/apple/T3Module.swift');
    expect(module).toContain('transport = T3Transport(persistent: !context.agent, dataDirectory: context.data,');
  });

  test('X63: Edit › Redo is the host\'s item and Edit › Undo sends undo: up the focused text\'s chain (macos/tests/r8-keys)', async () => {
    const menus = await source('modules/apple/R8KeysMenus.swift');
    expect(menus).not.toContain('textHistory');
    expect(menus).not.toContain('@objc func redo(');
    expect(menus).not.toContain('NSSelectorFromString("redo:")');
    expect(menus).toContain('_ = Self.undoTarget(from: text)?.tryToPerform(#selector(undo(_:)), with: sender)');
    // ⌘Z with no editable focus is still thread.undo, as the reference binds it (`!terminalFocus && !editableFocus`).
    expect(menus).toContain('if keystroke(), let thread = command(Self.undoTitle), fire(thread) { return }');
  });
});

describe('RT-3: the shell\'s menu takes a Control-click and VoiceOver\'s show menu (macos/tests/contextmenu)', () => {
  test('a secondary click is a right-click or a Control-click, in the monitor, the replacement and the tail', async () => {
    const menu = await source('modules/apple/T3TextContextMenu.swift');
    expect(menu).toContain('event.type == .rightMouseDown || (event.type == .leftMouseDown && event.modifierFlags.contains(.control))');
    expect(menu.match(/isSecondaryClick\(event\)/g)?.length).toBe(3); // the monitor (handle), the replacement, the tail's mouseDown
    expect(menu).toContain('guard assistive() else { return false }');
    expect(menu).toContain('var assistive: () -> Bool = { NSWorkspace.shared.isVoiceOverEnabled || NSWorkspace.shared.isSwitchControlEnabled }');
  });

  test('selected page text: ExactKit\'s menu is known by the text view it aims at (main 6a7f16c6f: AppKit\'s own menu)', async () => {
    const menu = await source('modules/apple/T3TextContextMenu.swift');
    expect(menu).toContain('static let hostTextMenuView = "SelectionMenuText"');
    expect(menu).not.toContain('lookUpSelection');
  });
});

describe('X86 (#496): the citation comment\'s textarea sits straight in its popover, as the reference\'s', () => {
  test('no box holds it at 4rem to 10rem; the textarea keeps its own min and max height', async () => {
    const lines = (await source('citation-comment.contract')).split('\n');
    const at = lines.findIndex(line => line.trimStart().startsWith('textarea id="citation-comment-text"'));
    const popover = lines.findLastIndex((line, index) => index < at && line.trimStart().startsWith('column id="chip-popover"'));
    expect(lines[at]!.length - lines[at]!.trimStart().length).toBe(lines[popover]!.length - lines[popover]!.trimStart().length + 2);
    expect(lines[at]).toContain('field-sizing="content"');
    expect(lines[at]).toContain('min-height="4rem" max-height="10rem"');
    expect(lines.some(line => line.includes('column width="100%" min-height="4rem" max-height="10rem" overflow="hidden"'))).toBe(false);
  });
});
