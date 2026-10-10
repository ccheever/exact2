// realinput-1010g-followups (T3 Code 1e2ecbd975, MIT; see LICENSE-T3): what the real-input session realinput-1010g
// found in the shell's context menu. The AppKit side is macos/tests/contextmenu/realinput-1010g.swift; the inputs'
// autocorrect is text-entry.test.ts, the reply links external-link-menu.test.ts.
import { describe, test, expect } from 'bun:test';

const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();

describe('RG-1: the work group\'s tool icon takes its right-click', () => {
  test('the header\'s hidden timestamp takes no pointer, as TimelineRowTimestamp\'s pointer-events-none', async () => {
    // MessagesTimeline.tsx TimelineRowTimestamp: `pointer-events-none absolute … opacity-0`, and on the row's hover or
    // focus `pointer-events-auto static opacity-100`. The clone's absolute box sat over the row's first icon, so a
    // right-click there hit the invisible timestamp and the shell's menu had no Copy Image.
    const timeline = await source('timeline.contract');
    const stamp = timeline.split('\n').find(line => line.includes('box position=(hovering ? "relative" : "absolute") opacity=(hovering ? 1 : 0)'));
    expect(stamp).toContain('pointer-events=(hovering ? "auto" : "none")');
    // The work row's own timestamp (shell-tip.contract) already did.
    expect(await source('shell-tip.contract')).toContain('opacity=(visible ? 1 : 0) pointer-events=(visible ? "auto" : "none")');
  });

  test('the icon\'s hook box lets presses through to the row; the shell finds the icon below the node it hits', async () => {
    const icons = await source('timeline-icons.contract');
    expect(icons).toMatch(/box position="absolute"[^\n]* pointer-events="none"[^\n]* hatch="t3-tool-icon"/);
    const menu = await source('modules/apple/T3TextContextMenu.swift');
    expect(menu).toContain('static func drawnImage(at location: NSPoint, in view: NSView) -> CGImage?');
    expect(menu).toContain('if let image = drawnImage(at: event.locationInWindow, in: hit) { return image }');
  });
});

describe('RG-3: the shell\'s menus carry no system items', () => {
  test('Services, AutoFill and Writing Tools are off on the shell\'s menu and on WebKit\'s before it pops', async () => {
    const menu = await source('modules/apple/T3TextContextMenu.swift');
    expect(menu).toMatch(/static func withoutSystemItems\(_ menu: NSMenu\) \{\n\s+menu\.allowsContextMenuPlugIns = false\n\s+if #available\(macOS 15\.2, \*\) \{ menu\.automaticallyInsertsWritingToolsItems = false \}/);
    const web = await source('modules/apple/T3ShellWebView.swift');
    expect(web).toContain('NSMenu.didAddItemNotification');
    expect(web).toContain('if window != nil { T3ShellWebMenu.keepSystemItemsOut() }');
  });
});
