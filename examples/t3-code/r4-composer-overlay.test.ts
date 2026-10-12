import { describe, expect, test } from 'bun:test';
import { composerOverlay, composerOverlaySnapshot, contentKey, newOverlayHold, UNMEASURED_INSET } from './r4-composer-overlay';
import type { Obj } from './domain';

// Window-space frames [x, top, width, height] as T3ComposerFrames reports them.
const frames = (height: number, card = 118, extra: Obj = {}): Obj => ({ overlay: [276, 840 - height, 736, height], card: [276, 840 - height, 736, card], ...extra });
const input = (over: Partial<{ owner: string; frames: Obj; atEnd: boolean; pulls: number; content: string }>) =>
  ({ owner: 'o:p:t', frames: frames(170), atEnd: true, pulls: 0, content: '3:m3:10:0', ...over });

describe('composer overlay reservation (resolveComposerTimelineInset, TimelineListFooter)', () => {
  test('the transcript reserves the measured overlay plus the footer gap; unmeasured, an empty composer', () => {
    const hold = newOverlayHold();
    expect(composerOverlay(hold, input({ frames: {} })).transcriptTail).toBe(UNMEASURED_INSET + 12);
    expect(composerOverlay(hold, input({})).transcriptTail).toBe(170 + 12);
  });
  test('a taller composer while the reader is at the end does not move the transcript', () => {
    const hold = newOverlayHold();
    composerOverlay(hold, input({}));
    // Attachments and a multi-line draft: the overlay grows; the reservation waits (no follow-end jump).
    expect(composerOverlay(hold, input({ frames: frames(290, 238) })).transcriptTail).toBe(182);
    expect(composerOverlay(hold, input({ frames: frames(290, 238) })).transcriptTail).toBe(182);
    // The reader scrolls away: the end is simply further down now.
    expect(composerOverlay(hold, input({ frames: frames(290, 238), atEnd: false })).transcriptTail).toBe(302);
  });
  test('new rows, or a scroll toward the end while there, take the held reservation', () => {
    let hold = newOverlayHold();
    composerOverlay(hold, input({}));
    composerOverlay(hold, input({ frames: frames(290, 238) }));
    expect(composerOverlay(hold, input({ frames: frames(290, 238), content: '4:m4:2:0' })).transcriptTail).toBe(302);
    hold = newOverlayHold();
    composerOverlay(hold, input({}));
    composerOverlay(hold, input({ frames: frames(290, 238) }));
    expect(composerOverlay(hold, input({ frames: frames(290, 238), pulls: 1 })).transcriptTail).toBe(302);
  });
  test('a shorter composer releases the reservation at once', () => {
    const hold = newOverlayHold();
    composerOverlay(hold, input({ frames: frames(290, 238) }));
    expect(composerOverlay(hold, input({ frames: frames(170) })).transcriptTail).toBe(182);
  });
  test('a resting card keeps the expanded reservation; expanding again moves nothing', () => {
    const hold = newOverlayHold();
    composerOverlay(hold, input({ atEnd: false }));
    expect(composerOverlay(hold, input({ frames: frames(100, 48), atEnd: false })).transcriptTail).toBe(182);
    expect(composerOverlay(hold, input({ frames: frames(170), atEnd: false })).transcriptTail).toBe(182);
  });
  test('another thread rebuilds the reservation from its own composer', () => {
    const hold = newOverlayHold();
    composerOverlay(hold, input({ frames: frames(290, 238), atEnd: false }));
    expect(composerOverlay(hold, input({ owner: 'o:p:other', frames: frames(170) })).transcriptTail).toBe(182);
  });
  test('Scroll to end clears the dock banners but not a lone stash tab beside an empty dock', () => {
    const hold = newOverlayHold();
    // Overlay 200 tall from y 640: dock banner 40 tall on top.
    const banners = { overlay: [276, 640, 736, 200], card: [276, 680, 736, 118], dock: [276, 640, 700, 40], shoulder: [276, 640, 736, 40] };
    expect(composerOverlay(hold, input({ frames: banners })).scrollLift).toBe(200 + 12);
    const stash = { ...banners, dock: [276, 664, 700, 0], shoulder: [276, 640, 736, 24] };
    expect(composerOverlay(hold, input({ frames: stash })).scrollLift).toBe(176 + 12);
  });
  test('the content key changes with the rows a reader at the end sees', () => {
    expect(contentKey([])).toBe('0::0:0');
    expect(contentKey([{ id: 'a', body: 'hey' }])).toBe('1:a:3:0');
    expect(contentKey([{ id: 'a', body: 'hey', expanded: true }])).not.toBe(contentKey([{ id: 'a', body: 'hey' }]));
  });
  test('the snapshot reads T3Timeline only for its own thread', () => {
    const client = { origin: 'o', projectId: 'p', threadId: 't', presentation: { owner: 'o:p:t', atEnd: true, transcriptPulls: 0, frames: frames(170) } as Obj };
    expect(composerOverlaySnapshot(client, [{ id: 'a', body: 'x' }])).toEqual({ transcriptTail: 182, scrollLift: 182 });
    client.presentation = { ...client.presentation, frames: frames(250, 198) };
    expect(composerOverlaySnapshot(client, [{ id: 'a', body: 'x' }]).transcriptTail).toBe(182);
    client.presentation = { ...client.presentation, atEnd: false };
    expect(composerOverlaySnapshot(client, [{ id: 'a', body: 'x' }]).transcriptTail).toBe(262);
  });
});
