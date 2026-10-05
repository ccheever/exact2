// The empty main view (lane "pages"): the draft hero and its project menu
// (DraftHeroHeadline.tsx), the index landings (routes/_chat.index.tsx:
// NoProjectsHero, DraftStartError, HostedStaticOnboardingState) and the
// project retargeting the hero menu performs. Sources: T3 Code (MIT, see
// LICENSE-T3) apps/web/src/components/chat/DraftHeroHeadline.tsx,
// components/NoProjectsHero.tsx, components/Sidebar.logic.ts and
// packages/client-runtime/src/state/threadSort.ts.
import { arr, obj, str, applyShell, type Obj, type Shell } from './domain';
import { ClientError, bridgeReply, providerAvailable, type Native } from './protocol';
import { projectIdentity } from './presentation';
import { applySticky } from './composer-controls';
import type { T3Client } from './client';
import { welcomeShowing } from './pages-welcome';
import { cachedDisplayNames, heroGroups } from './r6-polish-groups';
import { openScratchProject } from './r11-upstream-scratch';

const timestamp = (value: unknown): number | null => {
  const parsed = typeof value === 'string' ? Date.parse(value) : NaN;
  return Number.isFinite(parsed) ? parsed : null;
};
const normalizePath = (value: unknown) => str(value).trim().replace(/\\/g, '/').replace(/\/+$/, '');

/** getThreadSortTimestamp(thread, "updated_at"): the latest user message, else updated/created. */
export function threadActivity(thread: Obj): number {
  const latest = timestamp(thread.latestUserMessageAt);
  if (latest !== null) return latest;
  let user: number | null = null;
  for (const message of arr(thread.messages)) {
    if (message.role !== 'user') continue;
    const at = timestamp(message.createdAt);
    if (at !== null) user = user === null ? at : Math.max(user, at);
  }
  return user ?? timestamp(thread.updatedAt) ?? timestamp(thread.createdAt) ?? Number.NEGATIVE_INFINITY;
}

/** sortProjectsForSidebar(projects, threads, "updated_at"): most recent activity first, then title. */
export function projectsByActivity(shell: Shell): Obj[] {
  const latest = new Map<string, number>();
  for (const thread of shell.threads) {
    if (thread.archivedAt) continue;
    const key = str(thread.projectId);
    latest.set(key, Math.max(latest.get(key) ?? Number.NEGATIVE_INFINITY, threadActivity(thread)));
  }
  return shell.projects
    .map(project => ({ project, at: latest.has(str(project.id)) ? latest.get(str(project.id))!
      : timestamp(project.updatedAt) ?? timestamp(project.createdAt) ?? Number.NEGATIVE_INFINITY }))
    .sort((left, right) => (right.at === left.at ? 0 : right.at > left.at ? 1 : -1)
      || str(left.project.title).localeCompare(str(right.project.title)) || str(left.project.id).localeCompare(str(right.project.id)))
    .map(entry => entry.project);
}

/** The index landing's target: the most recently active project (IndexDraftLanding). */
export function mostRecentProjectId(shell: Shell): string {
  return str(projectsByActivity(shell)[0]?.id);
}

/** ServerConfig.scratchWorkspaceRoot, the folder behind "No project" threads, when offered. */
export function scratchRoot(config: Obj): string {
  return str(config.scratchWorkspaceRoot).trim();
}
export function isScratchProject(project: Obj | undefined, root: string): boolean {
  return !!project && !!root && normalizePath(project.workspaceRoot) === normalizePath(root);
}

export type HeroProject = { id: string; name: string; mark: string; ink: string; surface: string; selected: boolean };
export type PagesHome = {
  landing: string; headline: string; projectName: string; projectNames: { id: string }[]; projects: HeroProject[];
  noProjectItem: boolean; scratchLine: boolean; scratchDraft: boolean; noProjectsDescription: string;
  scratchStart: boolean; savedEnvironments: number; firstRunPending: boolean; pullRequests: boolean;
};

/** The hero menu rows: the current project first, then the rest by activity; the Scratch project is "No project". */
export function heroProjects(client: Pick<T3Client, 'shell' | 'projectId' | 'config'>): HeroProject[] {
  const root = scratchRoot(client.config);
  const ordered = projectsByActivity(client.shell);
  const current = ordered.findIndex(project => project.id === client.projectId);
  if (current > 0) ordered.unshift(...ordered.splice(current, 1));
  return ordered.filter(project => !isScratchProject(project, root)).map(project => {
    const name = str(project.title, 'Untitled project');
    const identity = projectIdentity(name);
    return { id: str(project.id), name, mark: identity.projectMark, ink: identity.projectInk, surface: identity.projectSurface, selected: project.id === client.projectId };
  });
}

/**
 * Which empty view the main column shows. "no-environment": nothing paired
 * on this device (HostedStaticOnboardingState); "offline": a saved environment
 * that is not connected (the reference renders nothing until it bootstraps);
 * "no-projects": connected with no project (NoProjectsHero); "start-error":
 * projects exist but no draft target resolved (DraftStartError); "" the hero.
 */
export function landingKind(input: { connected: boolean; ready: boolean; savedEnvironments: number; connecting: boolean; projects: number; projectId: string; threadId: string }): string {
  if (!input.connected) return input.savedEnvironments === 0 && !input.connecting ? 'no-environment' : 'offline';
  if (!input.ready) return 'offline';
  if (input.projects === 0) return 'no-projects';
  if (!input.projectId && !input.threadId) return 'start-error';
  return '';
}

/** DraftHeroHeadline's four sentences: build, work (scratch draft), start (choose), add. */
export function heroHeadline(input: { scratchDraft: boolean; resolved: boolean; canChoose: boolean }): string {
  return input.scratchDraft ? 'work' : input.resolved ? 'build' : input.canChoose ? 'start' : 'add';
}

export async function savedEnvironmentCount(native: Native | null | undefined): Promise<number> {
  if (!native?.available) return 0;
  try {
    const reply = await bridgeReply(native, { op: 'environments' });
    return reply.ok ? arr(obj(reply.value).saved).filter(entry => str(entry.origin) && str(entry.environmentId)).length : 0;
  } catch { return 0; }
}

/** The resource behind the hero and the index landings. */
export async function pagesHome(client: T3Client, native: Native | null | undefined, firstRunPending = welcomeShowing(client)): Promise<PagesHome> {
  const saved = await savedEnvironmentCount(native);
  const root = client.ready ? scratchRoot(client.config) : '';
  const project = client.shell.projects.find(candidate => candidate.id === client.projectId);
  const scratchDraft = isScratchProject(project, root);
  const projects = client.ready ? heroGroups(client, heroProjects(client)) : []; // r6-polish: one row per logical project, by its group's name
  const resolved = !!project;
  const headline = heroHeadline({ scratchDraft, resolved, canChoose: projects.length > 0 });
  const connected = client.connection === 'connected';
  // r6-polish: the logical project's name (r6-polish-groups.ts), as DraftHeroHeadline's activeProjectDisplayName.
  const projectName = scratchDraft ? 'No project' : (project && cachedDisplayNames(client).get(client.projectId)) || str(project?.title, 'Choose a project');
  return {
    // /welcome renders NoProjectsHero beneath the wizard.
    landing: firstRunPending ? 'no-projects' : landingKind({ connected, ready: client.ready, savedEnvironments: saved, connecting: ['connecting', 'reconnecting'].includes(client.connection),
      projects: client.shell.projects.length, projectId: client.projectId, threadId: client.threadId }),
    headline, projectName, projectNames: [{ id: projectName }], projects,
    noProjectItem: !!root, scratchDraft,
    // "or start without a project" sits under the heading only while a real project (or choice) shows.
    scratchLine: !!root && !scratchDraft && (resolved || projects.length > 0),
    scratchStart: !!root,
    noProjectsDescription: root ? 'Add a project, or start without one.' : 'Add a project to start your first thread.',
    savedEnvironments: saved, firstRunPending,
    pullRequests: obj(obj(client.config.environment).capabilities).pullRequests === true,
  };
}

// ── Hero retargeting (pages:hero-*) ─────────────────────────────────────────

type ModelPick = { providerId: string; modelId: string; modelOptions: Obj[]; runtimeMode: string; interactionMode: string };
export type HeroCarry = { projectId: string; text: string; images: Obj[]; explicit: ModelPick | null; fromKey: string };
type HeroHost = Pick<T3Client, 'shell' | 'config' | 'projectId' | 'threadId' | 'environmentId' | 'local' | 'draftKey' | 'providerId' | 'modelId' | 'modelOptions' | 'runtimeMode' | 'interactionMode' | 'writable' | 'ready'>;

/** The model a fresh draft in this project starts with (T3Client.chooseDefaults). */
export function defaultPick(client: Pick<T3Client, 'shell' | 'config'> & Partial<Pick<T3Client, 'local'>>, projectId: string): { providerId: string; modelId: string } {
  const project = client.shell.projects.find(candidate => candidate.id === projectId);
  const settings = obj(client.config.settings);
  const overrides = obj(obj(settings.projectSettingsOverrides)[projectId]);
  const selection = obj(overrides.defaultModelSelection || project?.defaultModelSelection || settings.defaultModelSelection);
  const providers = arr(client.config.providers);
  const automatic = !str(selection.instanceId);
  const provider = automatic
    ? providers.find(candidate => candidate.status === 'ready' && providerAvailable(candidate)) || providers.find(providerAvailable)
    : providers.find(candidate => candidate.instanceId === selection.instanceId && providerAvailable(candidate));
  const models = arr(provider?.models);
  const model = automatic ? models.find(entry => entry.isDefault === true && entry.isCustom !== true) || models.find(entry => entry.isCustom !== true) || models[0]
    : models.find(entry => entry.slug === selection.model);
  const pick = { config: client.config, local: (client as Partial<T3Client>).local, providerId: provider && model ? str(provider.instanceId) : '', modelId: provider && model ? str(model.slug) : '', modelOptions: [] as Obj[] };
  // The composer's sticky model (applyStickyState) opens every new draft ahead of the configured default.
  if (pick.local) applySticky(pick as unknown as T3Client);
  return { providerId: pick.providerId, modelId: pick.modelId };
}

/**
 * Before the draft changes project: what moves with it. The prompt and its
 * images stay in the same composer session; a model the person picked stays
 * (hasExplicitComposerModelSelection), otherwise the new project's default
 * applies. `op` is "project" (menu row), "scratch" (No project) or "retry".
 */
export async function heroCarry(client: HeroHost, op: string, id: string, ensureScratch: () => Promise<string>): Promise<HeroCarry> {
  if (!client.ready) throw new ClientError('Wait for T3 to synchronize before choosing a project.');
  if (op === 'retry') return { projectId: mostRecentProjectId(client.shell), text: '', images: [], explicit: null, fromKey: '' };
  if (client.threadId) throw new ClientError('Choose a project from a new thread.');
  let projectId = id;
  if (op === 'scratch') {
    if (!scratchRoot(client.config)) throw new ClientError('This server cannot start threads without a project.');
    if (!client.writable) throw new ClientError('This connection is read-only.');
    projectId = await ensureScratch();
  } else if (op !== 'project') throw new ClientError(`Unknown action: hero-${op}`);
  if (!client.shell.projects.some(project => project.id === projectId)) throw new ClientError('That project is no longer available.');
  const fromKey = client.draftKey;
  const defaults = defaultPick(client, client.projectId);
  // Re-choosing the open project changes nothing (the menu's onValueChange ignores it).
  const explicit = client.providerId && (projectId === client.projectId || client.providerId !== defaults.providerId || client.modelId !== defaults.modelId)
    ? { providerId: client.providerId, modelId: client.modelId, modelOptions: client.modelOptions, runtimeMode: client.runtimeMode, interactionMode: client.interactionMode } : null;
  return { projectId, text: client.local.drafts[fromKey] || '', images: client.local.snapshotDrafts[fromKey] || [], explicit, fromKey };
}

/** After the switch: the prompt lands in the new project's draft, and an explicit model choice is restored. */
export function heroLand(client: HeroHost, carry: HeroCarry): void {
  if (!carry.fromKey) return;
  const toKey = client.draftKey;
  if (toKey !== carry.fromKey && carry.text) { client.local.drafts[toKey] = carry.text; delete client.local.drafts[carry.fromKey]; }
  if (toKey !== carry.fromKey && carry.images.length) { client.local.snapshotDrafts[toKey] = carry.images; delete client.local.snapshotDrafts[carry.fromKey]; }
  const pick = carry.explicit;
  if (pick && arr(client.config.providers).some(provider => provider.instanceId === pick.providerId && providerAvailable(provider)
    && arr(provider.models).some(model => model.slug === pick.modelId))) {
    client.providerId = pick.providerId; client.modelId = pick.modelId; client.modelOptions = pick.modelOptions;
    client.runtimeMode = pick.runtimeMode; client.interactionMode = pick.interactionMode;
  }
}

/** projects.ensureScratch, then the shell row it created (useScratchProject.openScratchProject). */
export async function ensureScratchProject(client: T3Client, native: Native): Promise<string> {
  return openScratchProject(client, native); // r11-upstream (845ddd9354): the shared opener
}
