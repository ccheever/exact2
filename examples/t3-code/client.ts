import { snapshotShortcut } from './snapshot-shortcut';
import { snapshotIdentity, snapshotDefaultProject, snapshotDestinationExists, withoutSnapshot, snapshotNoProjectMessage, snapshotFailureMessage } from './snapshot-adopt';
import { decodeClientPrefs, type ClientPrefs } from './settings-core';
import { decodeCustomThemes, type CustomTheme } from './settings-themes';
import { quitMode } from './palette-native';
import { shellFailure, shellSuccess, settingsFailure } from './shell-commands';
import { trackRpc } from './shell-slow';
import { configEventSideEffects } from './r3-protocol-config';
import { beginRead, traceRpc, statusTicket, settleTraces } from './r3-protocol-reader';
import { compatibilityProblem } from './r3-protocol-outdated';
import { type ComposerControlsPrefs, emptyComposerControls, decodeComposerControls, applySticky, applyStaged } from './composer-controls';
import { adoptStash } from './composer-editor-stash';
import { adoptComposerFiles } from './composer-editor-files';
import { sidebarOpened, sidebarRefreshed } from './sidebar-commands';
import { reconnectOnLaunch, launchFocus } from './r8-pointer-reconnect';
import { adoptSidebarPrefs } from './sidebar-state';
import { PROVIDER_OPS } from './providers';
import { CONNECTION_OPS } from './connections';
import { READ_OPS, WRITE_OPS, runOps, type OpOut } from './client-ops';
import { groupingModes, message, projectPath } from './client-shared';
import { letGo } from './let-go';
import { fleet } from './settings-b-fleet';
import { groupLabel } from './r6-polish-groups';
import { adoptModelPrefs } from './settings-b-models';
import { type RequestDraft } from './requests';
import { DiffState, DIFF_LOCAL_OPS } from './diff';
import { TELEMETRY_KEY, telemetryEvent } from './settings-a-telemetry';
import { mostRecentProjectId } from './pages-home';
import { adoptPagesPrefs } from './pages-prefs';
import { adoptShellPrefs } from './shell-prefs';
import { adoptFilesPrefs } from './r5-panels-prefs';
import { adoptSidebarWidth } from './r4-polish-sidebar-width'; // r4-polish: the stored sidebar width
import { VCS_STATUS_KEY, vcsStatusEvent } from './shell-vcs';
import { DEVICE_STATE_KEY, deviceStateEvent } from './r4-surfaces-device';
import { LIVE_KEYS, liveEvent } from './live-streams';
import { WORKTREE_SETUP_KEY, worktreeSetupEvent } from './timeline-worktree';
import { GIT_ACTION_KEY, gitActionEvent } from './r4-git-actions';
import { TERMINAL_METADATA_KEY, terminalMetadataEvent } from './terminal-drawer-view'; // terminal-drawer
import { adoptTerminalContexts } from './terminal-integrations';
import { providerAuthEvent } from './provider-auth-terminal';
import { adoptTerminalPrefs } from './terminal-ui-state'; // terminal-drawer
import { obj, str, num, arr, initialShell, applyShell, threadSnapshot, applyThread, mergeHistory,
  readyCheckpoint, type Obj, type Shell, type ThreadState } from './domain';
import { ClientError, bridgeReply, providerAvailable, applyConfig, type Native, type Files } from './protocol';

const localPath = 'app:/data/t3-code.json';
type Selection = { projectId: string; threadId: string };
export type Pending = { method: string; payload: Obj; description: string; threadId: string; text: string; uncertain: boolean };
const uncertainError = (pending: Pending) => `${pending.description} may have reached T3. Check the synchronized thread, then retry only if needed.`;
type Preferences = { selections: Record<string, Selection>; drafts: Record<string, string>; snapshotDrafts: Record<string, Obj[]>; snapshotReleases: string[]; sidebarWidth: number; sidebarOpen: boolean; pending: Record<string, Pending>; favoriteModels: string[]; groupingMode: string; lastGroupingMode?: string; clientSettings: ClientPrefs; customThemes: CustomTheme[]; groupingOverrides: Record<string, string>; composerControls: ComposerControlsPrefs; deviceSettings: { composerCollapseOnScroll: boolean; planModeEnabled: boolean; timestampFormat: string; appearanceMode: string; sendShortcut: string; snapShotShortcut: string; snapShotEnabled: boolean; snapShotIncludeAccessibility: boolean; snapShotPlaySound: boolean; snapShotSound: string; snapShotFlash: boolean; snapShotAnimations: boolean } };
const preferences = (): Preferences => ({ selections: {}, drafts: {}, snapshotDrafts: {}, snapshotReleases: [], sidebarWidth: 256, sidebarOpen: true, pending: {}, favoriteModels: [], groupingMode: 'repository', clientSettings: decodeClientPrefs({}), customThemes: [], groupingOverrides: {}, composerControls: emptyComposerControls(), deviceSettings: { composerCollapseOnScroll: true, planModeEnabled: false, timestampFormat: 'locale', appearanceMode: 'system', sendShortcut: 'enter', snapShotShortcut: 'shift+shift', snapShotEnabled: false, snapShotIncludeAccessibility: true, snapShotPlaySound: true, snapShotSound: 'soft-pop', snapShotFlash: true, snapShotAnimations: true } });
const scopeStrings = (value: unknown): string[] => Array.isArray(value) ? value.filter((entry): entry is string => typeof entry === 'string') : [];

const DEFAULT_ORIGIN = 'http://127.0.0.1:3773'; // T3's default server address (README)
export class T3Client {
  available = false;
  revision = 0;
  generation = -1;
  environmentId = '';
  origin = DEFAULT_ORIGIN;
  connection = 'disconnected';
  statusMessage = 'Connect to your T3 Code server.';
  error = '';
  shell: Shell = initialShell();
  thread: ThreadState | null = null;
  config: Obj = {};
  presentation: Obj = {};
  projectId = '';
  threadId = '';
  query = '';
  providerId = '';
  modelId = '';
  modelOptions: Obj[] = [];
  runtimeMode = 'approval-required';
  interactionMode = 'default';
  scopes: string[] = [];
  shellLive = false;
  threadLive = false;
  configLive = false;
  historyLoading = false;
  diffOpen = false;
  diffLoading = false;
  diffError = '';
  diffText = '';
  diffTitle = 'Changes';
  diffState = new DiffState();
  answers: Record<string, RequestDraft> = {};
  local = preferences();
  busy = false;
  private loaded = false;
  shellLoaded = false;
  private synchronizedGeneration = -1;
  private synchronizationEpoch = 0;
  private nextRefreshEpoch = 0;
  private refreshEpoch = 0;
  private commandEpoch = 0;
  private lastEvent = 0;
  threadEpoch = 0;
  threadSubscription = '';
  subscriptions: Record<string, string> = {};
  private needsFreshSnapshot = false;
  /** The refresh adopting captures, or -1. A newer read takes over one Exact let go mid-flight. */
  private snapshotAdopting = -1;
  private snapshotNotices = new Map<string, boolean>();
  /** Streams the transport scheduled to resubscribe on this session (`_retryDue`), and their failure text. */
  private streamRetryDue = new Set<string>();
  private streamError = '';

  get ready(): boolean {
    return this.connection === 'connected' && this.configLive && this.shellLive && (!this.threadId || this.threadLive);
  }
  get writable(): boolean {
    return this.ready && this.scopes.includes('orchestration:operate')
      && obj(obj(this.config.environment).capabilities).serverResolvedCommandContext === true;
  }
  get pending(): Pending | undefined { return this.local.pending[this.environmentId]; }
  get draftKey(): string { return `${this.environmentId}:${this.threadId || `new:${this.projectId}`}`; }
  get snapshotDrafts(): Obj[] { return this.local.snapshotDrafts[this.draftKey] || []; }
  get draft(): string { return this.local.drafts[this.draftKey] || ''; }
  get projection(): Obj { return this.thread?.projection || {}; }

  private changed(): void { this.revision++; }
  private ownedNative(native: Native, current: () => boolean): Native {
    return { available: native.available, watch: topic => native.watch(topic), later: async request => {
      if (!current()) throw new ClientError('This operation was superseded.', 'superseded');
      const result = await native.later(request);
      if (!current()) throw new ClientError('This operation was superseded.', 'superseded');
      return result;
    } };
  }
  raw(native: Native, request: unknown) { return bridgeReply(native, request); }
  async call(native: Native, request: unknown, expected = this.generation, write = false): Promise<Obj> {
    if (this.generation !== expected) throw new ClientError('The connection changed. Refresh before continuing.', 'stale');
    let response;
    // shell-slow.ts "Some requests are slow", with a trace the transport lists while pending (r3-protocol-reader.ts).
    const traced = traceRpc(this, request);
    try { response = await this.raw(native, { ...traced.request, generation: expected }); }
    catch (error) {
      if (error instanceof ClientError) throw error;
      throw new ClientError(message(error), 'transport', write);
    } finally { traced.done(); }
    if (!response.ok) throw new ClientError(response.error!.message, response.error!.kind, response.error!.uncertain, response.error!);
    if (response.generation !== expected || this.generation !== expected) {
      throw new ClientError('The connection changed. Refresh before continuing.', 'stale', write);
    }
    return obj(response.value);
  }
  async http(native: Native, path: string, expected = this.generation) {
    return this.call(native, { op: 'http', path }, expected);
  }
  async request(native: Native, method: string, payload: Obj, expected = this.generation, write = false) {
    return this.call(native, { op: 'request', method, payload }, expected, write);
  }
  async ids(native: Native, count: number): Promise<string[]> {
    const response = await this.raw(native, { op: 'ids', count });
    const ids = scopeStrings(response.value);
    if (!response.ok || ids.length !== count || ids.some(id => !id)) throw new ClientError('Could not allocate request identifiers.');
    return ids;
  }

  private async load(storage: Files): Promise<void> {
    if (this.loaded) return;
    let text = '';
    try { text = new TextDecoder().decode(await storage.fs.readFile(localPath)); }
    catch (error) {
      if (error instanceof ClientError) throw error;
      // Files adapters may report a missing first-launch file as an exception.
    }
    if (this.loaded) return;
    const saved = text ? obj(JSON.parse(text)) : {};
    if (saved.version === 1) {
      const next = preferences();
      adoptSidebarPrefs(next, saved);
      next.sidebarWidth = Math.max(208, Math.min(4096, num(saved.sidebarWidth, 256))); adoptSidebarWidth(next, saved);
      next.sidebarOpen = saved.sidebarOpen !== false;
      next.favoriteModels = scopeStrings(saved.favoriteModels).slice(0, 200);
      next.groupingMode = groupingModes.includes(str(saved.groupingMode)) ? str(saved.groupingMode) : 'repository';
      for (const [key, mode] of Object.entries(obj(saved.groupingOverrides))) if (groupingModes.includes(str(mode))) next.groupingOverrides[key] = str(mode);
      next.clientSettings = decodeClientPrefs(saved.clientSettings);
      next.customThemes = decodeCustomThemes(saved.customThemes);
      adoptModelPrefs(next, saved); // settings-b: model visibility/order (settings-b-models.ts)
      next.composerControls = decodeComposerControls(saved.composerControls);
      adoptTerminalContexts(next, saved);
      adoptStash(next, saved); // composer-editor: the prompt stash (composer-editor-stash.ts)
      adoptComposerFiles(next, saved); // composer-editor: folded pastes (composer-editor-files.ts)
      adoptPagesPrefs(next, saved); // pages: page preferences and the first-run flag (pages-prefs.ts)
      adoptShellPrefs(next, saved); // shell: notice dismissals and closed workspace cards (shell-prefs.ts)
      adoptFilesPrefs(next, saved); // r5-panels: Files explorer and render preferences (r5-panels-prefs.ts)
      adoptTerminalPrefs(next, saved); // terminal-drawer: each thread's drawer (terminal-ui-state.ts)
      if (groupingModes.includes(str(saved.lastGroupingMode))) next.lastGroupingMode = str(saved.lastGroupingMode);
      const device = obj(saved.deviceSettings);
      next.deviceSettings.composerCollapseOnScroll = device.composerCollapseOnScroll !== false;
      next.deviceSettings.planModeEnabled = device.planModeEnabled === true;
      next.deviceSettings.timestampFormat = ['locale', '12-hour', '24-hour'].includes(str(device.timestampFormat)) ? str(device.timestampFormat) : 'locale';
      next.deviceSettings.appearanceMode = ['system', 'light', 'dark'].includes(str(device.appearanceMode)) ? str(device.appearanceMode) : 'system';
      next.deviceSettings.snapShotShortcut = snapshotShortcut(str(device.snapShotShortcut)) || 'shift+shift';
      next.deviceSettings.snapShotEnabled = device.snapShotEnabled === true;
      next.deviceSettings.snapShotIncludeAccessibility = device.snapShotIncludeAccessibility !== false;
      next.deviceSettings.snapShotPlaySound = device.snapShotPlaySound !== false;
      next.deviceSettings.snapShotFlash = device.snapShotFlash !== false;
      next.deviceSettings.snapShotAnimations = device.snapShotAnimations !== false;
      next.deviceSettings.snapShotSound = device.snapShotSound === 'camera-shutter' ? 'camera-shutter' : 'soft-pop';
      next.deviceSettings.sendShortcut = ['enter', 'mod-enter', 'mod-enter-multiline'].includes(str(device.sendShortcut)) ? str(device.sendShortcut) : 'enter';
      for (const [key, value] of Object.entries(obj(saved.drafts))) {
        if (typeof value === 'string' && value.length <= 1_000_000) next.drafts[key] = value;
      }
      next.snapshotReleases = scopeStrings(saved.snapshotReleases).filter(id => /^[a-f0-9-]{36}$/i.test(id));
      for (const [key, value] of Object.entries(obj(saved.snapshotDrafts))) {
        next.snapshotDrafts[key] = arr(value).filter(image => /^[a-f0-9-]{36}$/i.test(str(image.id)) && image.mimeType === 'image/png' && num(image.sizeBytes) > 0 && num(image.sizeBytes) <= 10 * 1024 * 1024).slice(0, 100);
      }
      for (const [key, value] of Object.entries(obj(saved.selections))) {
        const selection = obj(value);
        next.selections[key] = { projectId: str(selection.projectId), threadId: str(selection.threadId) };
      }
      for (const [key, value] of Object.entries(obj(saved.pending))) {
        const entry = obj(value);
        if (str(entry.method) && str(obj(entry.payload).commandId)) next.pending[key] = {
          method: str(entry.method), payload: obj(entry.payload), description: str(entry.description),
          threadId: str(entry.threadId), text: str(entry.text), uncertain: true,
        };
      }
      this.local = next;
    }
    this.loaded = true; this.changed(); // r4-polish: sources gated on the read preferences (the first-run gate) ask again
  }
  async persist(storage: Files): Promise<void> {
    if (this.environmentId) this.local.selections[this.environmentId] = { projectId: this.projectId, threadId: this.threadId };
    // The native adapter serializes writes inside this app's transport queue.
    // A later answer must never await an abandoned answer's promise.
    await storage.fs.atomicWriteFile(localPath, new TextEncoder().encode(JSON.stringify({ version: 1, ...this.local })));
  }
  /** shell-prefs.ts: whether the saved preferences were read, and a save outside a command (a toast's dismissal). */
  get preferencesLoaded(): boolean { return this.loaded; }
  async savePreferences(storage: Files): Promise<void> { await this.save(storage); }
  private async save(storage: Files): Promise<void> {
    try { await this.persist(storage); } catch (error) { if (!letGo(error)) this.error = 'Could not save local drafts and preferences. Keep a copy before closing.'; }
  }

  // r13-store F5: Remove forgets the focused environment's address, selection and cached threads
  // (reference ConnectionsSettings.tsx:2600-2640: the pairing, credentials and cached threads go).
  dropFocus(environmentId: string): void {
    delete this.local.selections[environmentId];
    this.origin = DEFAULT_ORIGIN; this.environmentId = ''; this.projectId = ''; this.threadId = '';
    this.shell = initialShell(); this.shellLoaded = false; this.thread = null;
    this.config = {}; this.providerId = ''; this.modelId = ''; this.answers = {};
    this.diffOpen = false; this.diffText = ''; this.threadEpoch++;
  }

  adoptStatus(value: Obj, generation: number): void {
    if (!['disconnected', 'connecting', 'connected', 'reconnecting', 'error'].includes(str(value.state))
      || typeof value.origin !== 'string' || typeof value.environmentId !== 'string' || typeof value.message !== 'string') {
      throw new ClientError('The native bridge returned an invalid connection status. Reconnect and try again.', 'protocol');
    }
    if (generation < this.generation) return;
    const nextEnvironment = str(value.environmentId);
    const replaced = generation !== this.generation;
    const moved = nextEnvironment && nextEnvironment !== this.environmentId;
    this.origin = str(value.origin) || this.origin;
    this.connection = str(value.state, 'disconnected');
    this.statusMessage = str(value.message);
    // r5-composer: a ref list scrolled toward its end asks its source again for the next page (r5-composer-paging.ts).
    const scrolled = JSON.stringify(obj(value.presentation).scrollEnds ?? {}) !== JSON.stringify(this.presentation.scrollEnds ?? {});
    this.presentation = obj(value.presentation);
    if (scrolled) this.changed();
    if (moved) {
      this.environmentId = nextEnvironment;
      const selection = this.local.selections[nextEnvironment];
      this.projectId = selection?.projectId || '';
      this.threadId = selection?.threadId || '';
      this.shell = initialShell(); this.shellLoaded = false; this.thread = null;
      this.config = {}; this.providerId = ''; this.modelId = ''; this.answers = {};
      this.diffOpen = false; this.diffText = ''; this.threadEpoch++;
    }
    if (replaced) {
      this.generation = generation;
      this.shellLive = false; this.threadLive = false; this.configLive = false;
      this.synchronizedGeneration = -1; this.threadSubscription = '';
      this.subscriptions = {};
      this.lastEvent = 0;
    }
    if (this.connection !== 'connected') {
      this.shellLive = false; this.threadLive = false; this.configLive = false;
      this.synchronizedGeneration = -1;
      this.synchronizationEpoch++;
    }
  }

  async refresh(native: Native | null | undefined, storage: Files): Promise<void> {
    if (!native?.available) { this.available = false; return; }
    const epoch = ++this.nextRefreshEpoch;
    // The read gate holds this read's topics until it ends (r3-protocol-reader.ts, T3ReadGate.swift).
    const read = beginRead(native, epoch); native = read.native;
    native.watch('t3.status'); native.watch('t3.events');
    native.watch('t3.notify'); // r13-threads: a command's wake redraws the composer (Send's "Preparing machine")
    try {
      await this.load(storage);
      await this.raw(native, { op: 'devicePresentation', ...this.local.deviceSettings, confirmQuit: quitMode(this.local) });
      const ticket = statusTicket(this);
      const status = await this.raw(native, { op: 'status' });
      if (status.ok) settleTraces(this, obj(status.value), ticket);
      // Exact may discard an optimistic asynchronous reread without dispatching
      // it. Only a real reply can replace the active refresh's ownership.
      if (epoch < this.refreshEpoch) return;
      this.refreshEpoch = epoch;
      this.available = true;
      native = this.ownedNative(native, () => epoch === this.refreshEpoch);
      if (!status.ok) throw new ClientError(status.error!.message);
      this.adoptStatus(obj(status.value), status.generation);
      await reconnectOnLaunch(this, native, obj(status.value)); // r8-pointer D14: a relaunch reconnects (r8-pointer-reconnect.ts)
      await fleet.sync(native, launchFocus(this)); // settings-b: background environments (settings-b-fleet.ts)
      await this.flushSnapshotReleases(native, storage);
      if (this.connection !== 'connected') { if (this.local.deviceSettings.snapShotEnabled) await this.adoptSnapshots(native, storage); return; }
      if (this.synchronizedGeneration !== this.generation) await this.synchronize(native);
      else if (this.threadId && this.threadSubscription !== this.threadId) await this.openThread(native, this.threadId, this.thread !== null);
      await this.drain(native);
      if (this.streamRetryDue.size) { await this.retryStreams(native); await this.drain(native); }
      if (this.needsFreshSnapshot) {
        this.needsFreshSnapshot = false;
        this.shellLoaded = false; this.thread = null;
        await this.synchronize(native);
        await this.drain(native);
      }
      if (this.reconcilePending()) await this.save(storage);
      await sidebarRefreshed(this, native); // sidebar-commands.ts: a timed provider pill's repaint, new projects' favicons
      await this.flushSnapshotReleases(native, storage);
      if (this.local.deviceSettings.snapShotEnabled) await this.adoptSnapshots(native, storage);
    } catch (error) {
      if (epoch < this.refreshEpoch) return;
      this.available = true;
      if (!(error instanceof ClientError && ['stale', 'superseded'].includes(error.kind))) this.error = message(error);
    } finally { await read.end(); }
  }

  async synchronize(native: Native): Promise<void> {
    // Commands and watched snapshots can both bootstrap. Only the newest one
    // may install subscriptions, even when their answer lifetimes differ.
    const epoch = ++this.synchronizationEpoch;
    native = this.ownedNative(native, () => epoch === this.synchronizationEpoch);
    const generation = this.generation;
    this.synchronizedGeneration = -1;
    this.shellLive = false; this.threadLive = false;
    delete this.subscriptions.config; delete this.subscriptions.shell; this.streamRetryDue.clear();
    const auth = await this.http(native, '/api/auth/session', generation);
    if (auth.authenticated !== true) throw new ClientError('Pair with the T3 server again.', 'auth');
    this.scopes = scopeStrings(auth.scopes);
    const config = await this.request(native, 'server.getConfig', {}, generation);
    const environment = obj(config.environment);
    const incompatible = compatibilityProblem(environment); // r3-protocol-outdated.ts: the message names the direction
    if (incompatible) throw new ClientError(incompatible.message, 'protocol');
    if (str(environment.environmentId) !== this.environmentId) throw new ClientError('This address now belongs to a different T3 environment. Pair again.', 'identity');
    this.config = config; this.configLive = true;
    const configSub = await this.call(native, { op: 'subscribe', key: 'config', method: 'subscribeServerConfig', payload: { environmentThemes: true, usageLimitSources: true, usageLimitsCommand: true } }, generation);
    this.subscriptions.config = str(configSub.id);
    if (!this.shellLoaded) {
      this.shell = applyShell(this.shell, await this.http(native, '/api/orchestration/shell', generation));
      this.shellLoaded = true;
    }
    this.ensureSelection();
    const marker = config.shellResumeCompletionMarker === true;
    const shellSub = await this.call(native, { op: 'subscribe', key: 'shell', method: 'orchestration.subscribeShell', payload: {
      afterSequence: this.shell.sequence, ...(marker ? { requestCompletionMarker: true } : {}),
    } }, generation);
    this.subscriptions.shell = str(shellSub.id);
    this.shellLive = !marker;
    if (this.threadId) await this.openThread(native, this.threadId, this.thread !== null);
    else { this.thread = null; this.threadLive = true; this.chooseDefaults(); }
    if (generation === this.generation) {
      this.synchronizedGeneration = generation;
      this.error = !this.scopes.includes('orchestration:operate') ? 'This connection is read-only.'
        : obj(environment.capabilities).serverResolvedCommandContext !== true
          ? 'Update the T3 server to send messages from this client. Existing threads remain readable.' : '';
    }
    this.changed();
  }

  /** The same-session resubscribe a stream failure scheduled (client-runtime rpc/client.ts). */
  private async retryStreams(native: Native): Promise<void> {
    const due = this.streamRetryDue, generation = this.generation;
    this.streamRetryDue = new Set();
    if (due.has('config')) { await this.synchronize(native); }
    else {
      if (due.has('shell')) {
        const marker = this.config.shellResumeCompletionMarker === true;
        const shellSub = await this.call(native, { op: 'subscribe', key: 'shell', method: 'orchestration.subscribeShell', payload: {
          afterSequence: this.shell.sequence, ...(marker ? { requestCompletionMarker: true } : {}),
        } }, generation);
        this.subscriptions.shell = str(shellSub.id); this.shellLive = !marker;
      }
      if (due.has('thread') && this.threadId) await this.openThread(native, this.threadId, this.thread !== null);
    }
    if (this.error === this.streamError) this.error = '';
    this.streamError = '';
  }

  ensureSelection(): void {
    const selected = this.shell.threads.find(thread => thread.id === this.threadId);
    if (selected) this.projectId = str(selected.projectId);
    if (!this.shell.projects.some(project => project.id === this.projectId)) this.projectId = mostRecentProjectId(this.shell);
  }
  chooseDefaults(): void {
    const project = this.shell.projects.find(project => project.id === this.projectId);
    const settings = obj(this.config.settings);
    const overrides = obj(obj(settings.projectSettingsOverrides)[this.projectId]);
    const selection = obj(overrides.defaultModelSelection || project?.defaultModelSelection || settings.defaultModelSelection);
    const providers = arr(this.config.providers);
    const automatic = !str(selection.instanceId);
    const provider = automatic
      ? providers.find(provider => provider.status === 'ready' && providerAvailable(provider)) || providers.find(providerAvailable)
      : providers.find(provider => provider.instanceId === selection.instanceId && providerAvailable(provider));
    const models = arr(provider?.models);
    const model = automatic ? models.find(model => model.isDefault === true && model.isCustom !== true) || models.find(model => model.isCustom !== true) || models[0] : models.find(model => model.slug === selection.model);
    this.providerId = provider && model ? str(provider.instanceId) : '';
    this.modelId = provider && model ? str(model.slug) : '';
    this.modelOptions = !automatic && provider && model ? arr(selection.options) : [];
    this.runtimeMode = str(overrides.defaultRuntimeMode || settings.defaultRuntimeMode, 'approval-required');
    this.interactionMode = 'default';
    applySticky(this);
  }
  private updateSelectionFromThread(): void {
    const thread = obj(this.projection.thread);
    const selection = obj(thread.modelSelection);
    this.providerId = str(selection.instanceId);
    this.modelId = str(selection.model);
    this.modelOptions = arr(selection.options);
    this.runtimeMode = str(thread.runtimeMode, 'approval-required');
    this.interactionMode = str(thread.interactionMode, 'default');
    this.projectId = str(thread.projectId, this.projectId);
    applyStaged(this);
  }

  async openThread(native: Native, id: string, resume = false): Promise<void> {
    const epoch = ++this.threadEpoch, generation = this.generation;
    this.threadLive = false; this.threadSubscription = '';
    delete this.subscriptions.thread;
    if (!resume || str(obj(this.thread?.projection.thread).id) !== id) {
      const snapshot = await this.http(native, `/api/orchestration/threads/${encodeURIComponent(id)}/bounded`, generation);
      if (epoch !== this.threadEpoch || id !== this.threadId) return;
      this.thread = threadSnapshot(snapshot);
    }
    if (epoch !== this.threadEpoch || id !== this.threadId || !this.thread) return;
    this.updateSelectionFromThread();
    const marker = this.config.threadResumeCompletionMarker === true;
    const subscription = await this.call(native, { op: 'subscribe', key: 'thread', method: 'orchestration.subscribeThread', payload: {
      threadId: id, afterSequence: this.thread.sequence, acceptBoundedSnapshot: true,
      ...(marker ? { requestCompletionMarker: true } : {}),
    } }, generation);
    if (epoch !== this.threadEpoch || id !== this.threadId) return;
    this.subscriptions.thread = str(subscription.id);
    this.threadSubscription = id;
    this.threadLive = !marker;
  }

  private async drain(native: Native): Promise<void> {
    const generation = this.generation;
    for (;;) {
      const batch = await this.call(native, { op: 'events', after: this.lastEvent }, generation);
      if (batch.reset === true) this.needsFreshSnapshot = true;
      let through = this.lastEvent;
      let awaitingRegistration = false;
      for (const entry of arr(batch.events)) {
        const seq = num(entry.seq);
        if (seq <= this.lastEvent) continue;
        if (num(entry.generation, -1) !== generation) { through = Math.max(through, seq); continue; }
        const item = obj(entry.value), key = str(entry.key);
        if (!this.subscriptions[key] && (key === 'config' || key === 'shell' || (key === 'thread' && this.threadId))) {
          // Native can enqueue a subscription's first marker before its reply
          // reaches the answer installing it. Keep this entry and every later
          // entry in the inbox; that answer's completion will trigger a reread.
          awaitingRegistration = true;
          break;
        }
        through = Math.max(through, seq);
        if (key === TELEMETRY_KEY) { telemetryEvent(this, entry); continue; } // settings-a-telemetry.ts
        if (key === WORKTREE_SETUP_KEY) { worktreeSetupEvent(this, entry); continue; } // timeline-worktree.ts
        if (key === VCS_STATUS_KEY) { vcsStatusEvent(this, entry); continue; } // shell-vcs.ts: the workspace card's git status
        if (key === GIT_ACTION_KEY) { gitActionEvent(this, entry); continue; } // r4-git-actions.ts: the card's git.runStackedAction stream
        if (key === DEVICE_STATE_KEY) { deviceStateEvent(this, entry); continue; } // r4-surfaces-device.ts: the device hub state
        if (providerAuthEvent(this, entry)) continue;
        if (LIVE_KEYS.includes(key)) { liveEvent(this, entry); continue; } // live-streams.ts: scheduled tasks and project clones
        if (key === TERMINAL_METADATA_KEY) { terminalMetadataEvent(this, entry); continue; } // terminal-drawer-view.ts: terminal labels and sessions
        if (!this.subscriptions[key] || str(entry.subscriptionId) !== this.subscriptions[key]) continue;
        // T3Transport resubscribes a failed stream on this session after a backoff (250 ms
        // doubling to 30 s); an authorization failure waits for the next session (c5a929e).
        if (item._retryDue) { this.streamRetryDue.add(key); continue; }
        if (item._transportError || item._streamEnded) {
          const detail = obj(item._transportError);
          this.error = this.streamError = str(detail.message, 'The live stream ended. Reconnect to continue.');
          if (key === 'thread') this.threadLive = false;
          if (key === 'shell') this.shellLive = false;
          if (key === 'config') this.configLive = false;
          continue;
        }
        try {
          if (key === 'config') {
            this.config = applyConfig(this.config, item); this.configLive = true;
            configEventSideEffects(this, item); // r3-protocol-config.ts: the keybindings reload toast
            if (!this.threadId && !this.providerId) this.chooseDefaults();
          } else if (key === 'shell') {
            if (item.kind === 'synchronized') this.shellLive = true;
            else { this.shell = applyShell(this.shell, item); this.shellLoaded = true; }
            if ((item.kind === 'thread.removed' && item.threadId === this.threadId)
              || (item.kind === 'thread.updated' && item.location === 'archive' && obj(item.thread).id === this.threadId)) {
              this.threadId = ''; this.thread = null; this.threadSubscription = ''; this.threadEpoch++;
              delete this.subscriptions.thread; this.threadLive = true; this.chooseDefaults();
            }
            this.ensureSelection();
          } else if (key === 'thread' && this.thread && this.threadSubscription === this.threadId) {
            // A cancelled subscription can have retained inbox entries. Identity checks
            // reject them before their sequence or completion marker can touch this thread.
            const itemThread = str(obj(item.projection).thread && obj(obj(item.projection).thread).id)
              || str(obj(item.event).threadId);
            if (itemThread && itemThread !== this.threadId) continue;
            if (item.kind === 'synchronized') this.threadLive = true;
            else {
              this.thread = applyThread(this.thread, item);
              this.updateSelectionFromThread();
            }
          }
        } catch (error) {
          this.error = message(error);
          this.needsFreshSnapshot = true;
        }
      }
      if (batch.reset === true && arr(batch.events).length === 0) through = Math.max(through, num(batch.latest));
      this.lastEvent = through;
      const ack = through > 0 ? await this.call(native, { op: 'ack', through }, generation) : {};
      this.changed();
      if (awaitingRegistration) break;
      if (through >= Math.max(num(batch.latest), num(ack.latest))) break;
      if (arr(batch.events).length === 0 && num(ack.latest) <= through) break;
    }
  }

  private requireWrite(): void {
    if (!this.writable) throw new ClientError(this.connection !== 'connected' ? 'Reconnect before making changes.' : 'Wait for synchronization and check your connection permissions.');
    if (this.pending) throw new ClientError('Resolve the previous operation before sending another.');
  }
  reconcilePending(): boolean {
    const pending = this.pending;
    if (!pending) return false;
    const payload = pending.payload;
    let completed = false;
    if (pending.method === 'orchestration.launchThread') {
      completed = this.shell.threads.some(thread => thread.id === payload.threadId);
    } else if (payload.type === 'thread.fork') {
      completed = this.shell.threads.some(thread => thread.id === payload.targetThreadId);
    } else if (payload.type === 'project.create') {
      completed = this.shell.projects.some(project => project.id === payload.projectId);
    } else if (pending.threadId === this.threadId && this.threadLive) {
      if (payload.type === 'message.dispatch') completed = arr(this.projection.messages).some(value => value.id === payload.messageId);
      else if (payload.type === 'runtime-request.respond' || payload.type === 'thread.user-input.dismiss') completed = arr(this.projection.runtimeRequests).some(value => value.id === payload.requestId && value.status !== 'pending');
      else if (payload.type === 'run.interrupt') completed = arr(this.projection.runs).some(value => value.id === payload.runId && !['preparing', 'starting', 'running', 'waiting'].includes(str(value.status)));
      else if (payload.type === 'thread.model-selection.set') completed = JSON.stringify(obj(obj(this.projection.thread).modelSelection)) === JSON.stringify(payload.modelSelection);
      else if (payload.type === 'thread.runtime-mode.set') completed = obj(this.projection.thread).runtimeMode === payload.runtimeMode;
      else if (payload.type === 'thread.interaction-mode.set') completed = obj(this.projection.thread).interactionMode === payload.interactionMode;
    }
    if (!completed) return false;
    this.finishPending(pending);
    return true;
  }
  private finishPending(pending: Pending, environmentId = this.environmentId): void {
    // The outcome is known (acknowledged or reconciled), so "may have reached T3" is answered.
    if (this.error === uncertainError(pending)) this.error = '';
    if (pending.text) {
      const key = pending.method === 'orchestration.launchThread'
        ? `${environmentId}:new:${str(pending.payload.projectId)}` : `${environmentId}:${pending.threadId}`;
      if (this.local.drafts[key] === pending.text) delete this.local.drafts[key];
    }
    const key = pending.method === 'orchestration.launchThread' ? `${environmentId}:new:${str(pending.payload.projectId)}` : `${environmentId}:${pending.threadId}`;
    const sent = arr(pending.method === 'orchestration.launchThread' ? obj(pending.payload.initialMessage).attachments : pending.payload.attachments);
    if (sent.length) this.local.snapshotDrafts[key] = (this.local.snapshotDrafts[key] || []).filter(image => {
      if (!sent.some(attachment => attachment.id === image.uploadId)) return true;
      this.local.snapshotReleases.push(str(image.id)); return false;
    });
    delete this.local.pending[environmentId];
  }
  async write(native: Native, storage: Files, pending: Pending, beforeRequest?: () => void): Promise<Obj> {
    const generation = this.generation;
    const environmentId = this.environmentId;
    this.local.pending[environmentId] = pending;
    try { await this.persist(storage); }
    catch (error) {
      delete this.local.pending[environmentId];
      if (letGo(error)) throw error; // let-go.ts: nothing was sent, and it is not a failure
      throw new ClientError('Could not save this operation locally. Your draft is still available.');
    }
    try {
      beforeRequest?.();
      const result = await this.request(native, pending.method, pending.payload, generation, true);
      this.finishPending(pending, environmentId);
      await this.save(storage);
      await this.flushSnapshotReleases(native, storage);
      return result;
    } catch (error) {
      if (error instanceof ClientError && error.kind === 'superseded') throw error;
      if (error instanceof ClientError && error.uncertain) {
        pending.uncertain = true;
        this.error = uncertainError(pending);
      } else {
        delete this.local.pending[environmentId];
        this.error = message(error);
      }
      await this.save(storage);
      throw error;
    }
  }
  async dispatch(native: Native, storage: Files, payload: Obj, description: string, beforeRequest?: () => void): Promise<Obj> {
    return this.write(native, storage, { method: 'orchestration.dispatchCommand', payload, description,
      threadId: str(payload.threadId), text: str(payload.text), uncertain: false }, beforeRequest);
  }

  async command(op: string, id: string, value: string, n: number, native: Native | null | undefined, storage: Files): Promise<{ revision: number; message: string }> {
    if (!native?.available) return { revision: ++this.revision, message: 'Open this app on macOS to connect to T3 Code.' };
    const local = ['draft', 'answer', 'choice', 'previous-question', 'search', 'sidebar', 'dismiss-error', 'close-diff', 'favorite-model', 'copy-message', 'grouping-mode', 'grouping-override', 'device-setting', 'setting-snapshot', 'remove-snapshot', 'snapshot-preview-sound', 'snapshot-shortcut-record', 'snapshot-shortcut-save', 'copy-diagnostic'].includes(op) || op.startsWith('restlocal:') || op.startsWith('chatlocal:') || op.startsWith('editorlocal:') || op.startsWith('shelllocal:') || op.startsWith('cclocal:') || op.startsWith('sidebarlocal:') || op.startsWith('pageslocal:') || op.startsWith('terminallocal:') || op.startsWith('terminalpanellocal:') || DIFF_LOCAL_OPS.includes(op);
    const epoch = local ? this.commandEpoch : ++this.commandEpoch;
    if (!local) {
      if (this.busy && this.pending) this.pending.uncertain = true;
      this.busy = true;
      this.historyLoading = false; this.diffLoading = false;
      native = this.ownedNative(native, () => epoch === this.commandEpoch);
    }
    let resultMessage = '';
    const out: OpOut = { message: '', id, value }; // an area's ops hand back their message (client-ops.ts)
    try {
      await this.load(storage);
      await this.raw(native, { op: 'devicePresentation', ...this.local.deviceSettings, confirmQuit: quitMode(this.local) });
      if (await runOps(this, READ_OPS, op, id, value, n, native, storage, out)) ({ message: resultMessage, id, value } = out);
      else {
        this.requireWrite();
        if (await runOps(this, WRITE_OPS, op, id, value, n, native, storage, out)) ({ message: resultMessage, id, value } = out);
        else throw new ClientError(`Unknown action: ${op}`);
      }
      if ((op === 'device-setting' || op === 'settings-core') && native?.available) await this.raw(native, { op: 'devicePresentation', ...this.local.deviceSettings, confirmQuit: quitMode(this.local) });
      await this.save(storage);
      await this.flushSnapshotReleases(native, storage);
      shellSuccess(this, op, value, resultMessage, id);
    } catch (error) {
      ({ id, value } = out); // a group may have rewritten them (setting-snapshot's setup)
      if (error instanceof ClientError && error.kind === 'superseded') return { revision: this.revision, message: '' };
      resultMessage = message(error);
      // These commands return their error to the owning form. Do not copy it
      // into the unrelated transcript banner or a later form.
      const formCommand = ['settings-core', 'snapshot-shortcut-save', 'snapshot-shortcut-record', 'setting-snapshot', 'rename-project', 'remove-project', 'rename-group', 'remove-group', 'remove-group-member', 'setting-permissions', 'setting-model', 'setting-storage', 'setting-scoped', 'task-save', 'task-toggle', 'task-delete', 'task-run', 'keybinding-save', 'keybinding-remove', 'unarchive-thread', 'delete-archived-thread', 'provider-create', 'provider-name', 'provider-enabled', 'provider-remove'].includes(op) || PROVIDER_OPS.includes(op) || CONNECTION_OPS.includes(op) || op.startsWith('rest:') || op.startsWith('restlocal:') || op.startsWith('sb:');
      // Main-window actions the reference toasts (shell-commands.ts) leave the transcript banner alone.
      if (!this.pending?.uncertain && !formCommand) { if (!shellFailure(this, op, resultMessage)) this.error = resultMessage; else if (this.error === resultMessage) this.error = ''; }
      else if (formCommand && !this.pending?.uncertain && settingsFailure(this, op, id, value, resultMessage)) resultMessage = `toasted:${resultMessage}`;
    } finally { if (!local && epoch === this.commandEpoch) this.busy = false; this.changed(); }
    return { revision: this.revision, message: resultMessage };
  }

  /** handleNewThreadInActiveProject: the 'new-thread' op's switch to a draft in the current project. */
  /** Lane r6-pr: a pull request hand-off opens the project's draft (useNewThreadHandler) from inside its command. */
  async openProjectDraft(native: Native, projectId: string): Promise<void> { this.projectId = projectId; await this.openFreshDraft(native); }
  async openFreshDraft(native: Native): Promise<void> {
    this.threadId = ''; this.thread = null; this.threadSubscription = ''; this.threadEpoch++;
    delete this.subscriptions.thread;
    this.threadLive = true; this.diffOpen = false; this.answers = {}; this.chooseDefaults();
    if (this.connection === 'connected') await this.call(native, { op: 'unsubscribe', key: 'thread' });
  }
  configuredProviders() {
    return Object.entries(obj(obj(this.config.settings).providerInstances)).map(([id, entry]) => {
      const instance = obj(entry);
      return { id, name: str(instance.displayName, str(instance.driver)), driver: str(instance.driver), enabled: instance.enabled !== false };
    });
  }
  providerDrivers() {
    // Same first-instance catalog as pinned providerDriverMeta.ts; installed instances
    // enrich advertised authentication metadata, never decide whether a driver exists.
    const catalog = [['codex', 'Codex'], ['claudeAgent', 'Claude'], ['cursor', 'Cursor'], ['grok', 'Grok'], ['opencode', 'OpenCode'], ['antigravity', 'Antigravity'], ['pi', 'Pi'], ['acpRegistry', 'ACP Registry']];
    const drivers = new Map<string, { id: string; name: string; canAuthenticate: boolean }>(catalog.map(([id, name]) => [id, { id, name, canAuthenticate: false }]));
    for (const provider of arr(this.config.providers)) if (str(provider.driver)) {
      const driver = str(provider.driver), prior = drivers.get(driver);
      drivers.set(driver, { id: driver, name: prior?.name || driver, canAuthenticate: obj(provider.setup).canAuthenticate === true || prior?.canAuthenticate === true });
    }
    return [...drivers.values()];
  }
  /** Provider settings writes (providers.ts) share this connection's generation and write-uncertainty rules. */
  async rpc(native: Native, method: string, payload: Obj, write = false): Promise<Obj> { return this.request(native, method, payload, this.generation, write); }
  projectGroups() {
    const byKey = new Map<string, Obj[]>();
    for (const project of this.shell.projects) {
      const physical = `${this.environmentId}:${projectPath(project.workspaceRoot)}`;
      const mode = this.local.groupingOverrides[physical] || this.local.groupingMode;
      const identity = obj(project.repositoryIdentity), canonical = str(identity.canonicalKey), root = projectPath(identity.rootPath), path = projectPath(project.workspaceRoot);
      const relative = root && path.startsWith(root + '/') ? path.slice(root.length + 1) : '';
      const key = mode === 'separate' || !canonical ? physical : canonical + (mode === 'repository_path' && relative ? `::${relative}` : '');
      const members = byKey.get(key) || []; members.push(project); byKey.set(key, members);
    }
    return [...byKey].map(([key, members]) => {
      const name = groupLabel(members); // r6-polish: deriveProjectGroupLabel (r6-polish-groups.ts)
      return { key, name, members };
    });
  }
  /** Lane settings-core: settings reads/writes and the scope's t3.json, never other methods. */
  async settingsCoreRequest(native: Native, method: string, payload: Obj, write = false): Promise<Obj> {
    if (!['server.getSettings', 'server.updateSettings', 'projects.readFile'].includes(method)) throw new ClientError('Unsupported settings request.');
    return this.request(native, method, payload, this.generation, write);
  }
  async readSettings(native: Native, method: string, payload: Obj = {}): Promise<Obj> {
    if (!this.ready) throw new ClientError('Connect and synchronize before loading this section.');
    const allowed = ['orchestration.getArchivedShellSnapshot', 'server.getProcessDiagnostics', 'server.getProcessResourceHistory', 'server.getTraceDiagnostics', 'scheduledTasks.list', 'server.getConfig', 'server.getSettings'];
    if (!allowed.includes(method)) throw new ClientError('Unsupported settings query.');
    return this.request(native, method, payload);
  }
  /** Opens a thread as the selection: the row press, keyboard jumps and the sidebar's forward navigation. */
  async openSelected(native: Native, id: string): Promise<void> {
    if (!this.shell.threads.some(thread => thread.id === id)) { await this.openDraft(native, this.projectId); this.threadId = id; return; } // composer-fidelity G15: a missing thread shows NoActiveThreadState
    this.threadId = id; this.thread = null; this.diffOpen = false; this.answers = {};
    this.ensureSelection(); await this.openThread(native, id);
    sidebarOpened(this, native);
  }
  /** A fresh draft in a project: where archiving or parking the open thread lands. */
  async openDraft(native: Native, projectId: string): Promise<void> {
    if (projectId && this.shell.projects.some(project => project.id === projectId)) this.projectId = projectId;
    this.threadId = ''; this.thread = null; this.threadSubscription = ''; this.threadEpoch++;
    delete this.subscriptions.thread;
    this.threadLive = true; this.diffOpen = false; this.answers = {}; this.chooseDefaults();
    if (this.connection === 'connected') await this.call(native, { op: 'unsubscribe', key: 'thread' });
  }
  // Generation-guarded server access for settings-rest-commands.ts / settings-rest-data.ts.
  restAccess(native: Native) {
    return { request: (method: string, payload: Obj = {}, write = false) => this.request(native, method, payload, this.generation, write), http: (path: string) => this.http(native, path), ids: (count: number) => this.ids(native, count), call: (request: Obj) => this.call(native, request),
      dispatch: (storage: Files, payload: Obj, description: string) => this.dispatch(native, storage, payload, description), write: (storage: Files, pending: Pending) => this.write(native, storage, pending) };
  }
  get snapshotOwner() { return this.environmentId ? JSON.stringify([this.origin, this.environmentId, this.projectId, this.threadId]) : ''; }
  async adoptSnapshots(native: Native, storage: Files): Promise<void> {
    const adopting = this.refreshEpoch;
    if (this.snapshotAdopting === adopting) return;
    this.snapshotAdopting = adopting;
    const generation = this.generation, environment = this.environmentId, origin = this.origin, local = this.local;
    // A reconnect or preference reset is not a lost destination: stop and leave captures pending.
    const connected = () => generation === this.generation && environment === this.environmentId && origin === this.origin && local === this.local && this.ready;
    const dismiss = (id: string, owner: string) => this.call(native, { op: 'snapshotDismiss', id, owner }).catch(() => undefined);
    const report = (id: string, text: string, stranded = false) => { if (!this.snapshotNotices.has(id)) this.error = text; this.snapshotNotices.set(id, stranded); };
    try {
      const state = await this.snapshotState(native, true);
      if (!connected()) return;
      const pending = Array.isArray(state.captures) ? arr(state.captures) : scopeStrings(state.pending).map(id => ({ id, owner: this.snapshotOwner }));
      for (const item of pending) {
        if (!connected()) return;
        const id = str(item.id), owner = str(item.owner), identity = snapshotIdentity(owner, origin, environment);
        // A pinned destination that vanished stays pending, as in the reference, without refetching.
        if (!identity || this.snapshotNotices.get(id) === true) continue;
        if (!identity[2] && !this.shell.projects.length && this.shellLoaded) { await dismiss(id, owner); report(id, snapshotNoProjectMessage); continue; }
        const catalog = await this.http(native, '/api/orchestration/shell');
        if (!connected()) return;
        const threadId = identity[3], active = arr(catalog.threads).find(thread => thread.id === threadId);
        // Reference defaultProjectRef; this client keeps no project-order preference.
        const projectId = identity[2] || str(active?.projectId) || snapshotDefaultProject(arr(catalog.projects), environment);
        const key = `${environment}:${threadId || `new:${projectId}`}`;
        // null: connection/preferences changed (retry later); false: fresh catalog lost the destination.
        const fresh = async () => { const value = await this.http(native, '/api/orchestration/shell'); return connected() ? snapshotDestinationExists(value, projectId, threadId) : null; };
        const strand = async (saved: boolean) => {
          // Undo only this capture against the CURRENT array, commit, then release owned bytes.
          if (withoutSnapshot(local.snapshotDrafts, key, id)) await this.persist(storage);
          if (saved) { local.snapshotReleases.push(id); await this.flushSnapshotReleases(native, storage); }
          await dismiss(id, owner); report(id, snapshotNoProjectMessage, Boolean(identity[2]));
        };
        if (!snapshotDestinationExists(catalog, projectId, threadId)) { await strand(false); continue; }
        try {
          if (!(local.snapshotDrafts[key] || []).some(image => image.id === id)) {
            if ((local.snapshotDrafts[key] || []).length >= 100) throw new ClientError('Remove an attachment, then try this capture again.');
            const capture = await this.call(native, { op: 'snapshotRead', id, owner });
            if (capture.owner !== owner || capture.id !== id || capture.mimeType !== 'image/png' || num(capture.sizeBytes) < 1 || num(capture.sizeBytes) > 10 * 1024 * 1024) throw new ClientError('The captured window is too large or invalid.');
            try { await this.call(native, { op: 'snapshotDraftSave', id, owner }); }
            catch (error) { local.snapshotReleases.push(id); throw error; }
            const destination = await fresh();
            if (destination === null) return;
            if (!destination) { await strand(true); continue; }
            local.snapshotDrafts[key] = [...(local.snapshotDrafts[key] || []), { id, name: str(capture.name), mimeType: 'image/png', sizeBytes: num(capture.sizeBytes), source: obj(capture.source) }];
            try { await this.persist(storage); }
            catch (error) { withoutSnapshot(local.snapshotDrafts, key, id); local.snapshotReleases.push(id); throw error; }
            this.changed();
          }
          const destination = await fresh();
          if (destination === null) return;
          if (!destination) { await strand(true); continue; }
          // An empty destination becomes a local new-thread draft only after acquisition.
          if (!identity[2] && !this.projectId && connected()) {
            this.projectId = projectId; this.threadId = ''; this.chooseDefaults(); this.changed();
            await this.snapshotState(native, true).catch(() => undefined);
          }
          await this.call(native, { op: 'snapshotAcknowledge', id, owner, focus: key === this.draftKey, focusOwner: JSON.stringify([origin, environment, projectId, threadId]) });
          this.snapshotNotices.delete(id);
        } catch (error) {
          if (error instanceof ClientError && ['stale', 'superseded'].includes(error.kind)) throw error;
          await dismiss(id, owner); report(id, snapshotFailureMessage(id, message(error)));
        }
      }
    } finally { if (this.snapshotAdopting === adopting) this.snapshotAdopting = -1; }
  }
  async flushSnapshotReleases(native: Native, storage: Files): Promise<void> {
    if (!this.local.snapshotReleases.length) return;
    // First persist acknowledgment/removal and retry identities. Never delete
    // a file still referenced by the last durable draft or uncertain submission.
    try { await this.persist(storage); } catch { return; }
    const referenced = new Set(Object.values(this.local.snapshotDrafts).flat().map(image => str(image.id)));
    for (const id of [...new Set(this.local.snapshotReleases)]) {
      if (referenced.has(id)) continue;
      try {
        await this.call(native, { op: 'snapshotDraftRemove', id });
        this.local.snapshotReleases = this.local.snapshotReleases.filter(value => value !== id);
      } catch { /* The persisted retry identity stays until a later refresh. */ }
    }
    await this.save(storage);
  }
  async uploadSnapshots(native: Native, storage: Files): Promise<Obj[]> {
    const key = this.draftKey, generation = this.generation, owner = this.snapshotOwner;
    const current = () => key === this.draftKey && generation === this.generation && owner === this.snapshotOwner;
    const attachments: Obj[] = [];
    for (const image of this.snapshotDrafts) {
      if (!current()) throw new ClientError('The draft changed before upload.');
      let attachmentId = str(image.uploadId);
      if (!attachmentId) {
        const bytes = await this.call(native, { op: 'snapshotDraftRead', id: image.id });
        if (!current()) throw new ClientError('The draft changed before upload.');
        const upload = await this.request(native, 'attachments.createUploadUrl', { type: 'image', name: image.name, mimeType: image.mimeType, sizeBytes: image.sizeBytes });
        attachmentId = str(upload.attachmentId);
        if (!attachmentId || !str(upload.relativeUrl).startsWith('/api/attachments/upload/')) throw new ClientError('The server returned an invalid attachment upload.');
        try {
          if (!current()) throw new ClientError('The draft changed before upload.');
          await this.call(native, { op: 'uploadAttachment', path: upload.relativeUrl, base64: bytes.base64 });
          if (!current()) throw new ClientError('The draft changed during upload.');
          image.uploadId = attachmentId;
          await this.persist(storage);
        } catch (error) {
          delete image.uploadId;
          // Only this operation's newly minted upload is eligible for cleanup.
          if (generation === this.generation) await this.request(native, 'attachments.delete', { attachmentId }).catch(() => {});
          throw error;
        }
      }
      attachments.push({ type: 'image', id: attachmentId, name: image.name, mimeType: image.mimeType, sizeBytes: image.sizeBytes, source: image.source });
    }
    if (!current()) throw new ClientError('The draft changed before sending its attachments.');
    return attachments;
  }
  /** The platform's own availability probe for a recorded candidate (no keybinding list: checked in TS). */
  async snapshotCheck(native: Native, shortcut: string): Promise<Obj> { return this.call(native, { op: 'snapshotCheckShortcut', shortcut, bindings: [] }); }
  async snapshotState(native: Native, allowNavigation = false): Promise<Obj> {
    const owner = this.snapshotOwner;
    let reconciliationError = '';
    try { await this.call(native, { op: 'snapshotConfigure', owner, shortcut: this.local.deviceSettings.snapShotShortcut, enabled: this.local.deviceSettings.snapShotEnabled, includeAccessibility: this.local.deviceSettings.snapShotIncludeAccessibility, playSound: this.local.deviceSettings.snapShotPlaySound, sound: this.local.deviceSettings.snapShotSound, flash: this.local.deviceSettings.snapShotFlash, animations: this.local.deviceSettings.snapShotAnimations }); }
    catch (error) { reconciliationError = message(error); }
    const state = await this.call(native, { op: 'snapshotState', owner });
    if (!allowNavigation && owner !== this.snapshotOwner) throw new ClientError('The capture draft changed. Retry in the selected draft.');
    return { ...state, error: reconciliationError || str(state.error) };
  }
  async readNotices(native: Native): Promise<Obj> {
    if (!this.ready) throw new ClientError('Connect and synchronize to load the server notices.');
    return this.http(native, '/third-party-licenses.json');
  }
  async manageArchivedThread(native: Native, storage: Files, op: string, scope: string): Promise<void> {
    const [environmentId, projectId, id, extra] = scope.split(':');
    if (extra !== undefined || !id || environmentId !== this.environmentId) throw new ClientError('That environment or thread is no longer selected.');
    const archived = applyShell(initialShell(), await this.readSettings(native, 'orchestration.getArchivedShellSnapshot'));
    const thread = archived.threads.find(thread => thread.id === id && thread.projectId === projectId);
    if (!thread || !archived.projects.some(project => project.id === projectId)) throw new ClientError('That archived thread is no longer available.');
    const [commandId] = await this.ids(native, 1);
    await this.dispatch(native, storage, { type: op === 'unarchive-thread' ? 'thread.unarchive' : 'thread.delete', commandId, threadId: id }, op === 'unarchive-thread' ? 'Unarchive thread' : 'Delete archived thread');
    this.shell = applyShell(initialShell(), await this.http(native, '/api/orchestration/shell'));
    if (this.threadId === id && op === 'delete-archived-thread') {
      this.threadId = ''; this.thread = null; this.threadLive = true; this.threadEpoch++;
      delete this.subscriptions.thread;
    }
    if (op === 'delete-archived-thread') delete this.local.drafts[`${this.environmentId}:${id}`];
  }

  async history(native: Native): Promise<void> {
    if (!this.thread?.hasMore || !this.thread.historyCursor || this.historyLoading) return;
    const epoch = this.threadEpoch, id = this.threadId, cursor = this.thread.historyCursor;
    this.historyLoading = true;
    try {
      const page = await this.http(native, `/api/orchestration/threads/${encodeURIComponent(id)}/history?cursor=${encodeURIComponent(cursor)}`);
      if (epoch === this.threadEpoch && id === this.threadId && this.thread?.historyCursor === cursor) this.thread = mergeHistory(this.thread, page);
    } finally { this.historyLoading = false; }
  }

}
