// The inline workspace card's place and fold (T3 Code 1e2ecbd975, MIT, see LICENSE-T3:
// apps/web/src/components/chat/threadDetailsCardLayout.ts). Ported as is. This client's card has one
// density (its full content, scrolled when folded), so resolveThreadDetailsCardDensity is ported with
// its tests but the card does not yet switch to the compact or essential rows.
import type { PreviewMiniPlayerFrame } from './previewMiniPlayerLayout';
import { DETAILS_CARD_CLEARANCE } from './chat-canvas-layout';

export function resolveThreadDetailsCardDensity(height: number, content: { full: number; compact: number }) {
  if (content.full === 0 || content.full <= height) return 'full';
  if (content.compact === 0 || content.compact <= height) return 'compact';
  return 'essential';
}

/** The card's width (--thread-details-panel-width) and its gap to the canvas edges. */
export const THREAD_DETAILS_CARD_WIDTH = 280, THREAD_DETAILS_CARD_GAP = 12;

/**
 * The card pins to the top right while a readable chat lane fits beside it.
 * The chat canvas decides whether chat moves over to make room.
 */
export function resolveThreadDetailsCardLayout({
  container,
  lane,
  frame,
  overlapsDetailsCard = false,
}: {
  container: { width: number; height: number };
  lane: { padding: number; minChatWidth: number };
  frame: PreviewMiniPlayerFrame | null;
  overlapsDetailsCard?: boolean;
}) {
  const gap = THREAD_DETAILS_CARD_GAP;
  // Keep in sync with --thread-details-panel-width, which sizes the popover.
  const width = THREAD_DETAILS_CARD_WIDTH;
  const x = container.width - width - gap;
  if (x - DETAILS_CARD_CLEARANCE - lane.padding < lane.minChatWidth) return null;
  // Resizing consumes the height above the player. Dragging first tries to
  // clear the full card and folds it only when there is no readable placement.
  const height = overlapsDetailsCard && frame && frame.x + frame.width > x - gap && frame.x < x + width + gap
    ? Math.min(container.height - gap * 2, frame.y - gap * 2)
    : container.height - gap * 2;
  if (height < 160) return null;
  return { x, width, y: gap, height } as const;
}
