// realinput-1010g-followups (T3 Code 1e2ecbd975, MIT; see LICENSE-T3): what the real-input session realinput-1010g
// found in the shell's context menu. This file holds the contract side of RG-1; the shell's search below the hit (RG-1)
// and its system items (RG-3) are AppKit tests, macos/tests/contextmenu/realinput-1010g.swift; the inputs' autocorrect
// (RG-2) is text-entry.test.ts, the reply links (RG-4) external-link-menu.test.ts and markdown_links.rs.
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
  });
});
