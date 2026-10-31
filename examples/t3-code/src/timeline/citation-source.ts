// An assistant citation's "View source" (T3 Code, MIT; see LICENSE-T3:
// components/chat/AssistantCitationChip.tsx assistantCitationNavigation, ChatView's
// citationRequest, AssistantCitationSource.tsx, and at 7c3b3b3605ef
// chat/useAssistantCitationTarget.ts and citationProcedure.ts). The cited thread opens, its
// history pages toward an answer not yet loaded, a folded turn hiding the answer opens, and the
// answer scrolls to min(120pt, a third of the viewport) under the transcript's top: this names
// the row, and the chat column places it (app-main.contract citePlace).
import type { T3Client } from '../client/client';
import { arr, obj, str } from '../client/domain';
import { ClientError, type Native } from '../client/protocol';
import type { MinimapRow } from './timeline-minimap';
import { pushToast } from '../shell/toast';
import { letGo } from '../client/let-go';
import { sleep } from '../client/command-waits';

type CitedHistoryOutcome = 'found' | 'aborted' | { title: string; description: string };
const MAX_CITATION_PAGES = 20;
const COULD_NOT_LOAD = { title: 'Could not load the cited response', description: 'Load earlier turns, then click the citation to try again. Your saved quote is unchanged.' };
const UNAVAILABLE = { title: 'The cited response is unavailable', description: 'It may have been removed. The selected text is still saved in your citation.' };
const NOT_AN_ANSWER = { title: 'The citation does not refer to an assistant response', description: 'The selected text is still saved in your citation.' };
const bare = (id: string) => id.replace(/^(?:turn-item|message):/, '');

/**
 * loadCitedHistory: pages the open thread's history toward the cited message until it is loaded, the thread runs
 * out of history, or loading fails. Each read waits for the thread's detail to settle (loaded, no page in flight).
 */
export async function loadCitedHistory(client: T3Client, native: Native, threadId: string, messageId: string, pause: () => Promise<unknown>): Promise<CitedHistoryOutcome> {
  const wanted = bare(messageId);
  for (let pages = 0; ; pages++) {
    for (let wait = 0; client.threadId === threadId && (!client.thread || client.historyLoading) && wait < 40; wait++) await pause();
    if (client.threadId !== threadId) return 'aborted';
    if (!client.thread || client.historyLoading) return COULD_NOT_LOAD;
    // Older pages merge into the visible turn items.
    const item = arr(client.projection.visibleTurnItems).map(row => obj(row.item))
      .find(entry => (entry.type === 'assistant_message' || entry.type === 'user_message') && !!wanted && (bare(str(entry.messageId)) === wanted || bare(str(entry.id)) === wanted));
    if (item) return item.type === 'assistant_message' ? 'found' : NOT_AN_ANSWER;
    if (!client.thread.hasMore) return UNAVAILABLE;
    if (pages >= MAX_CITATION_PAGES) return COULD_NOT_LOAD;
    const cursor = client.thread.historyCursor;
    try { await client.history(native, messageId); } catch (error) { if (letGo(error)) throw error; return COULD_NOT_LOAD; }
    if (client.threadId !== threadId) return 'aborted';
    // A page that moved nothing (no cursor, or the thread is gone): there is nothing older to load.
    if (client.thread?.hasMore && client.thread.historyCursor === cursor) return UNAVAILABLE;
  }
}

/**
 * The assistant row a citation names. The served client cites a message by its
 * id ("message:provider:codex:native-item:…"); this client's rows carry the
 * turn item ("turn-item:provider:codex:native-item:…", inside the row's
 * [thread, item] key), so both compare without their kind prefix.
 */
export function citedRow(rows: readonly MinimapRow[], messageId: string): number {
  const bare = (id: string) => id.replace(/^(?:turn-item|message):/, '');
  const wanted = bare(messageId);
  if (!wanted) return -1;
  const item = (id: string) => { try { const key = JSON.parse(id) as unknown; return Array.isArray(key) ? String(key[1] ?? '') : id; } catch { return id; } };
  return rows.findIndex(row => row.kind === 'assistant' && (row.id === messageId || bare(item(row.id)) === wanted));
}

/** The transcript row a citation's View source places: the cited answer's, by the list's key. */
export function citationJump(rows: readonly MinimapRow[], index: number): string {
  return rows[index]!.id;
}

/**
 * Opens the cited thread when it is another one, waits (at most ~2 s) for its
 * rows to be drawn, then asks for the cited answer's placement (`place`, the
 * row's key). An answer a turn fold hides is not drawn: `unfold` opens that fold
 * (true when it did) and the rows are read again. A citation whose answer is gone
 * leaves the thread open where it is.
 */
export async function revealCitation(client: T3Client, native: Native, threadId: string, messageId: string, rows: () => readonly MinimapRow[], place: (key: string) => void,
  unfold: (messageId: string) => boolean = () => false): Promise<string> {
  if (!threadId || !client.shell.threads.some(thread => thread.id === threadId)) throw new ClientError('Thread no longer available');
  if (threadId !== client.threadId) await client.openSelected(native, threadId);
  const outcome = await loadCitedHistory(client, native, threadId, messageId, () => sleep(180));
  if (outcome === 'aborted') return '';
  if (outcome !== 'found') { pushToast(client, { kind: 'warning', ...outcome }); return ''; }
  for (let attempt = 0; attempt < 12; attempt++) {
    if (client.threadId !== threadId) return '';
    const current = rows(), index = citedRow(current, messageId);
    // The data source reads the rows again when woken (t3.notify); a wake let go is made again by the next answer.
    if (index < 0 && unfold(messageId)) {
      try { await native.later({ op: 'wake', topic: 't3.notify' }); } catch (error) { if (letGo(error)) throw error; }
    }
    if (index >= 0) {
      place(citationJump(current, index));
      // resolveAssistantCitationRange found no quote: the answer shows unhighlighted, with the reference's warning.
      if (quoteChanged(current, index, messageId)) pushToast(client, { kind: 'warning', title: 'The quoted text has changed', description: 'Showing the source response. The saved quote is unchanged.' });
      return '';
    }
    await sleep(180);
  }
  return '';
}

const flat = (text: string) => text.replace(/[*_`~#>]/g, '').replace(/\[([^\]]*)\]\([^)]*\)/g, '$1').replace(/\s+/g, ' ').trim();
/** Whether the citation's saved quote no longer reads in its answer (the quote comes from the citing prompt's link). */
export function quoteChanged(rows: readonly MinimapRow[], index: number, messageId: string): boolean {
  const bare = messageId.replace(/^(?:turn-item|message):/, '');
  for (const row of rows) {
    for (const match of row.body.matchAll(/\(t3-citation:\/\/v1\/([^\s)]+)\)/g)) {
      try {
        const url = new URL(`t3-citation://v1/${match[1]}`), parts = url.pathname.slice(1).split('/');
        if (decodeURIComponent(parts[2] ?? '').replace(/^(?:turn-item|message):/, '') !== bare) continue;
        const quote = flat(url.searchParams.get('text') ?? '');
        return !!quote && !flat(rows[index]!.body).includes(quote);
      } catch { /* an unreadable link names nothing */ }
    }
  }
  return false;
}
