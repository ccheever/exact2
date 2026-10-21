// The timeline minimap, adapted from T3 Code (MIT, see LICENSE-T3):
// components/chat/timelineMinimapItems.ts at 74541bab17 (one strip per user
// message, its preview the turn's final assistant text as plain text, the
// user's text literal). Which turns are on screen, the current one and the jump
// (MessagesTimeline.tsx's onSelect, 24pt under the top edge) are Contract's:
// ChatColumn in app-main.contract.
import { minimapPreviewText } from './minimap-text.js';

export interface MinimapRow { id: string; kind: string; body: string }
/** `row`: the turn's index in the drawn rows (data.messages), for the turn before the first visible row. */
export interface MinimapItem { id: string; index: number; label: string; excerpt: string; row: number }

const compact = (text: string) => text.replace(/\s+/g, ' ').trim();

// The reference renders a preview's Markdown only when it is hovered; every snapshot here carries every preview, so each
// answer's first 1,500 units are rendered once. The cache keeps only what the current rows use, so it never outgrows
// the thread (a streaming answer replaces its own entry).
let previews = new Map<string, string>();
function previewText(markdown: string, next: Map<string, string>): string {
  const key = markdown.slice(0, 1_500);
  if (!key) return '';
  let text = next.get(key) ?? previews.get(key);
  if (text === undefined) text = compact(minimapPreviewText(key));
  next.set(key, text);
  return text;
}

/** deriveTimelineMinimapItems and resolveTimelineMinimapPreview over the rows Contract draws. */
export function minimapItems(rows: readonly MinimapRow[]): MinimapItem[] {
  const items: MinimapItem[] = [], next = new Map<string, string>();
  rows.forEach((row, index) => {
    if (row.kind !== 'user') return;
    let excerpt = '';
    for (let next = index + 1; next < rows.length; next++) {
      const candidate = rows[next]!;
      if (candidate.kind === 'user') break;
      if (candidate.kind === 'assistant') excerpt = candidate.body;
    }
    items.push({ id: row.id, index: items.length, label: compact(row.body) || 'User message', excerpt: previewText(excerpt, next), row: index });
  });
  previews = next;
  return items;
}
