// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/r4-composer-overlay.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// The composer overlays the transcript (ChatView's data-chat-composer-overlay,
// adapted from T3 Code, MIT; see LICENSE-T3): the composer stack is absolute at
// the chat column's foot, and the transcript keeps a measured reservation at
// its end (TimelineListFooter: the overlay's height plus 16pt) so its last row
// can scroll clear of the composer.
//
// T3ComposerFrames.swift measures the stack (`overlay`), the card, the dock
// banners and their shoulder row in window space. The reservation follows
// resolveComposerTimelineInset: while the composer rests it keeps the height it
// had expanded, so resting and expanding again never move the transcript.
//
// The reference's list never moves for a footer change (MessagesTimeline
// maintainScrollAtEnd footerLayout: false). Exact's scrollFollowEnd follows any
// growth of the list while the reader is at its end, so a reservation that
// grows while the reader is there is held back until the reader asks for more:
// they scroll away (the new end is then simply further down), the rows change
// (the reference follows new rows to the end of the grown reservation), or they
// scroll toward the end while already there (T3Timeline.swift `transcriptPulls`).
// A smaller reservation applies at once. A new thread rebuilds it.
import { obj, type Obj } from './domain';

export type OverlayHold = { owner: string; inset: number; applied: number; content: string; pulls: number };
export type ComposerOverlay = { transcriptTail: number; scrollLift: number };

/** Before the first measurement: an empty expanded composer (118pt card, 20pt foot) and its strip. */
export const UNMEASURED_INSET = 170;
/** The transcript's end below its last row's own 12pt: the overlay's 8pt top padding and the footer's 16pt, less those 12. */
const TAIL_GAP = 12;
/** ScrollToEnd sits at the transcript's foot plus 6: the reference's clearance + 4, its py-1.5 and the overlay's 8pt top padding. */
const LIFT_GAP = 12;

function frame(frames: Obj, name: string): number[] {
  const value = frames[name];
  if (!Array.isArray(value) || value.length < 4) return [];
  const numbers = value.map(Number);
  return numbers.every(Number.isFinite) ? numbers : [];
}

export function newOverlayHold(): OverlayHold { return { owner: '', inset: 0, applied: -1, content: '', pulls: 0 }; }

/** What the rows look like to a reader at the end: their count and the last one's identity and length. */
export function contentKey(rows: ReadonlyArray<unknown>): string {
  const last = obj(rows[rows.length - 1]);
  const body = typeof last.body === 'string' ? last.body.length : typeof last.text === 'string' ? last.text.length : 0;
  return `${rows.length}:${typeof last.id === 'string' ? last.id : ''}:${body}:${last.expanded === true ? 1 : 0}`;
}

/**
 * The list's end reservation (`transcriptTail`, added to the last row's padding)
 * and how far the Scroll to end pill rises (`scrollLift`).
 * `atEnd` and `pulls` come from T3Timeline; `owner` is the thread's composer.
 */
export function composerOverlay(hold: OverlayHold, input: { owner: string; frames: Obj; atEnd: boolean; pulls: number; content: string }): ComposerOverlay {
  const { owner, frames, atEnd, pulls, content } = input;
  if (hold.owner !== owner) { hold.owner = owner; hold.inset = 0; hold.applied = -1; hold.content = content; hold.pulls = pulls; }
  const overlay = frame(frames, 'overlay'), card = frame(frames, 'card'), dock = frame(frames, 'dock'), shoulder = frame(frames, 'shoulder');
  const contentChanged = content !== hold.content, pulled = pulls !== hold.pulls;
  hold.content = content; hold.pulls = pulls;
  if (!overlay.length || overlay[3]! <= 0) {
    const tail = (hold.applied >= 0 ? hold.applied : UNMEASURED_INSET) + TAIL_GAP;
    return { transcriptTail: tail, scrollLift: (hold.applied >= 0 ? hold.applied : UNMEASURED_INSET) + LIFT_GAP };
  }
  const [, top, , height] = overlay as [number, number, number, number];
  const measured = Math.ceil(height);
  // A card no taller than its 48pt resting row is resting: keep the expanded reservation.
  const resting = card.length > 0 && card[3]! > 0 && card[3]! <= 60;
  hold.inset = resting ? Math.max(hold.inset, measured) : measured;
  if (hold.applied < 0 || hold.inset <= hold.applied || !atEnd || contentChanged || pulled) hold.applied = hold.inset;
  // resolveScrollToEndClearance: the pill clears the dock banners, which span the lane, but not
  // the stash's shoulder tab beside an empty dock (it is off to the side of the centred pill).
  const bottom = top + height;
  const clearTop = dock.length && dock[3]! > 0.5 ? top : shoulder.length ? shoulder[1]! + shoulder[3]! : top;
  return { transcriptTail: hold.applied + TAIL_GAP, scrollLift: Math.ceil(bottom - clearTop) + LIFT_GAP };
}

const holds = new WeakMap<object, OverlayHold>();
/** The snapshot's fields for one client (presentation.ts). */
export function composerOverlaySnapshot(client: { origin: string; projectId: string; threadId: string; presentation: Obj }, rows: ReadonlyArray<unknown>): ComposerOverlay {
  let hold = holds.get(client);
  if (!hold) { hold = newOverlayHold(); holds.set(client, hold); }
  const owner = `${client.origin}:${client.projectId}:${client.threadId}`;
  const presentation = client.presentation, mine = presentation.owner === owner;
  return composerOverlay(hold, { owner, frames: obj(presentation.frames), atEnd: !mine || presentation.atEnd !== false,
    pulls: Number(presentation.transcriptPulls) || 0, content: contentKey(rows) });
}
