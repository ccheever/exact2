import { describe, expect, test } from 'bun:test';
import { composerDrawer, composerStackHold, NO_DRAWER, type StackHold } from './composer-editor-drawer';

describe('ComposerCommandMenuLayer placement', () => {
  test('the layer sits card-wide on the chat column, its bottom edge on the card top', () => {
    // 840×620 window, sidebar 208pt: the chat column fills the rest; the card's top is at 457.
    const drawer = composerDrawer({ frames: { chat: [208, 0, 632, 620], card: [228, 457, 592, 144] } });
    expect(drawer).toEqual({ placed: true, left: 20, width: 592, bottom: 163, listMax: 288, cardX: 228, cardY: 457, cardWidth: 592, cardHeight: 144, hold: 0 });
  });
  test('a short window caps the list at the room above the card (at least 96pt for the layer)', () => {
    expect(composerDrawer({ frames: { chat: [0, 0, 600, 400], card: [20, 220, 560, 144] } }).listMax).toBe(195);
    expect(composerDrawer({ frames: { chat: [0, 0, 600, 300], card: [20, 40, 560, 144] } }).listMax).toBe(78);
  });
  test('until both frames are measured the drawer stays in the flow', () => {
    expect(composerDrawer({})).toEqual(NO_DRAWER);
    // The card's own frame is still known, for the context-drop ring.
    expect(composerDrawer({ frames: { card: [228, 457, 592, 144] } })).toEqual({ ...NO_DRAWER, cardX: 228, cardY: 457, cardWidth: 592, cardHeight: 144 });
    expect(composerDrawer({ frames: { chat: [208, 0, 632, 620], card: [228, 457, 0, 0] } }).placed).toBe(false);
    expect(composerDrawer({ frames: { chat: [208, 0, 632, 620], card: [228, 700, 592, 144] } }).placed).toBe(false);
  });
  test('resting keeps the stack height the composer had expanded, per thread', () => {
    const hold: StackHold = { owner: '', height: 0 };
    // Expanded: a 144pt card in a 176pt stack.
    expect(composerStackHold(hold, 't1', { frames: { card: [228, 444, 592, 144], stack: [208, 444, 632, 176] } })).toBe(176);
    // Resting (48pt card): the reservation stands while the stack is held.
    expect(composerStackHold(hold, 't1', { frames: { card: [228, 520, 592, 48], stack: [208, 444, 632, 176] } })).toBe(176);
    // A taller expanded stack (the branch strip) updates it; another thread starts over.
    expect(composerStackHold(hold, 't1', { frames: { card: [228, 412, 592, 144], stack: [208, 412, 632, 208] } })).toBe(208);
    expect(composerStackHold(hold, 't2', { frames: { card: [228, 520, 592, 48], stack: [208, 508, 632, 112] } })).toBe(0);
  });
});
