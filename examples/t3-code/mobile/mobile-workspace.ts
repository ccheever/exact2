// @ref llp/1107.011-responsive-workspace.decision.md#navigation-and-data-ownership
// Pinned mobile365aa87982 Stack.workspaceLocationFromState / AdaptiveWorkspaceLayout.
// Exact route entries cross this seam; React Navigation state never does.
import { adaptiveWorkspace, adaptiveInspectorResize, type AdaptiveWorkspaceInput } from './adaptive-workspace';
import { resolveThreadSelectionNavigationAction, resolveFileSelectionNavigationAction } from './adaptive-navigation';
import type { WorkspaceAuxiliaryPaneRole } from './layout-mobile';

export const MOBILE_SIDEBAR_ROUTE_KEY = 't3-workspace-sidebar';
export interface MobileWorkspaceEntry { id: string; name: string; url: string; params: Record<string, string> }
export interface MobileWorkspaceInspector {
  token: string; routeId: string; environmentId: string; threadId: string; generation: number;
  role: WorkspaceAuxiliaryPaneRole; kind: 'files' | 'git' | 'route' | 'changed-files';
  selectedPath: string; active: boolean; renderable: boolean;
}
export interface MobileWorkspaceOptions {
  width: number; height: number;
  primarySidebarPreferredVisible: boolean; supplementaryPanePreferredVisible: boolean; fileInspectorPreferredVisible: boolean;
  supplementaryPanePreferredWidth?: number | null; fileInspectorPreferredWidth?: number | null;
  /** Actual client generation, when root has a selected connection. */
  generation?: number;
  hasWorkspace?: boolean; selectedEnvironmentId?: string; selectedThreadId?: string;
  threadMode?: 'files' | 'git' | ''; threadModeOwner?: string;
  /** Root keeps registration through exit; a new owner replaces its token. */
  inspector?: MobileWorkspaceInspector | null;
  /** Focus-scoped role owner is separate from retained content ownership. */
  role?: { token: string; routeId: string; value: WorkspaceAuxiliaryPaneRole } | null;
  inspectorExitToken?: string; inspectorResizing?: boolean;
  dividerColor?: string; dividerActiveColor?: string;
  reducedMotion: boolean; appearance: string; background: string;
}
const object = (value: unknown): Record<string, unknown> => value !== null && typeof value === 'object' && !Array.isArray(value) ? value as Record<string, unknown> : {};
const text = (value: unknown) => typeof value === 'string' ? value : '';
const finite = (value: number) => Number.isFinite(value) ? Math.max(0, value) : 0;
const preferredWidth = (value: number | null | undefined) => typeof value === 'number' && Number.isFinite(value) && value > 0 ? value : undefined;
const pathname = (url: string) => url.split(/[?#]/, 1)[0] || '/';

/** Contract projects only these scalar entry fields, never its whole Router. */
export function mobileWorkspaceEntries(input: unknown): MobileWorkspaceEntry[] {
  if (!Array.isArray(input)) return [];
  return input.flatMap(raw => {
    const value = object(raw), id = typeof value.id === 'number' && Number.isSafeInteger(value.id) ? String(value.id) : text(value.id);
    if (!id || !text(value.name) || !text(value.url).startsWith('/') || text(value.url).startsWith('//')) return [];
    const params = Object.fromEntries(Object.entries(object(value.params)).filter((pair): pair is [string, string] => typeof pair[1] === 'string'));
    return [{ id, name: text(value.name), url: text(value.url), params }];
  });
}

/** Source overlays stay overlays even when their native presentation becomes a
 * card at a wider width. Flattened Exact settings/new-task children share the
 * ownership of their pinned source SettingsSheet/NewTaskSheet parent. */
export function mobileWorkspaceOverlay(name: string) {
  return name.startsWith('settings') || name.startsWith('newTask') || [
    'archive', 'connections', 'connect', 'environments', 'environmentDetail', 'connectOnboarding',
    'threadAgents', 'threadQueue', 'threadReviewComment', 'threadBrowser', 'threadDevices',
    'threadModel', 'threadModelOption', 'threadRuntime', 'threadModelFilter',
    // Source terminal output is local presentation, represented by an Exact route.
    'threadTerminalCapture', 'gitOverview', 'gitCommit', 'gitBranches', 'gitConfirm',
  ].includes(name);
}
export function mobileWorkspaceLocation(input: unknown) {
  const entries = mobileWorkspaceEntries(input), top = entries.at(-1);
  const workspace = entries.findLast(entry => !mobileWorkspaceOverlay(entry.name)) ?? top;
  const index = workspace ? entries.indexOf(workspace) : -1;
  return { entries, top, workspace, index, overlayCount: top && workspace ? entries.length - index - 1 : 0 };
}

/** Candidate describes source rendering, not permission or loaded data. Root
 * must supply a real renderable registration before reserving a trailing pane. */
export function mobileWorkspaceInspectorCandidate(name: string) {
  if (name === 'threadFiles') return { kind: 'files', role: 'inspector', main: 'chat', automatic: true } as const;
  if (name === 'threadFile') return { kind: 'files', role: 'inspector', main: 'file', automatic: true } as const;
  if (name === 'threadReview') return { kind: 'changed-files', role: 'inspector', main: 'review', automatic: true } as const;
  if (name === 'thread') return { kind: 'none', role: 'inspector', main: 'chat', automatic: false } as const;
  return { kind: 'none', role: 'supplementary', main: 'route', automatic: false } as const;
}

export function mobileWorkspaceThreadOwner(entry: MobileWorkspaceEntry | undefined, generation = 0) {
  return entry?.name === 'thread' && entry.params.threadEnvironment && entry.params.threadId
    ? JSON.stringify([entry.id, entry.params.threadEnvironment, entry.params.threadId, generation]) : '';
}

export function mobileWorkspace(input: unknown, options: MobileWorkspaceOptions) {
  const { top, workspace, overlayCount } = mobileWorkspaceLocation(input);
  const focusedCandidate = mobileWorkspaceInspectorCandidate(top?.name ?? '');
  const threadOwner = mobileWorkspaceThreadOwner(top, options.generation);
  const actualWorkspace = options.hasWorkspace === true
    && (options.selectedEnvironmentId === undefined || options.selectedEnvironmentId === top?.params.threadEnvironment)
    && (options.selectedThreadId === undefined || options.selectedThreadId === top?.params.threadId);
  const explicitMode = actualWorkspace && threadOwner && options.threadModeOwner === threadOwner
    && (options.threadMode === 'files' || options.threadMode === 'git') ? options.threadMode : '';
  const candidateKind = explicitMode || (focusedCandidate.kind === 'files' && !actualWorkspace ? 'none' : focusedCandidate.kind);
  const registration = options.inspector;
  const registered = !!registration?.token && !!registration.routeId && registration.renderable
    && (options.generation === undefined || registration.generation === options.generation);
  const role = options.role?.token && options.role.routeId === top?.id ? options.role.value
    : focusedCandidate.role === 'inspector' ? 'inspector' : null;
  const active = registered && registration!.active && registration!.routeId === top?.id
    && (!registration!.environmentId || registration!.environmentId === top?.params.threadEnvironment)
    && (!registration!.threadId || registration!.threadId === top?.params.threadId);
  const geometryInput: AdaptiveWorkspaceInput = { ...options, width: finite(options.width), height: finite(options.height),
    pathname: pathname(workspace?.url ?? '/'), focusedAuxiliaryPaneRole: role,
    supplementaryPanePreferredWidth: preferredWidth(options.supplementaryPanePreferredWidth),
    fileInspectorPreferredWidth: preferredWidth(options.fileInspectorPreferredWidth),
    inspectorRegistered: registered, inspectorActive: active };
  const geometry = adaptiveWorkspace(geometryInput);
  const inspectorContentWidth = geometry.inspectorMounted ? geometry.auxiliaryPaneWidth : 0;
  const configuration = JSON.stringify({ inspectorRouteKey: 't3-workspace-inspector',
    inspectorOwner: geometry.inspectorMounted ? registration!.token : '', inspectorExitToken: options.inspectorExitToken ?? '',
    inspectorContentWidth, inspectorVisible: geometry.inspectorVisible, inspectorResizing: options.inspectorResizing === true,
    dividerColor: options.dividerColor ?? '#808080', dividerActiveColor: options.dividerActiveColor ?? '#808080',
    sidebarRouteKey: MOBILE_SIDEBAR_ROUTE_KEY,
    viewportWidth: geometryInput.width, viewportHeight: geometryInput.height,
    usesSplitView: geometry.usesSplitView, sidebarVisible: geometry.sidebarVisible,
    sidebarContentWidth: geometry.sidebarContentWidth, sidebarTargetWidth: geometry.sidebarTargetWidth,
    contentSettledWidth: geometry.contentSettledWidth, inspectorTargetWidth: geometry.inspectorTargetWidth,
    reducedMotion: options.reducedMotion, appearance: options.appearance === 'dark' ? 'dark' : 'light', background: options.background });
  return { ...geometry, ready: !!workspace, topRouteId: top?.id ?? '', workspaceRouteId: workspace?.id ?? '',
    workspaceName: workspace?.name ?? '', workspaceURL: workspace?.url ?? '', workspacePath: geometryInput.pathname,
    environmentId: workspace?.params.threadEnvironment ?? '', threadId: workspace?.params.threadId ?? '',
    overlayCount, sidebarRouteKey: MOBILE_SIDEBAR_ROUTE_KEY, inspectorRouteKey: 't3-workspace-inspector', inspectorContentWidth, configuration,
    inspectorToken: registered ? registration!.token : '', inspectorRouteId: registered ? registration!.routeId : '',
    inspectorKind: registered ? registration!.kind : 'none', inspectorSelectedPath: registered ? registration!.selectedPath : '',
    threadOwner, inspectorCandidate: candidateKind, inspectorMain: focusedCandidate.main, inspectorAutomatic: candidateKind !== 'none' && focusedCandidate.automatic,
  };
}

export interface MobileWorkspaceNavigation {
  operation: 'none' | 'push' | 'replace'; sourceAction: string;
  requestRoute: string; anchorRouteId: string; anchorLocation: string; location: string;
  dismissOverlays: boolean; hideFileInspector: boolean; message: string;
}
function navigationResult(input: unknown, location: string): MobileWorkspaceNavigation {
  const { top, workspace, overlayCount } = mobileWorkspaceLocation(input);
  return { operation: 'none', sourceAction: '', requestRoute: top?.id ?? '', anchorRouteId: workspace?.id ?? '',
    anchorLocation: workspace?.url ?? '', location, dismissOverlays: overlayCount > 0, hideFileInspector: false, message: '' };
}

/** Apply in one root action: push/replace(go(nav, anchorLocation), location).
 * go keeps the actual anchor id while dismissing overlays; replace preserves
 * that id for the source set-params case. Never reconstruct an Exact Router. */
export function mobileWorkspaceThreadSelection(input: unknown, usesSplitView: boolean, environmentId: string, threadId: string): MobileWorkspaceNavigation {
  const location = `/threads/${encodeURIComponent(environmentId)}/${encodeURIComponent(threadId)}`;
  const result = navigationResult(input, location), { entries, workspace, index, overlayCount } = mobileWorkspaceLocation(input);
  if (!workspace || !environmentId || !threadId) return { ...result, message: 'Choose an available thread.' };
  // go() resolves the nearest URL, not an id. Refuse malformed projections that
  // cannot identify the intended underlying visit without changing the router.
  if (entries.slice(index + 1).some(entry => entry.url === workspace.url)) return { ...result, message: 'The workspace route changed.' };
  const action = resolveThreadSelectionNavigationAction({ usesSplitView, pathname: pathname(workspace.url) });
  const same = workspace.params.threadEnvironment === environmentId && workspace.params.threadId === threadId;
  const operation = action === 'set-params' && same && overlayCount === 0 ? 'none' : action === 'push' ? 'push' : 'replace';
  return { ...result, operation, sourceAction: action, hideFileInspector: overlayCount > 0 || operation === 'replace' };
}

/** File browser/preview are one workspace destination when the real persistent
 * inspector is supported. The source compact navigation retains the browser. */
export function mobileWorkspaceFileSelection(input: unknown, hasPersistentFileInspector: boolean, environmentId: string, threadId: string, path: string): MobileWorkspaceNavigation {
  const normalizedPath = path.split('/').filter(Boolean).join('/');
  const result = navigationResult(input, `/threads/${encodeURIComponent(environmentId)}/${encodeURIComponent(threadId)}/files/${encodeURIComponent(normalizedPath)}`);
  if (!environmentId || !threadId || !normalizedPath) return { ...result, message: 'Choose a file in the current workspace.' };
  const { entries, workspace, index } = mobileWorkspaceLocation(input);
  if (!workspace || workspace.params.threadEnvironment !== environmentId || workspace.params.threadId !== threadId
    || entries.slice(index + 1).some(entry => entry.url === workspace.url)) return { ...result, message: 'The workspace route changed.' };
  const action = workspace.name === 'thread' ? 'push' : workspace.name === 'threadFile' ? 'replace'
    : resolveFileSelectionNavigationAction({ hasPersistentFileInspector });
  return { ...result, operation: action === 'replace' ? 'replace' : 'push', sourceAction: action };
}


/** Native controls propose changes; only the current root registration owns them. */
export function mobileWorkspaceEvent(raw: string, owner: string, exitToken: string, visible: boolean, contentPaneWidth: number) {
  const empty = { kind: '', owner: '', token: '', phase: '', value: '', width: 0 };
  let event: Record<string, unknown>; try { event = object(JSON.parse(raw)); } catch { return empty; }
  if (!owner || event.owner !== owner) return empty;
  if (event.kind === 'inspector-closed') return exitToken && event.exitToken === exitToken && !visible
    ? { ...empty, kind: 'end-exit', owner, token: exitToken } : empty;
  if (!visible && !(event.kind === 'resize' && event.phase === 'end')) return empty;
  if (event.kind === 'search' && typeof event.value === 'string') return { ...empty, kind: 'search', owner, value: event.value };
  if (event.kind === 'close') return { ...empty, kind: 'close', owner };
  if (event.kind !== 'resize' || !['start', 'update', 'end', 'step'].includes(text(event.phase))) return empty;
  const width = event.startWidth, delta = event.translationX;
  if (typeof width !== 'number' || typeof delta !== 'number' || !Number.isFinite(width) || !Number.isFinite(delta) || width <= 0) return empty;
  return { ...empty, kind: 'resize', owner, phase: text(event.phase),
    width: adaptiveInspectorResize(width, delta, finite(contentPaneWidth)) };
}
