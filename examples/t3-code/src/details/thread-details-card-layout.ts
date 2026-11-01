// The inline workspace card's place and fold (T3 Code 1e2ecbd975, MIT, see LICENSE-T3:
// apps/web/src/components/chat/threadDetailsCardLayout.ts, ported as is), and ThreadDetailsCard's
// measurement of its content per density (ThreadDetailsCard.tsx, 74541bab17).
import type { PreviewMiniPlayerFrame } from '../device/previewMiniPlayerLayout';
import { DETAILS_CARD_CLEARANCE } from '../shell/chat-canvas-layout';

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

/** The layout frame names (layout-frames.ts) the card's content reports (shell-details.contract DetailsBody, git.contract GitRows). */
export const DETAILS_FRAMES = {
  /** The whole content, scrolled or not. */
  content: 'details-content',
  /** Only `full` shows these: the Run on and workspace rows, Automations and Lineage, the Git action group at rest. */
  fullTop: 'details-full-top', fullTail: 'details-full-tail', fullGit: 'details-full-git',
  /** The Lineage and Side chats sections (inside `fullTail`), which the density does not count. */
  lineage: 'details-lineage', sideChats: 'details-side-chats',
  /** The expanded list of the thread's other pull requests (ThreadDetailsPrRows), which the density does not count either. */
  prList: 'details-pr-list',
  /** The Version Control section, whose hairline only `full` draws. */
  vcs: 'details-vcs',
  /** Mounted only while the card shows its compact rows. */
  compact: 'details-density-compact',
} as const;

/** The heights the density fits (`full` without Lineage and Side chats), and `content`: the full layout's whole height. */
export type ThreadDetailsCardHeights = { full: number; compact: number; content: number };

const frameHeight = (value: unknown): number | null =>
  Array.isArray(value) && value.length >= 4 && Number.isFinite(Number(value[3])) ? Number(value[3]) : null;

/**
 * ThreadDetailsCard's measure (`element.offsetHeight` plus the card's 1 pt borders) for the density the frames were
 * laid out in. Lineage, Side chats and the expanded pull request list scroll as they expand, so the height a density must fit
 * leaves them out: counting them would fold away the section and workspace rows when the user asks to see more of
 * them. A full layout gives both fitting heights
 * (the compact one is the content without the parts only `full` shows, where the reference lays the compact rows out
 * to measure them; the result is the same height) and the whole content, where a floating player stays below the card.
 * A compact layout gives the compact height; the essential rows are not measured, as in the reference. Frames from no
 * recognisable layout (a density switch in flight) give nothing, so a height is never kept for the wrong density.
 */
export function measureThreadDetailsCard(frames: Record<string, unknown>): Partial<ThreadDetailsCardHeights> {
  const content = frameHeight(frames[DETAILS_FRAMES.content]);
  if (content === null) return {};
  const full = content + 2;
  const prList = frameHeight(frames[DETAILS_FRAMES.prList]) ?? 0;
  const top = frameHeight(frames[DETAILS_FRAMES.fullTop]);
  if (top !== null) {
    const folded = top + (frameHeight(frames[DETAILS_FRAMES.fullTail]) ?? 0) + (frameHeight(frames[DETAILS_FRAMES.fullGit]) ?? 0)
      + (frameHeight(frames[DETAILS_FRAMES.vcs]) === null ? 0 : 1);
    const scrolling = (frameHeight(frames[DETAILS_FRAMES.lineage]) ?? 0) + (frameHeight(frames[DETAILS_FRAMES.sideChats]) ?? 0);
    return { full: full - scrolling - prList, compact: Math.max(0, full - folded - prList), content: full };
  }
  return frameHeight(frames[DETAILS_FRAMES.compact]) === null ? {} : { compact: full - prList };
}
