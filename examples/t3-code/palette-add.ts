// The palette's Add project pages: reference CommandPalette.tsx (buildAddProjectSourceGroups,
// startAddProjectBrowse, the clone flow, the New project name step) with the path helpers of
// packages/client-runtime/src/state/projects.ts and operations/projects.ts.
import type { T3Client } from './client';
import { arr, obj, str, type Obj } from './domain';
import type { Native } from './protocol';
import { closedView, filterGroups, flatten, row, type Group, type Item, type PaletteView } from './palette';

export const SOURCES = ['github', 'gitlab', 'forgejo', 'bitbucket', 'azure-devops'] as const;
export const SOURCE_LABELS: Record<string, string> = { url: 'Git URL', github: 'GitHub', gitlab: 'GitLab', forgejo: 'Forgejo / Gitea', bitbucket: 'Bitbucket', 'azure-devops': 'Azure DevOps' };
const SOURCE_HINTS: Record<string, string> = { url: 'URL', github: 'owner/repo', forgejo: 'owner/repo', gitlab: 'group/project', bitbucket: 'workspace/repository', 'azure-devops': 'project/repository' };

// ── Paths ──────────────────────────────────────────────────────────────────
export const hasTrailingSeparator = (value: string) => /\/$/.test(value);
export function isBrowseQuery(value: string): boolean {
  return value.startsWith('./') || value.startsWith('../') || value.startsWith('/') || value.startsWith('~/');
}
export function browseDirectory(path: string): string {
  if (hasTrailingSeparator(path)) return path;
  const at = path.lastIndexOf('/');
  return at < 0 ? path : path.slice(0, at + 1);
}
export const browseLeaf = (path: string) => path.slice(path.lastIndexOf('/') + 1);
export const appendSegment = (path: string, segment: string) => `${browseDirectory(path)}${segment}/`;
export function ensureDirectory(path: string): string {
  const trimmed = path.trim();
  return !trimmed || hasTrailingSeparator(trimmed) ? trimmed : `${trimmed}/`;
}
function normalizePath(value: string): string {
  const trimmed = value.trim();
  return trimmed.length > 1 ? trimmed.replace(/\/+$/, '') : trimmed;
}
export function browseParent(path: string): string | null {
  const trimmed = normalizePath(path);
  if (trimmed.startsWith('/')) {
    const segments = trimmed.slice(1).split(/\/+/).filter(Boolean);
    if (!segments.length) return null;
    return segments.length === 1 ? '/' : `/${segments.slice(0, -1).join('/')}/`;
  }
  const at = trimmed.lastIndexOf('/');
  return at < 0 ? null : trimmed.slice(0, at + 1);
}
export function inferTitle(path: string): string {
  const segments = normalizePath(path).split('/').filter(Boolean);
  return segments[segments.length - 1] ?? normalizePath(path);
}
/** resolveProjectPathForDispatch: ./ and ../ resolve against the active project's root. */
export function resolvePath(value: string, cwd: string): string {
  const trimmed = value.trim();
  if (!(trimmed.startsWith('./') || trimmed.startsWith('../') || trimmed === '.' || trimmed === '..') || !cwd.startsWith('/')) return normalizePath(trimmed);
  const segments = cwd.split('/').filter(Boolean);
  for (const segment of trimmed.split('/')) {
    if (!segment || segment === '.') continue;
    if (segment === '..') segments.pop(); else segments.push(segment);
  }
  return normalizePath(`/${segments.join('/')}`);
}
const SHORTHAND = /^[A-Za-z0-9](?:[A-Za-z0-9-]{0,38})\/[A-Za-z0-9._-]+(?:\.git)?$/;
export function normalizeCloneUrl(input: string): string {
  const trimmed = input.trim();
  return SHORTHAND.test(trimmed) ? `https://github.com/${trimmed.endsWith('.git') ? trimmed : `${trimmed}.git`}` : trimmed;
}
export function cloneDirectoryName(value: string): string {
  const withoutQuery = (value.split(/[?#]/)[0] ?? '').trim();
  const scheme = withoutQuery.indexOf('://');
  const hasHost = scheme >= 0 || /^[^/\\:]+@[^/\\:]+:/.test(withoutQuery);
  const segments = (scheme >= 0 ? withoutQuery.slice(scheme + 3) : withoutQuery).split(/[/\\:]+/).filter(segment => segment.trim());
  if (hasHost && segments.length < 2) return '';
  const last = (segments[segments.length - 1] ?? '').trim();
  if (hasHost && segments.length === 2 && /^\d+$/.test(last)) return '';
  return last.endsWith('.git') ? last.slice(0, -4) : last;
}
export const cloneDestination = (directory: string, name: string) => name.trim() ? `${ensureDirectory(directory)}${name.trim()}` : directory;
export function newProjectFolder(name: string): string {
  return name.trim().normalize('NFKD').replace(/\p{M}/gu, '').toLowerCase().replace(/[^a-z0-9._-]+/g, '-').replace(/^-+|-+$/g, '') || 'project';
}
export function initialBrowseQuery(client: T3Client): string {
  const base = str(obj(client.config.settings).addProjectBaseDirectory).trim();
  return base ? ensureDirectory(base) : '~/';
}

// ── Flow state (per client): the clone step's repository and the New project option ──
export type CloneFlow = { source: string; repositoryInput: string; title: string; description: string; remoteUrl: string; pinned: string };
type Flow = { clone: CloneFlow | null; publish: boolean; discovery: { key: string; value: Obj } | null; discoveryFailed?: { key: string; revision: number }; browse: { key: string; value: Obj; error: string } | null };
const flows = new WeakMap<T3Client, Flow>();
export function flowOf(client: T3Client): Flow {
  let flow = flows.get(client);
  if (!flow) { flow = { clone: null, publish: false, discovery: null, browse: null }; flows.set(client, flow); }
  return flow;
}

export type Readiness = Record<string, { ready: boolean; hint: string }>;
/** buildAddProjectRemoteSourceReadiness over server.discoverSourceControl. */
export function readiness(discovery: Obj | null): Readiness {
  const unavailable = { ready: false, hint: 'Provider status unavailable. Open Settings -> Source Control and rescan.' };
  const result: Readiness = { url: { ready: true, hint: '' }, ...Object.fromEntries(SOURCES.map(source => [source, unavailable])) };
  if (!discovery) return result;
  for (const source of SOURCES) {
    const provider = arr(discovery.sourceControlProviders).find(candidate => str(candidate.kind) === source);
    if (!provider) continue;
    const auth = obj(provider.auth), detail = typeof auth.detail === 'string' ? auth.detail : str(obj(auth.detail).value);
    if (str(provider.status) !== 'available') result[source] = { ready: false, hint: str(provider.installHint) };
    else if (str(auth.status) === 'unauthenticated') result[source] = { ready: false, hint: detail || `${str(provider.label)} is not authenticated. Open Settings -> Source Control for setup guidance.` };
    else result[source] = { ready: true, hint: '' };
  }
  return result;
}
export function githubAccount(discovery: Obj | null): string | null {
  if (!readiness(discovery).github?.ready) return null;
  const auth = obj(arr(obj(discovery).sourceControlProviders).find(candidate => str(candidate.kind) === 'github')?.auth);
  return typeof auth.account === 'string' ? auth.account : str(obj(auth.account).value);
}
async function discover(client: T3Client, native: Native): Promise<Obj | null> {
  const flow = flowOf(client), key = `${client.origin}:${client.generation}`;
  if (flow.discovery?.key === key) return flow.discovery.value;
  // A failed discovery (often an answer let go mid-request) is not kept: the reference's query
  // asks again, so one lost request must not mark every provider "Setup Required" for the session.
  // Bounded: after a failure the next ask waits for the client's state to move (its revision), not every palette read.
  if (flow.discoveryFailed?.key === key && flow.discoveryFailed.revision === client.revision) return {};
  try { flow.discovery = { key, value: await client.restAccess(native).request('server.discoverSourceControl', {}) }; }
  catch { flow.discoveryFailed = { key, revision: client.revision }; return {}; }
  return flow.discovery.value;
}

export function sourceItems(client: T3Client, ready: Readiness, page: string): Item[] {
  const items: Item[] = [];
  if (str(client.config.newProjectsRoot)) items.push({ terms: ['new project', 'create', 'empty', 'repository', 'git init'],
    row: row({ key: 'new', op: 'page', arg: `${page}/new-project`, icon: 'folder-git', title: 'New project', description: 'Start a new Git repository from a name' }) });
  items.push({ terms: ['local', 'folder', 'directory', 'browse'], row: row({ key: 'local', op: 'browse', arg: `${page}/browse`, arg2: initialBrowseQuery(client), icon: 'folder-plus', title: 'Local folder', description: 'Browse a folder on disk' }) });
  const providers = [...SOURCES].sort((a, b) => Number(ready[b]!.ready) - Number(ready[a]!.ready) || SOURCE_LABELS[a]!.localeCompare(SOURCE_LABELS[b]!));
  for (const source of ['url', ...providers]) {
    const label = SOURCE_LABELS[source]!, ok = ready[source]!.ready;
    items.push({ terms: ['clone', 'remote', 'repository', 'repo', 'git', label, ...(ok ? [] : ['setup required'])],
      row: row({ key: source, kind: ok ? 'action' : 'disabled', op: 'page', arg: `${page}/clone:${source}`, icon: `source-${source}`, title: source === 'url' ? 'Git URL' : `${label} repository`,
        description: source === 'url' ? 'Clone from a remote URL' : `Clone ${label} ${SOURCE_HINTS[source]}`, badge: ok ? '' : 'Setup Required', badgeOp: ok ? '' : 'source-control' }) });
  }
  return items;
}

/** The Directories group for a browse query (filterFilesystemBrowseEntries + buildBrowseGroups). */
async function browseGroups(client: T3Client, native: Native, query: string, label: string, cwd: string): Promise<{ groups: Group[]; exact: string; parent: string; pending: boolean; error: string }> {
  const directory = browseDirectory(query), leaf = hasTrailingSeparator(query) ? '' : browseLeaf(query);
  const flow = flowOf(client), key = `${client.generation}:${directory}:${cwd}`;
  if (flow.browse?.key !== key) {
    try { flow.browse = { key, value: await client.restAccess(native).request('filesystem.browse', { partialPath: directory, ...(cwd ? { cwd } : {}) }), error: '' }; }
    catch (failure) { flow.browse = { key, value: {}, error: failure instanceof Error ? failure.message : 'Could not read that folder.' }; }
  }
  const entries = arr(flow.browse.value.entries), lower = leaf.toLowerCase(), hidden = leaf.startsWith('.');
  const visible = entries.filter(entry => str(entry.name).toLowerCase().startsWith(lower) && (hidden || !str(entry.name).startsWith('.')));
  const exact = leaf ? str(visible.find(entry => str(entry.name) === leaf)?.fullPath) : '';
  const parentPath = browseParent(directory);
  const items: Item[] = [];
  if (parentPath !== null && directory !== '~/') items.push({ terms: [query, '..'], row: row({ key: 'up', op: 'browse-to', arg: parentPath, icon: 'corner-left-up', title: '..' }) });
  for (const entry of visible) items.push({ terms: [query, str(entry.fullPath), str(entry.name)], row: row({ key: str(entry.fullPath), op: 'browse-to', arg: appendSegment(query, str(entry.name)), icon: 'folder', title: str(entry.name) }) });
  return { groups: [{ value: 'directories', label, items }], exact, parent: str(flow.browse.value.parentPath), pending: false, error: flow.browse.error };
}

export type AddContext = { page: string; query: string; highlighted: boolean };
/** Every add-project page; the root's browse mode (a path typed at the root) too. */
export async function addProjectView(client: T3Client, native: Native | null | undefined, context: AddContext): Promise<PaletteView> {
  const { page, query } = context;
  const parent = page.includes('/') ? page.slice(0, page.lastIndexOf('/')) : '';
  const submenu = page !== '';
  const base: PaletteView = { ...closedView, open: true, page, parent, back: submenu, backHint: submenu, addon: submenu ? 'back' : 'search', placeholder: 'Search...', enterLabel: 'Select' };
  if (!native?.available || !client.ready) return { ...base, empty: 'Connect an environment to add projects.' };
  const current = client.shell.projects.find(project => project.id === client.projectId);
  const cwd = str(current?.workspaceRoot);
  const leafPage = page.split('/').pop() ?? '';
  const flow = flowOf(client);
  const discovery = await discover(client, native);
  // New project: the name step.
  if (leafPage === 'new-project') {
    const root = str(client.config.newProjectsRoot), name = query.trim();
    const account = githubAccount(discovery);
    const preview = root ? cloneDestination(root, newProjectFolder(name)) : '';
    const groups: Group[] = [];
    if (account !== null && root) groups.push({ value: 'new-project-options', label: 'Options', items: [{ terms: [], row: row({ key: 'github', op: 'flow', arg: 'toggle-publish', icon: 'source-github',
      title: 'Create private repository on GitHub', description: name ? (account ? `${account}/${preview.split('/').pop()}` : preview.split('/').pop() ?? '') : (account || 'Your GitHub account'), checkbox: true, checked: flow.publish }) }] });
    groups.push({ value: 'new-project-existing', label: '', items: [{ terms: [], row: row({ key: 'existing', op: 'page', arg: parent.endsWith('add-project') ? parent : 'add-project', icon: 'folder-plus',
      title: 'Add existing project', description: 'Open a folder or clone a repository', trailing: 'chevron' }) }] });
    const rows = flatten(groups);
    return { ...base, addon: 'back', placeholder: 'Project name', rows, count: rows.length, autoHighlight: false, popOnEmpty: false,
      enterLabel: context.highlighted ? (rows.length > 1 ? 'Select' : 'Select') : 'Create', enterOp: 'flow', enterArg: 'new-project', enterArg2: name,
      accessory: 'Create', accessoryKey: 'Enter', accessoryEnabled: name.length > 0, accessoryOp: 'flow', accessoryArg: 'new-project', accessoryArg2: name, inputPaddingRight: 128,
      contextLabel: '', contextTitle: name || 'New project', contextDescription: root ? (name ? `Creates ${preview}` : `Goes in ${root}`) : '', contextIcon: root ? 'folder-git' : '' };
  }
  // Clone: the repository step, then the destination step.
  const cloneSource = leafPage.startsWith('clone:') ? leafPage.slice(6) : '';
  if (cloneSource && !(flow.clone && flow.clone.source === cloneSource && page.endsWith('/confirm'))) {
    const label = SOURCE_LABELS[cloneSource] ?? 'Git URL', button = cloneSource === 'url' ? 'Continue' : 'Lookup';
    return { ...base, addon: 'back', placeholder: cloneSource === 'url' ? 'Enter Git clone URL' : `Enter ${label} repository (${SOURCE_HINTS[cloneSource]})`, autoHighlight: false,
      empty: cloneSource === 'url' ? 'Enter a Git clone URL and press Enter to continue.' : 'Enter a repository path and press Enter to look it up.',
      enterLabel: button, enterOp: 'flow', enterArg: `clone-repository:${cloneSource}`, enterArg2: query.trim(), accessory: button, accessoryKey: 'Enter', accessoryEnabled: query.trim().length > 0,
      accessoryOp: 'flow', accessoryArg: `clone-repository:${cloneSource}`, accessoryArg2: query.trim(), inputPaddingRight: 128, contextIcon: '' };
  }
  const confirming = page.endsWith('/confirm') && flow.clone !== null;
  // Browsing: a typed path (root or submenu), Local folder, and the clone destination step.
  if (confirming || leafPage === 'browse' || isBrowseQuery(query)) {
    const relativeBlocked = (query.startsWith('./') || query.startsWith('../')) && !cwd;
    const browse = relativeBlocked ? { groups: [] as Group[], exact: '', parent: '', pending: false, error: '' }
      : await browseGroups(client, native, query, confirming ? 'Select where to clone' : 'Directories', cwd);
    const filtered = browse.groups[0]!.items.length ? browse.groups : [];
    const rows = flatten(filtered);
    const resolved = hasTrailingSeparator(query) ? (browse.parent || query.trim()) : (browse.exact || query.trim());
    const willCreate = query.trim().length > 0 && !context.highlighted && (hasTrailingSeparator(query) ? !browse.parent : !browse.exact);
    const label = confirming ? (willCreate ? 'Create & Clone' : 'Clone') : (willCreate ? 'Create & Add' : 'Add');
    const submitOp = confirming ? 'clone' : 'add-path';
    const clone = flow.clone;
    return { ...base, addon: submenu ? 'back' : 'folder-plus', placeholder: submenu ? 'Enter path (e.g. ~/projects/my-app)' : 'Enter project path (e.g. ~/projects/my-app)', rows, count: rows.length,
      autoHighlight: false, popOnEmpty: leafPage === 'browse', enterLabel: context.highlighted ? 'Select' : '', enterOp: 'flow', enterArg: submitOp, enterArg2: resolved,
      accessory: label, accessoryKey: context.highlighted ? '⌘ Enter' : 'Enter', accessoryEnabled: !relativeBlocked && query.trim().length > 0, accessoryOp: 'flow', accessoryArg: submitOp, accessoryArg2: resolved,
      inputPaddingRight: willCreate ? 152 : context.highlighted ? 120 : 96, footerAction: client.origin.includes('127.0.0.1') || client.origin.includes('localhost') ? 'Open in Finder' : '',
      contextLabel: confirming && clone ? 'Repository' : '', contextTitle: clone && confirming ? clone.title : '', contextDescription: clone && confirming ? clone.description : '', contextIcon: clone && confirming ? `source-${clone.source}` : '',
      empty: rows.length ? '' : relativeBlocked ? 'Relative paths require an active project.' : confirming ? 'Choose a destination path and press Enter to clone.'
        : willCreate ? 'Press Enter to create this folder and add it as a project.' : browse.error || '' };
  }
  // Sources.
  const ready = readiness(discovery);
  const groups = filterGroups([{ value: `sources`, label: 'Sources', items: sourceItems(client, ready, page || 'add-project') }], query, true, { projects: [], settings: [], threads: [] });
  const rows = flatten(groups);
  return { ...base, rows, count: rows.filter(entry => entry.index >= 0).length, empty: rows.length ? '' : 'No matching commands, projects, or threads.' };
}
