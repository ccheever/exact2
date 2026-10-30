// shared/threadLinks.ts and chat/MarkdownThreadLink.tsx at T3 Code v0.1.5 (MIT, see LICENSE-T3).
import { str, type Obj } from '../client/domain';
import { obj, threadSnapshot } from '../client/domain';
import type { T3Client } from '../client/client';
import type { Native } from '../client/protocol';
import { holdThreadDiff, reopenThreadDiff } from '../panel/panel-surfaces';
import { fleet, fleetThreadId, focusFleetThread, parseFleetThreadId, type EnvironmentFleet } from '../connections/settings-b-fleet';
import { contextLinks, contextTarget, replaceLinks } from '../composer/context-links';

const V1_PREFIX = 't3-thread://v1/';
const V2_PREFIX = 't3-thread://v2/';
const validId = (id: string) => id.trim().length > 0;

/** A v1 link names no environment; the message's own, or a unique match, stands in. */
export interface ThreadLink { environmentId: string | null; threadId: string }

/** The ids as written. Thread ids can hold percent escapes of their own, so they are not decoded. */
export function parseThreadLinkHref(href: string): ThreadLink | null {
  if (href.startsWith(V1_PREFIX)) {
    const threadId = href.slice(V1_PREFIX.length);
    return validId(threadId) ? { environmentId: null, threadId: threadId.trim() } : null;
  }
  if (!href.startsWith(V2_PREFIX)) return null;
  const path = href.slice(V2_PREFIX.length), slash = path.indexOf('/');
  if (slash < 0) return null;
  const environmentId = path.slice(0, slash), threadId = path.slice(slash + 1);
  return validId(environmentId) && validId(threadId) ? { environmentId: environmentId.trim(), threadId: threadId.trim() } : null;
}
export function percentDecodedThreadLinkId(id: string): string | null {
  try { const decoded = decodeURIComponent(id); return decoded !== id && validId(decoded) ? decoded : null; } catch { return null; }
}
/** The ids a link can name: as written, then decoded. */
const linkIds = (link: ThreadLink) => {
  const decoded = percentDecodedThreadLinkId(link.threadId);
  return decoded === null ? [link.threadId] : [link.threadId, decoded];
};
function threadLinkHref(link: ThreadLink): string {
  return link.environmentId === null ? `${V1_PREFIX}${link.threadId}` : `${V2_PREFIX}${link.environmentId}/${link.threadId}`;
}
/** Without a label, an autolink: clients show the thread's current title. */
export function formatThreadLink(link: ThreadLink, label?: string): string {
  const cleaned = label?.replace(/[[\]\\\r\n]/g, ' ').replace(/\s+/g, ' ').trim();
  return cleaned ? `[${cleaned}](${threadLinkHref(link)})` : `<${threadLinkHref(link)}>`;
}
export function hasThreadLinks(markdown: string): boolean { return markdown.includes('t3-thread://v'); }

/** "Thread on another machine" for a link to another environment, else "Thread no longer available". */
export function unknownThreadLinkLabel(link: ThreadLink, messageEnvironmentId: string): string {
  return link.environmentId === null || link.environmentId === messageEnvironmentId ? 'Thread no longer available' : 'Thread on another machine';
}

// Code comes first in the alternation so a link written inside a code span or fence is skipped.
const OUTSIDE_CODE = /(?<fence>(`{3,}|~{3,})[\s\S]*?(?:\2|$))|(?<span>(`+)[^\n]*?\4)|\[(?<label>[^\]\n]*)\]\((?<href>t3-thread:\/\/v[12]\/[^\s)]+)\)|<(?<autolink>t3-thread:\/\/v[12]\/[^\s<>]+)>/g;
type LinkGroups = { href?: string; autolink?: string; label?: string };

/**
 * The thread links outside code, each with its written label: null for an autolink, whose text is its own URL and so
 * no label (ChatMarkdown's `a` renderer), and for a link written with no text.
 */
export function threadLinksIn(markdown: string): { href: string; label: string | null }[] {
  if (!hasThreadLinks(markdown)) return [];
  const links: { href: string; label: string | null }[] = [];
  // A replace, not matchAll: Hermes's matchAll finds nothing with this expression's backreferences.
  markdown.replace(OUTSIDE_CODE, (source, ...args) => {
    const groups = args.at(-1) as LinkGroups, href = groups.href ?? groups.autolink;
    if (href === undefined || parseThreadLinkHref(href) === null) return source;
    const written = groups.label?.trim() ?? '';
    links.push({ href, label: written && parseThreadLinkHref(written) === null ? written : null });
    return source;
  });
  return links;
}

/**
 * Writes each link the message's environment knows with its thread's current title, as the chip shows it, for the
 * text a message copies and a plan's title. Any other link stays: its chip resolves it through the connected
 * environments, or says why it cannot. An autolink that stays is written as a link whose text is its URL, which
 * CommonMark draws it as (§6.5); the Markdown parser's own autolinks are http(s) and mail only.
 */
export function relabelThreadLinks(markdown: string, threads: Obj[], messageEnvironmentId = ''): string {
  if (!hasThreadLinks(markdown)) return markdown;
  return markdown.replace(OUTSIDE_CODE, (source, ...args) => {
    const groups = args.at(-1) as LinkGroups, href = groups.href ?? groups.autolink;
    const written = href === undefined ? null : parseThreadLinkHref(href);
    if (!written) return source;
    const kept = groups.autolink === undefined ? source : `[${groups.autolink}](${groups.autolink})`;
    if (written.environmentId !== null && written.environmentId !== messageEnvironmentId) return kept;
    const thread = linkIds(written).map(id => threads.find(candidate => candidate.id === id)).find(Boolean);
    const title = str(thread?.title).trim();
    return title ? formatThreadLink({ environmentId: written.environmentId, threadId: str(thread!.id) }, title) : kept;
  });
}
export interface ThreadLinkTarget {
  /** What select-thread opens: the thread's id here, or its qualified id in another connected environment. */
  id: string; threadId: string; thread: Obj; project: Obj | undefined; remote: boolean;
  /** The environment the thread lives in. */
  environmentId: string;
}
/**
 * MarkdownThreadLink's thread: the written id, then its percent-decoded form, in the link's environment (a v1 link's is
 * the message's); a v1 link this environment lacks opens the one thread of that id across the connected environments
 * (guessThreadLinkRef: none when two share it). A v2 link is never looked for elsewhere.
 */
export function resolveThreadLink(client: T3Client, link: ThreadLink, source: EnvironmentFleet = fleet): ThreadLinkTarget | null {
  const ids = linkIds(link), environmentId = link.environmentId ?? client.environmentId;
  const entries = [...source.entries.values()].filter(entry => entry.phase === 'connected' && entry.synchronized === entry.generation && entry.environmentId !== client.environmentId);
  const remote = (entry: (typeof entries)[number], thread: Obj, id: string): ThreadLinkTarget => ({ id: fleetThreadId(entry.environmentId, id), threadId: id, thread, remote: true,
    environmentId: entry.environmentId, project: entry.shell.projects.find(project => project.id === thread.projectId) });
  if (environmentId === client.environmentId) {
    for (const id of ids) {
      const thread = client.shell.threads.find(candidate => candidate.id === id);
      if (thread) return { id, threadId: id, thread, project: client.shell.projects.find(project => project.id === thread.projectId), remote: false, environmentId: client.environmentId };
    }
  } else {
    const entry = entries.find(candidate => candidate.environmentId === environmentId);
    for (const id of entry ? ids : []) {
      const thread = entry!.shell.threads.find(candidate => str(candidate.id) === id);
      if (thread) return remote(entry!, thread, id);
    }
  }
  if (link.environmentId !== null) return null;
  for (const id of ids) {
    const matches = entries.flatMap(entry => entry.shell.threads.filter(thread => str(thread.id) === id).map(thread => ({ entry, thread })));
    if (matches.length > 1) return null;
    const match = matches[0];
    if (match) return remote(match.entry, match.thread, id);
  }
  return null;
}
/** An archived thread is absent from the index but its bounded transcript still opens. */
export async function openThreadLink(client: T3Client, native: Native, id: string): Promise<void> {
  if (client.shell.threads.some(thread => thread.id === id)) return client.openSelected(native, id);
  const snapshot = threadSnapshot(await client.restAccess(native).http(`/api/orchestration/threads/${encodeURIComponent(id)}/bounded`));
  client.projectId = str(obj(snapshot.projection.thread).projectId, client.projectId);
  client.threadId = id; client.absentThreadId = ''; client.thread = snapshot; client.diffOpen = false;
  const diffHeld = holdThreadDiff(client);
  await client.openThread(native, id, true);
  client.ensureSelection();
  await reopenThreadDiff(client, native, diffHeld);
}
/**
 * A thread link's press: another environment's thread focuses that environment (as its sidebar row does); one here
 * opens. A link that resolved to nothing opens its thread here only when it names no other environment.
 */
export async function openThreadLinkTarget(client: T3Client, native: Native, target: string | ThreadLink): Promise<void> {
  if (typeof target !== 'string') {
    if (target.environmentId === null || target.environmentId === client.environmentId) await openThreadLink(client, native, target.threadId);
    return;
  }
  if (!parseFleetThreadId(target)) return openThreadLink(client, native, target);
  const focused = await focusFleetThread(client, native, target);
  client.adoptStatus(focused.value, focused.generation);
}
/** Question answers have no context-record field: project each thread chip to its thread, in its environment. */
export function projectPendingUserInputAnswer(answer: string | string[], records: Obj[]): string | string[] {
  if (typeof answer !== 'string' || !records.length) return answer;
  return replaceLinks(answer, contextLinks(answer), link => {
    const target = contextTarget(link.href), record = target?.kind === 'thread' ? records.find(record => record.contextId === target.id) : undefined;
    return record && str(record.threadId) ? formatThreadLink({ environmentId: str(record.environmentId) || null, threadId: str(record.threadId) }, link.label) : null;
  });
}
