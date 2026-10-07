// Pinned365aa87982 ThreadTerminalRouteScreen over existing terminal metadata/UI/transport owners.
// @ref llp/1106.007-mobile-terminal.decision.md#root-seam
import { mobileClient, mobileNative } from './client';
import { mobileSessionGrants } from './environment-detail';
import type { T3Client } from './shared/client';
import { obj, str } from './shared/domain';
import { bridgeReply, ClientError, nativeFiles, type Native, type Files } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { terminalDrawerView, knownSessions, launchFor, allocatableIds, revealTerminal } from './shared/terminal-drawer-view';
import { terminalUiStore, DEFAULT_TERMINAL_ID } from './shared/terminal-ui-state';
import { nextTerminalId, resolveTerminalSessionLabel } from './shared/terminal-labels';
import { mobileTheme } from './design';
import { MOBILE_THEME_ARTWORK } from './settings-theme-data';

export interface MobileTerminalTab { id: string; label: string; status: string; selected: boolean; running: boolean }
export interface MobileTerminalColors { background: string; foreground: string; mutedForeground: string; border: string; cursor: string; config: string }
export interface MobileTerminalSnapshot { revision: number; environmentId: string; threadId: string; terminalId: string; sessionKey: string;
  title: string; subtitle: string; ready: boolean; readOnly: boolean; sourceJSON: string; error: string; emptyTitle: string; emptyDetail: string;
  tabs: MobileTerminalTab[]; hostPlatform: string; colors: MobileTerminalColors; }
const owner = (client: T3Client) => JSON.stringify([client.generation, client.environmentId, client.threadId]);
const permissions = new WeakMap<T3Client, { owner: string; read: boolean; operate: boolean }>();
const busy = new WeakSet<T3Client>();
export function mobileTerminalColors(scheme: string, palette = 't3-code'): MobileTerminalColors {
  const dark = scheme === 'dark', colors = mobileTheme(scheme, palette).colors;
  const table = MOBILE_THEME_ARTWORK as Record<string, Record<string, { terminalBackground: string; terminalForeground: string; terminalCursor: string }>>;
  const roles = (table[palette] ?? table['t3-code']!)[dark ? 'dark' : 'light']!;
  const ansi = [dark ? '#141415' : '#1F1F21', '#ff2e3f', '#0dbe4e', '#ffca00', '#009fff', '#c635e4', '#08c0ef', '#c6c6c8'];
  const config = [`background = ${roles.terminalBackground}`, `foreground = ${roles.terminalForeground}`, `cursor-color = ${roles.terminalCursor}`,
    `cursor-text = ${roles.terminalBackground}`, ...[...ansi, ...ansi].map((color, index) => `palette = ${index}=${color}`)].join('\n') + '\n';
  return { background: roles.terminalBackground, foreground: roles.terminalForeground, mutedForeground: colors.muted ?? '', border: colors.border ?? '', cursor: roles.terminalCursor, config };
}
function hostPlatform(client: T3Client): string {
  const platform = obj(obj(client.config.environment).platform);
  const os = str(platform.os); return os === 'darwin' ? 'mac' : ['linux', 'windows'].includes(os) ? os : 'unknown';
}
export async function mobileTerminalPrepare(requestedId: string, scheme: string, palette: string, fontSize: number, now: number,
  nativeInput: Native | null | undefined, client: T3Client = mobileClient): Promise<MobileTerminalSnapshot> {
  const captured = owner(client), colors = mobileTerminalColors(scheme, palette), ref = { environmentId: client.environmentId, threadId: client.threadId };
  const data: MobileTerminalSnapshot = { revision: client.revision, ...ref, terminalId: requestedId, sessionKey: '', title: 'Terminal', subtitle: '', ready: false,
    readOnly: true, sourceJSON: '', error: '', emptyTitle: '', emptyDetail: '', tabs: [], hostPlatform: hostPlatform(client), colors };
  if (!ref.environmentId || !ref.threadId) return { ...data, emptyTitle: 'Thread not found', emptyDetail: 'Return to the conversation and open its terminal again.' };
  if (!nativeInput?.available || client.connection !== 'connected') return { ...data, emptyTitle: 'Terminal unavailable', emptyDetail: 'Reconnect to this environment and try again.' };
  const native = letGoAware(mobileNative(nativeInput));
  try {
    const session = await client.http(native, '/api/auth/session');
    if (captured !== owner(client)) throw new ClientError('The selected conversation changed.');
    const read = mobileSessionGrants(session, 'terminal:read'), operate = mobileSessionGrants(session, 'terminal:operate');
    permissions.set(client, { owner: captured, read, operate }); data.readOnly = !operate;
    if (!read && !operate) return { ...data, emptyTitle: 'Terminal access unavailable', emptyDetail: 'This connection does not have permission to view terminals.' };
    const drawer = await terminalDrawerView(client, native, 0, now);
    if (captured !== owner(client)) throw new ClientError('The selected conversation changed.');
    const known = knownSessions(client, ref), ui = terminalUiStore(client).getState();
    const fallback = known.find(item => ['running', 'starting'].includes(item.state.status)) ?? (!operate ? known[0] : undefined);
    const terminalId = requestedId || fallback?.target.terminalId || (operate ? DEFAULT_TERMINAL_ID : '');
    data.tabs = known.map(item => ({ id: item.target.terminalId, label: resolveTerminalSessionLabel(item.target.terminalId, item.state.summary),
      status: item.state.status, running: item.state.hasRunningSubprocess, selected: item.target.terminalId === terminalId }));
    if (!terminalId && drawer.metadata !== 'waiting' && !/^\d+ sessions$/.test(drawer.metadata)) return { ...data, emptyTitle: 'Could not load terminals', emptyDetail: drawer.metadata, error: drawer.metadata };
    if (!terminalId) return { ...data, emptyTitle: drawer.metadata === 'waiting' ? 'Loading terminals' : 'No terminal sessions',
      emptyDetail: drawer.metadata === 'waiting' ? 'Reading existing terminal sessions.' : 'Existing terminals will appear here when another client opens one.' };
    if (!operate && !known.some(item => item.target.terminalId === terminalId)) return { ...data, emptyTitle: 'Terminal unavailable', emptyDetail: 'That terminal session is no longer available.' };
    const existing = known.find(item => item.target.terminalId === terminalId)?.state.summary;
    const launch = launchFor(client, ref.threadId), detail = obj(client.projection.thread);
    const worktreePath = existing?.worktreePath ?? (str(detail.worktreePath) || launch?.worktreePath || null);
    const cwd = existing?.cwd || worktreePath || launch?.cwd || '';
    if (!cwd && operate) return { ...data, emptyTitle: 'Terminal unavailable', emptyDetail: 'This thread does not have a workspace.' };
    ui.ensureTerminal(ref, terminalId, { open: true, active: true });
    const project = client.shell.projects.find(item => item.id === client.projectId);
    data.subtitle = str(project?.title); data.terminalId = terminalId; data.sessionKey = JSON.stringify([ref.environmentId, ref.threadId, terminalId]);
    data.title = resolveTerminalSessionLabel(terminalId, existing); data.ready = true;
    data.sourceJSON = JSON.stringify({ ...ref, terminalId, cwd, worktreePath, env: launch?.env ?? {}, readOnly: !operate,
      autoFocus: operate, hostPlatform: data.hostPlatform, fontSize: Number.isFinite(fontSize) ? Math.max(6, Math.min(14, fontSize)) : 10.5,
      appearance: scheme === 'dark' ? 'dark' : 'light', themeConfig: colors.config, background: colors.background, foreground: colors.foreground, mutedForeground: colors.mutedForeground });
    return data;
  } catch (error) { if (letGo(error)) throw error; return { ...data, error: error instanceof Error ? error.message : 'Could not open terminal.', emptyTitle: 'Terminal unavailable' }; }
}

/** Header/tab lifecycle uses actual terminal RPCs; live input/output remain on the native stream. */
export async function mobileTerminalAction(action: string, terminalId: string, value: string, nativeInput: Native | null | undefined, files: Files,
  client: T3Client = mobileClient) {
  const captured = owner(client), ref = { environmentId: client.environmentId, threadId: client.threadId };
  const result = (message = '', selectedId = terminalId, closed = false) => ({ revision: client.revision, message, terminalId: selectedId, closed, ...ref });
  if (!nativeInput?.available) return result('Open T3 Code on your iPhone or iPad to use terminals.');
  if (busy.has(client)) return result('Wait for the current terminal change to finish.');
  const native = letGoAware(mobileNative(nativeInput)), storage = client === mobileClient ? nativeFiles(native) : files;
  let ownsBusy = false;
  try {
    const permission = permissions.get(client);
    if (!permission || permission.owner !== captured || !ref.threadId) throw new ClientError('Reopen the terminal to check its permissions.');
    if (!permission.operate && !['capture', 'hide-keyboard', 'select'].includes(action)) throw new ClientError('This connection does not have permission to operate terminals.');
    if (action === 'select') {
      if (!knownSessions(client, ref).some(item => item.target.terminalId === terminalId)) throw new ClientError('That terminal session is no longer available.');
      revealTerminal(client, ref, terminalId); return result();
    }
    if (['input', 'modifier', 'paste', 'capture', 'show-keyboard', 'hide-keyboard'].includes(action)) {
      const reply = await bridgeReply(native, { op: 'mobileTerminalControl', key: JSON.stringify([ref.environmentId, ref.threadId, terminalId]), action, value });
      if (!reply.ok) throw new ClientError(reply.error!.message); return result();
    }
    busy.add(client); ownsBusy = true;
    if (action === 'new') {
      const launch = launchFor(client, ref.threadId); if (!launch) throw new ClientError('This thread does not have a workspace.');
      const id = nextTerminalId(allocatableIds(client, ref));
      await client.rpc(native, 'terminal.open', { threadId: ref.threadId, terminalId: id, cwd: str(obj(client.projection.thread).worktreePath) || launch.cwd,
        worktreePath: str(obj(client.projection.thread).worktreePath) || launch.worktreePath, env: launch.env, cols: 80, rows: 24 });
      if (captured !== owner(client)) throw new ClientError('The selected conversation changed.');
      revealTerminal(client, ref, id); await client.persist(storage); return result('', id);
    }
    if (!terminalId) throw new ClientError('Choose a terminal first.');
    if (action === 'clear') { await client.rpc(native, 'terminal.clear', { threadId: ref.threadId, terminalId }); return result(); }
    if (action === 'close') {
      await client.rpc(native, 'terminal.close', { threadId: ref.threadId, terminalId, deleteHistory: true });
      if (captured !== owner(client)) throw new ClientError('The selected conversation changed.');
      terminalUiStore(client).getState().closeTerminal(ref, terminalId); await client.persist(storage); return result('', '', true);
    }
    throw new ClientError('Unknown terminal action.');
  } catch (error) { if (letGo(error)) throw error; return result(error instanceof Error ? error.message : 'Could not change terminal.'); }
  finally { if (ownsBusy) busy.delete(client); client.revision++; }
}
