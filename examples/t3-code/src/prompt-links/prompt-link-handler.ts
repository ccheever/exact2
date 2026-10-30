// T3 Code 6d450a4284 (MIT, LICENSE-T3): components/promptLinks/{NewThreadLinkView,ThreadLinkPrompt}.tsx, and
// components/desktop/DesktopWebLinkCoordinator.tsx for a web link, an HTML file or a `t3code://pair` link the system hands the app.
// The connection fleet supplies the reference's environment registry; draft and checkout operations are shared with the app.
import type { T3Client } from '../client/client';
import { arr, obj, str, type Obj } from '../client/domain';
import { ClientError, type Files, type Native } from '../client/protocol';
import { letGo } from '../client/let-go';
import { fleet, fleetThreadId, focusFleetProject, focusFleetThread } from '../connections/settings-b-fleet';
import { openScratchProject } from '../composer/scratch-project';
import { openRemoteScratch } from '../composer/scratch-machine';
import { stage } from '../composer/composer-controls';
import { projectsByActivity, isScratchProject } from '../pages/pages-home';
import { backgroundShellsBootstrapped } from '../pages/home-landing';
import { prEnvironments, type PrEnvironment } from '../pull-requests/pages-pr-environments';
import { actOnHandoff } from '../pull-requests/pages-pr-acton';
import { cloneDestination } from '../palette/palette-add';
import { startTrackedClone } from '../shell/project-clones-live';
import { pushToast } from '../shell/toast';
import { findPromptLinkProject, findPromptLinkPullRequestThread, resolvePromptLinkModelSelection } from './prompt-link-model';
import { primaryEnvironmentId } from '../connections/local-primary';
import { placeLinkedPrompt, warnAboutHiddenCharacters } from './linked-prompt';
import { parseNewThreadLink, parseThreadLinkPrompt, promptLinkAppPath, promptLinkPullRequestUrl, type NewThreadLink, type PromptLinkRepository } from './prompt-links';
import { isWebUrl, openFileInPreview, openUrlInPreview } from '../browser/browser-links';
import { activeRef } from '../terminal/terminal-drawer-view';
import type { ScopedThreadRef } from '../terminal/terminal-ui-state';
import { surfaceStore } from '../panel/panel-surfaces';
import { ensureDraftThreadId } from '../composer/draft-thread-id';
import { prLinksOpenInApp } from '../panel/pr-surface';
import { pullRequestPageSelection } from '../sidebar/legacy-sidebar-links';
import { readPairingAppLink, resolveRemotePairingTarget } from '../connections/remote';
import { runConnectionOp } from '../connections/connections';

type Project = Obj & { environmentId: string };
type Pending = { link: NewThreadLink; repository: PromptLinkRepository; project?: Project };
type State = { status: 'missing-repository' | 'pull-request-checkout' | 'failed' | 'opening'; title: string; description: string; pending: Pending | null; busy?: boolean };
const states = new WeakMap<T3Client, State>();
/** A `t3code://pair` link waiting on its confirmation, and the links already paired (a second click would spend nothing). */
const pairings = new WeakMap<T3Client, { pairingUrl: string; title: string; busy: boolean }>();
const paired = new WeakMap<T3Client, Set<string>>();
export const promptLinksReady = (client: T3Client): boolean => client.ready && backgroundShellsBootstrapped(client);
export const dismissPromptLink = (client: T3Client): void => { states.delete(client); };
export function promptLinkView(client: T3Client) {
  const state = states.get(client), pairing = pairings.get(client);
  return { show: !!state, title: state?.title ?? '', description: state?.description ?? '', busy: state?.busy === true || state?.status === 'opening',
    clone: state?.status === 'missing-repository', cloneLabel: state?.pending ? `Clone ${state.pending.repository.owner}/${state.pending.repository.name}` : '',
    withoutProject: state?.status === 'missing-repository', checkout: state?.status === 'pull-request-checkout',
    pairing: pairing?.title ?? '', pairingBusy: pairing?.busy === true };
}

const reportPairingFailure = (client: T3Client, error: unknown) =>
  pushToast(client, { kind: 'error', title: 'Could not add backend', description: error instanceof Error ? error.message : 'Failed to add backend.', stacked: true });
/** pairFromLink: any page can open a pairing link, so the user confirms the host before its one-time token is spent. */
function askPairing(client: T3Client, pairingUrl: string): void {
  if (paired.get(client)?.has(pairingUrl)) return;
  let origin: string;
  // The origin keeps the scheme visible, so a plain HTTP backend is not mistaken for HTTPS.
  try { origin = new URL(resolveRemotePairingTarget({ pairingUrl }).origin).origin; }
  catch (error) { reportPairingFailure(client, error); return; }
  pairings.set(client, { pairingUrl, title: `Add the backend at ${origin}?`, busy: false });
}
async function answerPairing(client: T3Client, native: Native, confirmed: boolean): Promise<string> {
  const pairing = pairings.get(client);
  if (!pairing || pairing.busy) return '';
  if (!confirmed) { pairings.delete(client); return ''; }
  pairing.busy = true;
  try {
    // The welcome wizard's pairing: no toast of its own, so this one names the link's outcome.
    await runConnectionOp(native, 'welcome-pair', '', pairing.pairingUrl, client.connection === 'connected', client);
  } catch (error) {
    if (letGo(error)) { pairing.busy = false; throw error; }
    pairings.delete(client);
    reportPairingFailure(client, error);
    return '';
  }
  pairings.delete(client);
  paired.set(client, (paired.get(client) ?? new Set()).add(pairing.pairingUrl));
  pushToast(client, { kind: 'success', title: 'Backend added', description: 'The environment is saved and will reconnect on app startup.' });
  return '';
}

function environments(client: T3Client): PrEnvironment[] { return prEnvironments(client).filter(environment => environment.connected); }
function projects(client: T3Client): Project[] {
  const list = environments(client);
  return projectsByActivity({ sequence: 0,
    projects: list.flatMap(environment => environment.projects.filter(project => !isScratchProject(project, str(environment.config.scratchWorkspaceRoot)))
      .map(project => ({ ...project, id: fleetThreadId(environment.id, str(project.id)), originalId: project.id, environmentId: environment.id }))),
    threads: list.flatMap(environment => environment.threads.map(thread => ({ ...thread, projectId: fleetThreadId(environment.id, str(thread.projectId)) }))),
  }).map(project => ({ ...project, id: project.originalId, environmentId: str(project.environmentId) }));
}
function lastActiveEnvironment(client: T3Client): PrEnvironment | null {
  const list = environments(client);
  const latest = list.flatMap(environment => environment.threads.map(thread => ({ environment, at: str(thread.updatedAt) })))
    .sort((left, right) => right.at.localeCompare(left.at))[0]?.environment;
  return latest ?? list.find(environment => environment.id === primaryEnvironmentId()) ?? null;
}
async function openProject(client: T3Client, native: Native, storage: Files, project: Project): Promise<void> {
  client.homePending = false; client.homeMove = null;
  if (project.environmentId !== client.environmentId) {
    const focused = await focusFleetProject(client, native, project.environmentId, str(project.id));
    client.adoptStatus(focused.value, focused.generation);
    await client.refresh(native, storage);
  } else await client.openProjectDraft(native, str(project.id));
}
/** The new-thread draft of the project a link opened: named, not read from the window, so a link opened from a thread
 *  never lands in that thread (ThreadLinkPrompt's isThreadRoutePathname guard). */
const draftKeyOf = (project: { environmentId: string; id?: unknown }): string => `${project.environmentId}:new:${str(project.id)}`;
function placeDraft(client: T3Client, link: NewThreadLink, key: string, extraPrompt = ''): void {
  const notes: string[] = [];
  if (link.kind === 'new' && link.provider !== null) {
    const resolved = resolvePromptLinkModelSelection(arr(client.config.providers), { provider: link.provider, model: link.model });
    if (resolved) stage(client, { providerId: resolved.selection.instanceId, modelId: resolved.selection.model, options: [] });
    if (!resolved || !resolved.exact) notes.push(!resolved
      ? `The link asked for ${link.provider}, which is not set up here, so your default model is used.`
      : `The link asked for ${link.model}, which ${link.provider} does not offer here, so its default model is used.`);
  }
  const text = [link.prompt?.text, extraPrompt].filter(Boolean).join('\n\n');
  if (text) placeLinkedPrompt(client, key, { text, hiddenCharacters: link.prompt?.hiddenCharacters ?? 0 }, notes);
}
/** scratchEnvironmentId: this machine's environment wins; the last active one only when this machine's offers no folder
 *  for a thread without a project; else the one environment that does. */
async function openScratch(client: T3Client, native: Native, storage: Files, link: NewThreadLink): Promise<void> {
  const list = environments(client), primary = primaryEnvironmentId();
  const available = (environment: PrEnvironment | null | undefined) => environment && str(environment.config.scratchWorkspaceRoot) ? environment : null;
  const offering = list.filter(candidate => !!available(candidate));
  const environment = available(list.find(candidate => candidate.id === primary)) ?? available(lastActiveEnvironment(client)) ?? (offering.length === 1 ? offering[0] : null);
  if (!environment) throw new ClientError('None of your machines can start a thread without a project right now.');
  let id: string;
  if (environment.id === client.environmentId) id = await openScratchProject(client, native);
  else {
    const entry = [...fleet.entries.values()].find(candidate => candidate.environmentId === environment.id);
    if (!entry) throw new ClientError('That environment is no longer connected.');
    id = await openRemoteScratch(client, native, entry);
  }
  await openProject(client, native, storage, { id, environmentId: environment.id });
  placeDraft(client, link, draftKeyOf({ id, environmentId: environment.id }), link.kind === 'pull-request' ? promptLinkPullRequestUrl(link.pullRequest) : '');
  states.delete(client);
}
async function openThread(client: T3Client, native: Native, storage: Files, environmentId: string, threadId: string): Promise<void> {
  client.homePending = false; client.homeMove = null;
  if (environmentId === client.environmentId) await client.openSelected(native, threadId);
  else {
    const focused = await focusFleetThread(client, native, fleetThreadId(environmentId, threadId));
    client.adoptStatus(focused.value, focused.generation);
    await client.refresh(native, storage);
  }
}

/** threadShowingBrowser: the thread on screen, when its panel is open on a Browser tab. An HTML file is served by this
 *  machine's environment, so only that environment's threads take one. */
function threadShowingBrowser(client: T3Client, native: Native, fileEnvironmentId: string | null): ScopedThreadRef | null {
  const ref = activeRef(client);
  const serverBrowser = obj(obj(client.config.environment).capabilities).serverBrowser === true;
  if (!ref || !(native.available || serverBrowser)) return null;
  if (fileEnvironmentId !== null && ref.environmentId !== fileEnvironmentId) return null;
  const panel = surfaceStore(client).panels.get(`${ref.environmentId}:${ref.threadId}`);
  return panel?.visible && panel.surfaces.find(surface => surface.id === panel.active)?.kind === 'browser' ? ref : null;
}
/** startLinkThread (the reference's startThread): a new thread without a project on this machine's environment, or on the one environment that offers
 *  threads without a project when there is no such machine. */
async function startLinkThread(client: T3Client, native: Native, storage: Files, report: (description?: string) => void): Promise<ScopedThreadRef | null> {
  const primary = primaryEnvironmentId(), offering = environments(client).filter(environment => !!str(environment.config.scratchWorkspaceRoot));
  const environment = primary !== null ? offering.find(candidate => candidate.id === primary) : offering.length === 1 ? offering[0] : undefined;
  if (!environment) { report(); return null; }
  try {
    let id: string;
    if (environment.id === client.environmentId) id = await openScratchProject(client, native);
    else {
      const entry = [...fleet.entries.values()].find(candidate => candidate.environmentId === environment.id);
      if (!entry) throw new ClientError('That environment is no longer connected.');
      id = await openRemoteScratch(client, native, entry);
    }
    await openProject(client, native, storage, { id, environmentId: environment.id });
    // The reference's new draft has its thread id from the start; here it is allocated now.
    if (!client.threadId && !activeRef(client) && client.environmentId) await ensureDraftThreadId(client, native);
  } catch (error) {
    if (letGo(error)) throw error;
    report(error instanceof Error ? error.message : 'An error occurred.');
    return null;
  }
  const ref = activeRef(client);
  if (!ref) report();
  return ref;
}
/** openLink: the browser panel of the thread on screen takes the link; else a new thread without a project opens with
 *  the panel maximized. A failure is a toast that names the link. */
async function openWebLink(client: T3Client, native: Native, storage: Files, url: string): Promise<string> {
  const report = (description = url) => { pushToast(client, { kind: 'error', title: 'Could not open the link', description, stacked: true }); };
  const isFile = url.startsWith('file:'), primary = primaryEnvironmentId();
  // This machine's environment serves an HTML file (httpBaseUrl): with none, no server can.
  if (isFile && primary === null) { report(); return ''; }
  const shown = threadShowingBrowser(client, native, isFile ? primary : null);
  const ref = shown ?? await startLinkThread(client, native, storage, report);
  if (!ref) return '';
  try {
    if (isFile) await openFileInPreview(client, native, ref, decodeURIComponent(new URL(url).pathname), '');
    else await openUrlInPreview(client, native, ref, url);
  } catch (error) {
    if (letGo(error)) throw error;
    report(error instanceof Error ? error.message : url);
  }
  // rightPanelStore.requestMaximize: a new thread's panel opens maximized.
  const panel = shown ? undefined : surfaceStore(client).panels.get(`${ref.environmentId}:${ref.threadId}`);
  if (panel?.surfaces.length) panel.maximized = true;
  return shown ? '' : 'sidebar:new-thread';
}

export async function openPromptLink(client: T3Client, native: Native, storage: Files, value: string): Promise<string> {
  const pairingUrl = readPairingAppLink(value);
  if (pairingUrl !== null) { askPairing(client, pairingUrl); return ''; }
  if (!promptLinkAppPath(value) && isWebUrl(value) && prLinksOpenInApp(client)) {
    // A pull request link a connected project reads opens the pull request view (openLink tries useOpenChangeRequestLink
    // with a plain click first). promptLinksReady already waits for the environments' projects.
    const selection = pullRequestPageSelection(client, value);
    if (selection) return `sidebar:pr-page:${selection}`;
  }
  // A web link leaves an open prompt-link dialog open, as the reference's does.
  if (!promptLinkAppPath(value) && (isWebUrl(value) || value.startsWith('file:'))) return await openWebLink(client, native, storage, value);
  dismissPromptLink(client);
  try {
    const path = promptLinkAppPath(value);
    if (!path) throw new ClientError('This link is not a T3 Code prompt or thread link.');
    const url = new URL(path, 'https://app.t3.codes');
    if (url.pathname !== '/new') {
      const [, environmentId, threadId] = url.pathname.split('/').map(decodeURIComponent);
      if (!environmentId || !threadId) throw new ClientError('This link does not name a thread.');
      const environment = environments(client).find(candidate => candidate.id === environmentId);
      if (!environment?.threads.some(thread => thread.id === threadId)) throw new ClientError('That thread is no longer available.');
      await openThread(client, native, storage, environmentId, threadId);
      const parsed = parseThreadLinkPrompt(url.search);
      if (!parsed.ok) pushToast(client, { kind: 'error', title: 'Could not open the linked prompt', description: parsed.error, stacked: true });
      else if (parsed.value) placeLinkedPrompt(client, `${environmentId}:${threadId}`, parsed.value);
      return 'sidebar:new-thread';
    }
    const parsed = parseNewThreadLink(url.search);
    if (!parsed.ok) throw new ClientError(parsed.error);
    const link = parsed.value;
    if (link.kind === 'pull-request') {
      const thread = findPromptLinkPullRequestThread(environments(client).flatMap(environment => environment.threads.map(thread => ({ ...thread, id: str(thread.id), environmentId: environment.id }))), link.pullRequest);
      if (thread) {
        await openThread(client, native, storage, str(thread.environmentId), str(thread.id));
        if (link.prompt) placeLinkedPrompt(client, `${thread.environmentId}:${thread.id}`, link.prompt);
        return 'sidebar:new-thread';
      }
    }
    const repository = link.kind === 'new' ? link.repository : link.pullRequest;
    if (!repository) { await openScratch(client, native, storage, link); return 'sidebar:new-thread'; }
    const project = findPromptLinkProject(projects(client), repository);
    const pending = { link, repository, ...(project ? { project } : {}) };
    if (!project) states.set(client, { status: 'missing-repository', title: `You don't have ${repository.owner}/${repository.name}`,
      description: 'Clone it to work on it here, or start without a project.', pending });
    else if (link.kind === 'pull-request') {
      if (link.prompt) warnAboutHiddenCharacters(client, link.prompt);
      states.set(client, { status: 'pull-request-checkout', title: `Check out ${repository.owner}/${repository.name}#${link.pullRequest.number}?`,
        description: "This puts the pull request's code in a new worktree and runs your project's setup script on it.", pending });
    } else {
      await openProject(client, native, storage, project);
      placeDraft(client, link, draftKeyOf(project));
      return 'sidebar:new-thread';
    }
    return '';
  } catch (error) {
    if (letGo(error)) throw error;
    states.set(client, { status: 'failed', title: 'Could not open the link', description: error instanceof Error ? error.message : 'An error occurred.', pending: null });
    return '';
  }
}

export async function promptLinkAction(client: T3Client, native: Native, storage: Files, action: string): Promise<string> {
  if (action === 'pair-confirm' || action === 'pair-cancel') return await answerPairing(client, native, action === 'pair-confirm');
  const state = states.get(client), pending = state?.pending;
  if (!state || !pending || state.status === 'opening' || state.busy) return '';
  states.set(client, action === 'checkout' ? { ...state, busy: true } : { ...state, status: 'opening', title: 'Opening the link...' });
  try {
    if (action === 'without-project') await openScratch(client, native, storage, pending.link);
    else if (action === 'checkout' && pending.link.kind === 'pull-request' && pending.project) {
      const environment = environments(client).find(candidate => candidate.id === pending.project!.environmentId);
      if (!environment) throw new ClientError('That environment is no longer connected.');
      const project = pending.project;
      const result = await actOnHandoff(client, native, { environmentId: project.environmentId, projectId: str(project.id), workspaceRoot: str(project.workspaceRoot), label: environment.label, machine: environment.machine },
        { projectId: project.id, workspaceRoot: project.workspaceRoot, url: promptLinkPullRequestUrl(pending.link.pullRequest) }, 'prompt-link', pending.link.prompt ? { prompt: pending.link.prompt.text } : null, 'worktree');
      if (!result) { states.set(client, state); return ''; }
      states.delete(client);
    } else if (action === 'clone') {
      const environment = lastActiveEnvironment(client);
      if (!environment || !str(environment.config.newProjectsRoot) || obj(obj(environment.config.environment).capabilities).projectCloneTracking !== true)
        throw new ClientError('This machine cannot clone from a link. Update T3 Code on it, or clone the repository from the command palette.');
      if (environment.id !== client.environmentId) {
        const focused = await focusFleetProject(client, native, environment.id, '');
        client.adoptStatus(focused.value, focused.generation);
        await client.refresh(native, storage);
      }
      const { owner, name } = pending.repository;
      const result = await startTrackedClone(client, native, `https://github.com/${owner}/${name}.git`, cloneDestination(str(environment.config.newProjectsRoot), name), name);
      if (!result.ok) { states.set(client, state); return ''; }
      if (!result.projectId) throw new ClientError('The clone started, but its project did not appear.');
      if (pending.link.kind === 'pull-request') states.set(client, { status: 'failed', title: `Cloning ${owner}/${name}`, description: 'Open this link again when the clone finishes.', pending: null });
      else {
        await openProject(client, native, storage, { id: result.projectId, environmentId: environment.id });
        placeDraft(client, pending.link, draftKeyOf({ id: result.projectId, environmentId: environment.id }));
        states.delete(client);
      }
    } else { states.set(client, state); return ''; }
    return 'sidebar:new-thread';
  } catch (error) {
    if (letGo(error)) { if (states.get(client)?.pending === pending) states.set(client, state); throw error; }
    states.set(client, { status: 'failed', title: 'Could not open the link', description: error instanceof Error ? error.message : 'An error occurred.', pending: null });
    return '';
  }
}
