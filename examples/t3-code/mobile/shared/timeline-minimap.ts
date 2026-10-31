// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/timeline-minimap.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// The timeline minimap, adapted from T3 Code (MIT, see LICENSE-T3):
// components/chat/timelineMinimapItems.ts (one strip per user message, its
// preview the turn's final assistant text), MessagesTimeline.logic.ts
// (resolveTimelineMinimapCurrentIndex) and MessagesTimeline.tsx's
// onSelect, which scrolls the turn 24pt under the top edge. Which turns are on
// screen comes from the native transcript (modules/apple/T3TimelineTurns.swift).
import { str, type Obj } from './domain';
import { ClientError, type Native } from './protocol';
import type { T3Client } from './client';

export interface MinimapRow { id: string; kind: string; body: string }
export interface MinimapItem { id: string; index: number; label: string; excerpt: string; inView: boolean; row: number }

const compact = (text: string) => text.replace(/\s+/g, ' ').trim();

/** deriveTimelineMinimapItems over the rows Contract draws, with the native in-view set. */
export function minimapItems(rows: readonly MinimapRow[], inView: ReadonlySet<string>): MinimapItem[] {
  const items: MinimapItem[] = [];
  rows.forEach((row, index) => {
    if (row.kind !== 'user') return;
    let excerpt = '';
    for (let next = index + 1; next < rows.length; next++) {
      const candidate = rows[next]!;
      if (candidate.kind === 'user') break;
      if (candidate.kind === 'assistant') excerpt = candidate.body;
    }
    items.push({ id: row.id, index: items.length, label: compact(row.body) || 'User message', excerpt: compact(excerpt), inView: inView.has(row.id), row: index });
  });
  return items;
}

/** The reader's turn: the first strip in view, else the last one above the viewport (reported natively). */
export function minimapCurrent(items: readonly MinimapItem[], above: string): number {
  const visible = items.find(item => item.inView);
  if (visible) return visible.index;
  return items.find(item => item.id === above)?.index ?? -1;
}

/** The status the native transcript reports for this thread's rows. */
export function nativeTurns(client: T3Client): { inView: Set<string>; above: string } {
  const presentation: Obj = client.presentation;
  const ids = Array.isArray(presentation.turnsInView) ? presentation.turnsInView.filter((id): id is string => typeof id === 'string') : [];
  return { inView: new Set(ids), above: str(presentation.turnAbove) };
}

/** onSelect: bring the turn's row to 24pt below the top of the transcript. */
export async function jumpToTurn(client: T3Client, native: Native, id: string, value: string, rows: readonly MinimapRow[]): Promise<string> {
  // The chevrons name a strip by its position; the strips by their row.
  const target = id || (minimapItems(rows, new Set())[Number(value)]?.id ?? '');
  const index = rows.findIndex(row => row.id === target && row.kind === 'user');
  if (index < 0) throw new ClientError('That message is no longer in this thread.');
  await client.restAccess(native).call({ op: 'timelineJump', id: target, index, count: rows.length });
  return '';
}
