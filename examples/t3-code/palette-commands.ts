// What the palette's rows do on the server (reference CommandPalette.tsx executeItem,
// handleAddProjectForEnvironment, submitNewProject/useNewProject, submitAddProjectCloneFlow
// and the Restart agent session action). Each returns how the overlay continues: close,
// stay, or move to another page with a new query. Failures are toasts, as in the reference.
import type { T3Client } from './client';
import { applyShell, arr, initialShell, obj, str, type Obj } from './domain';
import type { Files, Native } from './protocol';
import { dismissToast, pushToast } from './toast';
import { openFileSurface, openSurface } from './r4-surfaces-panel';
import { cloneDestination, cloneDirectoryName, flowOf, githubAccount, inferTitle, initialBrowseQuery, normalizeCloneUrl, resolvePath, SOURCE_LABELS } from './palette-add';
import { sortedThreads } from './palette';
import { activeTarget } from './palette-files';
import { favoriteEditor } from './keyboard-dispatch';
import { openFavoriteHere } from './remote-open'; // remote Open: an SSH environment's workspace opens over SSH
import { linkPullRequest } from './palette-linkpr';
import { openScratchProject } from './r11-upstream-scratch';
import { startTrackedClone } from './project-clones-live';
import { cloneTracking } from './live-streams';

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
async function createProject(client: T3Client, native: Native, storage: Files, cwd: string, failureTitle: string): Promise<PaletteResult> {
  const access = client.restAccess(native);
  const existing = client.shell.projects.find(project => str(project.workspaceRoot).replace(/\/+$/, '') === cwd);
  if (existing) return openProject(client, str(existing.id));
  try {
    const [commandId, projectId] = await access.ids(2);
    await access.write(storage, { method: 'projects.mutate', description: 'Add project', threadId: '', text: '', uncertain: false,
      payload: { type: 'project.create', commandId, projectId: projectId!, title: inferTitle(cwd), workspaceRoot: cwd, createWorkspaceRootIfMissing: true, defaultModelSelection: null } });
    await refreshShell(client, native);
    if (!client.shell.projects.some(project => project.id === projectId)) throw new Error('The project was created but is not yet available. Refresh before selecting it.');
    return done(client, projectId!);
  } catch (error) {
    pushToast(client, { kind: 'error', title: failureTitle, description: message(error) });
    return stay(client, false, message(error));
  }
}

export async function paletteCommand(client: T3Client, native: Native | null | undefined, storage: Files, op: string, id: string, value: string): Promise<PaletteResult> {
  // A thread row and New thread in <project>: the palette closes and the window navigates.
  if (op === 'thread') return done(client, '', id);
  if (op === 'new-thread') return done(client, id);
  if (!native?.available) return stay(client, false, 'Open this app on macOS to connect to T3 Code.');
  const flow = flowOf(client);
  const access = client.restAccess(native);
  const current = client.shell.projects.find(project => project.id === client.projectId);
  const cwd = str(current?.workspaceRoot);
  try {
    if (op === 'restart-session') return await restartSession(client, native, storage, id);
    if (op === 'open-project') return openProject(client, id);
    if (op === 'toggle-publish') { flow.publish = !flow.publish; return stay(client); }
    // ProjectFilePicker / ProjectContentSearchDialog: rightPanelStore.openFile (r4-surfaces-panel.ts); Open Favorite stays the editor.
    if (op === 'open-file') { await openFileSurface(client, native, id, Number(value) || 0); client.revision++; return done(client); }
    if (op === 'thread-pull-requests') { await openSurface(client, native, 'pull-requests'); client.revision++; return done(client); }
    if (op === 'open-favorite') return await openInEditor(client, native, op, id, value);
    if (op === 'scratch') return await startScratch(client, native);
    if (op === 'link-pr') return (await linkPullRequest(client, access, storage, id)) ? done(client) : stay(client, false);
    if (!client.ready) {
      pushToast(client, { kind: 'error', title: 'Environment unavailable', description: `${str(obj(client.config.environment).label, 'The selected environment')} is not connected.` });
      return stay(client, false, 'Environment unavailable');
    }
    if (['add-path', 'new-project', 'clone', 'pick-folder'].includes(op) && client.pending) {
      pushToast(client, { kind: 'error', title: op === 'clone' ? 'Clone failed' : 'Failed to add project', description: 'Resolve the previous operation before sending another.' });
      return stay(client, false);
    }
    if (op === 'pick-folder') {
      // Open in Finder: the native folder picker, starting at the browsed folder.
      const picked = obj(await access.call({ op: 'pickFolder', path: id }));
      const path = str(picked.path);
      return path ? await createProject(client, native, storage, path, 'Failed to add project') : stay(client);
    }
    if (op === 'add-path') {
      const raw = id.trim();
      if (!raw) return stay(client);
      if ((raw.startsWith('./') || raw.startsWith('../')) && !cwd) {
        pushToast(client, { kind: 'error', title: 'Failed to add project', description: 'Relative paths require an active project.' });
        return stay(client, false);
      }
      return await createProject(client, native, storage, resolvePath(raw, cwd), 'Failed to add project');
    }
    if (op === 'new-project') {
      const name = id.trim();
      if (!name) return stay(client);
      let created: Obj;
      try { created = await access.request('projects.createNew', { name }, true); }
      catch (error) { pushToast(client, { kind: 'error', title: 'Could not create the project', description: message(error) }); return stay(client, false, message(error)); }
      const workspaceRoot = str(created.workspaceRoot), commitError = str(created.commitError), projectId = str(created.projectId);
      pushToast(client, commitError ? { kind: 'warning', title: `Created ${name} without a first commit`, description: `${commitError} The project is in ${workspaceRoot}.` }
        : { kind: 'success', title: `Created ${name}`, description: workspaceRoot });
      if (flow.publish) {
        const account = githubAccount(flow.discovery?.value ?? null);
        const folder = workspaceRoot.split('/').filter(Boolean).pop() ?? '';
        access.request('sourceControl.publishRepository', { cwd: workspaceRoot, provider: 'github', repository: account ? `${account}/${folder}` : folder, visibility: 'private' }, true)
          .then(result => { pushToast(client, { kind: 'success', title: 'Published to GitHub', description: str(obj(obj(result).repository).nameWithOwner) }); client.revision++; })
          .catch(error => { pushToast(client, { kind: 'error', title: 'Could not create the GitHub repository', description: `${message(error)} Use Publish Repository in the Git menu to try again.` }); client.revision++; });
      }
      flow.publish = false;
      try { await refreshShell(client, native); } catch { /* the shell stream catches up */ }
      if (!client.shell.projects.some(project => project.id === projectId)) {
        pushToast(client, { kind: 'error', title: 'Failed to open project', description: 'The project was created. It will appear in the sidebar once this client catches up.' });
        return done(client);
      }
      return done(client, projectId);
    }
    if (op.startsWith('clone-repository:')) {
      const source = op.slice('clone-repository:'.length), raw = id.trim();
      if (!raw) return stay(client);
      const parentPath = initialBrowseQuery(client);
      if (source === 'url') {
        flow.clone = { source, repositoryInput: raw, title: raw, description: normalizeCloneUrl(raw), remoteUrl: normalizeCloneUrl(raw), pinned: cloneDirectoryName(raw) };
        return { ...stay(client), page: `${value}/confirm`, query: cloneDestination(parentPath, cloneDirectoryName(raw)) };
      }
      let repository: Obj;
      try { repository = await access.request('sourceControl.lookupRepository', { provider: source, repository: raw }); }
      catch (error) { pushToast(client, { kind: 'error', title: 'Repository lookup failed', description: message(error) }); return stay(client, false, message(error)); }
      const nameWithOwner = str(repository.nameWithOwner, raw);
      const remoteUrl = source === 'github' || source === 'forgejo' ? str(repository.url) : str(repository.sshUrl, str(repository.url));
      flow.clone = { source, repositoryInput: raw, title: nameWithOwner, description: str(repository.url, remoteUrl), remoteUrl, pinned: cloneDirectoryName(nameWithOwner) };
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
      if (cloneTracking(client.config)) {
        const started = await startTrackedClone(client, native, clone.remoteUrl, destinationPath, name); // project-clones-live.ts
        if (!started.ok) return stay(client, false, started.message);
        flow.clone = null;
        return done(client, started.projectId);
      }
      // The blocking clone (servers without clone tracking): the palette waits for git,
      // then adds the project; progress shows as the reference's clone toasts.
      const loading = pushToast(client, { kind: 'loading', title: `Cloning ${name}`, description: `${SOURCE_LABELS[clone.source] ?? 'Git'} · ${destinationPath}` });
      let cloned: Obj;
      try { cloned = await access.request('sourceControl.cloneRepository', { remoteUrl: clone.remoteUrl, destinationPath }, true); }
      catch (error) {
        dismissLoading(client, loading);
        pushToast(client, { kind: 'error', title: 'Clone failed', description: message(error) });
        return stay(client, false, message(error));
      }
      dismissLoading(client, loading);
      pushToast(client, { kind: 'success', title: `Cloned ${name}`, description: str(cloned.cwd, destinationPath) });
      flow.clone = null;
      return await createProject(client, native, storage, str(cloned.cwd, destinationPath), 'Failed to add project');
    }
    throw new Error(`Unknown palette command: ${op}`);
  } catch (error) {
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
  const editor = favoriteEditor(client.config);
  if (op === 'open-favorite') { if (id) await openFavoriteHere(client, native, id, editor); return done(client); }
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
/** chat.newWithoutProject: the environment's scratch project, created on first use. */
async function startScratch(client: T3Client, native: Native): Promise<PaletteResult> {
  try {
    return done(client, await openScratchProject(client, native)); // r11-upstream (845ddd9354): the shared opener
  } catch (error) {
    pushToast(client, { kind: 'error', title: 'Could not start without a project', description: message(error) });
    return done(client);
  }
}
