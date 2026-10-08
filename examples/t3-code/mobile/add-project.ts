// Pinned T3 Code 365aa87982 AddProjectScreen and operations/projects.
// @ref llp/1107.005-composer-and-transcript.decision.md#new-task-ownership
// Root owns reads, mutations and the literal 15-second arrival deadline.
import { mobileClient } from './client';
import { mobileHomeSources } from './home';
import { mobileSessionGrants } from './mobile-grants';
import { arr, obj, str, applyShell, initialShell, type Obj } from './shared/domain';
import { machineKind } from './shared/connections';
import { composerNow } from './shared/composer-controls';
import { fleet } from './shared/settings-b-fleet';
import { bridgeReply, ClientError, type Native } from './shared/protocol';
import { letGo } from './shared/let-go';
import { settingsNative, settingsSources, settingsEndpoint, settingsEndpointCurrent, settingsCall,
  type MobileSettingsSource, type MobileSettingsEndpoint } from './settings-server-source';
import { sourceFromParam, stringParam, platformFromOs, resolveAddProjectEnvironment,
  buildAddProjectRemoteSourceReadiness, sortAddProjectProviderSources, addProjectRemoteSourceLabel,
  addProjectRemoteSourcePathHint, addProjectRemoteSourceProvider, getFilesystemBrowsePath,
  filterFilesystemBrowseEntries, getPinnedBrowseFilter, appendBrowsePathSegment,
  getCloneDestinationBrowsePath, getCloneDestinationPath, getAddProjectInitialQuery,
  normalizePastedCloneUrl, getDefaultCloneUrl, getCloneDirectoryName, resolveAddProjectPath,
  findExistingAddProject, inferProjectTitleFromPath, buildProjectCreateCommand,
  getNewProjectPathPreview, getNewProjectGitHubTarget, getNewProjectGitHubRepository,
  isWindowsPlatform, type AddProjectRemoteSource } from './add-project-path';

export interface AddProjectRoute { id: string; name: string; url: string }
export interface AddProjectNavigation {
  token: string; kind: string; query: string; routeId: string; environmentId: string; projectId: string;
  title: string; cloning: boolean; source: string; remoteUrl: string; repositoryName: string;
}
export interface AddProjectScreen {
  id: string; name: string; title: string; available: boolean; environmentId: string; environmentLabel: string;
  error: string; busy: boolean; uncertain: boolean; loading: boolean; permissionPending: boolean;
  canCreate: boolean; canClone: boolean; canRead: boolean; hasNewProject: boolean;
  input: string; placeholder: string; primaryLabel: string; primaryDisabled: boolean;
  pathPreview: string; repositoryTitle: string; remoteUrl: string; source: string;
  publish: boolean; showPublish: boolean; publishLabel: string; publishSubtitle: string; showMachines: boolean;
  browsing: boolean; browseLoading: boolean; browseError: string; browseDirectory: string; parentPath: string;
  folders: { id: string; name: string; directory: string; separated: boolean }[];
  environments: { id: string; label: string; symbol: string; selected: boolean; disabled: boolean }[];
  sources: { id: string; title: string; hint: string; ready: boolean }[];
  alerts: { id: string; title: string; message: string }[];
}
export interface AddProjectSnapshot {
  owner: string; activeId: string; revision: number; screens: AddProjectScreen[];
  navigation: AddProjectNavigation; waitOperation: string; waitRoute: string; needsPrepare: boolean;
}
interface EnvironmentOption {
  environmentId: string; label: string; platform: string; machine: string; baseDirectory: string | null;
  newProjectsRoot: string | null; connectionState: string; supportsCloneTracking: boolean;
}
interface Arrival { operation: string; environmentId: string; projectId: string; title: string; cloning: boolean; generation: number; key: string; focused: boolean }
interface Draft {
  route: AddProjectRoute; environmentId: string | null; initializedEnvironment: string; endpointKey: string;
  source: AddProjectRemoteSource; input: string; publish: boolean; repositoryName: string; repositoryTitle: string; remoteUrl: string;
  session: Obj | null; discovery: Obj | null; sessionError: string; discoveryError: string;
  error: string; prepareError: string; busy: boolean; uncertain: boolean; preparing: boolean; browsing: boolean; executing: boolean; noticing: boolean;
  readSerial: number; browseSerial: number; actionSerial: number; browseKey: string; browseError: string; entries: Obj[];
  arrival: Arrival | null; navigation: AddProjectNavigation; alerts: { id: string; title: string; message: string }[];
}
interface Controller { owner: string; activeId: string; sources: MobileSettingsSource[]; loaded: boolean; catalogRevision: number; drafts: Map<string, Draft>; serial: number }
const controllers = new WeakMap<object, Controller>();
const blankNavigation = (): AddProjectNavigation => ({ token: '', kind: '', query: '', routeId: '', environmentId: '', projectId: '', title: '', cloning: false, source: '', remoteUrl: '', repositoryName: '' });
const arrivalMessage = 'The project was created but has not reached this device yet. It will appear in the project list once the connection catches up.';
const message = (error: unknown) => error instanceof Error ? error.message : 'An error occurred.';
const routeNames = ['addProject', 'addProjectRepository', 'addProjectDestination', 'addProjectLocal', 'addProjectNew'];
const symbol: Record<string, string> = { server: 'server.rack', cloud: 'cloud', linux: 'terminal', desktop: 'desktopcomputer', laptop: 'laptopcomputer', 'mac-mini': 'macmini', 'mac-studio': 'macstudio' };
function controller(): Controller {
  let value = controllers.get(mobileClient);
  if (!value) { value = { owner: '', activeId: '', sources: [], loaded: false, catalogRevision: -1, drafts: new Map(), serial: 0 }; controllers.set(mobileClient, value); }
  return value;
}
function draft(route: AddProjectRoute): Draft {
  const at = route.url.indexOf('?'), query = new URLSearchParams(at < 0 ? '' : route.url.slice(at + 1));
  const param = (key: string) => stringParam(query.get(key) ?? undefined);
  return { route, environmentId: param('environmentId'), initializedEnvironment: '', endpointKey: '', source: sourceFromParam(param('source') ?? undefined),
    input: '', publish: false, repositoryName: param('repositoryName')?.trim() ?? '', repositoryTitle: param('repositoryTitle') ?? '', remoteUrl: param('remoteUrl') ?? '',
    session: null, discovery: null, sessionError: '', discoveryError: '', error: '', prepareError: '', busy: false, uncertain: false, preparing: false, browsing: false, executing: false, noticing: false,
    readSerial: 0, browseSerial: 0, actionSerial: 0, browseKey: '', browseError: '', entries: [], arrival: null, navigation: blankNavigation(), alerts: [] };
}
function options(state: Controller, value: Draft): EnvironmentOption[] {
  return state.sources.filter(source => source.enabled && source.phase === 'connected' && (source.focused
    ? mobileClient.environmentId === source.environmentId && mobileClient.connection === 'connected'
    : fleet.entries.get(source.key)?.phase === 'connected')).map(source => ({
    environmentId: source.environmentId, label: source.label, machine: machineKind(source.config),
    platform: platformFromOs(str(obj(obj(source.config.environment).platform).os)),
    baseDirectory: typeof obj(source.config.settings).addProjectBaseDirectory === 'string' ? str(obj(source.config.settings).addProjectBaseDirectory) : null,
    newProjectsRoot: typeof source.config.newProjectsRoot === 'string' ? source.config.newProjectsRoot : null,
    connectionState: source.phase, supportsCloneTracking: obj(obj(source.config.environment).capabilities).projectCloneTracking === true,
  })).filter(option => value.route.name !== 'addProjectNew' || option.newProjectsRoot !== null)
    .sort((a, b) => a.label < b.label ? -1 : a.label > b.label ? 1 : 0);
}
function selected(state: Controller, value: Draft): EnvironmentOption | null {
  const available = options(state, value);
  return resolveAddProjectEnvironment(available, value.environmentId)
    ?? (value.route.name === 'addProject' ? available[0] ?? null : null);
}
function initialize(state: Controller, value: Draft) {
  const environment = selected(state, value);
  if (!environment || environment.environmentId === value.initializedEnvironment) return;
  value.initializedEnvironment = environment.environmentId;
  value.endpointKey = ''; value.session = null; value.discovery = null; value.sessionError = ''; value.discoveryError = '';
  value.browseKey = ''; value.entries = []; value.browseError = ''; value.browseSerial++; value.browsing = false;
  if (['addProjectLocal', 'addProjectDestination'].includes(value.route.name))
    value.input = getCloneDestinationPath(getAddProjectInitialQuery(environment.baseDirectory), value.repositoryName);
}
function requireDraft(owner: string, id: string): { state: Controller; value: Draft } {
  const state = controller(), value = state.drafts.get(id);
  if (!owner || state.owner !== owner || state.activeId !== id || !value) throw new ClientError('The Add Project screen changed.', 'superseded');
  return { state, value };
}
function current(state: Controller, value: Draft, owner: string) {
  return controller() === state && state.owner === owner && state.activeId === value.route.id && state.drafts.get(value.route.id) === value;
}
function ownedNative(native: Native, check: () => void): Native {
  const base = settingsNative(native);
  return { available: base.available, watch: topic => { check(); base.watch(topic); }, later: async request => { check(); const reply = await base.later(request); check(); return reply; } };
}
function touch(state: Controller) {
  const catalogWasCurrent = state.catalogRevision === mobileClient.revision;
  mobileClient.revision++;
  if (catalogWasCurrent) state.catalogRevision = mobileClient.revision;
}
function operation(state: Controller, value: Draft) { return `${state.owner}:${value.route.id}:${++state.serial}`; }
function alert(state: Controller, value: Draft, title: string, text: string) { value.alerts.push({ id: operation(state, value), title, message: text }); }
function navigate(state: Controller, value: Draft, fields: Partial<AddProjectNavigation>) {
  const navigation = { ...blankNavigation(), routeId: value.route.id, token: operation(state, value), ...fields };
  const query = new URLSearchParams();
  if (['new', 'local', 'repository', 'destination', 'draft'].includes(navigation.kind)) query.set('environmentId', navigation.environmentId);
  if (['repository', 'destination'].includes(navigation.kind)) query.set('source', navigation.source);
  if (navigation.kind === 'destination') { query.set('remoteUrl', navigation.remoteUrl); query.set('repositoryTitle', navigation.title); query.set('repositoryName', navigation.repositoryName); }
  if (navigation.kind === 'draft') { query.set('projectId', navigation.projectId); query.set('title', navigation.title); if (navigation.cloning) query.set('cloning', '1'); }
  navigation.query = query.size ? `?${query}` : ''; value.navigation = navigation;
}
function shellProject(environmentId: string, projectId: string) {
  return mobileHomeSources().find(source => source.environmentId === environmentId)?.shell.projects.find(project => project.id === projectId && project.archivedAt == null);
}
function inspectArrival(state: Controller, value: Draft) {
  const arrival = value.arrival;
  if (!arrival || value.uncertain || value.executing) return;
  const valid = arrival.focused ? mobileClient.environmentId === arrival.environmentId && mobileClient.generation === arrival.generation && mobileClient.connection === 'connected'
    : fleet.entries.get(arrival.key)?.generation === arrival.generation && fleet.entries.get(arrival.key)?.phase === 'connected';
  if (!valid) { value.arrival = null; value.busy = false; value.error = arrivalMessage; if (value.route.name === 'addProjectNew') value.input = ''; return; }
  if (!shellProject(arrival.environmentId, arrival.projectId)) return;
  value.arrival = null; value.busy = false;
  navigate(state, value, { kind: 'draft', environmentId: arrival.environmentId, projectId: arrival.projectId, title: arrival.title, cloning: arrival.cloning });
}

/** Call from the root projection with every retained Add Project route and the current top route. */
export function mobileAddProjectObserve(owner: string, routes: readonly AddProjectRoute[], activeId: string): AddProjectSnapshot {
  const state = controller();
  if (state.owner !== owner) { state.owner = owner; state.drafts.clear(); state.sources = []; state.loaded = false; state.catalogRevision = -1; }
  if (state.activeId !== activeId) {
    const previous = state.drafts.get(state.activeId);
    if (previous) { previous.readSerial++; previous.browseSerial++; previous.actionSerial++; previous.preparing = false; previous.browsing = false; }
  }
  state.activeId = activeId;
  const retained = new Set(routes.filter(route => routeNames.includes(route.name)).map(route => route.id));
  for (const id of state.drafts.keys()) if (!retained.has(id)) state.drafts.delete(id);
  for (const route of routes) if (retained.has(route.id)) {
    let value = state.drafts.get(route.id);
    if (!value || value.route.url !== route.url) { value = draft(route); state.drafts.set(route.id, value); }
    initialize(state, value); inspectArrival(state, value);
  }
  return mobileAddProjectSnapshot();
}
function screen(state: Controller, value: Draft): AddProjectScreen {
  const environment = selected(state, value), available = options(state, value), name = value.route.name;
  const canCreate = !!environment && mobileSessionGrants(value.session, 'orchestration:operate');
  const canClone = canCreate && mobileSessionGrants(value.session, 'source-control:write');
  const canRead = !!environment && mobileSessionGrants(value.session, 'filesystem:read');
  const readiness = buildAddProjectRemoteSourceReadiness(value.discovery), github = getNewProjectGitHubTarget(value.discovery);
  const browse = getFilesystemBrowsePath(value.input, environment?.platform ?? '');
  const filter = getPinnedBrowseFilter(browse.filterQuery, value.repositoryName, environment?.platform ?? '');
  const entries = filterFilesystemBrowseEntries(value.entries.map(entry => ({ fullPath: str(entry.fullPath), name: str(entry.name) })), filter).visibleEntries;
  const permissionPending = !!environment && !value.session && !value.sessionError;
  const showMachines = available.length > 1 || (!environment && available.length > 0);
  const primaryLabel = name === 'addProjectNew' ? 'Create project' : name === 'addProjectLocal' ? 'Add project'
    : name === 'addProjectDestination' ? 'Clone project' : value.source === 'url' ? 'Continue' : 'Lookup repository';
  const title = name === 'addProjectNew' ? 'New project' : name === 'addProjectLocal' ? 'Local folder'
    : name === 'addProjectDestination' ? 'Clone destination' : name === 'addProjectRepository' ? addProjectRemoteSourceLabel(value.source) : 'Add project';
  return { id: value.route.id, name, title, available: !!environment, environmentId: environment?.environmentId ?? '', environmentLabel: environment?.label ?? '',
    error: value.error || value.prepareError, busy: value.busy, uncertain: value.uncertain, loading: value.preparing, permissionPending,
    canCreate, canClone, canRead, hasNewProject: environment?.newProjectsRoot != null,
    input: value.input, placeholder: name === 'addProjectNew' ? 'Project name' : name === 'addProjectRepository'
      ? value.source === 'url' ? 'https://github.com/org/repo.git' : addProjectRemoteSourcePathHint(value.source) : '~/projects/my-app',
    primaryLabel, primaryDisabled: !environment || value.busy || value.uncertain || value.browsing || !value.input.trim()
      || (name === 'addProjectLocal' || name === 'addProjectNew' ? !canCreate : name === 'addProjectDestination' ? !canClone || !value.remoteUrl : !canClone),
    pathPreview: environment?.newProjectsRoot == null ? '' : `${value.input.trim() ? `Creates ${getNewProjectPathPreview(environment.newProjectsRoot, value.input.trim())}` : `Goes in ${environment.newProjectsRoot}`}${showMachines ? ` on ${environment.label}` : ''}`,
    repositoryTitle: value.repositoryTitle, remoteUrl: value.remoteUrl, source: value.source, publish: value.publish,
    showPublish: github !== null, publishLabel: 'Create private repository on GitHub',
    publishSubtitle: github ? value.input.trim() && environment?.newProjectsRoot != null
      ? getNewProjectGitHubRepository(github, getNewProjectPathPreview(environment.newProjectsRoot, value.input.trim())) : github.account ?? '' : '', showMachines,
    browsing: value.browsing, browseLoading: permissionPending || value.preparing && !value.entries.length,
    browseError: value.browseError || value.sessionError || (!canRead && !permissionPending && environment ? 'This connection cannot browse host folders.' : ''),
    browseDirectory: browse.directoryPath, parentPath: browse.canBrowseUp ? browse.parentPath ?? '' : '',
    folders: canRead ? entries.map((entry, index) => ({ id: str(entry.fullPath) || `${browse.directoryPath}${entry.name}`, name: entry.name,
      directory: browse.directoryPath, separated: index > 0 || browse.canBrowseUp })) : [],
    environments: available.map(option => ({ id: option.environmentId, label: option.label, symbol: symbol[option.machine] ?? 'server.rack',
      selected: option.environmentId === environment?.environmentId, disabled: value.busy })),
    sources: (['url', ...sortAddProjectProviderSources(readiness)] as AddProjectRemoteSource[]).map(source => ({ id: source, title: source === 'url' ? 'Git URL' : `${addProjectRemoteSourceLabel(source)} repository`,
      hint: !canClone ? 'This connection cannot clone projects.' : readiness[source].ready ? source === 'url' ? 'Clone from a remote URL'
        : `Clone ${addProjectRemoteSourceLabel(source)} ${addProjectRemoteSourcePathHint(source)}` : readiness[source].hint ?? '',
      ready: canClone && readiness[source].ready })), alerts: value.alerts.map(item => ({ ...item })) };
}
export function mobileAddProjectSnapshot(): AddProjectSnapshot {
  const state = controller(), value = state.drafts.get(state.activeId);
  if (value) inspectArrival(state, value);
  const environment = value ? selected(state, value) : null;
  const source = state.sources.find(source => source.environmentId === environment?.environmentId);
  const generation = source?.focused ? mobileClient.generation : fleet.entries.get(source?.key ?? '')?.generation ?? -1;
  const key = source ? `${source.key}:${generation}` : '';
  const browse = value && ['addProjectLocal', 'addProjectDestination'].includes(value.route.name) ? getFilesystemBrowsePath(value.input, environment?.platform ?? '').directoryPath : '';
  return { owner: state.owner, activeId: state.activeId, revision: mobileClient.revision,
    screens: [...state.drafts.values()].map(value => screen(state, value)),
    navigation: value && !value.noticing && !value.alerts.length ? value.navigation : blankNavigation(), waitOperation: value?.arrival?.operation ?? '', waitRoute: value?.arrival ? value.route.id : '',
    needsPrepare: !!value && !value.preparing && !value.busy && !value.browsing && (!state.loaded || state.catalogRevision !== mobileClient.revision || value.endpointKey !== key
      || !!key && !value.session && !value.sessionError || !!browse && !!value.session && mobileSessionGrants(value.session, 'filesystem:read') && value.browseKey !== `${key}:${browse}`) };
}

async function endpointFor(state: Controller, value: Draft, native: Native): Promise<MobileSettingsEndpoint> {
  state.sources = await settingsSources(native); state.loaded = true; initialize(state, value);
  const environment = selected(state, value), matches = state.sources.filter(source => source.environmentId === environment?.environmentId);
  if (!environment || matches.length !== 1) { value.endpointKey = ''; value.session = null; throw new ClientError('This environment is no longer connected.'); }
  const endpoint = settingsEndpoint(matches[0]!, native);
  if (!settingsEndpointCurrent(endpoint)) throw new ClientError('This environment is no longer connected.');
  return endpoint;
}
async function grant(endpoint: MobileSettingsEndpoint, value: Draft, permissions: string[]) {
  value.session = null;
  const session = await settingsCall(endpoint, { op: 'http', path: '/api/auth/session' }); value.session = session; value.sessionError = '';
  if (permissions.some(permission => !mobileSessionGrants(session, permission))) throw new ClientError(permissions.includes('source-control:write')
    ? 'This connection cannot clone projects.' : permissions.includes('filesystem:read') ? 'This connection cannot browse host folders.' : 'This connection cannot add projects.');
}
export async function mobileAddProjectPrepare(owner: string, id: string, nativeInput?: Native | null): Promise<AddProjectSnapshot> {
  const { state, value } = requireDraft(owner, id), serial = ++value.readSerial;
  const check = () => { if (!current(state, value, owner) || serial !== value.readSerial) throw new ClientError('The Add Project read changed.', 'superseded'); };
  value.preparing = true; value.prepareError = '';
  try {
    if (!nativeInput?.available) throw new ClientError('Open T3 Code on your iPhone or iPad to add projects.');
    const native = ownedNative(nativeInput, check), endpoint = await endpointFor(state, value, native);
    const key = `${endpoint.source.key}:${endpoint.generation}`;
    if (value.endpointKey !== key) { value.endpointKey = key; value.session = null; value.discovery = null; value.sessionError = ''; value.discoveryError = ''; value.browseKey = ''; value.entries = []; }
    try { await grant(endpoint, value, []); }
    catch (error) { if (letGo(error)) throw error; value.sessionError = message(error); }
    if (['addProject', 'addProjectNew'].includes(value.route.name) && value.discovery === null && !value.discoveryError) {
      try { value.discovery = await settingsCall(endpoint, { op: 'request', method: 'server.discoverSourceControl', payload: {} }); }
      catch (error) { if (letGo(error)) throw error; value.discoveryError = message(error); }
    }
    if (['addProjectLocal', 'addProjectDestination'].includes(value.route.name)) {
      const directory = getFilesystemBrowsePath(value.input, selected(state, value)?.platform ?? '').directoryPath;
      if (directory && mobileSessionGrants(value.session, 'filesystem:read') && value.browseKey !== `${key}:${directory}`) {
        const browseSerial = value.browseSerial;
        try {
          const reply = await settingsCall(endpoint, { op: 'request', method: 'filesystem.browse', payload: { partialPath: directory } });
          if (browseSerial === value.browseSerial) { value.entries = arr(reply.entries); value.browseError = ''; value.browseKey = `${key}:${directory}`; }
        } catch (error) { if (letGo(error)) throw error; if (browseSerial === value.browseSerial) { value.browseError = message(error); value.browseKey = `${key}:${directory}`; } }
      }
    }
  } catch (error) { if (letGo(error)) throw error; if (current(state, value, owner)) value.prepareError = state.loaded && !selected(state, value) ? '' : message(error); }
  finally { if (current(state, value, owner) && value.readSerial === serial) { value.preparing = false; touch(state); state.catalogRevision = mobileClient.revision; } }
  return mobileAddProjectSnapshot();
}
export function mobileAddProjectEdit(owner: string, id: string, field: string, input: string): AddProjectSnapshot {
  const { state, value } = requireDraft(owner, id);
  if (field === 'alert-dismiss') value.alerts = value.alerts.filter(alert => alert.id !== input);
  else if (field === 'navigation-consumed' && value.navigation.token === input) value.navigation = blankNavigation();
  else if (!value.busy) {
    if (field === 'input') { value.input = input; value.browseSerial++; value.browsing = false; value.error = ''; }
    else if (field === 'publish') value.publish = input === 'true';
    else if (field === 'environment' && ['addProject', 'addProjectNew'].includes(value.route.name)) {
      if (options(state, value).some(option => option.environmentId === input)) { value.environmentId = input; value.readSerial++; value.preparing = false; initialize(state, value); }
    } else if (field === 'refresh') { value.endpointKey = ''; value.browseKey = ''; value.discoveryError = ''; value.error = ''; }
  }
  touch(state); return mobileAddProjectSnapshot();
}
export async function mobileAddProjectBrowse(owner: string, id: string, directory: string, selectedName: string, nativeInput?: Native | null): Promise<AddProjectSnapshot> {
  const { state, value } = requireDraft(owner, id);
  if (value.busy || !['addProjectLocal', 'addProjectDestination'].includes(value.route.name)) return mobileAddProjectSnapshot();
  value.readSerial++; value.preparing = false;
  const serial = ++value.browseSerial, environmentId = selected(state, value)?.environmentId;
  const check = () => { if (!current(state, value, owner) || serial !== value.browseSerial || selected(state, value)?.environmentId !== environmentId) throw new ClientError('The folder selection changed.', 'superseded'); };
  value.browsing = true; value.browseError = '';
  try {
    if (!nativeInput?.available) throw new ClientError('This connection cannot browse host folders.');
    const native = ownedNative(nativeInput, check), endpoint = await endpointFor(state, value, native);
    await grant(endpoint, value, ['filesystem:read']);
    const path = selectedName ? appendBrowsePathSegment(directory, selectedName) : directory;
    const next = value.repositoryName && selectedName ? getCloneDestinationBrowsePath({ browseDirectoryPath: directory,
      selectedDirectoryName: selectedName, cloneDirectoryName: value.repositoryName, caseSensitive: !isWindowsPlatform(selected(state, value)?.platform ?? '') }) : getCloneDestinationPath(path, value.repositoryName);
    const reply = await settingsCall(endpoint, { op: 'request', method: 'filesystem.browse', payload: { partialPath: path } });
    check(); value.input = next; value.entries = arr(reply.entries); value.browseKey = `${endpoint.source.key}:${endpoint.generation}:${getFilesystemBrowsePath(next, selected(state, value)?.platform ?? '').directoryPath}`;
  } catch (error) { if (letGo(error)) throw error; if (current(state, value, owner) && serial === value.browseSerial) value.browseError = message(error); }
  finally { if (current(state, value, owner) && serial === value.browseSerial) { value.browsing = false; touch(state); } }
  return mobileAddProjectSnapshot();
}

async function refreshShell(endpoint: MobileSettingsEndpoint) {
  const shell = applyShell(initialShell(), await settingsCall(endpoint, { op: 'http', path: '/api/orchestration/shell' }));
  if (endpoint.source.focused) { mobileClient.shell = shell; mobileClient.shellLoaded = true; }
  else { const entry = fleet.entries.get(endpoint.source.key); if (entry) { entry.shell = shell; entry.synchronized = endpoint.generation; fleet.revision++; } }
}
function awaitProject(state: Controller, value: Draft, endpoint: MobileSettingsEndpoint, projectId: string, title: string, cloning: boolean) {
  value.arrival = { operation: operation(state, value), environmentId: endpoint.source.environmentId, projectId, title, cloning,
    generation: endpoint.generation, key: endpoint.source.key, focused: endpoint.source.focused };
}
async function createLocal(state: Controller, value: Draft, endpoint: MobileSettingsEndpoint, workspaceRoot: string, dispatch: (method: string, payload: Obj) => Promise<Obj>) {
  await grant(endpoint, value, ['orchestration:operate']);
  const projects = mobileHomeSources().flatMap(source => source.shell.projects.map(project => ({ id: str(project.id), title: str(project.title), environmentId: source.environmentId, workspaceRoot: str(project.workspaceRoot) })));
  const existing = findExistingAddProject({ projects, environmentId: endpoint.source.environmentId, path: workspaceRoot });
  if (existing) { alert(state, value, 'Project already exists', str(existing.title)); navigate(state, value, { kind: 'draft', environmentId: endpoint.source.environmentId,
    projectId: str(existing.id), title: str(existing.title) }); return; }
  const [commandId, projectId] = await mobileClient.ids(endpoint.remote, 2);
  await dispatch('projects.mutate', buildProjectCreateCommand({ commandId: commandId!, projectId: projectId!, workspaceRoot }));
  awaitProject(state, value, endpoint, projectId!, inferProjectTitleFromPath(workspaceRoot), false);
}

/** A separate root mutation presents one queued notice without cancelling the create answer. */
export async function mobileAddProjectAlert(owner: string, id: string, token: string, nativeInput?: Native | null): Promise<AddProjectSnapshot> {
  const { state, value } = requireDraft(owner, id), notice = value.alerts.find(item => item.id === token);
  if (!notice || value.noticing) return mobileAddProjectSnapshot();
  const check = () => { if (!current(state, value, owner)) throw new ClientError('The Add Project notice changed.', 'superseded'); };
  value.alerts = value.alerts.filter(item => item.id !== token); value.noticing = true;
  try {
    if (!nativeInput?.available) throw new ClientError(notice.message);
    const reply = await bridgeReply(ownedNative(nativeInput, check), { op: 'mobileAlert', kind: 'info', title: notice.title, message: notice.message });
    if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind);
  } catch (error) { if (letGo(error)) throw error; if (current(state, value, owner)) value.error = message(error); }
  finally { if (state.owner === owner && state.drafts.get(id) === value) { value.noticing = false; touch(state); } }
  return mobileAddProjectSnapshot();
}

export function mobileAddProjectDeadline(owner: string, id: string, token: string): AddProjectSnapshot {
  const { state, value } = requireDraft(owner, id); inspectArrival(state, value);
  if (value.arrival?.operation === token) { value.arrival = null; value.busy = false; value.error = arrivalMessage; if (value.route.name === 'addProjectNew') value.input = ''; touch(state); }
  return mobileAddProjectSnapshot();
}
export async function mobileAddProjectAction(owner: string, id: string, kind: string, input: string, nativeInput?: Native | null): Promise<AddProjectSnapshot> {
  if (kind === 'deadline') return mobileAddProjectDeadline(owner, id, input);
  if (kind === 'alert') return mobileAddProjectAlert(owner, id, input, nativeInput);
  const { state, value } = requireDraft(owner, id);
  if (value.busy || value.uncertain || value.browsing || value.navigation.kind === 'draft') return mobileAddProjectSnapshot();
  if (kind === 'connections') { navigate(state, value, { kind: 'connections' }); touch(state); return mobileAddProjectSnapshot(); }
  if (kind === 'submit' && ['addProjectNew', 'addProjectRepository'].includes(value.route.name) && !value.input.trim()) return mobileAddProjectSnapshot();
  const serial = ++value.actionSerial, environmentId = selected(state, value)?.environmentId;
  const check = () => { if (!current(state, value, owner) || serial !== value.actionSerial || selected(state, value)?.environmentId !== environmentId) throw new ClientError('The Add Project action changed.', 'superseded'); };
  value.readSerial++; value.preparing = false;
  value.busy = true; value.executing = true; value.error = '';
  try {
    if (!nativeInput?.available) throw new ClientError('Open T3 Code on your iPhone or iPad to add projects.');
    const native = ownedNative(nativeInput, check), endpoint = await endpointFor(state, value, native);
    const environment = selected(state, value)!;
    const dispatch = async (method: string, payload: Obj, creation = true) => {
      check(); if (!settingsEndpointCurrent(endpoint)) throw new ClientError('The connection changed. Reopen Add Project.', 'stale');
      let refused = false;
      const remote: Native = { available: endpoint.remote.available, watch: topic => endpoint.remote.watch(topic), later: async request => {
        const reply = await endpoint.remote.later(request); refused = obj(reply).ok === false && obj(obj(reply).error).uncertain !== true; return reply;
      } };
      try { const result = await settingsCall({ ...endpoint, remote }, { op: 'request', method, payload }); return result; }
      catch (error) { if (!refused && creation) value.uncertain = true; throw error; }
    };
    if (['new', 'local', 'repository', 'existing'].includes(kind)) {
      if (kind === 'new' && environment.newProjectsRoot === null) throw new ClientError('This environment cannot create projects from a name.');
      const source = sourceFromParam(input);
      await grant(endpoint, value, kind === 'repository' ? ['orchestration:operate', 'source-control:write'] : kind === 'local' ? ['orchestration:operate'] : []);
      if (kind === 'repository' && source !== 'url') {
        const discovery = await settingsCall(endpoint, { op: 'request', method: 'server.discoverSourceControl', payload: {} }); value.discovery = discovery;
        if (!buildAddProjectRemoteSourceReadiness(discovery)[source].ready) throw new ClientError('This source control provider is not ready.');
      }
      navigate(state, value, { kind: kind === 'existing' ? 'source' : kind, environmentId: environment.environmentId, source });
    } else if (kind === 'submit' && value.route.name === 'addProjectRepository') {
      if (!value.input.trim()) return mobileAddProjectSnapshot();
      await grant(endpoint, value, ['orchestration:operate', 'source-control:write']);
      const provider = addProjectRemoteSourceProvider(value.source);
      const repository = provider ? await settingsCall(endpoint, { op: 'request', method: 'sourceControl.lookupRepository', payload: { provider, repository: value.input.trim() } }) : null;
      const remoteUrl = repository ? getDefaultCloneUrl({ provider: str(repository.provider) || provider!, url: str(repository.url), sshUrl: str(repository.sshUrl) }) : normalizePastedCloneUrl(value.input);
      const title = repository ? str(repository.nameWithOwner) : remoteUrl;
      navigate(state, value, { kind: 'destination', environmentId: environment.environmentId, source: value.source, remoteUrl, title, repositoryName: getCloneDirectoryName(title) });
    } else if (kind === 'submit' && value.route.name === 'addProjectNew') {
      const name = value.input.trim(); if (!name || environment.newProjectsRoot === null) return mobileAddProjectSnapshot();
      await grant(endpoint, value, ['orchestration:operate']);
      const created = await dispatch('projects.createNew', { name }), projectId = str(created.projectId), workspaceRoot = str(created.workspaceRoot);
      if (!projectId || !workspaceRoot) { value.uncertain = true; throw new ClientError('The project may have been created. Refresh the project list before trying again.'); }
      awaitProject(state, value, endpoint, projectId, name, false);
      if (typeof created.commitError === 'string') alert(state, value, 'Created without a first commit', created.commitError);
      const target = getNewProjectGitHubTarget(value.discovery);
      if (value.publish && target) {
        try {
          await grant(endpoint, value, ['source-control:write']);
          await dispatch('sourceControl.publishRepository', { cwd: workspaceRoot, provider: 'github', repository: getNewProjectGitHubRepository(target, workspaceRoot), visibility: 'private' }, false);
        } catch (error) { if (letGo(error)) throw error; alert(state, value, 'Could not create the GitHub repository', message(error)); }
      }
    } else if (kind === 'submit' && ['addProjectLocal', 'addProjectDestination'].includes(value.route.name)) {
      const resolved = resolveAddProjectPath({ rawPath: value.input, platform: environment.platform, currentProjectCwd: null });
      if (!resolved.ok) throw new ClientError(resolved.error);
      if (value.route.name === 'addProjectLocal') await createLocal(state, value, endpoint, resolved.path, dispatch);
      else {
        if (!value.remoteUrl) throw new ClientError('Choose a repository to clone.');
        await grant(endpoint, value, ['orchestration:operate', 'source-control:write']);
        if (environment.supportsCloneTracking) {
          const [projectId] = await mobileClient.ids(endpoint.remote, 1), title = inferProjectTitleFromPath(resolved.path);
          await dispatch('projectClone.start', { projectId: projectId!, title, createdAt: new Date(composerNow(mobileClient)).toISOString(), remoteUrl: value.remoteUrl, destinationPath: resolved.path });
          awaitProject(state, value, endpoint, projectId!, title, true);
        } else {
          const cloned = await dispatch('sourceControl.cloneRepository', { remoteUrl: value.remoteUrl, destinationPath: resolved.path });
          if (!str(cloned.cwd)) throw new ClientError('The clone did not return its workspace path.');
          await createLocal(state, value, endpoint, str(cloned.cwd), dispatch);
        }
      }
    } else throw new ClientError('This Add Project action is unavailable.');
    if (value.arrival) {
      try { await refreshShell(endpoint); }
      catch (error) { if (letGo(error)) throw error; /* An acknowledged create waits for the existing shell stream or root deadline. */ }
    }
  } catch (error) {
    if (letGo(error)) throw error;
    if (current(state, value, owner)) value.error = value.uncertain ? `The request may have reached the server. Check the project list before trying again. ${message(error)}` : message(error);
  } finally {
    if (state.owner === owner && state.drafts.get(id) === value) {
      value.executing = false; value.busy = value.arrival !== null && !value.uncertain; touch(state);
    }
  }
  return mobileAddProjectSnapshot();
}
