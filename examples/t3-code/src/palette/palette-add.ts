// Adapted from T3 Code 9c161462c9 (MIT); see LICENSE-T3.
import { sourceControlHosts } from '../shared/source-control-hosts';
// The palette's Add project pages: reference CommandPalette.tsx (buildAddProjectSourceGroups,
// startAddProjectBrowse, the clone flow, the New project name step) with the path helpers of
// packages/client-runtime/src/state/projects.ts and operations/projects.ts.
import type { T3Client } from '../client/client';
import { arr, obj, str, type Obj } from '../client/domain';
import type { Native } from '../client/protocol';
import { closedView, filterGroups, flatten, row, type Group, type Item, type PaletteView, type SearchExtra } from './palette';
import { letGo } from '../client/let-go';
import { addProjectEnvironments, addTarget, isNewProjectPage, newProjectEnvironmentId, newProjectEnvironments, pageEnvironmentId, targetRequest, type AddEnvironment } from './palette-environments';
import { fleet, type EnvironmentFleet } from '../connections/settings-b-fleet';

export const SOURCES = sourceControlHosts.map(host => host.kind);
export const SOURCE_LABELS: Record<string, string> = { url: 'Git URL', ...Object.fromEntries(sourceControlHosts.map(host => [host.kind, host.pickerLabel])) };
const SOURCE_HINTS: Record<string, string> = { url: 'URL', ...Object.fromEntries(sourceControlHosts.map(host => [host.kind, host.pathHint])) };

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
/** getAddProjectInitialQueryForEnvironment: the environment's base directory, else `~/`. */
export function initialBrowseQuery(config: Obj): string {
  const base = str(obj(config.settings).addProjectBaseDirectory).trim();
  return base ? ensureDirectory(base) : '~/';
}

// ── Flow state (per client): the clone step's repository and the New project option ──
export type CloneFlow = { source: string; repositoryInput: string; title: string; description: string; remoteUrl: string; pinned: string };
type Flow = { clone: CloneFlow | null; publish: boolean; publishKind: string; discovery: { key: string; value: Obj } | null; discoveryFailed?: { key: string; revision: number }; browse: { key: string; value: Obj; error: string } | null };
const flows = new WeakMap<T3Client, Flow>();
export function flowOf(client: T3Client): Flow {
  let flow = flows.get(client);
  if (!flow) { flow = { clone: null, publish: false, publishKind: '', discovery: null, browse: null }; flows.set(client, flow); }
  return flow;
}

export type Readiness = Record<string, { ready: boolean; hint: string }>;
/** buildAddProjectRemoteSourceReadiness over server.discoverSourceControl. */
export function readiness(discovery: Obj | null): Readiness {
  const unavailable = { ready: false, hint: 'Provider status unavailable. Open Settings -> Source control and rescan.' };
  const result: Readiness = { url: { ready: true, hint: '' }, ...Object.fromEntries(SOURCES.map(source => [source, unavailable])) };
  if (!discovery) return result;
  for (const source of SOURCES) {
    const provider = arr(discovery.sourceControlProviders).find(candidate => str(candidate.kind) === source);
    if (!provider) continue;
    const auth = obj(provider.auth), detail = typeof auth.detail === 'string' ? auth.detail : str(obj(auth.detail).value);
    if (str(provider.status) !== 'available') result[source] = { ready: false, hint: str(provider.installHint) };
    else if (str(auth.status) === 'unauthenticated') result[source] = { ready: false, hint: detail || `${str(provider.label)} is not authenticated. Open Settings -> Source control for setup guidance.` };
    else result[source] = { ready: true, hint: '' };
  }
  return result;
}
export function newProjectPublishTargets(discovery: Obj | null) {
  const ready = readiness(discovery);
  return sourceControlHosts.flatMap(definition => {
    if (!definition.inferOwner || !ready[definition.kind]?.ready) return [];
    const account = obj(arr(obj(discovery).sourceControlProviders).find(provider => provider.kind === definition.kind)?.auth).account;
    const owner = typeof account === 'string' ? account : str(obj(account).value);
    return owner ? [{ definition, owner }] : [];
  });
}
/** sourceControlEnvironment.discovery on the environment being added to. */
async function discover(client: T3Client, native: Native, target: AddEnvironment): Promise<Obj | null> {
  const flow = flowOf(client), key = `${target.environmentId}:${target.focused ? client.origin : 'fleet'}:${target.generation}`;
  if (flow.discovery?.key === key) return flow.discovery.value;
  // A failed discovery (often an answer let go mid-request) is not kept: the reference's query
  // asks again, so one lost request must not mark every provider "Setup Required" for the session.
  // Bounded: after a failure the next ask waits for the client's state to move (its revision), not every palette read.
  if (flow.discoveryFailed?.key === key && flow.discoveryFailed.revision === client.revision) return {};
  try { flow.discovery = { key, value: await targetRequest(client, native, target, 'server.discoverSourceControl', {}) }; }
  catch { flow.discoveryFailed = { key, revision: client.revision }; return {}; }
  return flow.discovery.value;
}

export function sourceItems(config: Obj, ready: Readiness, page: string): Item[] {
  const items: Item[] = [];
  if (str(config.newProjectsRoot)) items.push({ terms: ['new project', 'create', 'empty', 'repository', 'git init'],
    row: row({ key: 'new', op: 'page', arg: `${page}/new-project`, icon: 'folder-git', title: 'New project', description: 'Start a new Git repository from a name' }) });
  items.push({ terms: ['local', 'folder', 'directory', 'browse'], row: row({ key: 'local', op: 'browse', arg: `${page}/browse`, arg2: initialBrowseQuery(config), icon: 'folder-plus', title: 'Local folder', description: 'Browse a folder on disk' }) });
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
async function browseGroups(client: T3Client, native: Native, target: AddEnvironment, query: string, label: string, cwd: string): Promise<{ groups: Group[]; exact: string; parent: string; pending: boolean; error: string }> {
  const directory = browseDirectory(query), leaf = hasTrailingSeparator(query) ? '' : browseLeaf(query);
  const flow = flowOf(client), key = `${target.environmentId}:${target.generation}:${directory}:${cwd}`;
  if (flow.browse?.key !== key) {
    try { flow.browse = { key, value: await targetRequest(client, native, target, 'filesystem.browse', { partialPath: directory, ...(cwd ? { cwd } : {}) }), error: '' }; }
    catch (failure) { if (letGo(failure)) throw failure; flow.browse = { key, value: {}, error: failure instanceof Error ? failure.message : 'Could not read that folder.' }; }
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

/** buildEnvironmentItem: an environment of the Environments step; choosing it opens its Sources. */
export function environmentItem(option: AddEnvironment): Item {
  return { terms: [option.label, option.environmentId, option.isPrimary ? 'this device' : ''],
    row: row({ key: option.environmentId, op: 'page', arg: `add-project/env:${option.environmentId}`, icon: 'machine', project: option.machine, title: option.label,
      description: option.isPrimary ? 'This device' : option.environmentId }) };
}

/**
 * New project's Environments (CommandPalette.tsx newProjectMachineGroup): buildEnvironmentItem, the chosen one checked;
 * choosing one moves the project there (switchNewProjectEnvironment).
 */
function newProjectEnvironmentItem(option: AddEnvironment, chosen: boolean): Item {
  return { terms: [], row: row({ key: option.environmentId, op: 'flow', arg: 'new-project-environment', arg2: option.environmentId, icon: 'machine', project: option.machine,
    title: option.label, description: option.isPrimary ? 'This device' : option.environmentId, trailing: chosen ? 'check' : '' }) };
}
/**
 * showExistingProjectSources: back to the Sources page New project came from while its environment is the chosen one,
 * else the chosen environment's Sources in that page's place: under the Environments step when it came from there,
 * else on their own (`env:<id>`, whose Back is the root, as the reference pops New project before it pushes them).
 * With one connected environment `add-project` is its Sources.
 */
function existingProjectSources(client: T3Client, parent: string, chosen: string, source: EnvironmentFleet): string {
  const options = addProjectEnvironments(client, source);
  const fromSources = parent === 'add-project' || /^(add-project\/)?env:[^/]+$/.test(parent);
  if (fromSources && (pageEnvironmentId(parent) || options[0]?.environmentId) === chosen) return parent;
  if (options.length <= 1 || !chosen) return 'add-project';
  return parent.startsWith('add-project') ? `add-project/env:${chosen}` : `env:${chosen}`;
}

/** `fallback`: what a list page (Environments, Sources) searches when nothing on it matches, the root's results. */
export type AddContext = { page: string; query: string; highlighted: boolean; fallback?: SearchExtra };
/**
 * Every add-project page; the root's browse mode (a path typed at the root) too. With several connected
 * environments `add-project` is the Environments step and each one's Sources is `add-project/env:<id>`
 * (openAddProjectFlow), or `env:<id>` when the root's New project opens them; with one, `add-project` is its
 * Sources. The later pages nest under it.
 */
export async function addProjectView(client: T3Client, native: Native | null | undefined, context: AddContext, source: EnvironmentFleet = fleet): Promise<PaletteView> {
  const { page, query } = context;
  const parent = page.includes('/') ? page.slice(0, page.lastIndexOf('/')) : '';
  const submenu = page !== '';
  const base: PaletteView = { ...closedView, open: true, page, parent, back: submenu, backHint: submenu, addon: submenu ? 'back' : 'search', placeholder: 'Search...', enterLabel: 'Select' };
  if (!native?.available) return { ...base, empty: 'Connect an environment to add projects.' };
  const options = addProjectEnvironments(client, source);
  if (page === 'add-project' && options.length > 1 && !isBrowseQuery(query)) {
    const rows = flatten(filterGroups([{ value: 'environments', label: 'Environments', items: options.map(environmentItem) }], query, true, context.fallback ?? { projects: [], settings: [], threads: [] }));
    return { ...base, rows, count: rows.filter(entry => entry.index >= 0).length, empty: rows.length ? '' : 'No matching commands, projects, or threads.' };
  }
  if (isNewProjectPage(page)) return await newProjectView(client, native, context, base, parent, source);
  const target = addTarget(client, page, source);
  if (!target) return { ...base, empty: 'Connect an environment to add projects.' };
  // Relative paths resolve against the current project only on its own environment (currentProjectCwdForBrowse).
  const current = target.focused ? client.shell.projects.find(project => project.id === client.projectId) : undefined;
  const cwd = str(current?.workspaceRoot);
  const leafPage = page.split('/').pop() ?? '';
  const flow = flowOf(client);
  const discovery = await discover(client, native, target);
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
      : await browseGroups(client, native, target, query, confirming ? 'Select where to clone' : 'Directories', cwd);
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
      inputPaddingRight: willCreate ? 152 : context.highlighted ? 120 : 96, footerAction: target.isPrimary ? 'Open in Finder' : '', // canOpenProjectFromFileManager: the primary environment only (local-primary.ts)
      contextLabel: confirming && clone ? 'Repository' : '', contextTitle: clone && confirming ? clone.title : '', contextDescription: clone && confirming ? clone.description : '', contextIcon: clone && confirming ? `source-${clone.source}` : '',
      empty: rows.length ? '' : relativeBlocked ? 'Relative paths require an active project.' : confirming ? 'Choose a destination path and press Enter to clone.'
        : willCreate ? 'Press Enter to create this folder and add it as a project.' : browse.error || '' };
  }
  // Sources.
  const ready = readiness(discovery);
  const groups = filterGroups([{ value: `sources`, label: 'Sources', items: sourceItems(target.config, ready, page || 'add-project') }], query, true, context.fallback ?? { projects: [], settings: [], threads: [] });
  const rows = flatten(groups);
  return { ...base, rows, count: rows.filter(entry => entry.index >= 0).length, empty: rows.length ? '' : 'No matching commands, projects, or threads.' };
}

/**
 * New project: the name step (CommandPalette.tsx newProjectFlow). Environments (newProjectMachineGroup) lists those that
 * can make a project when there are several, or when the chosen one no longer can; then the publish option, on the
 * chosen environment's discovery, then Add existing project.
 */
async function newProjectView(client: T3Client, native: Native, context: AddContext, base: PaletteView, parent: string, source: EnvironmentFleet): Promise<PaletteView> {
  const options = newProjectEnvironments(client, source), chosen = newProjectEnvironmentId(client, context.page, source);
  const target = addTarget(client, context.page, source), flow = flowOf(client), name = context.query.trim();
  const root = str(target?.config.newProjectsRoot);
  const targets = target ? newProjectPublishTargets(await discover(client, native, target)) : [];
  const publish = targets.find(candidate => candidate.definition.kind === flow.publishKind) ?? targets[0];
  const preview = root ? cloneDestination(root, newProjectFolder(name)) : '';
  const groups: Group[] = [];
  if (options.length > 1 || (options.length > 0 && !options.some(option => option.environmentId === chosen))) {
    groups.push({ value: 'new-project-machines', label: 'Environments', items: options.map(option => newProjectEnvironmentItem(option, option.environmentId === chosen)) });
  }
  if (publish && root) {
    groups.push({ value: 'new-project-options', label: 'Options', items: [{ terms: [], row: row({ key: 'publish', op: 'flow', arg: 'toggle-publish', icon: `source-${publish.definition.kind}`,
      title: targets.length > 1 ? 'Create private repository' : `Create private repository on ${publish.definition.label}`, description: name ? `${publish.owner}/${preview.split('/').pop()}` : publish.owner, checkbox: true, checked: flow.publish,
      publishHost: publish.definition.kind, publishHostLabel: publish.definition.label,
      publishHosts: targets.length > 1 ? targets.map(candidate => ({ value: candidate.definition.kind, label: candidate.definition.label, selected: candidate === publish })) : [] }) }] });
  }
  groups.push({ value: 'new-project-existing', label: '', items: [{ terms: [], row: row({ key: 'existing', op: 'page', arg: existingProjectSources(client, parent, chosen, source), icon: 'folder-plus',
    title: 'Add existing project', description: 'Open a folder or clone a repository', trailing: 'chevron' }) }] });
  const rows = flatten(groups);
  return { ...base, addon: 'back', placeholder: 'Project name', rows, count: rows.length, autoHighlight: false, popOnEmpty: false,
    // Enter runs a highlighted row (the footer reads Toggle on the publish option, palette.contract's enterText); ⌘ Enter
    // always creates (inputKey).
    enterLabel: context.highlighted ? 'Select' : 'Create', enterOp: 'flow', enterArg: 'new-project', enterArg2: name,
    accessory: 'Create', accessoryKey: context.highlighted ? '⌘ Enter' : 'Enter', accessoryEnabled: name.length > 0, accessoryOp: 'flow', accessoryArg: 'new-project', accessoryArg2: name,
    inputPaddingRight: context.highlighted ? 152 : 128,
    contextLabel: '', contextTitle: name || 'New project', contextDescription: root ? (name ? `Creates ${preview}` : `Goes in ${root}`) : '', contextIcon: root ? 'folder-git' : '' };
}
