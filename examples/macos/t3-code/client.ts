import { obj, str, num, arr, initialShell, applyShell, threadSnapshot, applyThread, mergeHistory,
  readyCheckpoint, type Obj, type Shell, type ThreadState } from './domain';
import { ClientError, bridgeReply, parsePairing, activeRun, providerAvailable, modelSelection, sendPayload,
  launchPayload, applyConfig, type Native, type Files } from './protocol';

const localPath = 'app:/data/t3-code.json';
type Selection = { projectId: string; threadId: string };
export type Pending = { method: string; payload: Obj; description: string; threadId: string; text: string; uncertain: boolean };
type Preferences = { selections: Record<string, Selection>; drafts: Record<string, string>; sidebarWidth: number; sidebarOpen: boolean; pending: Record<string, Pending> };
const preferences = (): Preferences => ({ selections: {}, drafts: {}, sidebarWidth: 256, sidebarOpen: true, pending: {} });
const message = (error: unknown) => error instanceof Error ? error.message : 'The operation failed.';
const scopeStrings = (value: unknown): string[] => Array.isArray(value) ? value.filter((entry): entry is string => typeof entry === 'string') : [];

export class T3Client {
  available = false;
  revision = 0;
  generation = -1;
  environmentId = '';
  origin = 'http://127.0.0.1:3773';
  connection = 'disconnected';
  statusMessage = 'Connect to your T3 Code server.';
  error = '';
  shell: Shell = initialShell();
  thread: ThreadState | null = null;
  config: Obj = {};
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
  answers: Record<string, string | string[]> = {};
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

  get ready(): boolean {
    return this.connection === 'connected' && this.configLive && this.shellLive && (!this.threadId || this.threadLive);
  }
  get writable(): boolean {
    return this.ready && this.scopes.includes('orchestration:operate')
      && obj(obj(this.config.environment).capabilities).serverResolvedCommandContext === true;
  }
  get pending(): Pending | undefined { return this.local.pending[this.environmentId]; }
  get draftKey(): string { return `${this.environmentId}:${this.threadId || `new:${this.projectId}`}`; }
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
    try { response = await this.raw(native, { ...obj(request), generation: expected }); }
    catch (error) {
      if (error instanceof ClientError) throw error;
      throw new ClientError(message(error), 'transport', write);
    }
    if (!response.ok) throw new ClientError(response.error!.message, response.error!.kind, response.error!.uncertain);
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
      next.sidebarWidth = Math.max(208, Math.min(400, num(saved.sidebarWidth, 256)));
      next.sidebarOpen = saved.sidebarOpen !== false;
      for (const [key, value] of Object.entries(obj(saved.drafts))) {
        if (typeof value === 'string' && value.length <= 100_000) next.drafts[key] = value;
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
    this.loaded = true;
  }
  private async persist(storage: Files): Promise<void> {
    if (this.environmentId) this.local.selections[this.environmentId] = { projectId: this.projectId, threadId: this.threadId };
    // The native adapter serializes writes inside this app's transport queue.
    // A later answer must never await an abandoned answer's promise.
    await storage.fs.atomicWriteFile(localPath, new TextEncoder().encode(JSON.stringify({ version: 1, ...this.local })));
  }
  private async save(storage: Files): Promise<void> {
    try { await this.persist(storage); } catch { this.error = 'Could not save local drafts and preferences. Keep a copy before closing.'; }
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
    native.watch('t3.status'); native.watch('t3.events');
    try {
      await this.load(storage);
      const status = await this.raw(native, { op: 'status' });
      // Exact may discard an optimistic asynchronous reread without dispatching
      // it. Only a real reply can replace the active refresh's ownership.
      if (epoch < this.refreshEpoch) return;
      this.refreshEpoch = epoch;
      this.available = true;
      native = this.ownedNative(native, () => epoch === this.refreshEpoch);
      if (!status.ok) throw new ClientError(status.error!.message);
      this.adoptStatus(obj(status.value), status.generation);
      if (this.connection !== 'connected') return;
      if (this.synchronizedGeneration !== this.generation) await this.synchronize(native);
      else if (this.threadId && this.threadSubscription !== this.threadId) await this.openThread(native, this.threadId, this.thread !== null);
      await this.drain(native);
      if (this.needsFreshSnapshot) {
        this.needsFreshSnapshot = false;
        this.shellLoaded = false; this.thread = null;
        await this.synchronize(native);
        await this.drain(native);
      }
      if (this.reconcilePending()) await this.save(storage);
    } catch (error) {
      if (epoch < this.refreshEpoch) return;
      this.available = true;
      if (!(error instanceof ClientError && ['stale', 'superseded'].includes(error.kind))) this.error = message(error);
    }
  }

  private async synchronize(native: Native): Promise<void> {
    // Commands and watched snapshots can both bootstrap. Only the newest one
    // may install subscriptions, even when their answer lifetimes differ.
    const epoch = ++this.synchronizationEpoch;
    native = this.ownedNative(native, () => epoch === this.synchronizationEpoch);
    const generation = this.generation;
    this.synchronizedGeneration = -1;
    this.shellLive = false; this.threadLive = false;
    delete this.subscriptions.config; delete this.subscriptions.shell;
    const auth = await this.http(native, '/api/auth/session', generation);
    if (auth.authenticated !== true) throw new ClientError('Pair with the T3 server again.', 'auth');
    this.scopes = scopeStrings(auth.scopes);
    const config = await this.request(native, 'server.getConfig', {}, generation);
    const environment = obj(config.environment);
    if (num(environment.orchestrationProtocolVersion) !== 2) throw new ClientError('This client requires T3 orchestration protocol 2.', 'protocol');
    if (str(environment.environmentId) !== this.environmentId) throw new ClientError('This address now belongs to a different T3 environment. Pair again.', 'identity');
    this.config = config; this.configLive = true;
    const configSub = await this.call(native, { op: 'subscribe', key: 'config', method: 'subscribeServerConfig', payload: {} }, generation);
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

  private ensureSelection(): void {
    const selected = this.shell.threads.find(thread => thread.id === this.threadId);
    if (selected) this.projectId = str(selected.projectId);
    if (!this.shell.projects.some(project => project.id === this.projectId)) this.projectId = str(this.shell.projects[0]?.id);
  }
  private chooseDefaults(): void {
    const project = this.shell.projects.find(project => project.id === this.projectId);
    const settings = obj(this.config.settings);
    const overrides = obj(obj(settings.projectSettingsOverrides)[this.projectId]);
    const selection = obj(overrides.defaultModelSelection || project?.defaultModelSelection || settings.defaultModelSelection);
    const providers = arr(this.config.providers);
    const provider = providers.find(provider => provider.instanceId === selection.instanceId && providerAvailable(provider));
    const valid = provider && arr(provider.models).some(model => model.slug === selection.model);
    this.providerId = valid ? str(provider.instanceId) : '';
    this.modelId = valid ? str(selection.model) : '';
    this.modelOptions = valid ? arr(selection.options) : [];
    this.runtimeMode = 'approval-required';
    this.interactionMode = 'default';
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
        if (!this.subscriptions[key] || str(entry.subscriptionId) !== this.subscriptions[key]) continue;
        if (item._transportError || item._streamEnded) {
          const detail = obj(item._transportError);
          this.error = str(detail.message, 'The live stream ended. Reconnect to continue.');
          if (key === 'thread') this.threadLive = false;
          if (key === 'shell') this.shellLive = false;
          if (key === 'config') this.configLive = false;
          this.synchronizedGeneration = -1;
          continue;
        }
        try {
          if (key === 'config') {
            this.config = applyConfig(this.config, item); this.configLive = true;
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
    } else if (payload.type === 'project.create') {
      completed = this.shell.projects.some(project => project.id === payload.projectId);
    } else if (pending.threadId === this.threadId && this.threadLive) {
      if (payload.type === 'message.dispatch') completed = arr(this.projection.messages).some(value => value.id === payload.messageId);
      else if (payload.type === 'runtime-request.respond') completed = arr(this.projection.runtimeRequests).some(value => value.id === payload.requestId && value.status !== 'pending');
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
    if (pending.text) {
      const key = pending.method === 'orchestration.launchThread'
        ? `${environmentId}:new:${str(pending.payload.projectId)}` : `${environmentId}:${pending.threadId}`;
      if (this.local.drafts[key] === pending.text) delete this.local.drafts[key];
    }
    delete this.local.pending[environmentId];
  }
  private async write(native: Native, storage: Files, pending: Pending): Promise<Obj> {
    const generation = this.generation;
    const environmentId = this.environmentId;
    this.local.pending[environmentId] = pending;
    try { await this.persist(storage); }
    catch {
      delete this.local.pending[environmentId];
      throw new ClientError('Could not save this operation locally. Your draft is still available.');
    }
    try {
      const result = await this.request(native, pending.method, pending.payload, generation, true);
      this.finishPending(pending, environmentId);
      await this.save(storage);
      return result;
    } catch (error) {
      if (error instanceof ClientError && error.kind === 'superseded') throw error;
      if (error instanceof ClientError && error.uncertain) {
        pending.uncertain = true;
        this.error = `${pending.description} may have reached T3. Check the synchronized thread, then retry only if needed.`;
      } else {
        delete this.local.pending[environmentId];
        this.error = message(error);
      }
      await this.save(storage);
      throw error;
    }
  }
  private async dispatch(native: Native, storage: Files, payload: Obj, description: string): Promise<Obj> {
    return this.write(native, storage, { method: 'orchestration.dispatchCommand', payload, description,
      threadId: str(payload.threadId), text: str(payload.text), uncertain: false });
  }

  async command(op: string, id: string, value: string, n: number, native: Native | null | undefined, storage: Files): Promise<{ revision: number; message: string }> {
    if (!native?.available) return { revision: ++this.revision, message: 'Open this app on macOS to connect to T3 Code.' };
    const local = ['draft', 'answer', 'choice', 'search', 'sidebar', 'dismiss-error', 'close-diff'].includes(op);
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
      if (op === 'connect' || op === 'reconnect') {
        const target = parsePairing(id || this.origin, value);
        const response = await this.raw(native, { op: 'connect', ...target });
        if (!response.ok) throw new ClientError(response.error!.message, response.error!.kind);
        this.adoptStatus(obj(response.value), response.generation);
        this.error = '';
      } else if (op === 'disconnect' || op === 'forget') {
        const response = await this.raw(native, { op: 'disconnect', forget: op === 'forget' });
        if (response.ok) this.adoptStatus(obj(response.value), response.generation);
      } else if (op === 'draft') {
        if (value.length > 100_000) throw new ClientError('Keep a draft under 100,000 characters.');
        this.local.drafts[this.draftKey] = value;
      } else if (op === 'search') this.query = value;
      else if (op === 'sidebar') {
        if (n) this.local.sidebarWidth = Math.min(400, Math.max(208, n));
        if (value === 'open' || value === 'closed') this.local.sidebarOpen = value === 'open';
        else if (!n) this.local.sidebarOpen = !this.local.sidebarOpen;
      } else if (op === 'dismiss-error') this.error = '';
      else if (op === 'close-diff') { this.diffOpen = false; this.diffLoading = false; }
      else if (op === 'select-project' || op === 'new-thread') {
        if (id && this.shell.projects.some(project => project.id === id)) this.projectId = id;
        this.threadId = ''; this.thread = null; this.threadSubscription = ''; this.threadEpoch++;
        delete this.subscriptions.thread;
        this.threadLive = true; this.diffOpen = false; this.answers = {}; this.chooseDefaults();
        if (this.connection === 'connected') await this.call(native, { op: 'unsubscribe', key: 'thread' });
      } else if (op === 'select-thread') {
        if (!this.shell.threads.some(thread => thread.id === id)) throw new ClientError('That thread is no longer available.');
        this.threadId = id; this.thread = null; this.diffOpen = false; this.answers = {};
        this.ensureSelection(); await this.openThread(native, id);
      } else if (op === 'refresh') {
        this.shellLoaded = false; this.thread = null; this.error = '';
        await this.synchronize(native);
      } else if (op === 'history') await this.history(native);
      else if (op === 'diff') await this.diff(native);
      else if (op === 'answer' || op === 'choice') this.updateAnswer(op, id, value);
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
      } else {
        this.requireWrite();
        if (op === 'send') await this.send(native, storage, value);
        else if (op === 'add-project') await this.addProject(native, storage, id, value);
        else if (op === 'provider' || op === 'model') await this.changeModel(native, storage, op, id);
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
        else if (op === 'submit-answers') await this.submitAnswers(native, storage, id);
        else throw new ClientError(`Unknown action: ${op}`);
      }
      await this.save(storage);
    } catch (error) {
      if (error instanceof ClientError && error.kind === 'superseded') return { revision: this.revision, message: '' };
      resultMessage = message(error);
      if (!this.pending?.uncertain) this.error = resultMessage;
    } finally { if (!local && epoch === this.commandEpoch) this.busy = false; this.changed(); }
    return { revision: this.revision, message: resultMessage };
  }

  private async send(native: Native, storage: Files, value: string): Promise<void> {
    const text = value || this.draft;
    if (!text.trim()) throw new ClientError('Write a message first.');
    if (text.length > 100_000) throw new ClientError('Keep a message under 100,000 characters.');
    if (!this.projectId) throw new ClientError('Choose or add a project first.');
    this.local.drafts[this.draftKey] = text;
    const provider = arr(this.config.providers).find(provider => provider.instanceId === this.providerId);
    if (!provider || !providerAvailable(provider)) throw new ClientError('This provider is unavailable. Configure it in T3 Code.');
    if (!arr(provider.models).some(model => model.slug === this.modelId)) throw new ClientError('Choose one of the models advertised by T3.');
    if (Array.isArray(provider.supportedRuntimeModes) && !provider.supportedRuntimeModes.includes(this.runtimeMode)) {
      throw new ClientError('Choose a permission mode supported by this provider.');
    }
    const [commandId, messageId, threadId] = await this.ids(native, 3);
    if (this.threadId) {
      await this.dispatch(native, storage, sendPayload(commandId, this.threadId, messageId, text), 'Send');
    } else {
      const payload = launchPayload(commandId, threadId, messageId, this.projectId, text,
        modelSelection(this.providerId, this.modelId, this.modelOptions), this.runtimeMode, this.interactionMode);
      const result = await this.write(native, storage, { method: 'orchestration.launchThread', payload,
        description: 'Create thread', threadId, text, uncertain: false });
      this.threadId = str(result.threadId, threadId);
      await this.openThread(native, this.threadId);
    }
  }
  private async addProject(native: Native, storage: Files, path: string, title: string): Promise<void> {
    if (!path.trim()) throw new ClientError('Enter the project folder on the T3 server.');
    const [commandId, projectId] = await this.ids(native, 2);
    await this.write(native, storage, { method: 'projects.mutate', description: 'Add project', threadId: '', text: '', uncertain: false,
      payload: { type: 'project.create', commandId, projectId, title: title.trim() || path.trim().replace(/\/$/, '').split('/').pop() || 'Project', workspaceRoot: path.trim(), createWorkspaceRootIfMissing: false } });
    this.projectId = projectId; this.threadId = ''; this.thread = null; this.threadLive = true; this.chooseDefaults();
  }
  private async changeModel(native: Native, storage: Files, op: string, id: string): Promise<void> {
    const providerId = op === 'provider' ? id : this.providerId;
    const provider = arr(this.config.providers).find(provider => provider.instanceId === providerId);
    if (!provider || !providerAvailable(provider)) throw new ClientError('This provider is unavailable. Configure it in T3 Code.');
    const models = arr(provider.models);
    const modelId = op === 'model' ? id : str(models.find(model => model.isDefault === true)?.slug || models[0]?.slug);
    if (!models.some(model => model.slug === modelId)) throw new ClientError('That model is no longer advertised by T3.');
    if (this.threadId && (provider.requiresNewThreadForModelChange === true || activeRun(this.projection))) {
      throw new ClientError('Start a new thread or wait for the active turn before changing this model.');
    }
    if (this.threadId) {
      const [commandId] = await this.ids(native, 1);
      await this.dispatch(native, storage, { type: 'thread.model-selection.set', commandId, threadId: this.threadId,
        modelSelection: modelSelection(providerId, modelId) }, 'Change model');
    }
    this.providerId = providerId; this.modelId = modelId;
    this.modelOptions = [];
  }
  private async changeModelOption(native: Native, storage: Files, id: string, value: string): Promise<void> {
    const provider = arr(this.config.providers).find(provider => provider.instanceId === this.providerId);
    const model = arr(provider?.models).find(model => model.slug === this.modelId);
    const descriptor = arr(obj(model?.capabilities).optionDescriptors).find(option => option.id === id && option.type === 'select');
    if (!descriptor || !arr(descriptor.options).some(option => option.id === value)) {
      throw new ClientError('That model option is no longer offered by this provider.');
    }
    if (activeRun(this.projection)) throw new ClientError('Wait for the active turn before changing reasoning effort.');
    if (this.threadId && provider?.requiresNewThreadForModelChange === true) throw new ClientError('Start a new thread to change this model option.');
    const options = [...this.modelOptions.filter(option => option.id !== id), { id, value }];
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
    if (this.threadId) {
      const [commandId] = await this.ids(native, 1);
      const field = op === 'runtime' ? 'runtimeMode' : 'interactionMode';
      await this.dispatch(native, storage, { type: `thread.${op === 'runtime' ? 'runtime' : 'interaction'}-mode.set`, commandId, threadId: this.threadId, [field]: value }, 'Change mode');
    }
    if (op === 'runtime') this.runtimeMode = value; else this.interactionMode = value;
  }

  private pendingRequest(id: string): { request: Obj; item: Obj } {
    const request = arr(this.projection.runtimeRequests).find(request => request.id === id && request.status === 'pending');
    const item = arr(this.projection.turnItems).slice().reverse().find(item => item.requestId === id);
    if (!request || !item) throw new ClientError('This request has already been resolved.');
    if (obj(request.responseCapability).type === 'not_resumable') throw new ClientError('This request can no longer be answered. Continue the thread in T3 Code.');
    return { request, item };
  }
  private async approve(native: Native, storage: Files, id: string, decision: string): Promise<void> {
    const { request, item } = this.pendingRequest(id);
    if (obj(request.responseCapability).type !== 'live' || item.type !== 'approval_request') throw new ClientError('This approval is no longer live.');
    const options = arr(item.options);
    const allowed = options.length ? options.map(option => str(option.decision)) : ['accept', 'acceptForSession', 'decline', 'cancel'];
    if (!allowed.includes(decision)) throw new ClientError('Choose one of the approval options offered by the provider.');
    const [commandId] = await this.ids(native, 1);
    await this.dispatch(native, storage, { type: 'runtime-request.respond', commandId, threadId: this.threadId, requestId: id, decision }, 'Approval');
  }
  private updateAnswer(op: string, id: string, value: string): void {
    const separator = id.indexOf('::');
    const requestId = id.slice(0, separator), questionId = id.slice(separator + 2);
    const { item } = this.pendingRequest(requestId);
    const question = arr(item.questions).find(question => question.id === questionId);
    if (!question) throw new ClientError('That question is no longer available.');
    if (op === 'answer') {
      if (question.allowCustomAnswer === false) throw new ClientError('Choose one of the offered options.');
      this.answers[id] = value;
    } else {
      if (!arr(question.options).some(option => (typeof option.value === 'string' ? option.value : str(option.label)) === value)) throw new ClientError('That option is no longer available.');
      if (question.multiSelect === true) {
        const current = Array.isArray(this.answers[id]) ? this.answers[id] as string[] : [];
        this.answers[id] = current.includes(value) ? current.filter(answer => answer !== value) : [...current, value];
      } else this.answers[id] = value;
    }
  }
  private async submitAnswers(native: Native, storage: Files, id: string): Promise<void> {
    const { item } = this.pendingRequest(id);
    if (item.type !== 'user_input_request') throw new ClientError('This is not a question request.');
    const answers: Obj = {};
    for (const question of arr(item.questions)) {
      const questionId = str(question.id), answer = this.answers[`${id}::${questionId}`];
      const offered = typeof answer === 'string' && arr(question.options).some(option => (typeof option.value === 'string' ? option.value : str(option.label)) === answer);
      const empty = answer === undefined || (typeof answer === 'string' ? !offered && !answer.trim() : answer.length === 0);
      if (empty && question.required !== false) throw new ClientError(`Answer “${str(question.header, str(question.question))}” first.`);
      if (!empty) answers[questionId] = answer;
    }
    const [commandId] = await this.ids(native, 1);
    await this.dispatch(native, storage, { type: 'runtime-request.respond', commandId, threadId: this.threadId, requestId: id, answers }, 'Answers');
  }
  private async history(native: Native): Promise<void> {
    if (!this.thread?.hasMore || !this.thread.historyCursor || this.historyLoading) return;
    const epoch = this.threadEpoch, id = this.threadId, cursor = this.thread.historyCursor;
    this.historyLoading = true;
    try {
      const page = await this.http(native, `/api/orchestration/threads/${encodeURIComponent(id)}/history?cursor=${encodeURIComponent(cursor)}`);
      if (epoch === this.threadEpoch && id === this.threadId && this.thread?.historyCursor === cursor) this.thread = mergeHistory(this.thread, page);
    } finally { this.historyLoading = false; }
  }
  private async diff(native: Native): Promise<void> {
    this.diffOpen = true; this.diffText = ''; this.diffError = '';
    if (!this.thread) { this.diffError = 'Select a thread to view its changes.'; return; }
    const count = readyCheckpoint(this.thread);
    if (count === null) { this.diffError = 'No completed checkpoint is available for this thread.'; return; }
    const epoch = this.threadEpoch, id = this.threadId;
    this.diffLoading = true;
    this.diffTitle = `Changes through turn ${count}`;
    try {
      const result = await this.request(native, 'orchestration.getFullThreadDiff', { threadId: id, toTurnCount: count, ignoreWhitespace: false });
      if (epoch === this.threadEpoch && id === this.threadId && this.diffOpen) this.diffText = str(result.diff);
    } catch (error) { if (epoch === this.threadEpoch) this.diffError = message(error); }
    finally { if (epoch === this.threadEpoch) this.diffLoading = false; }
  }
}
