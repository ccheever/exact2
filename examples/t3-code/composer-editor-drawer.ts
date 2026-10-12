// ComposerCommandMenuLayer (ChatComposer.tsx): the command and stash drawers
// are a fixed layer over the chat, not part of the composer's flow, so opening
// one never resizes the transcript above it. The layer sits on the chat column
// (T3ComposerFrames.swift measures it as `chat`) with its bottom edge on the
// card's top (`card`); the drawer's own 17pt seam reaches under the card.
import { obj, type Obj } from './domain';

export type ComposerDrawer = { placed: boolean; left: number; width: number; bottom: number; listMax: number; cardX: number; cardY: number; cardWidth: number; cardHeight: number; hold: number };
/** Until both frames are measured the drawer stays in the composer's flow. */
export const NO_DRAWER: ComposerDrawer = { placed: false, left: 0, width: 0, bottom: 0, listMax: 288, cardX: 0, cardY: 0, cardWidth: 0, cardHeight: 0, hold: 0 };
const SEAM = 17; // --chat-composer-attachment-overlap: calc(1rem + 1px)

function frame(frames: Obj, name: string): number[] {
  const value = frames[name];
  if (!Array.isArray(value) || value.length < 4) return [];
  const numbers = value.map(Number);
  return numbers.every(Number.isFinite) ? numbers : [];
}

/** Where the layer goes in the chat column, from the window-space frames of the column and the card. */
export function composerDrawer(presentation: Obj): ComposerDrawer {
  const frames = obj(presentation.frames);
  const card = frame(frames, 'card'), chat = frame(frames, 'chat');
  if (!card.length) return NO_DRAWER;
  const [cardX, cardTop, cardWidth, cardHeight] = card as [number, number, number, number];
  const unplaced = { ...NO_DRAWER, cardX, cardY: cardTop, cardWidth: Math.max(0, cardWidth), cardHeight: Math.max(0, cardHeight) };
  if (!chat.length) return unplaced;
  const [chatX, chatTop, chatWidth, chatHeight] = chat as [number, number, number, number];
  if (cardWidth <= 0 || chatWidth <= 0 || chatHeight <= 0 || cardTop < chatTop || cardTop > chatTop + chatHeight) return unplaced;
  // maxHeight: max(96, rect.top - 24 + overlap) from the window top, less the seam and the drawer's top border; the list caps at max-h-72.
  const room = Math.max(96, cardTop - chatTop - 24 + SEAM) - SEAM - 1;
  return { ...unplaced, placed: true, left: cardX - chatX, width: cardWidth, bottom: chatTop + chatHeight - cardTop, listMax: Math.min(288, room) };
}

/**
 * resolveComposerTimelineInset: while the composer rests, its stack keeps the
 * height it had expanded, so resting and expanding again (reaching the end, a
 * fold opening there) never resizes the transcript. The reservation belongs to
 * one thread's composer (`owner`) and is rebuilt from that thread's own
 * expanded stack. A card taller than its 48pt resting row is expanded.
 */
export type StackHold = { owner: string; height: number };
export function composerStackHold(hold: StackHold, owner: string, presentation: Obj): number {
  const frames = obj(presentation.frames);
  const card = frame(frames, 'card'), stack = frame(frames, 'stack');
  if (hold.owner !== owner) { hold.owner = owner; hold.height = 0; }
  if (card.length && stack.length && card[3]! > 60 && stack[3]! > 0) hold.height = stack[3]!;
  return hold.height;
}
