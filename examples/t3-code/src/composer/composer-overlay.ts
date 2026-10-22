// The composer overlays the transcript (ChatView's data-chat-composer-overlay,
// adapted from T3 Code, MIT; see LICENSE-T3): the composer stack is absolute at
// the chat column's foot, and the transcript keeps a measured reservation at
// its end (TimelineListFooter: the overlay's height plus 16pt) so its last row
// can scroll clear of the composer.
//
// The layout frames (layout-frames.ts) hold the stack (`overlay`), the card, the
// dock banners and their shoulder row in window space. The reservation follows
// resolveComposerTimelineInset: while the composer rests it keeps the height it
// had expanded, so resting and expanding again never move the transcript, and
// notices opened from the stack's peek (ComposerDock's `notices` frame, 0.75rem
// collapsed) cover the transcript instead of growing it (T3 Code 74541bab17:
// ChatView's expandedNoticesHeight).
//
// The reference's list never moves for a footer change (MessagesTimeline
// maintainScrollAtEnd footerLayout: false). Exact's scrollFollowEnd follows any
// growth of the list while the reader is at its end, so a reservation that
// grows while the reader is there is held back until the reader asks for more:
// they scroll away (the new end is then simply further down), the rows change
// (the reference follows new rows to the end of the grown reservation), or they
// scroll toward the end while already there (transcript-facts.ts `pulls`).
// A smaller reservation applies at once. A new thread rebuilds it.
import { obj, type Obj } from '../client/domain';
import { transcriptFacts } from '../timeline/transcript-facts';
import { layoutFrames } from '../shell/layout-frames';
import { clampInterfaceFontSize } from '../appearance/appearance-fonts';

export type OverlayHold = { owner: string; inset: number; applied: number; content: string; pulls: number };
/**
 * `composerReserved`: the composer's height the chat column keeps for it (ChatView composerReservedHeight), held through
 * a resting card as the reservation is. The terminal drawer's height cap and the empty timeline's centring read it.
 */
export type ComposerOverlay = { transcriptTail: number; scrollLift: number; composerReserved: number };

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
 * `atEnd` and `pulls` come from the window's reading of the transcript (transcript-facts.ts); `owner` is the thread's composer.
 */
export function composerOverlay(hold: OverlayHold, input: { owner: string; frames: Obj; atEnd: boolean; pulls: number; content: string; rem?: number }): ComposerOverlay {
  const { owner, frames, atEnd, pulls, content, rem = 16 } = input;
  if (hold.owner !== owner) { hold.owner = owner; hold.inset = 0; hold.applied = -1; hold.content = content; hold.pulls = pulls; }
  const overlay = frame(frames, 'overlay'), card = frame(frames, 'card'), dock = frame(frames, 'dock'), shoulder = frame(frames, 'shoulder'), notices = frame(frames, 'notices');
  const contentChanged = content !== hold.content, pulled = pulls !== hold.pulls;
  hold.content = content; hold.pulls = pulls;
  if (!overlay.length || overlay[3]! <= 0) {
    const tail = (hold.applied >= 0 ? hold.applied : UNMEASURED_INSET) + TAIL_GAP;
    return { transcriptTail: tail, scrollLift: (hold.applied >= 0 ? hold.applied : UNMEASURED_INSET) + LIFT_GAP, composerReserved: hold.inset || UNMEASURED_INSET };
  }
  const [, top, , height] = overlay as [number, number, number, number];
  // resolveComposerTimelineInset: the opened notices' growth comes off before rounding.
  const expandedNotices = notices.length ? Math.max(0, notices[3]! - 0.75 * rem) : 0;
  const measured = Math.ceil(height - expandedNotices);
  // A card no taller than its 48pt resting row is resting: keep the expanded reservation.
  const resting = card.length > 0 && card[3]! > 0 && card[3]! <= 60;
  hold.inset = resting ? Math.max(hold.inset, measured) : measured;
  if (hold.applied < 0 || hold.inset <= hold.applied || !atEnd || contentChanged || pulled) hold.applied = hold.inset;
  // resolveScrollToEndClearance: the pill clears the dock banners, which span the lane, but not
  // the stash's shoulder tab beside an empty dock (it is off to the side of the centred pill).
  const bottom = top + height;
  const clearTop = dock.length && dock[3]! > 0.5 ? top : shoulder.length ? shoulder[1]! + shoulder[3]! : top;
  return { transcriptTail: hold.applied + TAIL_GAP, scrollLift: Math.ceil(bottom - clearTop) + LIFT_GAP, composerReserved: hold.inset };
}

const holds = new WeakMap<object, OverlayHold>();
/** The snapshot's fields for one client (presentation.ts). */
export function composerOverlaySnapshot(client: { origin: string; projectId: string; threadId: string; local?: unknown }, rows: ReadonlyArray<unknown>): ComposerOverlay {
  let hold = holds.get(client);
  if (!hold) { hold = newOverlayHold(); holds.set(client, hold); }
  const owner = `${client.origin}:${client.projectId}:${client.threadId}`;
  const transcript = transcriptFacts(client), mine = transcript.owner === owner;
  return composerOverlay(hold, { owner, frames: layoutFrames(client), atEnd: !mine || transcript.atEnd,
    pulls: transcript.pulls, content: contentKey(rows), rem: clampInterfaceFontSize(obj(obj(client.local).clientSettings).fontSizeInterface) });
}
