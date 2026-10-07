// @ref llp/1107.011-responsive-workspace.decision.md#navigation-and-data-ownership
// Pinned365aa87982: AdaptiveWorkspaceLayout focus tokens, ThreadRouteScreen,
// ThreadInspectorContentStack and workspace-inspector-pane. Root owns the clock,
// route stack, preferences, actual data and native presentation.
import type { MobileWorkspaceInspector } from './mobile-workspace';

type Mode = '' | 'files' | 'git' | 'route';
type RouteKind = 'thread' | 'files' | 'file' | 'review' | 'other';
export interface WorkspaceInspectorFocus {
  routeId: string; kind: RouteKind; environmentId: string; threadId: string;
  generation: number; cwd: string; selectedPath: string;
  /** Real content can be supplied. This is not a permission grant. */
  renderable: boolean;
  /** Source fileInspector.supported, for opening Files/Git or route mode. */
  candidateSupported: boolean;
}
export interface WorkspaceInspectorInput {
  focus: WorkspaceInspectorFocus | null; liveRouteIds: readonly string[];
  now: number; reducedMotion: boolean; columnSupported: boolean; columnWidth: number; resizing: boolean;
}
export type WorkspaceInspectorEvent =
  | { kind: 'mode'; owner: string; mode: 'files' | 'git' }
  | { kind: 'deactivate'; token: string }
  | { kind: 'release-role'; token: string }
  | { kind: 'deadline'; token: string }
  | { kind: 'end-exit'; token: string };
interface Role { token: string; routeId: string; value: 'inspector' }
interface RouteMode { routeId: string; kind: Exclude<RouteKind, 'other'>; mode: Mode; revealed: string }
interface Content {
  registration: MobileWorkspaceInspector; owner: string; cwd: string; mode: Exclude<Mode, ''>;
  stack: boolean; mounted: boolean; visited: Exclude<Mode, ''>[];
  prewarmToken: string; prewarmAt: number; exitToken: string; exitAt: number;
}
interface State {
  version: 1; serial: number; columnWidth: number; resizing: boolean; focus: WorkspaceInspectorFocus | null;
  roleKey: string; role: Role | null; desired: string;
  routes: RouteMode[]; content: Content | null;
}
const EXIT_MS = 260, PREWARM_MS = 350;
const object = (value: unknown): Record<string, unknown> => value !== null && typeof value === 'object' && !Array.isArray(value) ? value as Record<string, unknown> : {};
const text = (value: unknown) => typeof value === 'string' ? value : '';
const number = (value: unknown) => typeof value === 'number' && Number.isFinite(value) && value >= 0 ? value : 0;
const mode = (value: unknown): Mode => value === 'files' || value === 'git' || value === 'route' ? value : '';
const empty = (): State => ({ version: 1, serial: 0, columnWidth: 0, resizing: false, focus: null, roleKey: '', desired: '', role: null, content: null, routes: [] });
function focus(value: unknown): WorkspaceInspectorFocus | null {
  const raw = object(value), routeId = text(raw.routeId);
  if (!routeId) return null;
  const kind: RouteKind = raw.kind === 'thread' || raw.kind === 'files' || raw.kind === 'file' || raw.kind === 'review' ? raw.kind : 'other';
  return { routeId, kind, environmentId: text(raw.environmentId), threadId: text(raw.threadId), generation: number(raw.generation),
    cwd: text(raw.cwd), selectedPath: text(raw.selectedPath), renderable: raw.renderable === true, candidateSupported: raw.candidateSupported === true };
}
/** Captured by user mode actions. A changed workspace cannot receive a late click. */
export function workspaceInspectorFocusOwner(value: WorkspaceInspectorFocus | null): string {
  return value ? JSON.stringify([value.routeId, value.kind, value.environmentId, value.threadId, value.generation, value.cwd]) : '';
}
function decode(serialized: string): State {
  try {
    const raw = object(JSON.parse(serialized));
    if (raw.version !== 1 || !Number.isSafeInteger(raw.serial) || number(raw.serial) !== raw.serial || !Array.isArray(raw.routes)) return empty();
    const state: State = { version: 1, serial: raw.serial as number, columnWidth: number(raw.columnWidth), resizing: raw.resizing === true, focus: focus(raw.focus), roleKey: text(raw.roleKey), desired: text(raw.desired),
      role: null, content: null, routes: raw.routes.flatMap(value => {
        const entry = object(value);
        return text(entry.routeId) && (entry.kind === 'thread' || entry.kind === 'files' || entry.kind === 'file' || entry.kind === 'review')
          ? [{ routeId: text(entry.routeId), kind: entry.kind, mode: mode(entry.mode), revealed: text(entry.revealed) }] : [];
      }) };
    const role = object(raw.role);
    if (text(role.token) && text(role.routeId) && role.value === 'inspector') state.role = { token: text(role.token), routeId: text(role.routeId), value: 'inspector' };
    const content = object(raw.content), registration = object(content.registration), currentMode = mode(content.mode);
    const kind = registration.kind;
    if (text(registration.token) && text(registration.routeId) && currentMode && (kind === 'files' || kind === 'git' || kind === 'route' || kind === 'changed-files')) {
      state.content = { registration: { token: text(registration.token), routeId: text(registration.routeId), environmentId: text(registration.environmentId),
        threadId: text(registration.threadId), generation: number(registration.generation), role: 'inspector', kind,
        selectedPath: text(registration.selectedPath), active: registration.active === true, renderable: registration.renderable === true },
        owner: text(content.owner), cwd: text(content.cwd), mode: currentMode, stack: content.stack === true, mounted: content.mounted === true,
        visited: Array.isArray(content.visited) ? [...new Set(content.visited.map(mode).filter((value): value is Exclude<Mode, ''> => !!value))] : [],
        prewarmToken: text(content.prewarmToken), prewarmAt: number(content.prewarmAt), exitToken: text(content.exitToken), exitAt: number(content.exitAt) };
    }
    return state;
  } catch { return empty(); }
}
function token(state: State, kind: string) { return `${kind}:${++state.serial}`; }
function deactivate(state: State, now: number, reducedMotion: boolean) {
  if (!state.content?.registration.active) return;
  state.content.registration.active = false;
  state.content.exitToken = token(state, 'exit');
  state.content.exitAt = now + (reducedMotion ? 0 : EXIT_MS);
}
function prewarm(state: State, content: Content, now: number) {
  content.prewarmToken = ''; content.prewarmAt = 0;
  if (!content.mounted || !content.stack || content.mode === 'route') return;
  const alternate = content.mode === 'files' ? 'git' : 'files';
  if (!content.visited.includes(alternate)) {
    content.prewarmToken = token(state, 'prewarm'); content.prewarmAt = now + PREWARM_MS;
  }
}
function currentMode(state: State, current: WorkspaceInspectorFocus | null): Mode {
  if (!current || current.kind === 'other') return '';
  if (current.kind === 'file' || current.kind === 'review') return 'route';
  const selected = state.routes.find(entry => entry.routeId === current.routeId)?.mode ?? '';
  return selected === 'files' && !current.cwd ? '' : selected;
}

/** One synchronous, serializable transition. Calling it again with unchanged
 * input is idempotent. Root adopts serialized only from a mutation, then drives
 * the returned deadlines or an actual native end-exit callback. */
export function workspaceInspectorTransition(serialized: string, input: WorkspaceInspectorInput, event?: WorkspaceInspectorEvent) {
  const state = decode(serialized), current = focus(input.focus), previous = state.focus;
  const now = number(input.now), owner = workspaceInspectorFocusOwner(current);
  const columnWidth = input.columnSupported ? number(input.columnWidth) : 0;
  const geometryChanged = columnWidth !== state.columnWidth || input.resizing !== state.resizing;
  state.columnWidth = columnWidth; state.resizing = input.resizing;
  let revealInspector = false;
  const live = new Set(input.liveRouteIds);
  if (current) live.add(current.routeId);
  state.routes = state.routes.filter(entry => live.has(entry.routeId));
  if (previous?.kind === 'thread' && (previous.routeId !== current?.routeId || current?.kind !== 'thread')) {
    const previousMode = state.routes.find(entry => entry.routeId === previous.routeId);
    if (previousMode) previousMode.mode = '';
  }
  state.focus = current;
  let selected = current && state.routes.find(entry => entry.routeId === current.routeId);
  if (current && current.kind !== 'other' && (!selected || selected.kind !== current.kind)) {
    state.routes = state.routes.filter(entry => entry.routeId !== current.routeId);
    selected = { routeId: current.routeId, kind: current.kind, mode: current.kind === 'thread' ? '' : 'route', revealed: '' };
    state.routes.push(selected);
  }
  // The role's focus lease is separate from the renderer's retained exit lease.
  const roleKey = current && current.kind !== 'other' ? owner : '';
  if (roleKey !== state.roleKey) {
    state.roleKey = roleKey;
    state.role = roleKey && current ? { token: token(state, 'role'), routeId: current.routeId, value: 'inspector' } : null;
  }
  if (event?.kind === 'mode' && event.owner === owner && selected && (current?.kind === 'thread' || current?.kind === 'files') && current.candidateSupported && current.renderable
    && (event.mode === 'git' || !!current.cwd)) {
    selected.mode = event.mode === 'files' && current.kind === 'files' ? 'route' : event.mode; revealInspector = true;
  }
  if (current?.kind === 'files' && selected && current.candidateSupported && current.cwd && !selected.revealed) {
    selected.revealed = 'files'; revealInspector = true;
  }
  const reviewReveal = current?.kind === 'review' && current.environmentId && current.threadId ? JSON.stringify([current.environmentId, current.threadId]) : '';
  if (reviewReveal && selected && selected.revealed !== reviewReveal) {
    selected.revealed = reviewReveal; revealInspector = true;
  }
  const activeMode = currentMode(state, current);
  const eligible = !!current && current.kind !== 'other' && !!activeMode && current.renderable
    && !!current.environmentId && !!current.threadId
    && (!(current.kind === 'files' || current.kind === 'file') || current.candidateSupported && !!current.cwd);
  const desired = eligible ? JSON.stringify([owner, current!.kind, activeMode, current!.selectedPath]) : '';
  if (desired !== state.desired) {
    state.desired = desired;
    if (eligible && current && activeMode) {
      const stack = current.kind === 'thread' || current.kind === 'files', old = state.content;
      // React preserves mountedModes when the same stack component remains
      // mounted through a renderer replacement. resetKeys reset its boundaries,
      // not its visited-mode state. An actual unmount starts fresh.
      const visited = stack && old?.stack && old.mounted && input.columnSupported ? [...old.visited] : [];
      if (input.columnSupported && !visited.includes(activeMode)) visited.push(activeMode);
      const kind = current.kind === 'review' ? 'changed-files' : current.kind === 'file' ? 'files' : activeMode;
      const content: Content = { registration: { token: token(state, 'content'), routeId: current.routeId,
        environmentId: current.environmentId, threadId: current.threadId, generation: current.generation,
        role: 'inspector', kind, selectedPath: current.kind === 'file' ? current.selectedPath : '', active: true, renderable: true },
        owner, cwd: current.cwd, mode: activeMode, stack, mounted: input.columnSupported, visited,
        prewarmToken: '', prewarmAt: 0, exitToken: '', exitAt: 0 };
      state.content = content;
      // Content-only refreshes do not restart a source effect keyed by mode.
      if (old?.stack && stack && old.mounted && content.mounted && old.mode === activeMode) {
        content.prewarmToken = old.prewarmToken; content.prewarmAt = old.prewarmAt;
      } else prewarm(state, content, now);
    } else deactivate(state, now, input.reducedMotion);
  }
  const content = state.content;
  // Unsupported geometry unmounts only the renderer. Source registration hooks
  // and onClosed still live above that conditional subtree. An active lease
  // remains active. The source close effect restarts when inspectorWidth changes,
  // including null/unsupported transitions, and old completions must be rejected.
  if (content && content.mounted !== input.columnSupported) {
    content.mounted = input.columnSupported;
    content.visited = input.columnSupported ? [content.mode] : [];
    prewarm(state, content, now);
  }
  if (event?.kind === 'deactivate' && content?.registration.token === event.token) deactivate(state, now, input.reducedMotion);
  if (content && !content.registration.active && geometryChanged) {
    content.exitToken = token(state, 'exit');
    content.exitAt = now + (input.reducedMotion ? 0 : EXIT_MS);
  }
  if (event?.kind === 'release-role' && state.role?.token === event.token) state.role = null;
  if (content && !content.registration.active && input.reducedMotion) content.exitAt = Math.min(content.exitAt, now);
  if (event?.kind === 'end-exit' || event?.kind === 'deadline') {
    if (content && !content.registration.active && content.exitToken && event.token === content.exitToken
      && (event.kind === 'end-exit' || now >= content.exitAt)) state.content = null;
    else if (event.kind === 'deadline' && content?.prewarmToken && event.token === content.prewarmToken && now >= content.prewarmAt
      && (content.registration.active || now < content.exitAt)) {
      const alternate = content.mode === 'files' ? 'git' : 'files';
      if (content.mounted && content.stack && content.mode !== 'route' && !content.visited.includes(alternate)) content.visited.push(alternate);
      content.prewarmToken = ''; content.prewarmAt = 0;
    }
  }
  const next = JSON.stringify(state);
  return { ...project(state, next), changed: next !== serialized, revealInspector };
}

/** Reads only committed state. It never reconciles focus or creates tokens. */
export function workspaceInspectorSnapshot(serialized: string) {
  const state = decode(serialized);
  return project(state, JSON.stringify(state));
}
function project(state: State, serialized: string) {
  const result = state.content;
  return { serialized, focusOwner: workspaceInspectorFocusOwner(state.focus), mode: currentMode(state, state.focus),
    roleJSON: state.role ? JSON.stringify(state.role) : '', registrationJSON: result ? JSON.stringify(result.registration) : '',
    contentOwner: result?.owner ?? '', contentRouteId: result?.registration.routeId ?? '', contentMode: result?.mode ?? '',
    contentEnvironmentId: result?.registration.environmentId ?? '', contentThreadId: result?.registration.threadId ?? '',
    contentGeneration: result?.registration.generation ?? 0,
    kind: result?.registration.kind ?? '', cwd: result?.cwd ?? '', selectedPath: result?.registration.selectedPath ?? '',
    active: result?.registration.active ?? false, mounted: result?.mounted ?? false,
    mountedFiles: !!result?.mounted && result.visited.includes('files'), mountedGit: !!result?.mounted && result.visited.includes('git'),
    mountedRoute: !!result?.mounted && result.visited.includes('route'),
    exitToken: result?.exitToken ?? '', exitAt: result?.exitAt ?? 0,
    prewarmToken: result?.prewarmToken ?? '', prewarmAt: result?.prewarmAt ?? 0 };
}
