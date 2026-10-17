// The timeline minimap, adapted from T3 Code (MIT, see LICENSE-T3):
// components/chat/timelineMinimapItems.ts (one strip per user message, its
// preview the turn's final assistant text). Which turns are on screen, the
// current one and the jump (MessagesTimeline.tsx's onSelect, 24pt under the top
// edge) are Contract's: ChatColumn in app-main.contract.
export interface MinimapRow { id: string; kind: string; body: string }
/** `row`: the turn's index in the drawn rows (data.messages), for the turn before the first visible row. */
export interface MinimapItem { id: string; index: number; label: string; excerpt: string; row: number }

const compact = (text: string) => text.replace(/\s+/g, ' ').trim();

/** deriveTimelineMinimapItems over the rows Contract draws. */
export function minimapItems(rows: readonly MinimapRow[]): MinimapItem[] {
  const items: MinimapItem[] = [];
  rows.forEach((row, index) => {
    if (row.kind !== 'user') return;
    let excerpt = '';
    for (let next = index + 1; next < rows.length; next++) {
      const candidate = rows[next]!;
      if (candidate.kind === 'user') break;
      if (candidate.kind === 'assistant') excerpt = candidate.body;
    }
    items.push({ id: row.id, index: items.length, label: compact(row.body) || 'User message', excerpt: compact(excerpt), row: index });
  });
  return items;
}
