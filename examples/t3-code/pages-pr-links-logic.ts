// Pull request links in pull request text and between threads (task pr-links-previews-and-routing):
// `#N` and commit autolinks, the hover card's target, the linked-thread palette results, whether a
// thread already links a pull request, the link/unlink dispatch and the thread picker's rows.
// Sources: T3 Code (MIT, see LICENSE-T3) at 1e2ecbd975 — apps/web/src/components/pullRequest/
// pullRequestMarkdown.logic.ts (remarkPullRequestAutolinks), packages/shared/src/changeRequestUrl.ts
// (changeRequestRepositoryUrl, pullRequestCandidateUrlFromReferenceAutolink,
// matchesLinkedPullRequestUrl), packages/shared/src/threadPullRequests.ts (threadPullRequestKeysEqual),
// apps/web/src/lib/openPullRequestLink.ts (resolvePullRequestPreviewTarget),
// apps/web/src/components/CommandPalette.logic.ts (buildLinkedThreadActionItems),
// apps/web/src/hooks/usePullRequestLinking.ts (isLinked),
// packages/client-runtime/src/threadPullRequestCompatibility.ts (planThreadPullRequestMutation) and
// apps/web/src/components/pullRequest/PullRequestThreadLinks.tsx (ThreadPicker, the count's label).
//
// Port change: the reference adds its autolinks as a remark plugin over the parsed Markdown tree.
// This clone parses Markdown in Rust (macos/src/markdown.rs renderPullRequestMarkdown), so the same
// rule runs over the Markdown source instead and writes each autolink as a Markdown link. The
// reference's tree never holds a match inside code (fenced, indented, inline), a link, a link
// reference, an image, raw HTML or a bare URL (a GFM autolink literal is a link node), so the
// source scan steps over exactly those. Its word guards read the text node's own neighbours: where a
// node starts or ends at an emphasis delimiter, a code span or a link, the neighbour is "none".
import { num, obj, str, type Obj } from './domain';
import { parseChangeRequestUrl, threadPullRequestKey, findProjectForChangeRequest, type ChangeRequestLink } from './palette-linkpr';
import { visiblePullRequests } from './shell-pr';
import { repositorySelector } from './composer-editor-menu';

// ── Autolinks (remarkPullRequestAutolinks) ─────────────────────────────────

export type PullRequestAutolink = { href: string; text: string; kind: 'reference' | 'commit' };

// Sticky: tried at each position the scan reaches, as the reference retries one character on.
const AUTOLINK_CANDIDATE = /#[1-9]\d*|[0-9a-f]{40}/iuy;
const WORD = /[A-Za-z0-9_]/u;
const COMMIT_PREFIX = /[\s([{]/u;
const FENCE = /^\s{0,3}((?:`{3,})|(?:~{3,}))(.*)$/u;
const LIST_ITEM = /^\s{0,3}(?:[-*+]|\d{1,9}[.)])(?:\s|$)/u;
const DEFINITION = /^\s{0,3}\[((?:\\.|[^\]\\])+)\]:\s*\S/u;
const HTML_BLOCK = /^\s{0,3}<(?:!--|\?|![A-Za-z]|\/?[A-Za-z][A-Za-z0-9-]*(?:[\s/>]|$))/u;
const indentOf = (line: string) => { let width = 0; for (const character of line) { if (character === ' ') width += 1; else if (character === '\t') width += 4 - (width % 4); else break; } return width; };
const blank = (line: string) => line.trim().length === 0;
const normalizeLabel = (label: string) => label.trim().replace(/\s+/gu, ' ').toLowerCase();

/** The run of one delimiter character around `at`: its bounds and whether it could open or close (not inside a word, for `_`). */
function delimiterRun(text: string, at: number): { start: number; end: number; flanking: boolean } {
  const character = text[at]!;
  let start = at, end = at;
  while (start > 0 && text[start - 1] === character) start -= 1;
  while (end + 1 < text.length && text[end + 1] === character) end += 1;
  const before = text[start - 1], after = text[end + 1];
  const space = (value: string | undefined) => value === undefined || /\s/u.test(value);
  const alnum = (value: string | undefined) => value !== undefined && /[\p{L}\p{N}]/u.test(value);
  // Left- or right-flanking; an underscore inside a word (`snake_case`) is text.
  const flanking = !(space(before) && space(after)) && !(character === '_' && alnum(before) && alnum(after));
  return { start, end: end + 1, flanking };
}
/**
 * An emphasis or strikethrough delimiter, which ends one text node and starts the next. Only a run
 * with a partner of the same character in the block pairs; a lone `_` or `*` stays text.
 */
function delimiterAt(text: string, at: number, paired: ReadonlySet<string>): boolean {
  const character = text[at];
  return (character === '*' || character === '_' || character === '~') && paired.has(character) && delimiterRun(text, at).flanking;
}
/** The delimiter characters with at least two flanking runs in the block. */
function pairedDelimiters(text: string): Set<string> {
  const paired = new Set<string>();
  for (const character of ['*', '_', '~']) {
    let runs = 0;
    for (let index = 0; index < text.length;) {
      if (text[index] !== character) { index += 1; continue; }
      const run = delimiterRun(text, index);
      if (run.flanking) runs += 1;
      index = run.end;
    }
    if (runs >= 2) paired.add(character);
  }
  return paired;
}

/** The end (exclusive) of a code span opened by the backtick run at `at`, or -1 when the run closes nothing. */
function codeSpanEnd(text: string, at: number): number {
  let run = 0;
  while (text[at + run] === '`') run += 1;
  let cursor = at + run;
  while (cursor < text.length) {
    const next = text.indexOf('`', cursor);
    if (next < 0) return -1;
    let closing = 0;
    while (text[next + closing] === '`') closing += 1;
    if (closing === run) return next + closing;
    cursor = next + closing;
  }
  return -1;
}
/** The index after the bracket that closes the one at `at` (escapes and code spans skipped), or -1. */
function closingBracket(text: string, at: number, open: string, close: string): number {
  let depth = 0;
  for (let index = at; index < text.length; index += 1) {
    const character = text[index]!;
    if (character === '\\') { index += 1; continue; }
    if (character === '`' && open === '[') { const end = codeSpanEnd(text, index); if (end > 0) { index = end - 1; continue; } }
    if (character === '<' && open === '(') { const end = text.indexOf('>', index); if (end > 0 && !text.slice(index, end).includes('\n')) { index = end; continue; } }
    if (character === open) depth += 1;
    else if (character === close) { depth -= 1; if (depth === 0) return index + 1; }
  }
  return -1;
}

type Scan = { links: Map<string, PullRequestAutolink>; repositoryUrl: string; definitions: Set<string> };

/** One run of prose (a paragraph's lines, blank lines between such runs): every candidate a parsed text node would hold, rewritten. */
function autolinkProse(text: string, scan: Scan): string {
  let out = '';
  // Where the current text node began: after a code span, a link, raw HTML or an autolink.
  let nodeStart = 0;
  const paired = pairedDelimiters(text);
  const startsNode = (at: number) => at === nodeStart || delimiterAt(text, at - 1, paired);
  for (let index = 0; index < text.length;) {
    const character = text[index]!;
    if (character === '\\' && index + 1 < text.length) { out += text.slice(index, index + 2); index += 2; continue; }
    if (character === '`') {
      const end = codeSpanEnd(text, index);
      if (end > 0) { out += text.slice(index, end); index = nodeStart = end; continue; }
      let run = 0;
      while (text[index + run] === '`') run += 1;
      out += text.slice(index, index + run); index += run; continue;
    }
    if (character === '[' || (character === '!' && text[index + 1] === '[')) {
      const open = character === '!' ? index + 1 : index;
      const labelEnd = closingBracket(text, open, '[', ']');
      if (labelEnd > 0) {
        const following = text[labelEnd];
        const end = following === '(' ? closingBracket(text, labelEnd, '(', ')') : following === '[' ? closingBracket(text, labelEnd, '[', ']') : -1;
        const label = normalizeLabel(text.slice(open + 1, labelEnd - 1));
        const referenced = following === '[' ? normalizeLabel(text.slice(labelEnd + 1, Math.max(labelEnd + 1, end - 1))) || label : label;
        if (end > 0 && (following === '(' || scan.definitions.has(referenced))) { out += text.slice(index, end); index = nodeStart = end; continue; }
        if (end < 0 && scan.definitions.has(label)) { out += text.slice(index, labelEnd); index = nodeStart = labelEnd; continue; }
      }
      out += character; index += 1; continue;
    }
    if (character === '<') {
      const rest = text.slice(index);
      const tag = /^<!--[\s\S]*?-->|^<(?:[A-Za-z][A-Za-z0-9+.-]{1,31}:[^\s<>]*|[^\s<>@]+@[^\s<>]+|\/?[A-Za-z][A-Za-z0-9-]*(?:\s[^<>]*)?\/?|[?!][^<>]*)>/u.exec(rest);
      if (tag) { out += tag[0]; index = nodeStart = index + tag[0].length; continue; }
    }
    if (/^(?:https?:\/\/|www\.)/iu.test(text.slice(index, index + 8)) && (index === 0 || /[\s*_~(]/u.test(text[index - 1]!))) {
      let end = index;
      while (end < text.length && !/[\s<]/u.test(text[end]!)) end += 1;
      out += text.slice(index, end); index = nodeStart = end; continue;
    }
    AUTOLINK_CANDIDATE.lastIndex = index;
    const match = AUTOLINK_CANDIDATE.exec(text);
    if (match !== null && match.index === index) {
      const matched = match[0], reference = matched.startsWith('#');
      const before = startsNode(index) ? undefined : text[index - 1];
      const afterAt = index + matched.length;
      const after = afterAt >= text.length || delimiterAt(text, afterAt, paired) ? undefined : text[afterAt];
      const guarded = (before !== undefined && (reference ? WORD.test(before) : !COMMIT_PREFIX.test(before))) || (after !== undefined && WORD.test(after));
      if (!guarded) {
        const href = reference ? `${scan.repositoryUrl}/issues/${matched.slice(1)}` : `${scan.repositoryUrl}/commit/${matched}`;
        const label = reference ? matched : matched.slice(0, 7);
        if (!scan.links.has(href)) scan.links.set(href, { href, text: label, kind: reference ? 'reference' : 'commit' });
        out += `[${label}](${href})`;
        index = afterAt;
        continue;
      }
    }
    out += character; index += 1;
  }
  return out;
}

/**
 * remarkPullRequestAutolinks over Markdown source: `#N` → `[#N](<repo>/issues/N)`, a 40-hex sha →
 * `[<first 7>](<repo>/commit/<sha>)`, never inside code, links, link references, images, raw HTML or
 * bare URLs. The autolinks it made come back once each, in order.
 */
export function autolinkPullRequestMarkdown(markdown: string, repositoryUrl: string): { text: string; links: PullRequestAutolink[] } {
  const scan: Scan = { links: new Map(), repositoryUrl: repositoryUrl.replace(/\/+$/u, ''), definitions: new Set() };
  if (!scan.repositoryUrl) return { text: markdown, links: [] };
  const lines = markdown.split('\n');
  for (const line of lines) { const definition = DEFINITION.exec(line); if (definition) scan.definitions.add(normalizeLabel(definition[1]!)); }
  const out: string[] = [];
  let prose: string[] = [];
  let fence: string | null = null, htmlEnd: 'blank' | 'comment' | null = null, inList = false, previousBlank = true, previousCode = false;
  const flush = () => { if (prose.length) out.push(autolinkProse(prose.join('\n'), scan)); prose = []; };
  for (const line of lines) {
    if (fence !== null) {
      const match = FENCE.exec(line);
      if (match && match[1]![0] === fence[0] && match[1]!.length >= fence.length && match[2]!.trim().length === 0) fence = null;
      out.push(line); continue;
    }
    if (htmlEnd !== null) {
      out.push(line);
      if (htmlEnd === 'comment' ? line.includes('-->') : blank(line)) htmlEnd = null;
      previousBlank = blank(line); continue;
    }
    const opening = FENCE.exec(line);
    if (opening && !(opening[1]![0] === '`' && opening[2]!.includes('`'))) { flush(); fence = opening[1]!; out.push(line); previousBlank = false; previousCode = false; continue; }
    if (blank(line)) { flush(); out.push(line); previousBlank = true; continue; }
    // Indented code: four columns after a blank line (eight inside a list, whose items indent their own text).
    if ((previousBlank || previousCode) && indentOf(line) >= (inList ? 8 : 4)) { flush(); out.push(line); previousCode = true; previousBlank = false; continue; }
    previousCode = false;
    if (previousBlank && HTML_BLOCK.test(line)) {
      flush(); out.push(line);
      const comment = /^\s{0,3}<!--/u.test(line);
      if (comment ? !line.includes('-->') : true) htmlEnd = comment ? 'comment' : 'blank';
      previousBlank = false; continue;
    }
    if (DEFINITION.test(line)) { flush(); out.push(line); previousBlank = false; continue; }
    if (LIST_ITEM.test(line)) inList = true;
    else if (previousBlank && indentOf(line) === 0) inList = false;
    prose.push(line); previousBlank = false;
  }
  flush();
  return { text: out.join('\n'), links: [...scan.links.values()] };
}

// ── Change request URLs (changeRequestUrl.ts) ──────────────────────────────

/** The repository root behind a recognised change-request URL, without PR-specific state. */
export function changeRequestRepositoryUrl(targetUrl: string): string | null {
  if (parseChangeRequestUrl(targetUrl) === null) return null;
  const url = new URL(targetUrl);
  const repositoryPath = /^(.*?)\/-\/merge_requests\/\d+(?:\/|$)/iu.exec(url.pathname)?.[1]
    ?? /^(.*?)(?:\/pulls?\/\d+|\/-\/merge_requests\/\d+|\/pull-requests\/\d+|\/pullrequest\/\d+)(?:\/|$)/iu.exec(url.pathname)?.[1];
  if (!repositoryPath) return null;
  url.pathname = repositoryPath; url.search = ''; url.hash = '';
  return url.toString();
}

/**
 * The pull-request URL a GitHub-style `#123` autolink might name. GitHub writes every bare reference
 * through `/issues/`, pull requests included, so this is only a candidate: the caller reads it as a
 * pull request before treating it as one.
 */
export function pullRequestCandidateUrlFromReferenceAutolink(targetUrl: string): string | null {
  let url: URL;
  try { url = new URL(targetUrl); } catch { return null; }
  const hostname = url.hostname.toLowerCase();
  if ((url.protocol !== 'https:' && url.protocol !== 'http:') || !(hostname === 'github.com' || hostname.endsWith('.github.com') || hostname.split('.').includes('github'))) return null;
  const match = /^\/([^/]+\/[^/]+)\/issues\/(\d+)(?:\/|$)/u.exec(url.pathname);
  if (match?.[1] === undefined || match[2] === undefined) return null;
  url.pathname = `/${match[1]}/pull/${match[2]}`;
  return url.toString();
}

/** Match a stored PR without requiring its project to remain available. */
export function matchesLinkedPullRequestUrl(linkedPullRequest: Obj, targetUrl: string): boolean {
  const linked = parseChangeRequestUrl(str(linkedPullRequest.url)), target = parseChangeRequestUrl(targetUrl);
  return linked !== null && target !== null && linked.host === target.host && linked.repository === target.repository && linked.number === target.number && linked.authority === target.authority;
}

type KeySource = { host: string; repository: string; number: number; authority?: string; url?: string };
const keySource = (value: Obj | ChangeRequestLink): KeySource => {
  const record = value as Obj;
  return { host: str(record.host), repository: str(record.repository), number: num(record.number), ...(typeof record.authority === 'string' ? { authority: record.authority } : {}),
    ...(typeof record.url === 'string' ? { url: record.url } : {}) };
};
const keyString = (key: KeySource) => { const normalized = threadPullRequestKey(key); return `${normalized.host}/${normalized.repository}#${normalized.number}`; };
/** Identity comparison for links: host-level, case-insensitive on host and repository. */
export function threadPullRequestKeysEqual(left: Obj | ChangeRequestLink, right: Obj | ChangeRequestLink): boolean { return keyString(keySource(left)) === keyString(keySource(right)); }

// ── The hover card's target (openPullRequestLink.ts) ───────────────────────

/** The project this environment reads a pull request link through, or null where the link stays an ordinary one. */
export function resolvePullRequestPreviewTarget(input: { environmentId: string | null; projects: ReadonlyArray<Obj>; pullRequestsEnabled: boolean; url: string }): { environmentId: string; input: Obj } | null {
  if (!input.pullRequestsEnabled || input.environmentId === null) return null;
  const parsed = parseChangeRequestUrl(input.url);
  if (parsed === null) return null;
  const project = findProjectForChangeRequest(input.projects.filter(candidate => str(candidate.environmentId) === input.environmentId), parsed);
  if (project === undefined) return null;
  return { environmentId: input.environmentId, input: { projectId: str(project.id), host: parsed.authority ?? parsed.host,
    repository: repositorySelector(obj(project.repositoryIdentity)) || parsed.repository, number: parsed.number } };
}

// ── Linked threads (CommandPalette.logic.ts, PullRequestThreadLinks.tsx) ───

/** A PR's relations include archived threads that normal palette search omits. */
export function buildLinkedThreadActionItems(input: { environmentId: string; threads: ReadonlyArray<Obj>; query: string }): Array<{ value: string; title: string; description: string; searchTerms: string[]; environmentId: string; threadId: string }> {
  return input.threads.map(thread => ({
    value: `thread:${input.environmentId}:${str(thread.id)}`,
    title: str(thread.title) || 'Untitled thread',
    description: thread.archivedAt == null ? 'Linked thread' : 'Archived thread',
    searchTerms: [input.query, str(thread.title)],
    environmentId: input.environmentId,
    threadId: str(thread.id),
  }));
}
/** The count button's name and tooltip lead. */
export function linkedThreadsLabel(count: number): string {
  return count > 0 ? `Linked from ${count} ${count === 1 ? 'thread' : 'threads'}` : 'Linked threads';
}

export type LinkMode = 'multiple' | 'single' | 'unsupported';
/** usePullRequestLinking.isLinked: the thread's visible links (one legacy link on a single-link server). */
export function isPullRequestLinked(thread: Obj | null, url: string, mode: LinkMode): boolean {
  if (thread === null || mode === 'unsupported') return false;
  if (mode !== 'multiple') return thread.linkedPullRequest != null && typeof thread.linkedPullRequest === 'object' && matchesLinkedPullRequestUrl(obj(thread.linkedPullRequest), url);
  const parsed = parseChangeRequestUrl(url);
  return parsed !== null && visiblePullRequests(thread.pullRequests).some(entry => threadPullRequestKeysEqual(entry, parsed));
}

/**
 * planThreadPullRequestMutation, as the dispatch payload (no commandId): multi-link servers link and
 * unlink by key; single-link servers update the thread's one legacy link (`thread.metadata.update`,
 * the clone's name for it), which needs the exact checkout; unsupported servers get nothing.
 */
export function planThreadPullRequestMutation(input: { mode: LinkMode; threadId: string; reference: { host: string; repository: string; number: number; url: string; authority?: string };
  legacyProjectId: string | null; legacyRepository?: string; linked: boolean }): Obj | null {
  const { repository, number, url } = input.reference;
  // The key's host, repository and number are what the server stores (a Forgejo port recovered from the URL).
  const key = threadPullRequestKey(input.reference);
  if (input.mode === 'multiple') {
    return input.linked ? { type: 'thread.pull-request.link', threadId: input.threadId, ...key, url, source: 'manual' }
      : { type: 'thread.pull-request.unlink', threadId: input.threadId, ...key };
  }
  if (input.mode === 'single') {
    if (input.linked && input.legacyProjectId === null) return null;
    return { type: 'thread.metadata.update', threadId: input.threadId,
      linkedPullRequest: input.linked && input.legacyProjectId !== null ? { projectId: input.legacyProjectId, repository: input.legacyRepository ?? repository, number, url } : null };
  }
  return null;
}

/** ThreadPicker: this environment's unarchived threads matching "title project", newest first, each saying whether it already links the pull request. */
export function threadPickerCandidates(input: { threads: ReadonlyArray<Obj>; projects: ReadonlyArray<Obj>; query: string; url: string; mode: LinkMode }): Array<{ id: string; title: string; project: string; linked: boolean }> {
  const names = new Map(input.projects.map(project => [str(project.id), str(project.title)]));
  const search = input.query.trim().toLocaleLowerCase();
  return input.threads
    .filter(thread => thread.archivedAt == null && `${str(thread.title)} ${names.get(str(thread.projectId)) ?? ''}`.toLocaleLowerCase().includes(search))
    .sort((left, right) => str(right.updatedAt).localeCompare(str(left.updatedAt)))
    .map(thread => ({ id: str(thread.id), title: str(thread.title) || 'Untitled thread', project: names.get(str(thread.projectId)) ?? '', linked: isPullRequestLinked(thread, input.url, input.mode) }));
}

/** Every thread of a list that links the pull request (the shell's own answer, for a server without `pullRequests.linkedThreads`). */
export function threadsLinking(threads: ReadonlyArray<Obj>, url: string, mode: LinkMode): Obj[] {
  return threads.filter(thread => isPullRequestLinked(thread, url, mode));
}
