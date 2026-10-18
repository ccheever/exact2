// shared/threadLinks.ts at T3 Code 9c161462c9 (MIT, see LICENSE-T3).
import { str, type Obj } from '../client/domain';
import { obj, threadSnapshot } from '../client/domain';
import type { T3Client } from '../client/client';
import type { Native } from '../client/protocol';

const PREFIX = 't3-thread://v1/';
const validId = (id: string) => id.trim().length > 0;
export function parseThreadLinkHref(href: string): string | null {
  const id = href.startsWith(PREFIX) ? href.slice(PREFIX.length) : '';
  return validId(id) ? id.trim() : null;
}
export function percentDecodedThreadLinkId(id: string): string | null {
  try { const decoded = decodeURIComponent(id); return decoded !== id && validId(decoded) ? decoded : null; } catch { return null; }
}
export function formatThreadLink(id: string, label: string): string {
  return `[${label.replace(/[[\]\\\r\n]/g, ' ').replace(/\s+/g, ' ').trim() || id}](${PREFIX}${id})`;
}
const OUTSIDE_CODE = /(?<fence>(`{3,}|~{3,})[\s\S]*?(?:\2|$))|(?<span>(`+)[^\n]*?\4)|\[[^\]\n]*\]\((?<href>t3-thread:\/\/v1\/[^\s)]+)\)/g;
export function relabelThreadLinks(markdown: string, threads: Obj[]): string {
  if (!markdown.includes(`](${PREFIX}`)) return markdown;
  return markdown.replace(OUTSIDE_CODE, (source, ...args) => {
    const href = (args.at(-1) as { href?: string }).href;
    if (!href) return source;
    const written = parseThreadLinkHref(href);
    if (!written) return source;
    const thread = threads.find(thread => thread.id === written) ?? threads.find(thread => thread.id === percentDecodedThreadLinkId(written));
    return str(thread?.title).trim() ? formatThreadLink(str(thread!.id), str(thread!.title)) : source;
  });
}
/** An archived thread is absent from the index but its bounded transcript still opens. */
export async function openThreadLink(client: T3Client, native: Native, id: string): Promise<void> {
  if (client.shell.threads.some(thread => thread.id === id)) return client.openSelected(native, id);
  const snapshot = threadSnapshot(await client.restAccess(native).http(`/api/orchestration/threads/${encodeURIComponent(id)}/bounded`));
  client.projectId = str(obj(snapshot.projection.thread).projectId, client.projectId);
  client.threadId = id; client.absentThreadId = ''; client.thread = snapshot; client.diffOpen = false; client.answers = {};
  await client.openThread(native, id, true);
  client.ensureSelection();
}
/** Question answers have no context-record field: project each thread chip to its actual id. */
export function projectPendingUserInputAnswer(answer: string | string[], records: Obj[]): string | string[] {
  if (typeof answer !== 'string') return answer;
  return answer.replace(/\[([^\]\n]*)\]\(t3-context:\/\/v1\/thread\/([a-z0-9_-]{1,128})\)/gi, (source, label: string, id: string) => {
    const record = records.find(record => record.contextId === id);
    return record && str(record.threadId) ? formatThreadLink(str(record.threadId), label) : source;
  });
}
