import { describe, expect, test } from 'bun:test';
import { composerOverlay, composerOverlaySnapshot, contentKey, newOverlayHold, UNMEASURED_INSET } from '../../src/composer/composer-overlay';
import type { Obj } from '../../src/client/domain';
import { noteTranscriptFacts } from '../../src/timeline/transcript-facts';
import { setLayoutFrames } from '../support/layout-frames';

// Window-space frames [x, top, width, height] as app.contract measures them (layout-frames.ts).
const frames = (height: number, card = 118, extra: Obj = {}): Obj => ({ overlay: [276, 840 - height, 736, height], card: [276, 840 - height, 736, card], ...extra });
const input = (over: Partial<{ owner: string; frames: Obj; atEnd: boolean; pulls: number; content: string; rem: number }>) =>
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
  test('the reserved composer height follows the overlay and holds while the card rests', () => {
    const hold = newOverlayHold();
    expect(composerOverlay(hold, input({ frames: {} })).composerReserved).toBe(UNMEASURED_INSET);
    expect(composerOverlay(hold, input({ frames: frames(290, 238), atEnd: false })).composerReserved).toBe(290);
    // A resting card (48pt row) keeps the expanded height, so the terminal drawer's cap does not move.
    expect(composerOverlay(hold, input({ frames: frames(96, 48) })).composerReserved).toBe(290);
    expect(composerOverlay(hold, input({ frames: frames(170) })).composerReserved).toBe(170);
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
  // resolveComposerTimelineInset's expandedNoticesHeight (T3 Code 74541bab17, #20086): opened notices cover the transcript.
  test('opening and closing the stacked notices keeps the reservation, working or resting', () => {
    const hold = newOverlayHold();
    const stack = (height: number, notices: number, card = 118) => ({ ...frames(height, card), notices: [276, 840 - height, 736, notices] });
    // A working thread: every snapshot has new content, so a growth would apply at once.
    expect(composerOverlay(hold, input({ frames: stack(172, 12), content: '1' })).transcriptTail).toBe(172 + 12);
    expect(composerOverlay(hold, input({ frames: stack(532, 372), content: '2' })).transcriptTail).toBe(172 + 12);
    expect(composerOverlay(hold, input({ frames: stack(470, 372, 48), content: '3' })).transcriptTail).toBe(172 + 12);
    expect(composerOverlay(hold, input({ frames: stack(110, 12, 48), content: '4' })).transcriptTail).toBe(172 + 12);
    // A larger interface font: the collapsed peek is 0.75rem of it.
    const large = newOverlayHold();
    expect(composerOverlay(large, input({ frames: stack(531.6, 375.2), content: '1', rem: 20 })).transcriptTail).toBe(Math.ceil(531.6 - 360.2) + 12);
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
  test('the snapshot reads the transcript\'s viewport only for its own thread', () => {
    const client = { origin: 'o', projectId: 'p', threadId: 't' };
    setLayoutFrames(client, frames(170));
    noteTranscriptFacts(client, [0, '', '', '', 'o:p:t', false, true, 0]);
    expect(composerOverlaySnapshot(client, [{ id: 'a', body: 'x' }])).toEqual({ transcriptTail: 182, scrollLift: 182, composerReserved: 170 });
    setLayoutFrames(client, frames(250, 198));
    expect(composerOverlaySnapshot(client, [{ id: 'a', body: 'x' }]).transcriptTail).toBe(182);
    noteTranscriptFacts(client, [0, '', '', '', 'o:p:t', false, false, 0]);
    expect(composerOverlaySnapshot(client, [{ id: 'a', body: 'x' }]).transcriptTail).toBe(262);
  });
});
