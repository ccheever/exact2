// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/r5-composer-citation.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// r5-composer: an assistant citation's "View source" (T3 Code, MIT; see LICENSE-T3:
// components/chat/AssistantCitationChip.tsx assistantCitationNavigation, ChatView's
// citationRequest and AssistantCitationSource.tsx). The cited thread opens and its
// answer scrolls to min(120pt, a third of the viewport) under the transcript's top,
// measured from the answer's own row (timeline.contract registers each assistant
// row as a `cite:` jump target for modules/apple/T3TimelineTurns.swift).
import type { T3Client } from './client';
import { ClientError, type Native } from './protocol';
import type { MinimapRow } from './timeline-minimap';
import { pushToast } from './toast';

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

/** The jump request for row `index` (padding-top 2 separates the hooked row from its Markdown). */
export function citationJump(rows: readonly MinimapRow[], index: number) {
  return { op: 'timelineJump', id: `cite:${rows[index]!.id}`, index, count: rows.length, lead: 'citation', inset: 2 };
}

/**
 * Opens the cited thread when it is another one, waits (at most ~2 s) for its
 * rows to be drawn, then brings the cited answer into view. A citation whose
 * answer is gone leaves the thread open where it is.
 */
export async function revealCitation(client: T3Client, native: Native, threadId: string, messageId: string, rows: () => readonly MinimapRow[]): Promise<string> {
  if (!threadId || !client.shell.threads.some(thread => thread.id === threadId)) throw new ClientError('Thread no longer available');
  const access = client.restAccess(native);
  if (threadId !== client.threadId) await client.openSelected(native, threadId);
  for (let attempt = 0; attempt < 12; attempt++) {
    if (client.threadId !== threadId) return '';
    const current = rows(), index = citedRow(current, messageId);
    if (index >= 0) {
      await access.call(citationJump(current, index));
      // resolveAssistantCitationRange found no quote: the answer shows unhighlighted, with the reference's warning.
      if (quoteChanged(current, index, messageId)) pushToast(client, { kind: 'warning', title: 'The quoted text has changed', description: 'Showing the source response. The saved quote is unchanged.' });
      return '';
    }
    await access.call({ op: 'timelineSleep', ms: 180 }).catch(() => undefined);
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
