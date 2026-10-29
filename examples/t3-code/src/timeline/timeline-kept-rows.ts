// The transcript's rows its virtualized list keeps mounted wherever the reader has scrolled (T3 Code 7c3b3b3605ef, MIT,
// see LICENSE-T3: MessagesTimeline's alwaysRenderKeys, 6838e31b9c): the first and last message, so a screen reader's
// heading navigation wraps from one end of a long thread to the other; a row whose sound is playing, which would lose its
// player once the window let it go; and the row of an app shown full screen. The list takes them as `keep-mounted`
// (app-main.contract ChatColumn, LLP 1010 §6.11).
import type { T3Client } from '../client/client';

type Row = { id: string; kind: string; inlineDocuments?: readonly { fullscreen: boolean }[] };

/** isMessageRow: a message, not a system row. A workflow's opening stands in its user message's place. */
const MESSAGE_KINDS = new Set(['user', 'workflow', 'assistant']);
/** The list keeps at most this many rows (LLP 1010 §6.11). */
const MAX_KEPT = 64;

/** The sounds playing on the open thread, by row: each row's chips (an attachment or a Markdown node). */
const playing = new WeakMap<T3Client, { threadId: string; rows: Map<string, Set<string>> }>();
function playingOn(client: T3Client): Map<string, Set<string>> {
  let state = playing.get(client);
  // A sound stops with its thread's transcript: the next thread starts with none.
  if (!state || state.threadId !== client.threadId) { state = { threadId: client.threadId, rows: new Map() }; playing.set(client, state); }
  return state.rows;
}

/** `chatlocal:audio-playing`: `value` is "1" or "0" (started or stopped) and the chip's id, `row` the row it sits in. */
export function noteAudioPlaying(client: T3Client, row: string, value: string): void {
  const rows = playingOn(client), chip = value.slice(1), chips = rows.get(row) ?? new Set<string>();
  if (value.startsWith('1')) chips.add(chip); else chips.delete(chip);
  if (chips.size) rows.set(row, chips); else rows.delete(row);
}

/** The keys `keep-mounted` names, in the order MessagesTimeline lists them, each once. */
export function keptTranscriptRows(client: T3Client, rows: readonly Row[]): string[] {
  const first = rows.find(row => MESSAGE_KINDS.has(row.kind));
  let last: Row | undefined;
  for (let at = rows.length - 1; at >= 0 && !last; at--) if (MESSAGE_KINDS.has(rows[at]!.kind)) last = rows[at];
  const present = new Set(rows.map(row => row.id));
  const keys = [
    ...(first ? [first.id] : []),
    ...(last && last !== first ? [last.id] : []),
    ...rows.filter(row => row.kind === 'inline-document' && row.inlineDocuments?.some(document => document.fullscreen)).map(row => row.id),
    ...[...playingOn(client).keys()].filter(row => present.has(row)),
  ];
  return [...new Set(keys)].slice(0, MAX_KEPT);
}

/** The snapshot's rows and the ones its list keeps (presentation.ts). */
export function withKeptRows<T extends Row>(client: T3Client, messages: T[]): { messages: T[]; transcriptKeep: string[] } {
  return { messages, transcriptKeep: keptTranscriptRows(client, messages) };
}
