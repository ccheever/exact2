import { forgetDraftThreadId, launchThreadId } from './r7-handoff-thread'; // lane r7-handoff
import { snapshotShortcut, snapshotConflict, commandLabel } from './snapshot-shortcut';
import { removeAttachmentReferences } from './r4-composer-attachments';
import { isSnapshotPermissionMessage } from './snapshot-settings';
import { snapshotIdentity, snapshotDefaultProject, snapshotDestinationExists, withoutSnapshot, snapshotNoProjectMessage, snapshotFailureMessage } from './snapshot-adopt';
import { bindingId, shortcutInput, whenExpression, validShortcut, validWhen } from './keybinding-settings';
import { validateTaskInput } from './scheduled-settings';
import { scopedSettingPatch } from './scoped-settings';
import { applyCoreSetting, applyDeviceSetting, decodeClientPrefs, type ClientPrefs } from './settings-core';
import { decodeCustomThemes, type CustomTheme } from './settings-themes';
import { restCommand, restLocal } from './settings-rest-commands';
import { chatCommand } from './chat-commands';
import { quitMode } from './palette-native';
import { shellCommand, shellLocal, shellFailure, shellSuccess, settingsFailure } from './shell-commands';
import { trackRpc } from './shell-slow';
import { configEventSideEffects } from './r3-protocol-config';
import { beginRead, traceRpc, statusTicket, settleTraces } from './r3-protocol-reader';
import { compatibilityProblem } from './r3-protocol-outdated';
import { composerNow, type ComposerControlsPrefs, emptyComposerControls, decodeComposerControls, applySticky, applyStaged, stagesChanges, stage, rememberModel, rememberOptions, stagedFor, clearStaged, nextTurnCommands, resolveDispatchMode, followUpBehavior, withDispatchMode, planFollowUp, resolvePlanSubmission } from './composer-controls';
import { additiveGesture, fanoutSelections, sendFanout, setFanout, toggleFanout } from './r3-composer-controls-fanout';
import { composerCommand, composerLocal, acknowledgeWoke, lockedProviderReason, applyOptionChoice, backgroundStarted } from './composer-controls-commands';
import { queuedEdit, saveQueuedEdit } from './composer-controls-queue';
import { fanoutBase, workspaceStrategy } from './composer-controls-branch';
import { isUsageLimitsCommand, usageLimitsOffered, openUsageLimits } from './composer-controls-usage';
import { chatLocal } from './timeline-presentation';
import { editorLocal, withMessageContext } from './composer-editor';
import { sendIntent } from './composer-editor-intent';
import { launchTitle } from './composer-editor-title';
import { promptLengthMessage } from './composer-editor-menu';
import { adoptStash } from './composer-editor-stash';
import { adoptComposerFiles, composerFileAttachments } from './composer-editor-files';
import { startThreadSearch } from './sidebar-presentation';
import { sidebarCommand, sidebarLocal, sidebarSelecting, sidebarOpened, sidebarRefreshed } from './sidebar-commands';
import { reconnectOnLaunch, launchFocus } from './r8-pointer-reconnect';
import { adoptSidebarPrefs } from './sidebar-state';
import { runProviderOp, PROVIDER_OPS } from './providers';
import { runConnectionOp, CONNECTION_OPS } from './connections';
import { fleet, parseFleetThreadId, focusFleetThread } from './settings-b-fleet';
import { groupLabel } from './r6-polish-groups';
import { adoptModelPrefs } from './settings-b-models';
import { settingsBCommand } from './settings-b-commands';
import { type RequestDraft, activeInput, pendingRequests, setCustomAnswer, chooseOption, advanceQuestion, previousQuestion, dismissPayload, approvalPayload, dismissThreadError } from './requests';
import { DiffState, DIFF_LOCAL_OPS, adoptDiff, diffPaths, diffRequest, diffView, selectCheckpoint, selectScope } from './diff';
import { rememberDiffLayout } from './settings-appearance-look';
import { TELEMETRY_KEY, telemetryEvent } from './settings-a-telemetry';
import { threadPhase } from './composer-presentation';
import { heroCarry, heroLand, ensureScratchProject, mostRecentProjectId } from './pages-home';
import { pagesLocal } from './pages-commands';
import { adoptPagesPrefs } from './pages-prefs';
import { adoptShellPrefs } from './shell-prefs';
import { adoptFilesPrefs } from './r5-panels-prefs';
import { requestDiff } from './r11-device-diff';
import { adoptSidebarWidth, storeSidebarWidth } from './r4-polish-sidebar-width'; // r4-polish: the stored sidebar width
import { VCS_STATUS_KEY, vcsStatusEvent } from './shell-vcs';
import { DEVICE_STATE_KEY, deviceStateEvent } from './r4-surfaces-device';
import { WORKTREE_SETUP_KEY, worktreeSetupEvent } from './timeline-worktree';
import { GIT_ACTION_KEY, gitActionEvent } from './r4-git-actions';
import { effectiveWorktreeRules, obj, str, num, arr, initialShell, applyShell, threadSnapshot, applyThread, mergeHistory,
  readyCheckpoint, messages, type Obj, type Shell, type ThreadState } from './domain';
import { ClientError, bridgeReply, parsePairing, activeRun, providerAvailable, modelSelection, sendPayload,
  launchPayload, applyConfig, type Native, type Files } from './protocol';

const localPath = 'app:/data/t3-code.json';
type Selection = { projectId: string; threadId: string };
export type Pending = { method: string; payload: Obj; description: string; threadId: string; text: string; uncertain: boolean };
const uncertainError = (pending: Pending) => `${pending.description} may have reached T3. Check the synchronized thread, then retry only if needed.`;
type Preferences = { selections: Record<string, Selection>; drafts: Record<string, string>; snapshotDrafts: Record<string, Obj[]>; snapshotReleases: string[]; sidebarWidth: number; sidebarOpen: boolean; pending: Record<string, Pending>; favoriteModels: string[]; groupingMode: string; lastGroupingMode?: string; clientSettings: ClientPrefs; customThemes: CustomTheme[]; groupingOverrides: Record<string, string>; composerControls: ComposerControlsPrefs; deviceSettings: { composerCollapseOnScroll: boolean; planModeEnabled: boolean; timestampFormat: string; appearanceMode: string; sendShortcut: string; snapShotShortcut: string; snapShotEnabled: boolean; snapShotIncludeAccessibility: boolean; snapShotPlaySound: boolean; snapShotSound: string; snapShotFlash: boolean; snapShotAnimations: boolean } };
const groupingModes = ['repository', 'repository_path', 'separate'];
const projectPath = (value: unknown) => str(value).trim().replace(/\\/g, '/').replace(/\/+$/, '');
const preferences = (): Preferences => ({ selections: {}, drafts: {}, snapshotDrafts: {}, snapshotReleases: [], sidebarWidth: 256, sidebarOpen: true, pending: {}, favoriteModels: [], groupingMode: 'repository', clientSettings: decodeClientPrefs({}), customThemes: [], groupingOverrides: {}, composerControls: emptyComposerControls(), deviceSettings: { composerCollapseOnScroll: true, planModeEnabled: false, timestampFormat: 'locale', appearanceMode: 'system', sendShortcut: 'enter', snapShotShortcut: 'shift+shift', snapShotEnabled: false, snapShotIncludeAccessibility: true, snapShotPlaySound: true, snapShotSound: 'soft-pop', snapShotFlash: true, snapShotAnimations: true } });
const message = (error: unknown) => error instanceof Error ? error.message : 'The operation failed.';
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
  private shellLoaded = false;
  private synchronizedGeneration = -1;
  private synchronizationEpoch = 0;
  private nextRefreshEpoch = 0;
  private refreshEpoch = 0;
  private commandEpoch = 0;
  private lastEvent = 0;
  private threadEpoch = 0;
  private threadSubscription = '';
  private subscriptions: Record<string, string> = {};
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
  private raw(native: Native, request: unknown) { return bridgeReply(native, request); }
  private async call(native: Native, request: unknown, expected = this.generation, write = false): Promise<Obj> {
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
  private async http(native: Native, path: string, expected = this.generation) {
    return this.call(native, { op: 'http', path }, expected);
  }
  private async request(native: Native, method: string, payload: Obj, expected = this.generation, write = false) {
    return this.call(native, { op: 'request', method, payload }, expected, write);
  }
  private async ids(native: Native, count: number): Promise<string[]> {
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
      adoptStash(next, saved); // composer-editor: the prompt stash (composer-editor-stash.ts)
      adoptComposerFiles(next, saved); // composer-editor: folded pastes (composer-editor-files.ts)
      adoptPagesPrefs(next, saved); // pages: page preferences and the first-run flag (pages-prefs.ts)
      adoptShellPrefs(next, saved); // shell: notice dismissals and closed workspace cards (shell-prefs.ts)
      adoptFilesPrefs(next, saved); // r5-panels: Files explorer and render preferences (r5-panels-prefs.ts)
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
  private async persist(storage: Files): Promise<void> {
    if (this.environmentId) this.local.selections[this.environmentId] = { projectId: this.projectId, threadId: this.threadId };
    // The native adapter serializes writes inside this app's transport queue.
    // A later answer must never await an abandoned answer's promise.
    await storage.fs.atomicWriteFile(localPath, new TextEncoder().encode(JSON.stringify({ version: 1, ...this.local })));
  }
  /** shell-prefs.ts: whether the saved preferences were read, and a save outside a command (a toast's dismissal). */
  get preferencesLoaded(): boolean { return this.loaded; }
  async savePreferences(storage: Files): Promise<void> { await this.save(storage); }
  private async save(storage: Files): Promise<void> {
    try { await this.persist(storage); } catch { this.error = 'Could not save local drafts and preferences. Keep a copy before closing.'; }
  }

  // r13-store F5: Remove forgets the focused environment's address, selection and cached threads
  // (reference ConnectionsSettings.tsx:2600-2640: the pairing, credentials and cached threads go).
  private dropFocus(environmentId: string): void {
    delete this.local.selections[environmentId];
    this.origin = DEFAULT_ORIGIN; this.environmentId = ''; this.projectId = ''; this.threadId = '';
    this.shell = initialShell(); this.shellLoaded = false; this.thread = null;
    this.config = {}; this.providerId = ''; this.modelId = ''; this.answers = {};
    this.diffOpen = false; this.diffText = ''; this.threadEpoch++;
  }

  private adoptStatus(value: Obj, generation: number): void {
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

  private async synchronize(native: Native): Promise<void> {
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

  private ensureSelection(): void {
    const selected = this.shell.threads.find(thread => thread.id === this.threadId);
    if (selected) this.projectId = str(selected.projectId);
    if (!this.shell.projects.some(project => project.id === this.projectId)) this.projectId = mostRecentProjectId(this.shell);
  }
  private chooseDefaults(): void {
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

  private async openThread(native: Native, id: string, resume = false): Promise<void> {
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
  private reconcilePending(): boolean {
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
  private async write(native: Native, storage: Files, pending: Pending, beforeRequest?: () => void): Promise<Obj> {
    const generation = this.generation;
    const environmentId = this.environmentId;
    this.local.pending[environmentId] = pending;
    try { await this.persist(storage); }
    catch {
      delete this.local.pending[environmentId];
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
  private async dispatch(native: Native, storage: Files, payload: Obj, description: string, beforeRequest?: () => void): Promise<Obj> {
    return this.write(native, storage, { method: 'orchestration.dispatchCommand', payload, description,
      threadId: str(payload.threadId), text: str(payload.text), uncertain: false }, beforeRequest);
  }

  async command(op: string, id: string, value: string, n: number, native: Native | null | undefined, storage: Files): Promise<{ revision: number; message: string }> {
    if (!native?.available) return { revision: ++this.revision, message: 'Open this app on macOS to connect to T3 Code.' };
    const local = ['draft', 'answer', 'choice', 'previous-question', 'search', 'sidebar', 'dismiss-error', 'close-diff', 'favorite-model', 'copy-message', 'grouping-mode', 'grouping-override', 'device-setting', 'setting-snapshot', 'remove-snapshot', 'snapshot-preview-sound', 'snapshot-shortcut-record', 'snapshot-shortcut-save', 'copy-diagnostic'].includes(op) || op.startsWith('restlocal:') || op.startsWith('chatlocal:') || op.startsWith('editorlocal:') || op.startsWith('shelllocal:') || op.startsWith('cclocal:') || op.startsWith('sidebarlocal:') || op.startsWith('pageslocal:') || DIFF_LOCAL_OPS.includes(op);
    const epoch = local ? this.commandEpoch : ++this.commandEpoch;
    if (!local) {
      if (this.busy && this.pending) this.pending.uncertain = true;
      this.busy = true;
      this.historyLoading = false; this.diffLoading = false;
      native = this.ownedNative(native, () => epoch === this.commandEpoch);
    }
    let resultMessage = '';
    try {
      await this.load(storage);
      await this.raw(native, { op: 'devicePresentation', ...this.local.deviceSettings, confirmQuit: quitMode(this.local) });
      if (op === 'connect' || op === 'reconnect') {
        const target = parsePairing(id || this.origin, value);
        // Retry without a new credential probes a live socket instead of replacing it (T3Transport retryNow, 9333509).
        const response = await this.raw(native, { op: op === 'reconnect' && !target.credential ? 'retry' : 'connect', ...target });
        if (!response.ok) throw new ClientError(response.error!.message, response.error!.kind);
        this.adoptStatus(obj(response.value), response.generation);
        this.error = '';
      } else if (op === 'disconnect' || op === 'forget') {
        const focused = this.environmentId;
        const response = await this.raw(native, { op: 'disconnect', forget: op === 'forget' });
        if (response.ok) this.adoptStatus(obj(response.value), response.generation);
        if (response.ok && op === 'forget' && !str(obj(response.value).origin)) this.dropFocus(focused);
      } else if (CONNECTION_OPS.includes(op)) {
        const focused = op === 'environment-forget' && value === this.environmentId ? value : '';
        const result = await runConnectionOp(native, op, id, value, this.connection === 'connected', this);
        if (result.status) this.adoptStatus(result.status, result.generation);
        if (focused && result.status && !str(result.status.origin)) this.dropFocus(focused);
        this.error = '';
      } else if (op === 'draft') {
        if (value.length > 1_000_000) throw new ClientError('Keep a draft under 1,000,000 characters.');
        if (!setCustomAnswer(this, value)) this.local.drafts[this.draftKey] = value;
       } else if (op === 'favorite-model') {
        const key = JSON.stringify([id, value]);
        if (!arr(this.config.providers).some(provider => provider.instanceId === id && arr(provider.models).some(model => model.slug === value))) {
          throw new ClientError('That model is no longer advertised by T3.');
        }
        this.local.favoriteModels = this.local.favoriteModels.includes(key)
          ? this.local.favoriteModels.filter(entry => entry !== key) : [...this.local.favoriteModels, key].slice(-200);
      } else if (op === 'copy-message') {
        const message = messages(this.thread).find(message => message.id === id && ['user', 'assistant', 'plan'].includes(message.kind));
        if (!message) throw new ClientError('That message is no longer available.');
        await this.call(native, { op: 'copyText', text: message.body });
        resultMessage = 'Copied message';
      } else if (op === 'copy-diagnostic') {
        if (!value || value.length > 256) throw new ClientError('That trace ID is unavailable.');
        await this.call(native, { op: 'copyText', text: value });
        resultMessage = 'Copied trace ID';
      } else if (op === 'grouping-mode') {
        if (!groupingModes.includes(value)) throw new ClientError('Unsupported grouping mode.');
        this.local.groupingMode = value;
      } else if (op === 'device-setting') {
        if (!applyDeviceSetting(this.local, id, value)) throw new ClientError('Unsupported device setting.');
      } else if (op === 'remove-snapshot') {
        if (this.pending) throw new ClientError('Resolve the pending submission before removing its attachments.');
        const previous = this.snapshotDrafts;
        const image = previous.find(image => image.id === id);
        if (!image) throw new ClientError('That draft attachment is unavailable.');
        const key = this.draftKey, prompt = this.local.drafts[key];
        this.local.snapshotDrafts[key] = previous.filter(image => image.id !== id);
        await removeAttachmentReferences(this, native, key, 'image', id); // r4-composer: every chip goes with it (composerDraftStore removeImage)
        try { await this.persist(storage); } catch (error) { this.local.snapshotDrafts[key] = previous; if (prompt !== undefined) this.local.drafts[key] = prompt; throw error; }
        this.local.snapshotReleases.push(id);
        await this.flushSnapshotReleases(native, storage);
        if (str(image.uploadId)) {
          try { await this.request(native, 'attachments.delete', { attachmentId: image.uploadId }); }
          catch { resultMessage = 'Removed from draft. Server upload cleanup failed; the server will expire unused uploads.'; }
        }
      } else if (op === 'snapshot-shortcut-record') {
        await this.call(native, { op: 'snapshotRecordShortcut', record: value === 'true' });
      } else if (op === 'snapshot-shortcut-save') {
        const shortcut = snapshotShortcut(value);
        if (!shortcut) throw new ClientError('Add a modifier and a letter, number or function key.');
        if (!this.ready) throw new ClientError('Connect to recheck T3 shortcut conflicts before saving.');
        const owner = this.snapshotOwner, generation = this.generation;
        const current = () => { if (owner !== this.snapshotOwner || generation !== this.generation || !this.ready) throw new ClientError('The shortcut scope changed. Reopen settings and try again.'); };
        const config = await this.request(native, 'server.getConfig', {}); current();
        const conflict = snapshotConflict(config, shortcut);
        if (conflict) throw new ClientError(`T3 Code already uses this for "${commandLabel(conflict)}".`);
        const check = await this.call(native, { op: 'snapshotCheckShortcut', shortcut, bindings: arr(config.keybindings).map(binding => ({ key: shortcutInput(obj(binding.shortcut)), command: str(binding.command) })) }); current();
        if (check.available !== true) throw new ClientError(str(check.message) || 'This shortcut is unavailable.');
        const previous = this.local.deviceSettings, settings = { ...previous, snapShotShortcut: shortcut };
        this.local.deviceSettings = settings;
        // Commit the choice before installation. A failed disk write must never
        // replace a working native registration with an unsaved choice.
        try {
          await this.persist(storage); current();
          try { await this.call(native, { op: 'snapshotConfigure', owner, shortcut, enabled: settings.snapShotEnabled, includeAccessibility: settings.snapShotIncludeAccessibility, playSound: settings.snapShotPlaySound, sound: settings.snapShotSound, flash: settings.snapShotFlash, animations: settings.snapShotAnimations }); }
          catch (error) { if (!(error instanceof ClientError) || error.kind !== 'SnapShot' || !isSnapshotPermissionMessage(error.message)) throw error; }
        } catch (error) {
          this.local.deviceSettings = previous;
          try { await this.persist(storage); } catch { throw new ClientError('Could not save the shortcut. Keep settings open and retry; the previous native binding is retained.'); }
          throw error;
        }
        await this.call(native, { op: 'snapshotRecordShortcut', record: false });
      } else if (op === 'snapshot-preview-sound') {
        if (!['soft-pop', 'camera-shutter'].includes(value)) throw new ClientError('Choose a supported sound.');
        const preview = await this.call(native, { op: 'snapshotPlaySound', sound: value });
        if (preview.played !== true) throw new ClientError('The snapshot sound did not play.');
        resultMessage = value === 'soft-pop' ? 'Played Whoosh' : 'Played Click';
      } else if (op === 'setting-snapshot') {
        // Setup wizard (reference setupSnapShot): Allow opens System Settings;
        // continue runs a discarded test capture, then turns capture on.
        const setup = id === 'setup' && ['allow-screen-recording', 'allow-accessibility', 'continue'].includes(value);
        if (setup) await this.call(native, { op: 'snapshotSetup', action: value === 'continue' ? 'test-mac-capture' : value });
        if (setup && value !== 'continue') resultMessage = '';
        else {
        if (setup) { id = 'snapShotEnabled'; value = 'true'; }
        if ((!['snapShotEnabled', 'snapShotIncludeAccessibility', 'snapShotPlaySound', 'snapShotFlash', 'snapShotAnimations'].includes(id) || !['true', 'false'].includes(value)) && !(id === 'snapShotSound' && ['soft-pop', 'camera-shutter'].includes(value))) throw new ClientError('Unsupported snapshot setting.');
        const settings = { ...this.local.deviceSettings, [id]: id === 'snapShotSound' ? value : value === 'true', ...(id === 'snapShotSound' ? { snapShotPlaySound: true } : {}) };
        try { await this.call(native, { op: 'snapshotConfigure', owner: this.snapshotOwner, shortcut: settings.snapShotShortcut, enabled: settings.snapShotEnabled, includeAccessibility: settings.snapShotIncludeAccessibility, playSound: settings.snapShotPlaySound, sound: settings.snapShotSound, flash: settings.snapShotFlash, animations: settings.snapShotAnimations }); }
        catch (error) {
          const feedbackChoice = ['snapShotPlaySound', 'snapShotSound', 'snapShotFlash', 'snapShotAnimations'].includes(id);
          if (!feedbackChoice || !(error instanceof ClientError) || error.kind !== 'SnapShot' || !isSnapshotPermissionMessage(error.message)) throw error;
        }
        this.local.deviceSettings = settings;
        }
      } else if (op === 'grouping-override') {
        const project = this.shell.projects.find(project => project.id === id);
        if (!project) throw new ClientError('That project is no longer available.');
        if (!groupingModes.includes(value) && value !== 'inherit') throw new ClientError('Unsupported grouping mode.');
        const key = `${this.environmentId}:${projectPath(project.workspaceRoot)}`;
        if (value === 'inherit') delete this.local.groupingOverrides[key];
        else this.local.groupingOverrides[key] = value;
      } else if (op.startsWith('restlocal:')) { resultMessage = await restLocal(this, native, storage, op.slice(10), id, value);
      } else if (op.startsWith('chatlocal:')) { resultMessage = await chatLocal(this, native, op.slice(10), id, value, storage);
      } else if (op.startsWith('pageslocal:')) { resultMessage = await pagesLocal(this, native, storage, op.slice(11), id, value);
      } else if (op.startsWith('editorlocal:')) { resultMessage = await editorLocal(this, native, op.slice(12), id, value, n);
      } else if (op.startsWith('shelllocal:')) { resultMessage = await shellLocal(this, native, op.slice(11), id, value);
      } else if (op.startsWith('cclocal:')) { resultMessage = await composerLocal(this, native, storage, op.slice(8), id, value);
      } else if (op.startsWith('sidebarlocal:')) { resultMessage = await sidebarLocal(this, native, op.slice(13), id, value);
      } else if (op === 'search') { this.query = value; startThreadSearch(this, native, value); }
      else if (op === 'sidebar') {
        if (n) { this.local.sidebarWidth = Math.min(4096, Math.max(208, n)); storeSidebarWidth(this); } // a drag ended (the toggle sends 0)
        if (value === 'open' || value === 'closed') this.local.sidebarOpen = value === 'open';
        else if (!n) this.local.sidebarOpen = !this.local.sidebarOpen;
      } else if (op === 'dismiss-error') { dismissThreadError(this); this.error = ''; }
      else if (op === 'close-diff') { this.diffOpen = false; this.diffLoading = false; }
      else if (op === 'diff-view' && id === 'copy') { await this.call(native, { op: 'copyText', text: value }); resultMessage = 'Copied file path'; }
      else if (op === 'diff-view') { diffView(this, id, value, diffPaths(this)); if (id === 'layout') rememberDiffLayout(this, value); }
      else if (op === 'select-project' || op === 'new-thread' || op.startsWith('pages:hero-')) {
        const carry = op.startsWith('pages:hero-') ? await heroCarry(this, op.slice(11), id, () => ensureScratchProject(this, native!)) : null;
        if (carry) id = carry.projectId;
        if (id && this.shell.projects.some(project => project.id === id)) this.projectId = id;
        this.threadId = ''; this.thread = null; this.threadSubscription = ''; this.threadEpoch++;
        delete this.subscriptions.thread;
        this.threadLive = true; this.diffOpen = false; this.answers = {}; this.chooseDefaults();
        if (this.connection === 'connected') await this.call(native, { op: 'unsubscribe', key: 'thread' });
        if (carry) heroLand(this, carry);
      } else if (op === 'select-thread') {
        if (parseFleetThreadId(id)) { const focused = await focusFleetThread(this, native, id); this.adoptStatus(focused.value, focused.generation); } // settings-b: another environment
        else if (!(await sidebarSelecting(this, native, id, value))) await this.openSelected(native, id);
      } else if (op === 'refresh') {
        this.shellLoaded = false; this.thread = null; this.error = '';
        await this.synchronize(native);
      } else if (op === 'history') await this.history(native);
      else if (['diff', 'checkpoint-diff', 'diff-scope', 'diff-refresh', 'diff-whitespace'].includes(op)) await this.diff(native, op, id, value, n);
      else if (op === 'answer') setCustomAnswer(this, value);
      else if (op === 'choice') chooseOption(this, id, value, text => this.carryAnswer(text));
      else if (op === 'previous-question') previousQuestion(this);
      else if (op === 'retry') {
        if (!this.writable) throw new ClientError('Wait for a writable connection to synchronize before retrying.');
        if (this.reconcilePending()) resultMessage = 'T3 already accepted the operation.';
        else if (this.pending) {
          const pending = this.pending;
          const result = await this.write(native, storage, { ...pending, uncertain: false });
          if (pending.method === 'orchestration.launchThread') {
            this.threadId = str(result.threadId, str(pending.payload.threadId));
            await this.openThread(native, this.threadId);
          }
        }
      } else if (op === 'settings-core') resultMessage = await applyCoreSetting(this, native, id, value);
      else {
        this.requireWrite();
        if (op === 'fork-message') {
          const source = messages(this.thread).find(message => message.id === id && message.kind === 'assistant');
          if (!source?.runId || !source.completed) {
            throw new ClientError('Only a completed response can be forked.');
          }
          const [commandId, targetThreadId] = await this.ids(native, 2);
          await this.dispatch(native, storage, { type: 'thread.fork', commandId, createdBy: 'user', creationSource: 'web',
            sourceThreadId: source.sourceThreadId || this.threadId, targetThreadId,
            sourcePoint: { type: 'run', runId: source.runId }, title: `${str(obj(this.projection.thread).title, 'Thread')} fork` }, 'Fork response');
          this.threadId = targetThreadId;
          await this.openThread(native, targetThreadId);
        } else if (op === 'send' && pendingRequests(this.projection).approvals.length) throw new ClientError('Resolve this approval request to continue.');
        else if (op === 'send' && activeInput(this)) await this.submitAnswers(native, storage, '', value);
        else if (op === 'send') await this.send(native, storage, value);
        else if (op === 'add-project') await this.addProject(native, storage, id, value);
        else if (op === 'rename-project' || op === 'remove-project') await this.manageProject(native, storage, op, id, value);
        else if (op === 'keybinding-save' || op === 'keybinding-remove') await this.manageKeybinding(native, id, value, op);
        else if (['task-save', 'task-toggle', 'task-delete', 'task-run'].includes(op)) await this.manageScheduledTask(native, id, value, op);
        else if (op === 'setting-scoped') await this.setScopedSetting(native, id, value);
        else if (op === 'setting-storage') await this.setStorageSettings(native, id, value);
        else if (op === 'unarchive-thread' || op === 'delete-archived-thread') await this.manageArchivedThread(native, storage, op, id);
        else if (op === 'setting-model') await this.setModelDefault(native, id, value);
        else if (op === 'setting-permissions') await this.setPermissionDefault(native, id, value);
        else if (op === 'rename-group' || op === 'remove-group') await this.manageGroup(native, storage, op, id, value);
        else if (op === 'remove-group-member') {
          this.shell = applyShell(this.shell, await this.http(native, '/api/orchestration/shell'));
          if (!this.projectGroups().some(group => group.key === value && group.members.some(member => member.id === id))) throw new ClientError('Project group membership changed. Reopen its settings.');
          await this.manageProject(native, storage, 'remove-project', id, '');
        }
        else if (PROVIDER_OPS.includes(op)) { resultMessage = await runProviderOp(this, native, op, id, value); if (!this.threadId) this.chooseDefaults(); this.error = ''; }
        else if (op === 'provider' || op === 'model') await this.changeModel(native, storage, op, id, value);
        else if (op === 'model-option') await this.changeModelOption(native, storage, id, value);
        else if (op === 'runtime' || op === 'interaction') await this.changeMode(native, storage, op, value);
        else if (op === 'unsettle') {
          const [commandId] = await this.ids(native, 1);
          await this.dispatch(native, storage, { type: 'thread.unsettle', commandId, threadId: this.threadId, reason: 'user' }, 'Un-settle thread');
        }
        else if (op === 'stop') {
          const run = activeRun(this.projection);
          if (!run) throw new ClientError('There is no active turn to stop.');
          const [commandId] = await this.ids(native, 1);
          await this.dispatch(native, storage, { type: 'run.interrupt', commandId, threadId: this.threadId, runId: str(run.id), holdQueue: true }, 'Stop');
        } else if (op === 'approval') await this.approve(native, storage, id, value);
        else if (op === 'submit-answers' || op === 'advance-question') await this.submitAnswers(native, storage, id);
        else if (op === 'pick-option') { if (chooseOption(this, id, value, text => this.carryAnswer(text))) await this.submitAnswers(native, storage, id.split('::')[0]!); }
        else if (op === 'dismiss-question') await this.dispatchRequest(native, storage, dismissPayload(this, id), 'Dismiss question');
        else if (op.startsWith('rest:')) resultMessage = await restCommand(this, native, storage, op.slice(5), id, value);
        else if (op.startsWith('chat:')) resultMessage = await chatCommand(this, native, storage, op.slice(5), id, value);
        else if (op.startsWith('shell:')) resultMessage = await shellCommand(this, native, storage, op.slice(6), id, value);
        else if (op.startsWith('sidebar:')) resultMessage = await sidebarCommand(this, native, storage, op.slice(8), id, value, n);
        else if (op.startsWith('cc:')) resultMessage = await composerCommand(this, native, storage, op.slice(3), id, value);
        else if (op.startsWith('sb:')) resultMessage = await settingsBCommand(this, native, op.slice(3), id, value, storage); // settings-b-commands.ts
        else throw new ClientError(`Unknown action: ${op}`);
      }
      if ((op === 'device-setting' || op === 'settings-core') && native?.available) await this.raw(native, { op: 'devicePresentation', ...this.local.deviceSettings, confirmQuit: quitMode(this.local) });
      await this.save(storage);
      await this.flushSnapshotReleases(native, storage);
      shellSuccess(this, op, value, resultMessage, id);
    } catch (error) {
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

  private async send(native: Native, storage: Files, value: string): Promise<void> {
    const selection = { generation: this.generation, environmentId: this.environmentId, origin: this.origin, projectId: this.projectId, threadId: this.threadId, providerId: this.providerId, modelId: this.modelId, options: JSON.stringify(this.modelOptions), runtimeMode: this.runtimeMode, interactionMode: this.interactionMode };
    const assertOwner = () => {
      if (selection.generation !== this.generation || selection.environmentId !== this.environmentId || selection.origin !== this.origin || selection.projectId !== this.projectId || selection.threadId !== this.threadId || selection.providerId !== this.providerId || selection.modelId !== this.modelId || selection.options !== JSON.stringify(this.modelOptions) || selection.runtimeMode !== this.runtimeMode || selection.interactionMode !== this.interactionMode) throw new ClientError('The draft or model changed before sending. Your original draft is preserved.');
    };
    if (queuedEdit(this)) return saveQueuedEdit(this, native, storage, value);
    // "/usage-limits" is answered locally from the provider snapshots; the agent never sees it.
    if (isUsageLimitsCommand(value || this.draft) && !this.snapshotDrafts.length && usageLimitsOffered(this)) { if (openUsageLimits(this, composerNow(this))) this.local.drafts[this.draftKey] = ''; return; }
    const plan = planFollowUp(this);
    const submission = plan ? resolvePlanSubmission(value || this.draft, plan.markdown) : null;
    const text = submission ? submission.text : value || this.draft;
    if (!text.trim() && !this.snapshotDrafts.length) throw new ClientError('Write a message or attach an image first.');
    if (promptLengthMessage(text)) throw new ClientError(promptLengthMessage(text));
    if (!this.projectId) throw new ClientError('Choose or add a project first.');
    if (!submission || submission.interactionMode === 'plan') this.local.drafts[this.draftKey] = text;
    const gesture = await this.call(native, { op: 'composerSendIntent' }).catch(() => ({}));
    const running = !!selection.threadId && threadPhase(this.projection) === 'running';
    const intent = sendIntent(this.config, gesture, running, !selection.threadId);
    const provider = arr(this.config.providers).find(provider => provider.instanceId === this.providerId);
    if (!provider || !providerAvailable(provider)) throw new ClientError('This provider is unavailable. Configure it in T3 Code.');
    if (!arr(provider.models).some(model => model.slug === this.modelId)) throw new ClientError('Choose one of the models advertised by T3.');
    if (Array.isArray(provider.supportedRuntimeModes) && !provider.supportedRuntimeModes.includes(this.runtimeMode)) {
      throw new ClientError('Choose a permission mode supported by this provider.');
    }
    const attachments = [...await this.uploadSnapshots(native, storage), ...await composerFileAttachments(this, native, text)]; // + folded pastes (composer-editor-files.ts)
    assertOwner();
    const [commandId, messageId, freshThreadId] = await this.ids(native, 3), launchKey = this.draftKey, threadId = selection.threadId ? freshThreadId : launchThreadId(this, launchKey, freshThreadId); // r7-handoff: a draft launches as its own id
    assertOwner();
    if (selection.threadId) {
      const key = this.draftKey, staged = stagedFor(this);
      for (const command of nextTurnCommands(obj(this.projection.thread), staged, submission?.interactionMode ?? '')) {
        const [modeCommandId] = await this.ids(native, 1);
        assertOwner();
        await this.dispatch(native, storage, { ...command, commandId: modeCommandId, threadId: selection.threadId }, 'Change mode', assertOwner);
      }
      const mode = submission ? 'auto' : resolveDispatchMode(running, followUpBehavior(this), intent === 'alternate');
      const payload = withDispatchMode(withMessageContext(this, sendPayload(commandId, selection.threadId, messageId, text, attachments), text), mode,
        modelSelection(selection.providerId, selection.modelId, JSON.parse(selection.options)));
      if (submission?.interactionMode === 'default' && plan) payload.sourcePlanRef = { threadId: selection.threadId, planId: plan.planId };
      await this.dispatch(native, storage, payload, 'Send', assertOwner);
      clearStaged(this, key);
      if (submission) this.interactionMode = submission.interactionMode;
      await acknowledgeWoke(this, selection.threadId, native);
      // composer.sendAndNewThread: the sent thread keeps running; a fresh new-thread composer opens in its project.
      if (intent === 'background' && this.threadId === selection.threadId) await this.openFreshDraft(native);
    } else if (fanoutSelections(this)) {
      // Several models: one background thread each, in its own worktree (r3-composer-controls-fanout.ts).
      await sendFanout(this, native, storage, { text, attachments, ...fanoutBase(this), runtimeMode: selection.runtimeMode, interactionMode: selection.interactionMode });
    } else {
      const payload = launchPayload(commandId, threadId, messageId, selection.projectId, text,
        modelSelection(selection.providerId, selection.modelId, JSON.parse(selection.options)), selection.runtimeMode, selection.interactionMode, attachments);
      payload.workspaceStrategy = workspaceStrategy(this);
      payload.title = launchTitle(text, str(this.snapshotDrafts[0]?.name), str(attachments.find(attachment => attachment.type === 'file')?.name)); // composer-editor-title.ts
      const result = await this.write(native, storage, { method: 'orchestration.launchThread', payload: withMessageContext(this, payload, text),
        description: 'Create thread', threadId, text, uncertain: false }, assertOwner);
      forgetDraftThreadId(this, launchKey);
      assertOwner();
      // composer.sendBackground: the thread starts out of view and a fresh draft stays open.
      if (intent === 'background') { backgroundStarted(this, str(result.threadId, threadId)); return; }
      this.threadId = str(result.threadId, threadId);
      await this.openThread(native, this.threadId);
    }
  }
  /** handleNewThreadInActiveProject: the 'new-thread' op's switch to a draft in the current project. */
  /** Lane r6-pr: a pull request hand-off opens the project's draft (useNewThreadHandler) from inside its command. */
  async openProjectDraft(native: Native, projectId: string): Promise<void> { this.projectId = projectId; await this.openFreshDraft(native); }
  private async openFreshDraft(native: Native): Promise<void> {
    this.threadId = ''; this.thread = null; this.threadSubscription = ''; this.threadEpoch++;
    delete this.subscriptions.thread;
    this.threadLive = true; this.diffOpen = false; this.answers = {}; this.chooseDefaults();
    if (this.connection === 'connected') await this.call(native, { op: 'unsubscribe', key: 'thread' });
  }
  private async addProject(native: Native, storage: Files, path: string, title: string): Promise<void> {
    if (!path.trim()) throw new ClientError('Enter the project folder on the T3 server.');
    const [commandId, projectId] = await this.ids(native, 2);
    await this.write(native, storage, { method: 'projects.mutate', description: 'Add project', threadId: '', text: '', uncertain: false,
      payload: { type: 'project.create', commandId, projectId, title: title.trim() || path.trim().replace(/\/$/, '').split('/').pop() || 'Project', workspaceRoot: path.trim(), createWorkspaceRootIfMissing: false } });
    this.shell = applyShell(initialShell(), await this.http(native, '/api/orchestration/shell'));
    if (!this.shell.projects.some(project => project.id === projectId)) throw new ClientError('The project was created but is not yet available. Refresh before selecting it.');
    this.projectId = projectId; this.threadId = ''; this.thread = null; this.threadLive = true; this.chooseDefaults();
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
  private async manageGroup(native: Native, storage: Files, op: string, key: string, title: string): Promise<void> {
    // Resolve the current server members, never a cached list from a dialog.
    this.shell = applyShell(this.shell, await this.http(native, '/api/orchestration/shell'));
    const group = this.projectGroups().find(group => group.key === key);
    if (!group) throw new ClientError('That project group is no longer available.');
    if (op === 'rename-group' && !title.trim()) throw new ClientError('Project title cannot be empty');
    for (const member of group.members) {
      if (!this.projectGroups().some(group => group.key === key && group.members.some(current => current.id === member.id))) {
        throw new ClientError('Project group membership changed. Reopen its settings.');
      }
      try { await this.manageProject(native, storage, op === 'rename-group' ? 'rename-project' : 'remove-project', str(member.id), title); }
      catch (error) { throw new ClientError(`Could not ${op === 'rename-group' ? 'rename' : 'remove'} checkout ${str(member.workspaceRoot)}: ${message(error)}`); }
    }
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
    if (!this.shell.threads.some(thread => thread.id === id)) throw new ClientError('That thread is no longer available.');
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
  private async flushSnapshotReleases(native: Native, storage: Files): Promise<void> {
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
  private async uploadSnapshots(native: Native, storage: Files): Promise<Obj[]> {
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
  private async manageArchivedThread(native: Native, storage: Files, op: string, scope: string): Promise<void> {
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
  private async manageKeybinding(native: Native, environmentId: string, text: string, op: string): Promise<void> {
    if (!environmentId || environmentId !== this.environmentId) throw new ClientError('That environment is no longer selected.');
    const input = obj(JSON.parse(text)), config = await this.request(native, 'server.getConfig', {});
    const previous = str(input.previous) ? arr(config.keybindings).find(binding => bindingId(binding) === input.previous) : undefined;
    if ((op === 'keybinding-remove' || input.previous) && !previous) throw new ClientError('That keybinding is no longer available.');
    const target = previous ? { key: shortcutInput(obj(previous.shortcut)), command: str(previous.command), ...(previous.whenAst ? { when: whenExpression(previous.whenAst) } : {}) } : {};
    let payload: Obj;
    if (op === 'keybinding-remove') payload = target;
    else {
      const key = str(input.key).trim(), command = str(input.command).trim(), when = str(input.when).trim();
      if (!key || key.length > 64 || !command || when.length > 256) throw new ClientError('Enter a command, shortcut (up to 64 characters) and valid condition (up to 256 characters).');
      if (!validShortcut(key)) throw new ClientError('Enter one key with supported shortcut modifiers.');
      if (!validWhen(when)) throw new ClientError('Enter a valid shortcut condition.');
      payload = { key, command, ...(when ? { when } : {}), ...(previous ? { replace: target } : {}) };
    }
    const result = await this.request(native, op === 'keybinding-remove' ? 'server.removeKeybinding' : 'server.upsertKeybinding', payload, this.generation, true);
    this.config = { ...this.config, keybindings: result.keybindings };
  }
  private async manageScheduledTask(native: Native, scope: string, text: string, op: string): Promise<void> {
    const [environmentId, projectScope, extra] = scope.split(':');
    if (extra !== undefined || environmentId !== this.environmentId) throw new ClientError('That environment is no longer selected.');
    const shell = applyShell(initialShell(), await this.http(native, '/api/orchestration/shell'));
    if (projectScope && !shell.projects.some(project => project.id === projectScope)) throw new ClientError('That checkout is no longer available.');
    const input = obj(JSON.parse(text)), tasks = arr((await this.request(native, 'scheduledTasks.list', {})).tasks);
    const task = tasks.find(task => task.id === input.id);
    if ((op !== 'task-save' || input.id) && (!task || (projectScope && task.projectId !== projectScope))) throw new ClientError('This scheduled task no longer exists in the selected scope.');
    let payload: Obj, method: string;
    if (op === 'task-save') {
      payload = validateTaskInput(input); method = 'scheduledTasks.upsert';
      if (input.id) payload.requireExisting = true;
      if (!shell.projects.some(project => project.id === payload.projectId) || (projectScope && payload.projectId !== projectScope)) throw new ClientError('Choose an existing project in this scope.');
      if (payload.threadId && !shell.threads.some(thread => thread.id === payload.threadId && thread.projectId === payload.projectId)) throw new ClientError('Choose a thread belonging to the selected project.');
      const selection = obj(payload.modelSelection), config = await this.request(native, 'server.getConfig', {});
      const provider = arr(config.providers).find(provider => provider.instanceId === selection.instanceId && providerAvailable(provider));
      if (!provider || !arr(provider.models).some(model => model.slug === selection.model && model.isUnavailable !== true)) throw new ClientError('Choose an available provider and model.');
      // Edits retain canonical provider options when the selection is unchanged.
      const original = obj(task?.modelSelection);
      if (original.instanceId === selection.instanceId && original.model === selection.model) payload.modelSelection = original;
    } else {
      method = op === 'task-toggle' ? 'scheduledTasks.setEnabled' : op === 'task-delete' ? 'scheduledTasks.delete' : 'scheduledTasks.runNow';
      payload = { id: str(input.id), ...(op === 'task-toggle' ? { enabled: input.enabled === true } : {}) };
    }
    await this.request(native, method, payload, this.generation, true);
  }
  private async setScopedSetting(native: Native, scope: string, text: string): Promise<void> {
    const [environmentId, projectId, extra] = scope.split(':');
    if (extra !== undefined || !environmentId || environmentId !== this.environmentId) throw new ClientError('That environment is no longer selected.');
    const shell = applyShell(initialShell(), await this.http(native, '/api/orchestration/shell'));
    if (projectId && !shell.projects.some(project => project.id === projectId)) throw new ClientError('That checkout is no longer available.');
    const input = obj(JSON.parse(text));
    const config = await this.request(native, 'server.getConfig', {});
    const capabilities = obj(obj(config.environment).capabilities);
    if (projectId && capabilities.projectSettingsOverrides !== true) throw new ClientError('Update the selected environment to configure project overrides.');
    const settings = await this.request(native, 'server.getSettings', {});
    const root = str(input.key).split('.')[0];
    if (!(root in settings)) throw new ClientError('Update the selected environment to configure this setting.');
    if (root === 'continueThreadsAfterServerUpdate' && capabilities.threadRestartContinuation !== true) throw new ClientError('Update the selected environment to configure restart continuation.');
    if (root === 'enableAgentDeviceAccess' && settings.enableDeviceSupport !== true) throw new ClientError('Enable the device hub before changing agent device access.');
    const patch = scopedSettingPatch(settings, projectId, str(input.key), input.value, input.inherit === true);
    const updated = await this.request(native, 'server.updateSettings', { patch }, this.generation, true);
    this.config = { ...this.config, settings: updated };
  }
  private async setStorageSettings(native: Native, scope: string, text: string): Promise<void> {
    const [environmentId, projectId, extra] = scope.split(':');
    if (extra !== undefined || environmentId !== this.environmentId) throw new ClientError('That environment is no longer selected.');
    const shell = applyShell(initialShell(), await this.http(native, '/api/orchestration/shell'));
    if (projectId && !shell.projects.some(project => project.id === projectId)) throw new ClientError('That checkout is no longer available.');
    const capabilities = obj(obj((await this.request(native, 'server.getConfig', {})).environment).capabilities);
    if (capabilities.storageCleanup !== true || (projectId && capabilities.projectWorktreeCleanup !== true)) throw new ClientError('Update the selected environment to configure storage cleanup.');
    const input = obj(JSON.parse(text)), key = str(input.key), value = input.value;
    const booleans = ['worktreeOnDelete', 'worktreeOnMerge', 'worktreeUnchanged'];
    const retention = ['worktreeAfterDays', 'browserArtifactsAfterDays', 'logsAfterDays'];
    if (key === 'mode') {
      if (!projectId || !['inherit', 'off', 'custom'].includes(str(value))) throw new ClientError('Unsupported worktree cleanup mode.');
    } else if (booleans.includes(key)) {
      if (typeof value !== 'boolean') throw new ClientError('Choose On or Off.');
    } else if (retention.includes(key)) {
      if (value !== null && (typeof value !== 'number' || !Number.isInteger(value) || value < 1 || value > 3650)) throw new ClientError('Retention must be between 1 and 3650 days.');
    } else throw new ClientError('Unsupported storage rule.');
    if (projectId && !['mode', ...booleans, 'worktreeAfterDays'].includes(key)) throw new ClientError('Artifact and log retention belongs to the environment.');
    const settings = await this.request(native, 'server.getSettings', {});
    let patch: Obj;
    if (projectId) {
      const current = { ...obj(obj(settings.projectSettingsOverrides)[projectId]) };
      const worktree = obj(current.worktreeCleanup);
      if (key === 'mode') {
        if (value === 'inherit') delete current.worktreeCleanup;
        else current.worktreeCleanup = value === 'off' ? { mode: 'off' } : { mode: 'custom', rules: effectiveWorktreeRules(settings, current) };
      } else {
        if (worktree.mode !== 'custom') throw new ClientError('Choose Custom before changing project cleanup rules.');
        current.worktreeCleanup = { mode: 'custom', rules: { ...effectiveWorktreeRules(settings, current), [key]: value } };
      }
      patch = { projectSettingsOverrides: { [projectId]: Object.keys(current).length ? current : null } };
    } else patch = { storageCleanup: { ...obj(settings.storageCleanup), [key]: value } };
    const result = await this.request(native, 'server.updateSettings', { patch }, this.generation, true);
    this.config = { ...this.config, settings: result }; this.error = '';
  }
  private async setModelDefault(native: Native, scope: string, value: string): Promise<void> {
    const separator = scope.indexOf(':');
    const environmentId = scope.slice(0, separator), projectId = scope.slice(separator + 1);
    if (separator < 0 || environmentId !== this.environmentId) throw new ClientError('That environment is no longer selected.');
    if (!['current', 'inherit', 'automatic'].includes(value) || (!projectId && value === 'inherit') || (projectId && value === 'automatic')) throw new ClientError('Unsupported model default action.');
    const shell = applyShell(initialShell(), await this.http(native, '/api/orchestration/shell'));
    if (projectId && !shell.projects.some(project => project.id === projectId)) throw new ClientError('That project is no longer available.');
    // Validate the advertised catalog again; a removed/disabled provider must not
    // become a new default from a stale chat selection.
    const config = await this.request(native, 'server.getConfig', {});
    const provider = arr(config.providers).find(provider => provider.instanceId === this.providerId && providerAvailable(provider));
    if (value === 'current' && (!provider || !arr(provider.models).some(model => model.slug === this.modelId))) throw new ClientError('The selected model is no longer available.');
    const settings = await this.request(native, 'server.getSettings', {});
    const selection = { instanceId: this.providerId, model: this.modelId, options: this.modelOptions };
    let patch: Obj = { defaultModelSelection: value === 'automatic' ? null : selection };
    if (projectId) {
      const current = { ...obj(obj(settings.projectSettingsOverrides)[projectId]) };
      if (value === 'inherit') delete current.defaultModelSelection;
      else current.defaultModelSelection = selection;
      patch = { projectSettingsOverrides: { [projectId]: Object.keys(current).length ? current : null } };
    }
    const updated = await this.request(native, 'server.updateSettings', { patch }, this.generation, true);
    this.config = { ...this.config, settings: updated };
    if (!this.threadId) this.chooseDefaults();
  }
  private async setPermissionDefault(native: Native, scope: string, value: string): Promise<void> {
    const separator = scope.indexOf(':');
    const environmentId = scope.slice(0, separator), projectId = scope.slice(separator + 1);
    if (separator < 0 || environmentId !== this.environmentId) throw new ClientError('That environment is no longer selected.');
    if (!['approval-required', 'full-access', 'inherit'].includes(value)) throw new ClientError('Unsupported permissions default.');
    if (projectId && !this.shell.projects.some(project => project.id === projectId)) throw new ClientError('That project is no longer available.');
    if (!projectId && value === 'inherit') throw new ClientError('Choose an environment default.');
    const settings = obj(this.config.settings);
    let patch: Obj = { defaultRuntimeMode: value };
    if (projectId) {
      const current = { ...obj(obj(settings.projectSettingsOverrides)[projectId]) };
      if (value === 'inherit') delete current.defaultRuntimeMode;
      else current.defaultRuntimeMode = value;
      patch = { projectSettingsOverrides: { [projectId]: Object.keys(current).length ? current : null } };
    }
    const updated = await this.request(native, 'server.updateSettings', { patch }, this.generation, true);
    this.config = { ...this.config, settings: updated };
    if (!this.threadId) this.chooseDefaults();
    this.error = '';
  }
  private async manageProject(native: Native, storage: Files, op: string, id: string, title: string): Promise<void> {
    const project = this.shell.projects.find(project => project.id === id);
    if (!project) throw new ClientError('That project is no longer available.');
    if (op === 'rename-project' && !title.trim()) throw new ClientError('Project title cannot be empty');
    const removedThreads = this.shell.threads.filter(thread => thread.projectId === id);
    const [commandId] = await this.ids(native, 1);
    await this.write(native, storage, { method: 'projects.mutate', description: op === 'rename-project' ? 'Rename project' : 'Remove project',
      threadId: '', text: '', uncertain: false, payload: op === 'rename-project'
        ? { type: 'project.update', commandId, projectId: id, title: title.trim() }
        : { type: 'project.delete', commandId, projectId: id, force: true } });
    // Refresh canonical records before selection/default resolution. The live
    // event can arrive after the mutation acknowledgment.
    this.shell = applyShell(initialShell(), await this.http(native, '/api/orchestration/shell'));
    if (op === 'remove-project') {
      for (const [key, selection] of Object.entries(this.local.selections)) {
        if (key === this.environmentId && selection.projectId === id) delete this.local.selections[key];
      }
      for (const thread of removedThreads) {
        if (thread.projectId === id) delete this.local.drafts[`${this.environmentId}:${thread.id}`];
      }
      delete this.local.drafts[`${this.environmentId}:new:${id}`];
      if (this.projectId === id) {
        this.threadId = ''; this.thread = null; this.threadSubscription = ''; this.threadEpoch++;
        delete this.subscriptions.thread;
        await this.call(native, { op: 'unsubscribe', key: 'thread' });
        this.projectId = str(this.shell.projects[0]?.id); this.threadLive = true; this.chooseDefaults();
      }
    }
    this.ensureSelection(); this.error = '';
  }
  private async changeModel(native: Native, storage: Files, op: string, id: string, instance = ''): Promise<void> {
    const providerId = op === 'provider' ? id : instance || this.providerId;
    const provider = arr(this.config.providers).find(provider => provider.instanceId === providerId);
    if (!provider || !providerAvailable(provider)) throw new ClientError('This provider is unavailable. Configure it in T3 Code.');
    const models = arr(provider.models);
    const modelId = op === 'model' ? id : str(models.find(model => model.isDefault === true)?.slug || models[0]?.slug);
    if (!models.some(model => model.slug === modelId)) throw new ClientError('That model is no longer advertised by T3.');
    if (this.threadId && provider.requiresNewThreadForModelChange === true && (providerId !== this.providerId || modelId !== this.modelId)) {
      throw new ClientError('Start a new thread to change this model.');
    }
    const locked = lockedProviderReason(this, providerId);
    if (locked) throw new ClientError(locked);
    // A draft's Shift-click (or Shift+Return) adds the model to a multi-model fan-out; a plain pick ends it.
    if (op === 'model' && await additiveGesture(this, native)) { const single = toggleFanout(this, providerId, modelId); if (!single) return; return this.changeModel(native, storage, 'model', single.model, single.instanceId); }
    setFanout(this, null);
    const remembered = rememberModel(this, providerId, modelId);
    if (stagesChanges(this)) { stage(this, { providerId, modelId, options: remembered }); return; }
    if (this.threadId) {
      const [commandId] = await this.ids(native, 1);
      await this.dispatch(native, storage, { type: 'thread.model-selection.set', commandId, threadId: this.threadId,
        modelSelection: modelSelection(providerId, modelId, remembered) }, 'Change model');
    }
    this.providerId = providerId; this.modelId = modelId;
    this.modelOptions = remembered;
  }
  private async changeModelOption(native: Native, storage: Files, id: string, value: string): Promise<void> {
    const provider = arr(this.config.providers).find(provider => provider.instanceId === this.providerId);
    const model = arr(provider?.models).find(model => model.slug === this.modelId);
    const options = applyOptionChoice(arr(obj(model?.capabilities).optionDescriptors), this.modelOptions, id, value);
    if (this.threadId && provider?.requiresNewThreadForModelChange === true) throw new ClientError('Start a new thread to change this model option.');
    rememberOptions(this, this.providerId, this.modelId, options);
    if (stagesChanges(this)) { stage(this, { options }); return; }
    if (this.threadId) {
      const [commandId] = await this.ids(native, 1);
      await this.dispatch(native, storage, { type: 'thread.model-selection.set', commandId, threadId: this.threadId,
        modelSelection: modelSelection(this.providerId, this.modelId, options) }, 'Change reasoning effort');
    }
    this.modelOptions = options;
  }
  private async changeMode(native: Native, storage: Files, op: string, value: string): Promise<void> {
    const modes = op === 'runtime' ? ['approval-required', 'auto-accept-edits', 'auto', 'full-access'] : ['default', 'plan'];
    if (!modes.includes(value)) throw new ClientError('That mode is not supported.');
    const provider = arr(this.config.providers).find(provider => provider.instanceId === this.providerId);
    if (op === 'runtime' && Array.isArray(provider?.supportedRuntimeModes) && !provider.supportedRuntimeModes.includes(value)) {
      throw new ClientError('This provider does not support that permission mode.');
    }
    if (op === 'interaction' && value === 'plan' && provider?.showInteractionModeToggle === false) throw new ClientError('This provider does not support plan mode.');
    if (stagesChanges(this)) { stage(this, op === 'runtime' ? { runtimeMode: value } : { interactionMode: value }); return; }
    if (this.threadId) {
      const [commandId] = await this.ids(native, 1);
      const field = op === 'runtime' ? 'runtimeMode' : 'interactionMode';
      await this.dispatch(native, storage, { type: `thread.${op === 'runtime' ? 'runtime' : 'interaction'}-mode.set`, commandId, threadId: this.threadId, [field]: value }, 'Change mode');
    }
    if (op === 'runtime') this.runtimeMode = value; else this.interactionMode = value;
  }

  private async dispatchRequest(native: Native, storage: Files, payload: Obj, description: string): Promise<void> {
    const [commandId] = await this.ids(native, 1);
    await this.dispatch(native, storage, { ...payload, commandId }, description);
  }
  private async approve(native: Native, storage: Files, id: string, decision: string): Promise<void> {
    await this.dispatchRequest(native, storage, approvalPayload(this, id, decision), 'Approval');
  }
  /** A displaced custom answer returns to the thread draft (carryDisplacedCustomAnswerIntoPrompt). */
  private carryAnswer(text: string): void {
    const prompt = this.local.drafts[this.draftKey] || '';
    this.local.drafts[this.draftKey] = prompt.trim() ? `${prompt.trimEnd()}\n\n${text}` : text;
  }
  /** The composer's Submit/Next and Return advance; the last question answers the provider. */
  private async submitAnswers(native: Native, storage: Files, id: string, custom?: string): Promise<void> {
    if (custom !== undefined && custom !== '') setCustomAnswer(this, custom);
    const requestId = id || activeInput(this)?.input.requestId || '';
    const answers = advanceQuestion(this, requestId);
    if (answers) await this.dispatchRequest(native, storage, { type: 'runtime-request.respond', threadId: this.threadId, requestId, answers }, 'Answers');
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
  /** The changes panel asks the server for the current selection; a stale answer never replaces a newer one. */
  private async diff(native: Native, op: string, id: string, value: string, n: number): Promise<void> {
    // Only opening commands open the panel; a refetch queued behind a close never reopens it.
    if (op !== 'diff' && op !== 'checkpoint-diff' && !this.diffOpen) {
      if (op === 'diff-whitespace') this.diffState.ignoreWhitespace = !this.diffState.ignoreWhitespace;
      return;
    }
    this.diffOpen = true; this.diffError = '';
    let request;
    try {
      if (op === 'checkpoint-diff') selectCheckpoint(this, n, id);
      else if (op === 'diff-scope') selectScope(this, value);
      else if (op === 'diff') selectScope(this, 'branch'); // generic opens show Changes (diff.ts, upstream d1034d62b2)
      else if (op === 'diff-whitespace') this.diffState.ignoreWhitespace = !this.diffState.ignoreWhitespace;
      request = diffRequest(this);
    } catch (error) { this.diffText = ''; this.diffError = message(error); return; }
    if (request.scope !== this.diffState.scopeKey) this.diffText = '';
    const epoch = this.threadEpoch, threadId = this.threadId, asked = JSON.stringify(request);
    this.diffLoading = true;
    try {
      const result = await requestDiff(this.config, request, (method, payload) => this.request(native, method, payload)); // r11-device: DiffPanel's server-cwd retry
      if (epoch === this.threadEpoch && threadId === this.threadId && this.diffOpen && JSON.stringify(diffRequest(this)) === asked) {
        // A named file stays collapsed, as in the reference (onOpenTurnDiff leaves 'Expand <file>' false).
        this.diffText = adoptDiff(this, request, result);
      }
    } catch (error) { if (epoch === this.threadEpoch) this.diffError = message(error); }
    finally { if (epoch === this.threadEpoch) this.diffLoading = false; }
  }

}
