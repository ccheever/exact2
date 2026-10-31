import { sourceControlHost } from '../shared/source-control-hosts';
// What the palette's rows do on the server (reference CommandPalette.tsx executeItem,
// handleAddProjectForEnvironment, submitNewProject/useNewProject, submitAddProjectCloneFlow
// and the Restart agent session action). Each returns how the overlay continues: close,
// stay, or move to another page with a new query. Failures are toasts, as in the reference.
import type { T3Client } from '../client/client';
import { composerPromptLink, copyPromptLink } from '../prompt-links/copy-prompt-link';
import { applyShell, arr, initialShell, obj, str, type Obj } from '../client/domain';
import type { Files, Native } from '../client/protocol';
import { dismissToast, pushToast } from '../shell/toast';
import { openFileSurface, openSurface } from '../panel/panel-surfaces';
import { cloneDestination, cloneDirectoryName, flowOf, newProjectPublishTargets, inferTitle, initialBrowseQuery, normalizeCloneUrl, resolvePath, SOURCE_LABELS } from './palette-add';
import { sortedThreads } from './palette';
import { activeTarget } from './palette-files';
import { favoriteEditor } from '../shell/keyboard-dispatch';
import { openFavoriteHere } from '../connections/remote-open'; // remote Open: an SSH environment's workspace opens over SSH
import { announce } from '../shell/announce';
import { EDITOR_DEFINITIONS } from '../shared/editors';
import { lastEditor } from '../details/shell-details';
import { linkPullRequest } from './palette-linkpr';
import { linkFromPicker } from '../pull-requests/pages-pr-links';
import { openPrimaryScratch } from '../composer/scratch-project';
import { startTriage } from './palette-triage';
import { startTrackedClone } from '../shell/project-clones-live';
import { cloneTracking } from '../client/live-streams';
import { letGo } from '../client/let-go';
import { composerNow } from '../composer/composer-controls';
import { claimThreadOpen, pickedQuery } from './palette-search';
import { addTarget, chooseNewProjectEnvironment, newProjectEnvironments, targetRequest, type AddEnvironment } from './palette-environments';
import { fleet, fleetThreadId, focusFleetProject, parseFleetThreadId, type EnvironmentFleet } from '../connections/settings-b-fleet';
import { environmentCall } from '../pull-requests/pages-pr-environments';
import { findWhenShown } from '../sidebar/sidebar-search';
import { shownMessageMatch } from './palette-view';

export type PaletteResult = { revision: number; ok: boolean; close: boolean; page: string; query: string; project: string; thread: string; message: string };
const message = (error: unknown) => error instanceof Error ? error.message : 'An error occurred.';
const stay = (client: T3Client, ok = true, text = ''): PaletteResult => ({ revision: client.revision, ok, close: false, page: '-', query: '', project: '', thread: '', message: text });
const done = (client: T3Client, project = '', thread = ''): PaletteResult => ({ revision: client.revision, ok: true, close: true, page: '-', query: '', project, thread, message: '' });

async function refreshShell(client: T3Client, native: Native): Promise<void> {
  client.shell = applyShell(initialShell(), await client.restAccess(native).http('/api/orchestration/shell'));
}
/** Where an existing project opens: its latest active thread, else a new thread. */
function openProject(client: T3Client, projectId: string): PaletteResult {
  const latest = sortedThreads(client.shell.threads.filter(thread => thread.projectId === projectId))[0];
  return latest && latest.settledOverride !== 'settled' ? done(client, '', str(latest.id)) : done(client, projectId);
}
/**
 * handleAddProjectForEnvironment: an existing project opens; else the project is created on the target
 * environment. Another environment's project opens by its `fleet:` id, which moves the window there.
 */
async function createProject(client: T3Client, native: Native, storage: Files, target: AddEnvironment, cwd: string, failureTitle: string, source: EnvironmentFleet): Promise<PaletteResult> {
  const access = client.restAccess(native);
  const existing = target.projects.find(project => str(project.workspaceRoot).replace(/\/+$/, '') === cwd);
  if (existing) return target.focused ? openProject(client, str(existing.id)) : done(client, fleetThreadId(target.environmentId, str(existing.id)));
  try {
    const [commandId, projectId] = await access.ids(2);
    if (!target.focused) {
      await targetRequest(client, native, target, 'projects.mutate', { type: 'project.create', commandId, projectId: projectId!, title: inferTitle(cwd), workspaceRoot: cwd, createWorkspaceRootIfMissing: true, defaultModelSelection: null }, true, source);
      return done(client, fleetThreadId(target.environmentId, projectId!));
    }
    await access.write(storage, { method: 'projects.mutate', description: 'Add project', threadId: '', text: '', uncertain: false,
      payload: { type: 'project.create', commandId, projectId: projectId!, title: inferTitle(cwd), workspaceRoot: cwd, createWorkspaceRootIfMissing: true, defaultModelSelection: null } });
    await refreshShell(client, native);
    if (!client.shell.projects.some(project => project.id === projectId)) throw new Error('The project was created but is not yet available. Refresh before selecting it.');
    return done(client, projectId!);
  } catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: failureTitle, description: message(error) });
    return stay(client, false, message(error));
  }
}

/**
 * CommandPaletteThreadItems runThread: a thread picked from a search counts as opened by this client at the pick's moment,
 * for Recent Threads and the use boost, and its server records the open with its query (`thread.visit` with `opened`
 * and `searchQuery`), which ranks the thread first when that query is typed again. `id` is the row's (a background
 * environment's thread is `fleet:<environment>:<id>`); `pick` is "<moment in epoch ms> <query>", the query being the
 * root page's search ("" for other rows); the open thread is no pick. A message row opens find at its message once its
 * thread shows, as its run calls openThreadFindOnQuery.
 */
async function pickThread(client: T3Client, native: Native | null | undefined, id: string, pick: string, source: EnvironmentFleet): Promise<void> {
  const space = pick.indexOf(' '), at = Number(pick.slice(0, space)), query = pick.slice(space + 1);
  const message = shownMessageMatch(client, id, query);
  if (message) findWhenShown(client, id, message.match, message.query);
  if (space <= 0 || !Number.isFinite(at) || query.trim().length === 0 || query.startsWith('>') || id === client.threadId) return;
  const ref = parseFleetThreadId(id), environmentId = ref?.environmentId ?? client.environmentId, threadId = ref?.threadId ?? id;
  claimThreadOpen(environmentId, threadId, at);
  // A server that keeps no visits sends no `lastVisitedAt`. The open goes whether or not this client already claimed it,
  // and the pick does not wait for its reply (upstream's `void visitThread(…)`): a request with a `deliver` key returns
  // once the transport has sent it, and the reply lands in the inbox, where nothing reads it. A refused send is dropped,
  // as visitThread's reportFailure: false drops it.
  const threads = ref ? [...source.entries.values()].find(entry => entry.environmentId === environmentId)?.shell.threads ?? [] : client.shell.threads;
  const thread = threads.find(entry => entry.id === threadId);
  if (!thread || thread.lastVisitedAt === undefined || !native?.available) return;
  try {
    const [commandId] = await client.ids(native, 1);
    const visit = { op: 'request', method: 'orchestration.dispatchCommand', deliver: `palette-visit:${commandId}`,
      payload: { type: 'thread.visit', commandId, threadId, visitedAt: str(thread.updatedAt), opened: true, searchQuery: pickedQuery(query) } };
    await (ref ? environmentCall(client, native, environmentId, visit, true, source) : client.call(native, visit, client.generation, true));
  } catch (error) {
    if (letGo(error)) throw error;
  }
}

export async function paletteCommand(client: T3Client, native: Native | null | undefined, storage: Files, op: string, id: string, value: string, source: EnvironmentFleet = fleet): Promise<PaletteResult> {
  // A thread row and New thread in <project>: the palette closes and the window navigates. A thread row's `value` is its
  // pick (pickThread).
  if (op === 'thread') { await pickThread(client, native, id, value, source); return done(client, '', id); }
  if (op === 'new-thread') return done(client, id);
  if (!native?.available) return stay(client, false, 'Open this app on macOS to connect to T3 Code.');
  const flow = flowOf(client);
  const access = client.restAccess(native);
  try {
    if (op === 'copy-prompt-link') { await copyPromptLink(client, native, composerPromptLink(client, id === 'context')); return done(client); }
    if (op === 'restart-session') return await restartSession(client, native, storage, id);
    if (op === 'open-project') return openProject(client, id);
    if (op === 'publish-host') { if (newProjectPublishTargets(flow.discovery?.value ?? null).some(target => target.definition.kind === id)) flow.publishKind = id; return stay(client); }
    if (op === 'toggle-publish') { flow.publish = !flow.publish; return stay(client); }
    // switchNewProjectEnvironment: New project moves to the chosen environment, its name and options kept.
    if (op === 'new-project-environment') {
      if (newProjectEnvironments(client, source).some(option => option.environmentId === id)) chooseNewProjectEnvironment(client, value, id);
      return stay(client);
    }
    // ProjectFilePicker / ProjectContentSearchDialog: rightPanelStore.openFile (panel-surfaces.ts); Open Favorite stays the editor.
    if (op === 'open-file') { await openFileSurface(client, native, id, Number(value) || 0); client.revision++; return done(client); }
    if (op === 'thread-pull-requests') { await openSurface(client, native, 'pull-requests'); client.revision++; return done(client); }
    if (op === 'open-favorite') return await openInEditor(client, native, op, id, value);
    if (op === 'scratch') return await startScratch(client, native, storage, source);
    if (op === 'triage') return done(client, await startTriage(client, native));
    if (op === 'link-pr') return (await linkPullRequest(client, access, storage, id)) ? done(client) : stay(client, false);
    if (op === 'pr-link-thread') return (await linkFromPicker(client, native, storage, id)) ? done(client) : stay(client, false); // the thread picker
    // The add-project ops act on the environment the page chose (browseEnvironmentId).
    const target = addTarget(client, value, source);
    if (!target) {
      pushToast(client, { kind: 'error', title: 'Environment unavailable', description: `${str(obj(client.config.environment).label, 'The selected environment')} is not connected.` });
      return stay(client, false, 'Environment unavailable');
    }
    const request = (method: string, payload: Obj, write = false) => targetRequest(client, native, target, method, payload, write, source);
    const current = target.focused ? client.shell.projects.find(project => project.id === client.projectId) : undefined;
    const cwd = str(current?.workspaceRoot);
    if (['add-path', 'new-project', 'clone', 'pick-folder'].includes(op) && target.focused && client.pending) {
      pushToast(client, { kind: 'error', title: op === 'clone' ? 'Clone failed' : 'Failed to add project', description: 'Resolve the previous operation before sending another.' });
      return stay(client, false);
    }
    if (op === 'pick-folder') {
      // Open in Finder: the native folder picker, starting at the browsed folder.
      const picked = obj(await access.call({ op: 'pickFolder', path: id }));
      const path = str(picked.path);
      return path ? await createProject(client, native, storage, target, path, 'Failed to add project', source) : stay(client);
    }
    if (op === 'add-path') {
      const raw = id.trim();
      if (!raw) return stay(client);
      if ((raw.startsWith('./') || raw.startsWith('../')) && !cwd) {
        pushToast(client, { kind: 'error', title: 'Failed to add project', description: 'Relative paths require an active project.' });
        return stay(client, false);
      }
      return await createProject(client, native, storage, target, resolvePath(raw, cwd), 'Failed to add project', source);
    }
    if (op === 'new-project') {
      const name = id.trim();
      if (!name) return stay(client);
      const targets = newProjectPublishTargets(flow.discovery?.value ?? null);
      // getNewProjectPublishTarget: the chosen host, else the first ready one. With none (an environment chosen since
      // whose hosts are not ready) the project is made unpublished, as publishTo null makes it.
      const publishTarget = flow.publish ? targets.find(target => target.definition.kind === flow.publishKind) ?? targets[0] ?? null : null;
      let created: Obj;
      try { created = await request('projects.createNew', { name }, true); }
      catch (error) { if (letGo(error)) throw error; pushToast(client, { kind: 'error', title: 'Could not create the project', description: message(error) }); return stay(client, false, message(error)); }
      const workspaceRoot = str(created.workspaceRoot), commitError = str(created.commitError), projectId = str(created.projectId);
      pushToast(client, commitError ? { kind: 'warning', title: `Created ${name} without a first commit`, description: `${commitError} The project is in ${workspaceRoot}.` }
        : { kind: 'success', title: `Created ${name}`, description: workspaceRoot });
      if (publishTarget) {
        const target = publishTarget, account = target.owner;
        const folder = workspaceRoot.split('/').filter(Boolean).pop() ?? '';
        request('sourceControl.publishRepository', { cwd: workspaceRoot, provider: target.definition.kind, repository: account ? `${account}/${folder}` : folder, visibility: 'private' }, true)
          .then(result => { pushToast(client, { kind: 'success', title: `Published to ${target.definition.label}`, description: str(obj(obj(result).repository).nameWithOwner) }); client.revision++; })
          .catch(error => { if (letGo(error)) return; pushToast(client, { kind: 'error', title: `Could not create the ${target.definition.label} repository`, description: `${message(error)} Use Publish Repository in the Git menu to try again.` }); client.revision++; });
      }
      flow.publish = false;
      if (!target.focused) return done(client, fleetThreadId(target.environmentId, projectId));
      try { await refreshShell(client, native); } catch { /* the shell stream catches up */ }
      if (!client.shell.projects.some(project => project.id === projectId)) {
        pushToast(client, { kind: 'error', title: 'Failed to open project', description: 'The project was created. It will appear in the sidebar once this client catches up.' });
        return done(client);
      }
      return done(client, projectId);
    }
    if (op.startsWith('clone-repository:')) {
      const kind = op.slice('clone-repository:'.length), raw = id.trim();
      if (!raw) return stay(client);
      const parentPath = initialBrowseQuery(target.config);
      if (kind === 'url') {
        flow.clone = { source: kind, repositoryInput: raw, title: raw, description: normalizeCloneUrl(raw), remoteUrl: normalizeCloneUrl(raw), pinned: cloneDirectoryName(raw) };
        return { ...stay(client), page: `${value}/confirm`, query: cloneDestination(parentPath, cloneDirectoryName(raw)) };
      }
      let repository: Obj;
      try { repository = await request('sourceControl.lookupRepository', { provider: kind, repository: raw }); }
      catch (error) { if (letGo(error)) throw error; pushToast(client, { kind: 'error', title: 'Repository lookup failed', description: message(error) }); return stay(client, false, message(error)); }
      const nameWithOwner = str(repository.nameWithOwner, raw);
      const remoteUrl = sourceControlHost(kind)?.cloneTransport === 'https' ? str(repository.url) : str(repository.sshUrl, str(repository.url));
      flow.clone = { source: kind, repositoryInput: raw, title: nameWithOwner, description: str(repository.url, remoteUrl), remoteUrl, pinned: cloneDirectoryName(nameWithOwner) };
      return { ...stay(client), page: `${value}/confirm`, query: cloneDestination(parentPath, cloneDirectoryName(nameWithOwner)) };
    }
    if (op === 'clone') {
      const clone = flow.clone, raw = id.trim();
      if (!clone || !raw) return stay(client);
      if ((raw.startsWith('./') || raw.startsWith('../')) && !cwd) {
        pushToast(client, { kind: 'error', title: 'Clone failed', description: 'Relative paths require an active project.' });
        return stay(client, false);
      }
      const destinationPath = resolvePath(raw, cwd), name = inferTitle(destinationPath);
      // The server creates the project and clones in the background (projectCloneTracking): the
      // palette closes once git runs; progress is the clone's toast and its draft's banner.
      if (!target.focused && cloneTracking(target.config)) {
        try {
          const [projectId] = await access.ids(1);
          await request('projectClone.start', { projectId, title: name, createdAt: new Date(composerNow(client)).toISOString(), remoteUrl: clone.remoteUrl, destinationPath }, true);
          flow.clone = null;
          return done(client, fleetThreadId(target.environmentId, projectId!));
        } catch (error) {
          if (letGo(error)) throw error;
          pushToast(client, { kind: 'error', title: 'Clone failed', description: message(error), stacked: true });
          return stay(client, false, message(error));
        }
      }
      if (target.focused && cloneTracking(client.config)) {
        const started = await startTrackedClone(client, native, clone.remoteUrl, destinationPath, name); // project-clones-live.ts
        if (!started.ok) return stay(client, false, started.message);
        flow.clone = null;
        return done(client, started.projectId);
      }
      // The blocking clone (servers without clone tracking): the palette waits for git,
      // then adds the project; progress shows as the reference's clone toasts.
      const loading = pushToast(client, { kind: 'loading', title: `Cloning ${name}`, description: `${SOURCE_LABELS[clone.source] ?? 'Git'} · ${destinationPath}` });
      let cloned: Obj;
      try { cloned = await request('sourceControl.cloneRepository', { remoteUrl: clone.remoteUrl, destinationPath }, true); }
      catch (error) {
        dismissLoading(client, loading);
        if (letGo(error)) throw error;
        pushToast(client, { kind: 'error', title: 'Clone failed', description: message(error) });
        return stay(client, false, message(error));
      }
      dismissLoading(client, loading);
      pushToast(client, { kind: 'success', title: `Cloned ${name}`, description: str(cloned.cwd, destinationPath) });
      flow.clone = null;
      return await createProject(client, native, storage, target, str(cloned.cwd, destinationPath), 'Failed to add project', source);
    }
    throw new Error(`Unknown palette command: ${op}`);
  } catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: 'Unable to run command', description: message(error) });
    return { ...stay(client, false, message(error)), close: true };
  }
}
function dismissLoading(client: T3Client, id: number): void { dismissToast(client, id); }

/**
 * Restart agent session: detach every provider session of the thread (the next message
 * starts a fresh one), then re-probe the thread's provider with a fresh workspace scan.
 */
async function restartSession(client: T3Client, native: Native, storage: Files, threadId: string): Promise<PaletteResult> {
  const thread = client.shell.threads.find(candidate => candidate.id === threadId);
  if (!thread) throw new Error('That thread is no longer available.');
  const access = client.restAccess(native);
  const projection = threadId === client.threadId ? client.projection : {};
  const sessions = arr(obj(projection).providerSessions);
  if (sessions.length) {
    const [commandId] = await access.ids(1);
    for (const session of sessions) {
      await access.dispatch(storage, { type: 'provider-session.detach', commandId: `${commandId}:detach:${str(session.id)}`, threadId, providerSessionId: str(session.id), reason: 'client-requested' }, 'Restart agent session');
    }
  }
  pushToast(client, { kind: 'success', title: 'Agent session will restart', description: 'Your next message starts a fresh session.' });
  const project = client.shell.projects.find(candidate => candidate.id === thread.projectId);
  if (project) {
    const runtime = obj(thread.runtime);
    await access.request('server.refreshProviders', { instanceId: str(runtime.providerInstanceId, str(obj(thread.modelSelection).instanceId)), cwd: str(thread.worktreePath, str(project.workspaceRoot)), fresh: true }, true);
  }
  return done(client);
}

/**
 * editor.openFavorite and a chosen file or match: the preferred editor opens the target
 * (shell.openInEditor; `path:line` reaches editors that take a position).
 */
async function openInEditor(client: T3Client, native: Native, op: string, id: string, line: string): Promise<PaletteResult> {
  const editor = favoriteEditor(client.config, lastEditor(client)); // the last-used editor (t3-code.json)
  if (op === 'open-favorite') {
    // useOpenFavoriteEditorShortcut: a screen reader hears which editor took the workspace (editorLabelForPlatform).
    const opened = id ? await openFavoriteHere(client, native, id, editor, lastEditor(client)) : '';
    if (opened) announce(client, `Opened in ${EDITOR_DEFINITIONS.find(entry => entry.id === opened)?.label ?? 'Editor'}`);
    return done(client);
  }
  const target = activeTarget(client);
  const path = op === 'open-file' ? (target ? `${target.cwd.replace(/\/+$/, '')}/${id}` : '') : id;
  if (!path) throw new Error('Open a project to open its files.');
  if (!editor) {
    pushToast(client, { kind: 'error', title: 'Unable to open in editor', description: `No available editor can open ${path}.` });
    return done(client);
  }
  await client.restAccess(native).request('shell.openInEditor', { cwd: line ? `${path}:${line}` : path, editor });
  return done(client);
}
/**
 * chat.newWithoutProject: startScratchThread on scratchEnvironmentId's machine, the primary first, its scratch project
 * created on first use; on another machine the window moves to that project's new thread (handleNewThread).
 */
async function startScratch(client: T3Client, native: Native, storage: Files, source: EnvironmentFleet): Promise<PaletteResult> {
  try {
    const opened = await openPrimaryScratch(client, native, source);
    if (!opened || opened.environmentId === client.environmentId) return done(client, opened?.projectId ?? '');
    const focused = await focusFleetProject(client, native, opened.environmentId, opened.projectId, source);
    client.adoptStatus(focused.value, focused.generation);
    await client.refresh(native, storage);
    return done(client);
  } catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: 'Could not start without a project', description: message(error) });
    return done(client);
  }
}
