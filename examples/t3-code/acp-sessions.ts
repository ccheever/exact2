// ACP agent management on a provider's card, T3 Code 1e2ecbd975 (MIT, see LICENSE-T3):
// apps/web/src/components/settings/AcpSessionManagementSection.tsx:41-569 (Native sessions: list,
// Load more, Import, Delete, Log out; Agent providers: list, protocol, base URL, write-only
// headers, Save, Disable) and ProviderSettingsPanel.tsx:639-664 (acceptUrlAuthentication, the
// "Continue authentication" link of ProviderInstanceCard.tsx:994-1011).
// Changes: the section's React state lives here per connection and instance, and each button
// is an `upkeep:acp-*` op; the field drafts arrive as ops too (one per change). The project
// picker and each row read their pending flags from here, as the reference reads its state.
// The destructive confirms are the app's AppConfirm (app-settings.contract), which sends the
// delete or disable op only on Confirm. The link opens through the native `remoteEditorsOpen`
// op (the clone's ElectronShell.openExternal; an agent run records the URL).
import { arr, obj, str, type Obj } from './domain';
import { bridgeReply, ClientError, type Native } from './protocol';
import { letGo } from './let-go';

export type AcpToast = { kind: 'success' | 'error' | 'warning'; title: string; description?: string };
export interface AcpHost {
  config: Obj; writable: boolean; shell?: { projects: Obj[] };
  rpc(native: Native, method: string, payload: Obj, write?: boolean): Promise<Obj>;
}
type Draft = { apiType: string; baseUrl: string; headers: string };
type State = {
  projectId: string | null; sessions: Obj[]; nextCursor: string | null; loading: boolean; importingSessionId: string | null; deletingSessionId: string | null;
  loggingOut: boolean; providers: Obj[]; loadingProviders: boolean; savingProviderId: string | null; drafts: Record<string, Draft>; loads: number;
};
const states = new WeakMap<object, Map<string, State>>();
function stateOf(host: object, instanceId: string, projects: Obj[]): State {
  let byInstance = states.get(host);
  if (!byInstance) states.set(host, byInstance = new Map());
  let state = byInstance.get(instanceId);
  if (!state) byInstance.set(instanceId, state = { projectId: str(projects[0]?.id) || null, sessions: [], nextCursor: null, loading: false, importingSessionId: null,
    deletingSessionId: null, loggingOut: false, providers: [], loadingProviders: false, savingProviderId: null, drafts: {}, loads: 0 });
  // A project that left the environment is no longer offered.
  if (state.projectId !== null && !projects.some(project => str(project.id) === state!.projectId)) state.projectId = str(projects[0]?.id) || null;
  if (state.projectId === null && projects.length) state.projectId = str(projects[0]!.id);
  return state;
}
/** The environment's projects, as the reference's useProjects() filtered to it. */
export const acpProjects = (host: AcpHost): Obj[] => arr(host.shell?.projects).map(project => ({ id: str(project.id), title: str(project.title), workspaceRoot: str(project.workspaceRoot) }));
const draftFor = (provider: Obj): Draft => ({ apiType: str(obj(provider.current).apiType) || str(arr(provider.supported)[0] as unknown) || '', baseUrl: str(obj(provider.current).baseUrl), headers: '' });
const supportedOf = (provider: Obj): string[] => (Array.isArray(provider.supported) ? provider.supported : []).filter((value): value is string => typeof value === 'string');

/** The section's capabilities (canList, canImport, canLogout, canDelete, canConfigureProviders). */
export function acpCapabilities(provider: Obj | undefined) {
  const native = obj(provider?.nativeSessions);
  return { canList: native.canList === true, canImport: native.canLoad === true || native.canResume === true,
    canLogout: obj(provider?.auth).canLogout === true && obj(provider?.setup).canAuthenticate !== true, canDelete: native.canDelete === true,
    canConfigureProviders: provider?.configurableProviders === true };
}

/** AcpSessionManagementSection's view, or none (not an ACP agent, or nothing to manage). */
export function acpSectionView(host: AcpHost, instanceId: string, provider: Obj | undefined) {
  if (!provider || provider.driver !== 'acpRegistry') return [];
  const can = acpCapabilities(provider);
  if (!can.canList && !can.canLogout && !can.canConfigureProviders) return [];
  const projects = acpProjects(host), state = stateOf(host, instanceId, projects), readOnly = !host.writable;
  const projectOperationPending = state.loading || state.importingSessionId !== null || state.deletingSessionId !== null || state.loadingProviders || state.savingProviderId !== null;
  const projectTitle = str(projects.find(project => project.id === state.projectId)?.title);
  const pickerOptions = projects.map(project => ({ value: str(project.id), label: str(project.title), note: '', selected: project.id === state.projectId }));
  return [{
    key: instanceId, instanceId, readOnly,
    canList: can.canList, canLogout: can.canLogout, canConfigure: can.canConfigureProviders,
    status: can.canList && projects.length === 0 ? 'Add a project before importing sessions.' : '',
    showPicker: can.canList && projects.length > 0, projects: pickerOptions, projectTitle, pickerDisabled: readOnly || projectOperationPending,
    listLabel: state.loading ? 'Loading' : state.sessions.length === 0 ? 'List sessions' : 'Refresh', listDisabled: readOnly || state.loading || state.projectId === null,
    logoutLabel: state.loggingOut ? 'Logging out' : 'Log out', logoutDisabled: readOnly || state.loggingOut,
    showList: can.canList && (state.sessions.length > 0 || state.nextCursor !== null),
    sessions: can.canList ? state.sessions.map((session, index) => {
      const sessionId = str(session.sessionId), imported = session.importedThreadId !== null && session.importedThreadId !== undefined;
      return { key: sessionId, first: index === 0, sessionId, title: str(session.title) || sessionId,
        importLabel: imported ? 'Imported' : state.importingSessionId === sessionId ? 'Importing' : 'Import',
        importDisabled: readOnly || !can.canImport || imported || state.importingSessionId !== null,
        canDelete: can.canDelete, deleteLabel: state.deletingSessionId === sessionId ? 'Deleting' : 'Delete', deleteDisabled: readOnly || imported || state.deletingSessionId !== null,
        confirmKey: `${instanceId}|session|${sessionId}`, deleteTitle: `Permanently delete native ACP session "${str(session.title) || sessionId}"?` };
    }) : [],
    showMore: can.canList && state.nextCursor !== null, moreLabel: state.loading ? 'Loading' : 'Load more', moreDisabled: readOnly || state.loading,
    providersLabel: state.loadingProviders ? 'Loading' : state.providers.length === 0 ? 'List providers' : 'Refresh',
    providersDisabled: readOnly || state.loadingProviders || state.projectId === null,
    showProviderPicker: !can.canList && projects.length > 1,
    providers: state.providers.map((item, index) => {
      const providerId = str(item.providerId), draft = state.drafts[providerId] ?? draftFor(item), required = item.required === true, current = item.current !== null && item.current !== undefined;
      const busy = readOnly || state.savingProviderId !== null;
      return { key: `${providerId}:${state.loads}`, first: index === 0, providerId, status: `${current ? 'Configured' : 'Disabled'}${required ? ' · Required' : ''}`,
        saveLabel: state.savingProviderId === providerId ? 'Saving' : 'Save', saveDisabled: busy || draft.apiType.length === 0 || draft.baseUrl.length === 0,
        showDisable: !required && current, disableDisabled: busy, fieldsDisabled: busy,
        apiType: draft.apiType, protocols: supportedOf(item).map(protocol => ({ value: protocol, label: protocol, note: '', selected: protocol === draft.apiType })),
        baseUrl: draft.baseUrl, headers: draft.headers,
        confirmKey: `${instanceId}|provider|${providerId}`, disableTitle: `Disable ACP provider "${providerId}"?` };
    }),
  }];
}

/** The "Continue authentication" block (ProviderInstanceCard.tsx:994-1011); hidden for a read-only session (onAcceptUrlAuth is undefined there). */
export function urlAuthView(host: AcpHost, provider: Obj | undefined) {
  const action = obj(obj(provider?.auth).action);
  if (!host.writable || !str(action.elicitationId) || !str(action.url)) return [];
  return [{ key: str(action.elicitationId), elicitationId: str(action.elicitationId), url: str(action.url), message: str(action.message) }];
}

const failureMessage = (error: unknown, fallback: string) => error instanceof Error && error.message ? error.message : fallback;

/**
 * One ACP op (`upkeep:acp-<what>`) on instance `id`; `field` is the session or provider id (or
 * the elicitation id for `url-auth`), `value` the new project, protocol, text or URL. Toasts go
 * through `toast`.
 */
export async function acpOp(host: AcpHost, native: Native, what: string, id: string, field: string, value: string, toast: (toast: AcpToast) => void): Promise<void> {
  const provider = arr(host.config.providers).find(candidate => candidate.instanceId === id);
  const projects = acpProjects(host), state = stateOf(host, id, projects), projectId = state.projectId;
  const reportFailure = (title: string, error: unknown) => { if (letGo(error)) return; toast({ kind: 'error', title, description: failureMessage(error, 'The ACP operation failed.') }); };
  const call = (method: string, payload: Obj) => host.rpc(native, method, payload, true);
  const readOnly = !host.writable;
  switch (what) {
    case 'project': {
      if (readOnly || state.loading || state.importingSessionId !== null || state.deletingSessionId !== null || state.loadingProviders || state.savingProviderId !== null) return;
      if (!projects.some(project => project.id === value)) return;
      state.projectId = value; state.sessions = []; state.nextCursor = null; state.providers = []; state.drafts = {};
      return;
    }
    case 'list': case 'more': {
      if (readOnly || projectId === null || state.loading) return;
      const cursor = what === 'more' ? state.nextCursor : null;
      if (what === 'more' && cursor === null) return;
      state.loading = true;
      try {
        const result = await call('server.listAcpRegistrySessions', { instanceId: id, projectId, ...(cursor === null ? {} : { cursor }) });
        state.sessions = cursor === null ? arr(result.sessions) : [...state.sessions, ...arr(result.sessions)];
        state.nextCursor = typeof result.nextCursor === 'string' ? result.nextCursor : null;
      } catch (error) { reportFailure('Could not list ACP sessions', error); }
      finally { state.loading = false; }
      return;
    }
    case 'import': {
      const session = state.sessions.find(candidate => candidate.sessionId === field);
      if (readOnly || projectId === null || state.importingSessionId !== null || !session || (session.importedThreadId !== null && session.importedThreadId !== undefined)) return;
      state.importingSessionId = field;
      try {
        const result = await call('server.importAcpRegistrySession', { instanceId: id, projectId, sessionId: field, title: session.title ?? null, updatedAt: session.updatedAt ?? null });
        state.sessions = state.sessions.map(candidate => candidate.sessionId === field ? { ...candidate, importedThreadId: str(result.threadId) } : candidate);
        toast({ kind: 'success', title: result.imported === true ? 'ACP session imported' : 'ACP session already imported' });
      } catch (error) { reportFailure('Could not import ACP session', error); }
      finally { state.importingSessionId = null; }
      return;
    }
    case 'delete': {
      const session = state.sessions.find(candidate => candidate.sessionId === field);
      if (readOnly || projectId === null || state.deletingSessionId !== null || !session || (session.importedThreadId !== null && session.importedThreadId !== undefined)) return;
      state.deletingSessionId = field;
      try {
        await call('server.deleteAcpRegistrySession', { instanceId: id, projectId, sessionId: field });
        state.sessions = state.sessions.filter(candidate => candidate.sessionId !== field);
        toast({ kind: 'success', title: 'ACP session deleted' });
      } catch (error) { reportFailure('Could not delete ACP session', error); }
      finally { state.deletingSessionId = null; }
      return;
    }
    case 'providers': return loadProviders(host, native, id, state, reportFailure);
    case 'draft-api': case 'draft-url': case 'draft-headers': {
      const listed = state.providers.find(candidate => candidate.providerId === field);
      if (!listed || state.savingProviderId !== null) return;
      const draft = state.drafts[field] ?? draftFor(listed);
      state.drafts = { ...state.drafts, [field]: { ...draft, ...(what === 'draft-api' ? { apiType: value } : what === 'draft-url' ? { baseUrl: value } : { headers: value }) } };
      return;
    }
    case 'save': {
      const listed = state.providers.find(candidate => candidate.providerId === field);
      if (readOnly || projectId === null || state.savingProviderId !== null || !listed) return;
      const draft = state.drafts[field] ?? draftFor(listed);
      if (draft.apiType.length === 0 || draft.baseUrl.length === 0) return;
      let headers: Record<string, string> | undefined;
      if (draft.headers.trim().length > 0) {
        try {
          const parsed: unknown = JSON.parse(draft.headers);
          if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed) || !Object.values(parsed).every(entry => typeof entry === 'string')) {
            throw new Error('Headers must be a JSON object with string values.');
          }
          headers = parsed as Record<string, string>;
        } catch (error) { toast({ kind: 'error', title: 'Invalid provider headers', description: error instanceof Error ? error.message : 'Headers must be valid JSON.' }); return; }
      }
      state.savingProviderId = field;
      let saved = false;
      try {
        await call('server.setAcpRegistryProvider', { instanceId: id, projectId, providerId: field, apiType: draft.apiType, baseUrl: draft.baseUrl, ...(headers === undefined ? {} : { headers }) });
        saved = true;
      } catch (error) { reportFailure('Could not configure ACP provider', error); }
      finally { state.savingProviderId = null; }
      if (saved) { toast({ kind: 'success', title: 'ACP provider configured' }); await loadProviders(host, native, id, state, reportFailure); }
      return;
    }
    case 'disable': {
      const listed = state.providers.find(candidate => candidate.providerId === field);
      if (readOnly || projectId === null || state.savingProviderId !== null || !listed || listed.required === true) return;
      state.savingProviderId = field;
      let disabled = false;
      try { await call('server.disableAcpRegistryProvider', { instanceId: id, projectId, providerId: field }); disabled = true; }
      catch (error) { reportFailure('Could not disable ACP provider', error); }
      finally { state.savingProviderId = null; }
      if (disabled) { toast({ kind: 'success', title: 'ACP provider disabled' }); await loadProviders(host, native, id, state, reportFailure); }
      return;
    }
    case 'logout': {
      if (readOnly || state.loggingOut || !acpCapabilities(provider).canLogout) return;
      state.loggingOut = true;
      try {
        await call('server.logoutAcpRegistry', { instanceId: id });
        state.sessions = []; state.nextCursor = null;
        toast({ kind: 'success', title: 'Logged out of ACP agent' });
      } catch (error) { reportFailure('Could not log out of ACP agent', error); }
      finally { state.loggingOut = false; }
      return;
    }
    case 'url-auth': {
      // The link opens the provider's page while the consent goes to the owning environment.
      const action = obj(obj(provider?.auth).action);
      if (readOnly || !field || str(action.elicitationId) !== field) return;
      const accepting = call('server.acceptAcpRegistryUrlAuth', { instanceId: id, elicitationId: field }).then(result => ({ result, error: null as unknown }), error => ({ result: null, error }));
      try { await bridgeReply(native, { op: 'remoteEditorsOpen', url: str(action.url) }); } catch (error) { if (letGo(error)) throw error; }
      const { result, error } = await accepting;
      if (result && result.accepted !== true) { toast({ kind: 'warning', title: 'Authentication request expired', description: 'Refresh the provider and start the authentication flow again.' }); return; }
      if (error && !letGo(error)) toast({ kind: 'error', title: 'Could not continue authentication', description: failureMessage(error, 'The authentication request expired.') });
      return;
    }
    default: throw new ClientError(`Unknown action: acp-${what}`);
  }
}

async function loadProviders(host: AcpHost, native: Native, id: string, state: State, reportFailure: (title: string, error: unknown) => void): Promise<void> {
  if (!host.writable || state.projectId === null || state.loadingProviders) return;
  state.loadingProviders = true;
  try {
    const result = await host.rpc(native, 'server.listAcpRegistryProviders', { instanceId: id, projectId: state.projectId }, true);
    state.providers = arr(result.providers);
    state.drafts = Object.fromEntries(state.providers.map(provider => [str(provider.providerId), draftFor(provider)]));
    state.loads += 1;
  } catch (error) { reportFailure('Could not list ACP providers', error); }
  finally { state.loadingProviders = false; }
}
