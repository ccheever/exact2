// Pinned365aa87982 ThreadRouteScreen / ThreadGitControls (MIT, LICENSE-T3).
// Shared reducers own Git and terminal metadata; this adapter owns native menu admission.
// @ref llp/1109.005-composer-and-transcript.decision.md
import { mobileClient, mobileNative } from './client';
import type { T3Client } from './shared/client';
import { arr, obj, str, num, type Obj } from './shared/domain';
import { ClientError, nativeFiles, type Native, type Files } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { reviewOwner, assertReviewOwner } from './review-owner';
import { mobileSessionGrants } from './mobile-grants';
import { mobileGitAction, mobileGitRead, mobileGitAccess, mobileGitCurrentStatus } from './git-overview';
import { mobileGitStatus } from './git-overview-model';
import { gitState } from './shared/r4-git-actions';
import { resolveScripts } from './shared/settings-b-actions';
import { fleet } from './shared/settings-b-fleet';
import { environmentSources } from './shared/connections';
import { knownSessions, terminalDrawerView } from './shared/terminal-drawer-view';
import { projectScriptCwd, projectScriptRuntimeEnv } from './shared/terminal-drawer';
import { mergeTargetOf, latestMergeBackRun } from './shared/shell-lineage';
import { subscriptionSerial } from './shared/shell-vcs';
import { compactThreadBranch, compactThreadGitStatus, resolveQuickAction, threadNextTerminalId,
  threadScriptSymbol, threadTerminalLabel, threadTerminalStatus } from './thread-header-model';
import { mobileStageThreadScript, mobileThreadHeaderPending, mobileThreadHeaderObserveRoute } from './thread-header-terminal';
import { mobileReviewSnapshot } from './review-data';
export interface ThreadHeaderOptions { routeKey: string; routeIdentity: string; focused: boolean; split: boolean; sidebarVisible: boolean;
  canGoBack: boolean; returnToChat: boolean; foreground: string; review?: boolean; reviewRevision?: number; dark?: boolean;
  inspectorAvailable?: boolean; inspectorVisible?: boolean }
export interface ThreadHeaderSnapshot { owner: string; routeKey: string; version: string; configuration: string;
  ready: boolean; needsPrepare: boolean; error: string; pendingLaunch: string; pendingTerminalId: string }
export interface ThreadHeaderItem { id: string; title: string; subtitle: string; symbol: string; disabled: boolean }
export interface ThreadHeaderResult { revision: number; message: string; requestRoute: string; environmentId: string;
  threadId: string; navigation: string; location: string; sourceAction: string; key: string; pendingLaunch: string }
interface State { owner: string; routeKey: string; routeIdentity: string; visit: number; focused: boolean; prepared: boolean; reading: boolean; busy: boolean;
  read: boolean; operate: boolean; merge: boolean; metadataReady: boolean; metadataInvalidated: boolean; retry: boolean; invalidation: number;
  subscription: number; seq: number; error: string }
const states = new WeakMap<T3Client, State>();
function stateOf(client: T3Client): State {
  const owner = reviewOwner(client); let state = states.get(client);
  if (!state || state.owner !== owner) { state = { owner, routeKey: '', routeIdentity: '', visit: 0, focused: false, prepared: false, reading: false, busy: false,
    read: false, operate: false, merge: false, metadataReady: false, metadataInvalidated: false, retry: false, invalidation: 0, subscription: 0, seq: 0, error: '' }; states.set(client, state); }
  return state;
}
const fail = (error: unknown) => error instanceof Error ? error.message : 'Could not perform the thread action.';
function context(client: T3Client) {
  const thread = client.shell.threads.find(item => item.id === client.threadId);
  const project = client.shell.projects.find(item => item.id === thread?.projectId);
  const sessions = knownSessions(client, { environmentId: client.environmentId, threadId: client.threadId })
    .filter(item => item.state.status === 'running' || item.state.status === 'starting')
    .sort((a, b) => a.target.terminalId.localeCompare(b.target.terminalId, undefined, { numeric: true }));
  return { thread, project, sessions, scripts: project ? resolveScripts(obj(client.config.settings), project) : [] };
}
function item(id: string, title: string, subtitle: string, symbol: string, disabled = false): ThreadHeaderItem {
  return { id, title, subtitle, symbol, disabled };
}
function model(options: ThreadHeaderOptions, client: T3Client) {
  const state = stateOf(client), ctx = context(client), status = mobileGitCurrentStatus(client), git = gitState(client);
  const ready = client.ready && !!ctx.thread && !!ctx.project, focused = options.focused && ready;
  const quick = status?.isRepo === false ? { label: 'Git unavailable', disabled: true, kind: 'show_hint' as const, hint: 'This workspace is not a git repository.' } : resolveQuickAction(mobileGitStatus(status), state.busy || !!git.run?.running,
    status?.isDefaultRef === true, status?.hasPrimaryRemote === true);
  const access = mobileGitAccess(client), repository = status?.isRepo !== false;
  const needsWrite = quick.kind === 'run_action' || quick.kind === 'run_pull';
  const quickDisabled = !repository || quick.disabled || needsWrite && !access.write;
  const mergeTarget = mergeTargetOf(client.projection), mergeRun = str(latestMergeBackRun(client.projection)?.id);
  const branch = str(status?.refName) || str(ctx.thread?.branch) || 'Detached HEAD';
  const gitItems = [item('git:branch', compactThreadBranch(branch), compactThreadGitStatus(status), 'point.topleft.down.curvedto.point.bottomright.up', true),
    item('git:quick', quick.label, quickDisabled ? quick.hint || (!access.write ? 'This connection cannot change source control.' : '') : '',
      quick.kind === 'run_pull' ? 'arrow.down.circle' : quick.kind === 'open_pr' ? 'arrow.up.right.circle' : quick.action === 'commit' ? 'checkmark.circle' : ['push','commit_push'].includes(quick.action || '') ? 'arrow.up.circle' : 'arrow.up.right.circle', quickDisabled),
    item('git:review', 'Review changes', 'Turn diffs and worktree changes', 'text.bubble', !repository)];
  if (mergeTarget && mergeRun) gitItems.push(item('git:merge', 'Merge back to source', "Bring this thread's latest turn into its source", 'arrow.triangle.merge', state.busy || !state.merge));
  gitItems.push(item('git:more', 'More', 'Commit, files, branches', 'ellipsis'));
  const terminalItems = ctx.scripts.map(script => item(`terminal:script:${script.id}`, `${script.name}${script.runOnWorktreeCreate ? ' (setup)' : ''}`, script.command, threadScriptSymbol(script.icon), !state.operate));
  if (!terminalItems.length) terminalItems.push(item('terminal:no-scripts', 'No project scripts', 'This project has no saved scripts yet', 'play', true));
  for (const session of state.read && state.metadataReady ? ctx.sessions : []) {
    const summary = session.state.summary, cwd = str(summary?.cwd) || str(ctx.project?.workspaceRoot);
    terminalItems.push(item(`terminal:session:${session.target.terminalId}`, threadTerminalLabel(session.target.terminalId, summary ? obj(summary) : null),
      [threadTerminalStatus(session.state.status, session.state.hasRunningSubprocess), cwd.split('/').filter(Boolean).at(-1) || ''].filter(Boolean).join(' · '), 'terminal'));
  }
  terminalItems.push(item('terminal:new', 'Open new terminal', 'Start another shell for this thread', 'plus', !state.operate));
  const saved = fleet.saved.find(entry => entry.environmentId === client.environmentId);
  const environment = environmentSources(client, fleet.saved, fleet.entries).find(entry => entry.environmentId === client.environmentId);
  const reviewData = options.review ? mobileReviewSnapshot(options.dark === true, client) : null;
  const turns = reviewData?.sections.filter(section => section.id.startsWith('turn:')) ?? [];
  const reviewItem = (id: string, title: string, subtitle = '') => ({ ...item(`review:section:${id}`, title, subtitle, '', !id),
    selected: !!id && reviewData?.sectionId === id });
  // ReviewSheet/buildReviewSectionMenu: fixed primary choices, then the ready
  // turns in server order. Missing choices remain visible and disabled.
  const review = reviewData ? { primary: [reviewItem(reviewData.sections.find(section => section.id === 'git:branch-range')?.id ?? '', 'Changes'),
    reviewItem(reviewData.sections.find(section => section.id === 'git:working-tree')?.id ?? '', 'Uncommitted'),
    reviewItem(turns[0]?.id ?? '', 'Latest turn')], turns: turns.map(section => reviewItem(section.id, section.title, section.subtitle)),
    showSections: reviewData.sections.length > 0, inspectorAvailable: options.inspectorAvailable === true && reviewData.files.length > 0,
    inspectorVisible: options.inspectorVisible === true } : undefined;
  const configuration = { owner: state.owner, routeKey: options.routeKey, focused, title: reviewData?.title ?? str(ctx.thread?.title),
    subtitle: reviewData ? [reviewData.additions ? `+${reviewData.additions}` : '', reviewData.deletions ? `-${reviewData.deletions}` : '',
      reviewData.commentCount ? `${reviewData.commentCount} comment${reviewData.commentCount === 1 ? '' : 's'}` : ''].filter(Boolean).join(' · ')
      : [str(ctx.project?.title), str(saved?.mobileLabel) || environment?.label || ''].filter(Boolean).join(' · '),
    split: options.split, sidebarVisible: options.sidebarVisible, canGoBack: options.canGoBack, returnToChat: options.returnToChat,
    canOpenFiles: !review && !!str(ctx.project?.workspaceRoot), canOpenTerminal: !review && !!str(ctx.project?.workspaceRoot) && (state.read || state.operate),
    foreground: options.foreground, gitItems, terminalItems: review ? [] : terminalItems, review };
  // Include invisible command recipes too: a changed script command cannot use an old open menu.
  const version = JSON.stringify([configuration, options.routeIdentity, ctx.scripts, mergeTarget, mergeRun, quick, status?.refName, state.read, state.operate]);
  return { state, ctx, ready, quick, mergeTarget, mergeRun, configuration: { ...configuration, version }, version };
}
export function mobileThreadHeaderSnapshot(options: ThreadHeaderOptions, client: T3Client = mobileClient): ThreadHeaderSnapshot {
  const view = model(options, client), state = view.state, pending = mobileThreadHeaderPending(client);
  mobileThreadHeaderObserveRoute(JSON.stringify([options.routeKey, options.routeIdentity]), client);
  if (state.routeKey !== options.routeKey || state.routeIdentity !== options.routeIdentity || state.focused !== view.configuration.focused) state.visit++;
  state.routeIdentity = options.routeIdentity; state.routeKey = options.routeKey; state.focused = view.configuration.focused;
  return { owner: state.owner, routeKey: options.routeKey, version: view.version, configuration: JSON.stringify(view.configuration),
    ready: view.ready, needsPrepare: state.focused && !state.reading && (!state.prepared || state.retry), error: state.error,
    pendingLaunch: pending.token, pendingTerminalId: pending.terminalId };
}
/** Metadata stays in the copied shared reducer; only preparation/retry admission lives here. */
export function mobileThreadHeaderEvents(entries: unknown, client: T3Client = mobileClient) {
  const state = states.get(client); if (!state || state.owner !== reviewOwner(client)) return;
  for (const entry of arr(entries)) {
    if (entry.key !== 'terminal-metadata' || num(entry.generation, -1) !== client.generation) continue;
    const serial = subscriptionSerial(str(entry.subscriptionId)), seq = num(entry.seq);
    if (serial < state.subscription || serial === state.subscription && seq > 0 && seq <= state.seq) continue;
    if (serial > state.subscription) state.seq = 0;
    state.subscription = Math.max(serial, state.subscription); state.seq = Math.max(seq, state.seq);
    const value = obj(entry.value);
    if (value._retryDue || value._streamEnded) { state.metadataReady = false; state.metadataInvalidated = true; state.retry = true; state.invalidation++; }
    else if (value._transportError) { state.metadataReady = false; state.metadataInvalidated = true; }
    else if (['snapshot', 'upsert', 'remove'].includes(str(value.type))) { state.metadataReady = true; state.metadataInvalidated = false; }
  }
}
function scoped(client: T3Client, native: Native, state: State, routeKey: string): Native {
  const visit = state.visit;
  const assert = () => { assertReviewOwner(client, state.owner); if (states.get(client) !== state || state.routeKey !== routeKey || state.visit !== visit || !state.focused)
    throw new ClientError('The thread screen changed.', 'superseded'); };
  const base = letGoAware(mobileNative(native));
  return { available: base.available, watch: topic => base.watch(topic), later: async input => { assert(); const answer = await base.later(input); assert(); return answer; } };
}
export async function mobileThreadHeaderPrepare(owner: string, nativeInput: Native | null | undefined, now: number, client: T3Client = mobileClient) {
  const state = stateOf(client), route = state.routeKey, result = () => ({ owner, revision: client.revision, message: state.error });
  if (owner !== state.owner || !state.focused || state.reading || !nativeInput?.available) return result();
  state.reading = true; const invalidation = state.invalidation; let completed = true;
  try {
    const native = scoped(client, nativeInput, state, route), session = await client.http(native, '/api/auth/session');
    state.read = mobileSessionGrants(session, 'terminal:read'); state.operate = mobileSessionGrants(session, 'terminal:operate');
    state.merge = mobileSessionGrants(session, 'orchestration:operate');
    if (state.read) { const drawer = await terminalDrawerView(client, native, 0, now); if (!state.metadataInvalidated) state.metadataReady ||= /^\d+ sessions$/.test(drawer.metadata); }
    await mobileGitRead(owner, now, native, client);
    state.error = '';
  } catch (error) { if (letGo(error)) { completed = false; throw error; } state.error = fail(error); }
  finally { state.reading = false; state.prepared = completed; if (invalidation === state.invalidation) state.retry = false; client.revision++; }
  return result();
}
const locations: Record<string, string> = { threadFiles: '/files', gitOverview: '/git', threadReview: '/review', gitCommit: '/git/commit', gitConfirm: '/git-confirm', threadTerminal: '/terminal', thread: '', 'return-chat': '' };
export async function mobileThreadHeaderAction(raw: string, options: ThreadHeaderOptions, now: number, nativeInput: Native | null | undefined,
  files: Files, client: T3Client = mobileClient): Promise<ThreadHeaderResult> {
  let event: Obj; try { event = obj(JSON.parse(raw)); } catch { event = {}; }
  const view = model(options, client), state = view.state, id = str(event.itemID), route = options.routeKey, visit = state.visit;
  const out: ThreadHeaderResult = { revision: client.revision, message: '', requestRoute: route, environmentId: client.environmentId,
    threadId: client.threadId, navigation: '', location: '', sourceAction: id, key: '', pendingLaunch: '' };
  const navigate = (name: string, key = '') => { out.navigation = name; out.key = key;
    out.location = name === 'home' ? '/' : name === 'newTask' ? '/new' : name in locations
      ? `/threads/${encodeURIComponent(out.environmentId)}/${encodeURIComponent(out.threadId)}${locations[name]}` : ''; };
  if (!view.configuration.focused || state.busy || event.owner !== state.owner || event.routeKey !== route || event.version !== view.version
    || state.routeKey !== route || state.routeIdentity !== options.routeIdentity || !state.focused) return out;
  if (view.configuration.review && id.startsWith('review:')) {
    const review = view.configuration.review;
    const section = [...review.primary, ...review.turns].find(entry => entry.id === id && !entry.disabled);
    if (section && review.showSections) { out.navigation = 'review-section'; out.key = id.slice('review:section:'.length); }
    else if (id === 'review:back') navigate('return-chat');
    else if (id === 'review:sidebar' && options.split) out.navigation = 'sidebar';
    else if (id === 'review:inspector' && review.inspectorAvailable) out.navigation = 'review-inspector';
    return out;
  }
  const menuItem = [...view.configuration.gitItems, ...view.configuration.terminalItems].find(entry => entry.id === id);
  const special = view.configuration.review ? false : id === 'files' ? view.configuration.canOpenFiles : id === 'return-chat' ? options.split && options.returnToChat : id === 'home' ? !options.split && !options.canGoBack : ['sidebar', 'new-task'].includes(id) && options.split;
  if (menuItem ? menuItem.disabled || id.startsWith('terminal:') && !view.configuration.canOpenTerminal : !special) return out;
  if (view.configuration.review && id === 'git:review') return out;
  if (id === 'files' || id === 'git:more' || id === 'git:review' || ['sidebar','home','new-task','return-chat'].includes(id)) {
    navigate(({ files: 'threadFiles', 'git:more': 'gitOverview', 'git:review': 'threadReview', 'new-task': 'newTask', 'return-chat': 'return-chat', sidebar: 'sidebar', home: 'home' })[id] || ''); return out;
  }
  if (!nativeInput?.available) { out.message = 'Open T3 Code on your iPhone or iPad to use thread actions.'; return out; }
  state.busy = true;
  try {
    const native = scoped(client, nativeInput, state, route);
    if (id === 'git:quick') {
      const quick = view.quick;
      const reply = await mobileGitAction(state.owner, quick.kind === 'run_action' ? 'quick' : 'select',
        quick.kind === 'run_action' ? quick.action || '' : quick.kind === 'run_pull' ? 'pull' : 'pr', '', now, native, client);
      out.message = reply.message || reply.data.error;
      if (reply.destination === 'confirm') navigate('gitConfirm'); else if (reply.destination === 'commit') navigate('gitCommit');
    } else {
      const session = await client.http(native, '/api/auth/session');
      state.read = mobileSessionGrants(session, 'terminal:read'); state.operate = mobileSessionGrants(session, 'terminal:operate');
      state.merge = mobileSessionGrants(session, 'orchestration:operate');
      if (id === 'git:merge') {
        if (!state.merge) throw new ClientError('This connection cannot operate threads.');
        let issued = false, commandId = '';
        const capturedPending = () => client.local.pending[out.environmentId];
        const ownPending = () => !!commandId && capturedPending()?.payload.commandId === commandId;
        const assertMerge = () => {
          if (capturedPending() && !ownPending()) throw new ClientError('Another operation owns this connection.', 'superseded');
          // Once issued, the immutable write owns its acknowledgment independently
          // of presentation. Shared write captures the environment for completion.
          if (issued) return;
          const refuse = (text: string): never => { throw new ClientError(text, !issued && ownPending() ? 'client' : 'superseded'); };
          if (reviewOwner(client) !== state.owner || states.get(client) !== state || state.routeKey !== route || state.visit !== visit || !state.focused) refuse('The thread screen changed.');
          if (view.mergeTarget !== mergeTargetOf(client.projection) || view.mergeRun !== str(latestMergeBackRun(client.projection)?.id)) refuse('The merge source changed.');
          if (client.busy || !client.writable) refuse('Finish the pending command before merging.');
        };
        const base = letGoAware(mobileNative(nativeInput));
        const mergeNative: Native = { available: base.available, watch: topic => base.watch(topic), later: async input => {
          assertMerge(); if (obj(input).op === 'request' && obj(input).method === 'orchestration.dispatchCommand') issued = true;
          const answer = await base.later(input); assertMerge(); return answer;
        } };
        assertMerge(); [commandId] = await client.ids(mergeNative, 1); assertMerge();
        await client.dispatch(mergeNative, client === mobileClient ? nativeFiles(base) : files,
          { type: 'thread.merge_back', commandId, createdBy: 'user', creationSource: 'mobile', sourceThreadId: out.threadId,
            targetThreadId: view.mergeTarget, sourcePoint: { type: 'run', runId: view.mergeRun } }, 'Merge thread back', assertMerge);
        if (reviewOwner(client) === state.owner && states.get(client) === state && state.routeKey === route && state.visit === visit && state.focused) {
          out.threadId = view.mergeTarget; navigate('thread');
        }
      } else {
        const current = context(client), script = current.scripts.find(entry => `terminal:script:${entry.id}` === id);
        if (id.startsWith('terminal:session:')) {
          const terminalId = id.slice('terminal:session:'.length);
          if (!(state.read || state.operate) || !current.sessions.some(entry => entry.target.terminalId === terminalId)) throw new ClientError('The terminal session changed. Open the menu again.');
          navigate('threadTerminal', terminalId);
        } else {
          if (!state.operate) throw new ClientError('This connection cannot operate terminals.');
          if (id !== 'terminal:new' && (!script || JSON.stringify(script) !== JSON.stringify(view.ctx.scripts.find(entry => entry.id === script.id)))) throw new ClientError('The project script changed. Open the menu again.');
          const suffix = !state.metadataReady || !state.read ? (await client.ids(native, 1))[0] : undefined;
          const ids = context(client).sessions.map(entry => entry.target.terminalId);
          const terminalId = script && ids.length === 0 && suffix === undefined ? 'default' : threadNextTerminalId(ids, suffix);
          if (script) {
            if (JSON.stringify(context(client).scripts.find(entry => entry.id === script.id)) !== JSON.stringify(script)) throw new ClientError('The project script changed. Open the menu again.');
            const root = str(current.project?.workspaceRoot), worktreePath = str(obj(client.projection?.thread).worktreePath) || str(current.thread?.worktreePath) || null;
            out.pendingLaunch = mobileStageThreadScript(client, terminalId, { cwd: projectScriptCwd({ project: { cwd: root }, worktreePath }), worktreePath,
              env: projectScriptRuntimeEnv({ project: { cwd: root }, worktreePath }), initialInput: `${script.command}\r` });
          }
          navigate('threadTerminal', terminalId);
        }
      }
    }
  } catch (error) { if (letGo(error)) throw error; out.message = fail(error); }
  finally { state.busy = false; client.revision++; out.revision = client.revision; }
  return out;
}
