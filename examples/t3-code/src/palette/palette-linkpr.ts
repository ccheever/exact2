// The palette's "Link pull request to thread": T3 Code reference pullRequest/LinkPullRequestDialog.tsx and
// client-runtime/pullRequestLinkInput.ts (2359643cff) over each host's parseChangeRequestUrl and changeRequestUrl in
// the definitions registry's claim order (packages/source-control-*/src/shared/definition.ts), with
// web/pullRequestReference.ts, web/lib/openPullRequestLink.ts (project matching; 9c161462c9),
// hooks/usePullRequestLinking.ts and client-runtime/threadPullRequestCompatibility.ts.
// The dialog is a palette page drawn as a dialog (palette-dialog.contract).
import type { T3Client } from '../client/client';
import { obj, str, type Obj } from '../client/domain';
import type { Files } from '../client/protocol';
import { closedView, type PaletteView } from './palette';
import { letGo } from '../client/let-go';
import { changeRequestTerms, type ChangeRequestTerms } from '../shared/source-control-hosts';

export type ChangeRequestLink = { host: string; repository: string; number: number; authority?: string };
export type ResolvedLink = { host: string; repository: string; number: number; url: string };
type Project = Obj;

// ── Change request URLs: each host's parseChangeRequestUrl, in the registry's claim order ────────────────────────────
type Claim = { kind: string; route: RegExp; claims?: (hostname: string) => boolean; authority?: (url: URL) => boolean };
/** GitCafe has no self-hosted installs: production and staging are its only hosts. */
const GITCAFE_HOSTS = new Set(['git.cafe', 'staging.git.cafe']);
const CLAIMS: Claim[] = [
  // GitCafe serves pull requests at /pull/<n> and redirects the older /pulls/<n> there. Those are GitHub's and
  // Forgejo's routes on any hostname too, so GitCafe's own hosts claim first.
  { kind: 'gitcafe', route: /^\/([^/]+\/[^/]+)\/pulls?\/(\d+)(?:\/|$)/u, claims: hostname => GITCAFE_HOSTS.has(hostname) },
  // GitHub Enterprise Server runs on any hostname, so the route alone identifies a GitHub pull request.
  { kind: 'github', route: /^\/([^/]+\/[^/]+)\/pull\/(\d+)(?:\/|$)/u },
  { kind: 'gitlab', route: /^\/([^/]+(?:\/[^/]+)+)\/-\/merge_requests\/(\d+)(?:\/|$)/u },
  // A Forgejo server's identity includes its port, so the link carries it.
  { kind: 'forgejo', route: /^\/([^/]+(?:\/[^/]+)+)\/pulls\/(\d+)(?:\/|$)/u, authority: () => true },
  { kind: 'bitbucket', route: /^\/([^/]+\/[^/]+)\/pull-requests\/(\d+)(?:\/|$)/u, claims: hostname => hostname === 'bitbucket.org' || hostname.endsWith('.bitbucket.org') || hostname.split('.').includes('bitbucket') },
  // Azure DevOps Server runs on any hostname; servers on different ports are different servers, so a port stays.
  { kind: 'azure-devops', route: /^\/((?:[^/]+\/)*_git\/[^/]+)\/pullrequest\/(\d+)(?:\/|$)/u, authority: url => url.port !== '' },
];
/** The first host, in the registry's claim order, that reads `target` as one of its change requests. */
export function claimChangeRequestUrl(target: string): { kind: string; link: ChangeRequestLink } | null {
  let url: URL;
  try { url = new URL(target); } catch { return null; }
  if (url.protocol !== 'https:' && url.protocol !== 'http:') return null;
  const hostname = url.hostname.toLowerCase();
  for (const { kind, route, claims, authority } of CLAIMS) {
    if (claims && !claims(hostname)) continue;
    const match = route.exec(url.pathname);
    const repository = match?.[1], number = Number(match?.[2]);
    if (!repository || !Number.isSafeInteger(number) || number <= 0) continue;
    const link = { host: hostname, repository: repository.toLowerCase(), number };
    return { kind, link: authority?.(url) ? { ...link, authority: url.host.toLowerCase() } : link };
  }
  return null;
}
export function parseChangeRequestUrl(target: string): ChangeRequestLink | null {
  return claimChangeRequestUrl(target)?.link ?? null;
}
/** changeRequestTerminologyForUrl: the terms of the host whose change request `url` is, else GitHub's, the client default. */
export function changeRequestTermsForUrl(url: string): ChangeRequestTerms {
  return changeRequestTerms(claimChangeRequestUrl(url)?.kind ?? 'github');
}
/** canonicalizeGitCafePullRequestUrl: a GitCafe link is stored at its canonical /pulls/<n>; any other link as typed. */
export function canonicalPullRequestUrl(value: string): string {
  const claimed = claimChangeRequestUrl(value);
  return claimed?.kind === 'gitcafe' ? `https://${claimed.link.host}/${claimed.link.repository}/pulls/${claimed.link.number}` : value;
}
export function canonicalRepositoryKey(key: string): string {
  return key.replace(/^(?:ssh\.dev\.azure\.com|vs-ssh\.visualstudio\.com)\/v3\/([^/]+)\/([^/]+)\/([^/]+)$/u, 'dev.azure.com/$1/$2/_git/$3')
    .replace(/^([^.]+)\.visualstudio\.com\/(?:defaultcollection\/)?([^/]+)\/_git\/([^/]+)$/u, 'dev.azure.com/$1/$2/_git/$3');
}
/** remoteWebOrigin: an HTTP remote on `host` names the server's web origin, port included; SSH remotes do not. */
function remoteWebOrigin(host: string, remoteUrl: string): string {
  try {
    const remote = new URL(remoteUrl);
    if ((remote.protocol === 'http:' || remote.protocol === 'https:') && (remote.hostname.toLowerCase() === host.toLowerCase() || remote.host.toLowerCase() === host.toLowerCase())) return remote.origin;
  } catch { /* SSH remotes do not name the web origin. */ }
  return `https://${host}`;
}
/** Each host's changeRequestUrl: below the repository's browser URL when the project names one. */
function changeRequestUrlFor(kind: string, host: string, repository: string, number: number, remoteUrl: string, webUrl: string): string | null {
  const below = (route: string) => `${webUrl.replace(/\/+$/u, '')}/${route}`;
  switch (kind) {
    case 'github': {
      if (!Number.isSafeInteger(number) || number < 1) return null;
      if (webUrl) return below(`pull/${number}`);
      // Owner and repository names are only these characters, so nothing here is a dot segment `new URL` would reread.
      const segments = repository.split('/');
      if (segments.length !== 2 || !segments.every(segment => /^[\w.-]+$/u.test(segment) && segment !== '.' && segment !== '..')) return null;
      try { return new URL(`/${repository}/pull/${number}`, remoteWebOrigin(host, remoteUrl)).toString(); } catch { return null; }
    }
    case 'gitcafe': return webUrl ? below(`pulls/${number}`) : `https://${host}/${repository}/pulls/${number}`;
    case 'forgejo': return webUrl ? below(`pulls/${number}`) : `${remoteWebOrigin(host, remoteUrl)}/${repository}/pulls/${number}`;
    case 'gitlab': return webUrl ? below(`-/merge_requests/${number}`) : `https://${host}/${repository}/-/merge_requests/${number}`;
    case 'bitbucket': return webUrl ? below(`pull-requests/${number}`) : `https://${host}/${repository}/pull-requests/${number}`;
    case 'azure-devops': {
      if (webUrl) return below(`pullrequest/${number}`);
      const [canonicalHost = host, ...path] = canonicalRepositoryKey(`${host}/${repository}`.toLowerCase()).split('/');
      return `${remoteWebOrigin(canonicalHost, remoteUrl)}/${path.join('/')}/pullrequest/${number}`;
    }
    default: return null;
  }
}

// ── Typed references (each host's isChangeRequestReference and checkoutCommandArgument) ──────────────────────────────
const URL_PATTERNS = [/^https?:\/\/[^/\s]+\/(?:[^/\s]+\/)+[^/\s]+\/pulls\/(\d+)(?:[/?#].*)?$/i, /^https:\/\/github\.com\/[^/\s]+\/[^/\s]+\/pull\/(\d+)(?:[/?#].*)?$/i,
  /^https?:\/\/[^/?#\s]+\/[^/?#\s]+(?:\/[^/?#\s]+)+\/-\/merge_requests\/(\d+)(?:[/?#].*)?$/i,
  /^https:\/\/(?:dev\.azure\.com\/[^/\s]+\/[^/\s]+|[^/\s]+\.visualstudio\.com\/[^/\s]+)\/_git\/[^/\s]+\/pullrequest\/(\d+)(?:[/?#].*)?$/i];
function azureCheckout(args: string): string | null {
  const parts = args.trim().split(/\s+/).filter(Boolean);
  for (const [index, part] of parts.entries()) {
    if (part === '--id' || part === '-i') return parts[index + 1] ?? null;
    if (part.startsWith('--id=')) return part.slice(5) || null;
  }
  return parts.find(part => !part.startsWith('-')) ?? null;
}
export function parsePullRequestReference(input: string): string | null {
  const trimmed = input.trim();
  if (!trimmed) return null;
  const azure = /^az\s+repos\s+pr\s+checkout\s+(.+)$/i.exec(trimmed)?.[1];
  const normalized = /^tea\s+(?:pr|pulls)\s+checkout\s+(.+)$/i.exec(trimmed)?.[1]?.trim() ?? /^gh\s+pr\s+checkout\s+(.+)$/i.exec(trimmed)?.[1]?.trim()
    ?? /^glab\s+mr\s+checkout\s+(.+)$/i.exec(trimmed)?.[1]?.trim() ?? (azure ? azureCheckout(azure) : null) ?? trimmed;
  if (!normalized) return null;
  // GitCafe's and Bitbucket's references are their change request URLs, on their own hosts.
  const claimed = claimChangeRequestUrl(normalized)?.kind;
  if (URL_PATTERNS.some(pattern => pattern.exec(normalized)?.[1]) || claimed === 'gitcafe' || claimed === 'bitbucket') return normalized;
  return /^#?(\d+)$/.exec(normalized)?.[1] ?? null;
}

// ── Projects a link belongs to (apps/web/src/lib/openPullRequestLink.ts) ───
function identityOf(project: Project): Obj | null { const identity = obj(project.repositoryIdentity); return Object.keys(identity).length ? identity : null; }
function remoteOf(identity: Obj): string { return str(obj(identity.locator).remoteUrl); }
/** pullRequestHostOf (contracts/pullRequest.ts). */
export function pullRequestHost(identity: Obj, kind: string): string {
  if (kind === 'forgejo') {
    try { const remote = new URL(remoteOf(identity)); if (remote.protocol === 'http:' || remote.protocol === 'https:') return remote.host.toLowerCase(); } catch { /* SSH */ }
  }
  const host = str(identity.canonicalKey).split('/')[0]?.trim();
  return host ? host.toLowerCase() : kind;
}
function forgejoWeb(identity: Obj): URL | null {
  if (str(identity.provider) !== 'forgejo' || !str(identity.webUrl)) return null;
  try { const url = new URL(str(identity.webUrl)); return url.protocol === 'http:' || url.protocol === 'https:' ? url : null; } catch { return null; }
}
function authorityMatches(identity: Obj, link: ChangeRequestLink): boolean {
  if (link.authority === undefined) return true;
  try { const remote = new URL(remoteOf(identity)); if (remote.protocol === 'http:' || remote.protocol === 'https:') return remote.host.toLowerCase() === link.authority; } catch { /* SSH */ }
  return true;
}
function displayRepository(identity: Obj): string | null {
  return str(identity.displayName) || (str(identity.owner) && str(identity.name) ? `${str(identity.owner)}/${str(identity.name)}` : null);
}
export function findProjectForChangeRequest(projects: Project[], link: ChangeRequestLink): Project | undefined {
  return projects.find(project => {
    const identity = identityOf(project);
    if (!identity || !authorityMatches(identity, link)) return false;
    const kind = str(identity.provider);
    if (!kind) return false;
    const web = forgejoWeb(identity);
    if (web) return web.host.toLowerCase() === (link.authority ?? link.host).toLowerCase() && web.pathname.replace(/^\/+|\/+$/g, '').toLowerCase() === link.repository.toLowerCase();
    if (kind === 'azure-devops') return canonicalRepositoryKey(str(identity.canonicalKey).toLowerCase()) === canonicalRepositoryKey(`${link.host}/${link.repository}`.toLowerCase());
    const repository = displayRepository(identity), host = pullRequestHost(identity, kind);
    return repository !== null && repository.toLowerCase() === link.repository.toLowerCase() && (host === link.host.toLowerCase() || host === link.authority);
  });
}
export function findProjectOnChangeRequestHost(projects: Project[], link: ChangeRequestLink): Project | undefined {
  const own = findProjectForChangeRequest(projects, link);
  if (own) return own;
  if (canonicalRepositoryKey(`${link.host}/${link.repository}`.toLowerCase()).startsWith('dev.azure.com/')) return undefined;
  return projects.find(project => {
    const identity = identityOf(project);
    if (!identity) return false;
    const kind = str(identity.provider), web = forgejoWeb(identity);
    if (web) {
      const mount = web.pathname.replace(/^\/+|\/+$/g, '').split('/').slice(0, -2).join('/');
      return web.host.toLowerCase() === (link.authority ?? link.host).toLowerCase() && (!mount || link.repository.toLowerCase().startsWith(`${mount.toLowerCase()}/`));
    }
    const host = pullRequestHost(identity, kind);
    return !!kind && kind !== 'azure-devops' && authorityMatches(identity, link) && (host === link.host.toLowerCase() || host === link.authority);
  });
}

// ── Link mode and the dialog's resolution ──────────────────────────────────
export type LinkMode = 'multiple' | 'single' | 'unsupported';
export function linkMode(config: Obj): LinkMode {
  const caps = obj(obj(config.environment).capabilities);
  return caps.threadPullRequests === true ? 'multiple' : caps.threadPullRequestLinking === true ? 'single' : 'unsupported';
}
function canLink(projects: Project[], mode: LinkMode, url: string): boolean {
  const parsed = parseChangeRequestUrl(url);
  if (parsed === null || mode === 'unsupported') return false;
  return mode === 'multiple' || findProjectForChangeRequest(projects, parsed) !== undefined;
}
/** pullRequestLinkProject: the thread project's repository, so a bare number resolves to one of its change requests. */
function linkProject(identity: Obj | null) {
  const repository = identity ? displayRepository(identity) : null;
  if (!identity || repository === null) return null;
  // sourceControlClients.get(provider ?? "unknown"): a host no definition claims has no change request URL.
  const kind = str(identity.provider) || 'unknown', host = pullRequestHost(identity, kind);
  return { terms: changeRequestTerms(kind), webUrl: (number: number) => changeRequestUrlFor(kind, host, repository, number, remoteOf(identity), str(identity.webUrl)) };
}
/** resolveLinkPullRequestInput over the thread's own project; a GitCafe link resolves to its canonical URL. */
export function resolveLinkInput(reference: string, projects: Project[], projectId: string, mode: LinkMode): { link: ResolvedLink } | { error: string } | null {
  const trimmed = reference.trim();
  const parsed = parseChangeRequestUrl(trimmed) !== null ? trimmed : parsePullRequestReference(reference);
  if (parsed === null) return null;
  const url = parseChangeRequestUrl(parsed);
  if (url !== null) {
    if (!canLink(projects, mode, parsed)) return { error: `No project in this environment can read ${url.host}/${url.repository}.` };
    return { link: { host: url.host, repository: url.repository, number: url.number, url: canonicalPullRequestUrl(parsed) } };
  }
  const number = Number(parsed);
  if (!Number.isSafeInteger(number) || number < 1) return null;
  const project = projects.find(candidate => candidate.id === projectId);
  const own = linkProject(project ? identityOf(project) : null);
  if (own === null) return { error: 'Paste the full pull request URL; this thread has no repository to resolve the number against.' };
  const webUrl = own.webUrl(number);
  const web = webUrl === null ? null : parseChangeRequestUrl(webUrl);
  if (webUrl === null || web === null) return { error: `Paste a full URL; this project's host has no known ${own.terms.singular} URL.` };
  return { link: { host: web.host, repository: web.repository, number: web.number, url: webUrl } };
}

/** normalizeThreadPullRequestKey: the canonical host/repository a link is stored under. */
export function threadPullRequestKey(link: ChangeRequestLink & { url?: string }): { host: string; repository: string; number: number } {
  const parsed = link.url === undefined ? null : parseChangeRequestUrl(link.url);
  const authority = link.authority ?? (parsed?.repository === link.repository.trim().toLowerCase() && parsed.number === link.number ? parsed.authority : undefined);
  const canonical = canonicalRepositoryKey(`${(authority ?? link.host).trim().toLowerCase()}/${link.repository.trim().toLowerCase()}`);
  const at = canonical.indexOf('/');
  return { host: canonical.slice(0, at), repository: canonical.slice(at + 1), number: link.number };
}

// ── Dialog state, view and submit ──────────────────────────────────────────
type DialogState = { dirty: boolean; error: string; pending: boolean };
const dialogs = new WeakMap<T3Client, DialogState>();
function dialogOf(client: T3Client): DialogState {
  let state = dialogs.get(client);
  if (!state) { state = { dirty: false, error: '', pending: false }; dialogs.set(client, state); }
  return state;
}
/** Leaving the dialog forgets its validation (the reference resets on open). */
export function resetLinkDialog(client: T3Client): void { dialogs.delete(client); }

export const LINK_DESCRIPTION = "Attach a pull request to this thread by its URL, or by its number for this thread's repository.";
export function linkPullRequestView(client: T3Client, query: string): PaletteView {
  const state = dialogOf(client);
  if (query !== '') state.dirty = true;
  const thread = client.shell.threads.find(candidate => candidate.id === client.threadId);
  const resolved = resolveLinkInput(query, client.shell.projects, str(thread?.projectId), linkMode(client.config));
  const validation = !state.dirty ? '' : query.trim().length === 0 ? 'Paste a pull request URL or enter 123 / #123.'
    : resolved === null ? 'Use a pull request URL, 123, or #123.' : 'error' in resolved ? resolved.error : '';
  const link = resolved !== null && 'link' in resolved ? resolved.link : null;
  return { ...closedView, open: true, page: 'link-pr', panel: 'dialog', label: 'Link pull request', testId: 'link-pull-request-dialog', placeholder: 'Pull request URL or #42',
    contextDescription: LINK_DESCRIPTION, contextTitle: link ? `${link.host}/${link.repository} #${link.number}` : '', empty: validation || state.error, escapeLabel: 'Close',
    enterOp: 'flow', enterArg: 'link-pr', enterArg2: query, accessory: state.pending ? 'Linking...' : 'Link', accessoryEnabled: link !== null && !state.pending,
    accessoryOp: 'flow', accessoryArg: 'link-pr', accessoryArg2: query, autoHighlight: false, enterLabel: '' };
}

type Access = ReturnType<T3Client['restAccess']>;
/** submit(): link through the command this environment advertises; the dialog stays open on failure. */
export async function linkPullRequest(client: T3Client, access: Access, storage: Files, reference: string, threadId = client.threadId): Promise<boolean> {
  const state = dialogOf(client);
  state.dirty = true;
  state.error = '';
  client.revision++;
  // The thread chosen (the panel's picker), else the window's.
  const thread = client.shell.threads.find(candidate => candidate.id === threadId);
  if (!thread) { state.error = 'That thread is no longer available.'; return false; }
  const mode = linkMode(client.config), projects = client.shell.projects;
  const resolved = resolveLinkInput(reference, projects, str(thread.projectId), mode);
  if (resolved === null || 'error' in resolved) return false;
  const parsed = parseChangeRequestUrl(resolved.link.url)!;
  try {
    if (!canLink(projects, mode, resolved.link.url)) throw new Error('The pull request is not available in this environment.');
    const [commandId] = await access.ids(1);
    if (mode === 'multiple') {
      await access.dispatch(storage, { type: 'thread.pull-request.link', commandId: commandId!, threadId: str(thread.id),
        ...threadPullRequestKey({ ...parsed, url: resolved.link.url }), url: resolved.link.url, source: 'manual' }, 'Link pull request');
    } else {
      const legacy = findProjectForChangeRequest(projects, parsed);
      if (!legacy) throw new Error('This environment does not support linking this pull request.');
      const identity = identityOf(legacy) ?? {};
      await access.dispatch(storage, { type: 'thread.metadata.update', commandId: commandId!, threadId: str(thread.id),
        linkedPullRequest: { projectId: str(legacy.id), repository: displayRepository(identity) ?? parsed.repository, number: parsed.number, url: resolved.link.url } }, 'Link pull request');
    }
  } catch (error) {
    if (letGo(error)) throw error;
    state.error = error instanceof Error ? error.message : 'Could not link the pull request.';
    client.revision++;
    return false;
  }
  resetLinkDialog(client);
  return true;
}
