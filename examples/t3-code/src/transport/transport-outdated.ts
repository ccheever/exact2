// Hosts too old for this client (T3 Code 1e2ecbd975, MIT, see LICENSE-T3: connection/compatibility.ts,
// onboarding.ts preparePairingRegistration, outdatedHostUpdate.ts updateOutdatedHost, resolver.ts
// prepareForUpdate, state/server.ts updateServer). A host whose orchestration protocol is older cannot
// open a session, but one that can update itself is still paired (saved switched off) and can be
// updated from here: a bare socket without the protocol gate carries only the self-update RPCs, whose
// wire shape has not changed across protocol versions. Then the descriptor is polled until the host
// answers protocol 2, and the environment is switched back on. A connected server that is only older
// than this client runs the same job (`mode: "connected"`): its socket names protocol 2, the request
// carries the config's capabilities and extra payload, and the restart is proven by the descriptor
// reporting the target version.
//
// A job's socket is one of the transport's update sockets; its HTTP steps run in the answer the
// transport asks for each step (the descriptor, the ticket, each poll); the tick schedules polls.
import { TransportFailure, canSelfUpdate, compatibility, credential as parseCredential, decode, encode, exchangeForm, exchangeScopes, failure as wireFailure,
  failureMessage, failureReason, identifier, letGoAbort, origin as parseOrigin, path as onOrigin, protocolOf, request as wireRequest, PROTOCOL_VERSION } from './transport-wire';
import { json, type Http } from './transport-routes';
import { socketURL, type SocketEvent, type SocketHandle } from './transport-session';
import type { Catalog } from './transport-catalog';

/** OUTDATED_HOST_RESTART_TIMEOUT (outdatedHostUpdate.ts): an outdated host's restart. */
export const RESTART_TIMEOUT_MS = 240_000;
/** SERVER_UPDATE_RESUME_TIMEOUT (state/server.ts, reference 9c161462c9): a connected server's, covering the service launcher's 30-minute trial deadline. */
export const CONNECTED_RESTART_TIMEOUT_MS = 32 * 60_000;
export const SOCKET_OPEN_TIMEOUT_MS = 15_000;
export const POLL_INTERVAL_MS = 1_000;
export const UPDATE_SLOTS = 4;

type Obj = Record<string, unknown>;
const str = (value: unknown) => typeof value === 'string' ? value : '';
const obj = (value: unknown): Obj => typeof value === 'object' && value !== null && !Array.isArray(value) ? value as Obj : {};
export type Job = { status: string; stage: string; fromVersion: string; targetVersion: string; message: string; resultVersion: string; label: string; mode: string; attempt: string };
type Reply = { ok: true; generation: 0; value: unknown } | { ok: false; generation: 0; error: Obj };

interface Run {
  key: string; origin: string; environment: string; target: string; connected: boolean;
  capabilities: Obj; extras: Obj;
  phase: 'socket' | 'resume' | 'waiting';
  slot: number; step: number; url: string; openDeadline: number;
  /** The progress result (`complete`), which wins over a socket lost after it. */
  terminal: Obj | null;
  commitSent: boolean;
  label: string; selfUpdate: string; deadline: number; nextPollAt: number; stepRunning: boolean;
}

export interface OutdatedHooks { catalog: Catalog; http: Http; now(): number; wall(now: number): string; changed(): void }

export class OutdatedHosts {
  jobs: Record<string, Job> = {};
  private attempts = 0;
  private runs = new Map<string, Run>();
  private slots: Array<string | null> = Array.from({ length: UPDATE_SLOTS }, () => null);
  alive = true;
  /** Test seam: how long the descriptor poll waits between attempts, and how long the restart may take. */
  pollInterval = POLL_INTERVAL_MS;
  restartTimeout = RESTART_TIMEOUT_MS;
  connectedRestartTimeout = CONNECTED_RESTART_TIMEOUT_MS;

  constructor(private readonly hooks: OutdatedHooks) {}

  private ok(value: unknown): Reply { return { ok: true, generation: 0, value }; }
  private bad(failure: TransportFailure): Reply { return { ok: false, generation: 0, error: failure.json() }; }

  async perform(op: string, key: string, request: Obj): Promise<Reply> {
    if (!this.alive) return this.bad(new TransportFailure('Closed', 'The window was closed.'));
    if (op === 'fleetOutdatedJobs') return this.ok({ jobs: this.jobs });
    const [rawOrigin = '', environment = ''] = splitKey(key);
    let origin: string;
    try { origin = parseOrigin(rawOrigin); } catch (error) { return this.bad(error instanceof TransportFailure ? error : new TransportFailure('Address', 'The server address is invalid.')); }
    try {
      switch (op) {
        case 'fleetOutdatedProbe': return this.ok(await this.descriptor(origin));
        case 'fleetOutdatedPair': return this.ok(await this.pair(origin, str(request.credential), str(request.scope), str(request.legacyScope)));
        case 'fleetOutdatedUpdate': return this.ok(this.update(origin, environment, request));
        case 'fleetOutdatedAck': {
          const jobKey = `${origin}\n${environment}`;
          if (this.jobs[jobKey]?.status !== 'running') delete this.jobs[jobKey];
          return this.ok({ jobs: this.jobs });
        }
        default: throw new TransportFailure('Arguments', 'Unsupported outdated-host operation.');
      }
    } catch (error) {
      if (letGoAbort(error)) throw error;
      return this.bad(error instanceof TransportFailure ? error : new TransportFailure('Network', String(error)));
    }
  }

  destroy(): void { this.alive = false; this.runs.clear(); this.slots = this.slots.map(() => null); }

  // MARK: HTTP

  private async http(where: string, at: string, init: { method?: string; body?: string; bearer?: string } = {}): Promise<Obj> {
    const headers: Record<string, string> = { Accept: 'application/json' };
    if (init.body !== undefined) headers['Content-Type'] = 'application/x-www-form-urlencoded';
    if (init.bearer) headers.Authorization = `Bearer ${init.bearer}`;
    let reply;
    try { reply = await this.hooks.http(onOrigin(where, at), { method: init.method ?? 'GET', headers, body: init.body, timeoutMs: 15_000 }); }
    catch (error) {
      if (letGoAbort(error)) throw error;
      throw new TransportFailure('Network', String(error instanceof Error ? error.message : error).slice(0, 300));
    }
    const decoded = json(reply);
    if (reply.status < 200 || reply.status >= 300 || typeof decoded !== 'object' || decoded === null || Array.isArray(decoded)) {
      throw new TransportFailure(reply.status === 401 || reply.status === 403 ? 'Authentication' : 'HTTP', failureMessage(decoded, reply.status), false, failureReason(decoded));
    }
    return decoded as Obj;
  }
  private async descriptor(origin: string): Promise<Obj> {
    const value = await this.http('/.well-known/t3/environment', origin);
    if (!str(value.environmentId)) throw new TransportFailure('Protocol', 'The server did not describe its environment.');
    return value;
  }

  // MARK: Pairing (onboarding.ts preparePairingRegistration)

  private async pair(origin: string, input: string, scope: string, legacyScope: string): Promise<Obj> {
    const code = parseCredential(input, origin);
    if (!code) throw new TransportFailure('Credential', 'Enter a pairing code.');
    const descriptor = await this.descriptor(origin);
    const environment = str(descriptor.environmentId);
    const label = str(descriptor.label) || hostOf(origin) || 'this environment';
    const blocked = compatibility(descriptor, label);
    if (!blocked) throw new TransportFailure('Compatible', `${label} can pair normally.`);
    // Only an outdated host that can update itself is saved; anything else stays refused.
    if (!blocked.serverUpdateRequired) throw new TransportFailure('Protocol', blocked.message);
    let grant: Obj;
    try { grant = await exchangeScopes(scope, legacyScope, requested => this.http('/oauth/token', origin, { method: 'POST', body: exchangeForm(code, requested) })); }
    catch (error) {
      if (letGoAbort(error)) throw error;
      const failure = error as TransportFailure; throw new TransportFailure(failure.kind, failure.message.split(code).join('[redacted]'));
    }
    if (grant.token_type !== 'Bearer' || !str(grant.access_token)) throw new TransportFailure('Protocol', 'The server did not issue a bearer access token.');
    this.hooks.catalog.saveToken(str(grant.access_token), origin, environment);
    this.hooks.catalog.remember(origin, descriptor, this.hooks.wall(this.hooks.now()));
    this.hooks.catalog.setEnabled(environment, false);
    this.hooks.changed();
    return { origin, environmentId: environment, label, serverVersion: str(descriptor.serverVersion), message: blocked.message, serverUpdateRequired: true };
  }

  // MARK: Update (outdatedHostUpdate.ts updateOutdatedHost)

  private update(origin: string, environment: string, request: Obj): Obj {
    if (!environment) throw new TransportFailure('Arguments', 'Choose a saved environment to update.');
    const target = str(request.targetVersion).trim();
    if (!target || target.length > 128) throw new TransportFailure('Arguments', 'Choose a version to update to.');
    const key = `${origin}\n${environment}`;
    // Single-flight per environment (createOutdatedServerUpdateCommand concurrency).
    if (this.jobs[key]?.status === 'running') return { started: false };
    const connected = request.mode === 'connected';
    this.attempts += 1;
    this.jobs[key] = { status: 'running', stage: 'downloading', fromVersion: str(request.fromVersion) || target, targetVersion: target, message: '', resultVersion: '',
      label: str(request.label), mode: connected ? 'connected' : 'outdated', attempt: String(this.attempts) };
    this.hooks.changed();
    const extras: Obj = {};
    const given = obj(request.extras);
    if (typeof given.continueRunningThreads === 'boolean') extras.continueRunningThreads = given.continueRunningThreads;
    this.runs.set(key, { key, origin, environment, target, connected, capabilities: obj(request.capabilities), extras, phase: 'waiting', slot: -1, step: 0, url: '',
      openDeadline: 0, terminal: null, commitSent: false, label: '', selfUpdate: '', deadline: 0, nextPollAt: 0, stepRunning: false });
    this.assign();
    return { started: true, attempt: String(this.attempts) };
  }

  private setJob(key: string, fields: Partial<Job>): void {
    const job = this.jobs[key];
    if (!job) return;
    this.jobs[key] = { ...job, ...fields };
    this.hooks.changed();
  }
  private fail(key: string, message: string): void {
    this.end(key);
    this.setJob(key, { status: 'failed', message });
  }
  private end(key: string): void {
    const run = this.runs.get(key);
    if (run && run.slot >= 0) this.slots[run.slot] = null;
    this.runs.delete(key);
    this.assign();
  }
  /** A waiting job takes a free update slot. */
  private assign(): void {
    for (const run of this.runs.values()) {
      if (run.slot >= 0) continue;
      const free = this.slots.indexOf(null);
      if (free < 0) return;
      this.slots[free] = run.key; run.slot = free; run.step += 1;
    }
  }

  /** The slot table the transport publishes: each slot's job key and step. */
  slotTable(): Array<{ key: string; step: number }> {
    return this.slots.map(key => { const run = key ? this.runs.get(key) : undefined; return { key: run?.key ?? '', step: run?.step ?? 0 }; });
  }
  /** The socket URL for a slot's step, once its ticket is in. */
  socketFor(slot: number, key: string): string {
    const run = this.runs.get(key);
    return run && run.slot === slot && run.phase === 'socket' ? run.url : '';
  }

  /**
   * The answer the transport asks for a slot's step: the descriptor and ticket that open the job's socket, or one
   * poll while the host restarts. Resolves with the socket URL (or '').
   */
  async step(slot: number, key: string, step: number): Promise<string> {
    const run = this.runs.get(key);
    if (!this.alive || !run || run.slot !== slot || run.step !== step || run.stepRunning) return '';
    run.stepRunning = true;
    try {
      if (run.phase === 'waiting') return await this.open(run, step);
      if (run.phase === 'resume') await this.poll(run);
      return '';
    } finally { run.stepRunning = false; }
  }

  private async open(run: Run, step: number): Promise<string> {
    let descriptor: Obj;
    try { descriptor = await this.descriptor(run.origin); } catch (error) { if (letGoAbort(error)) throw error; this.fail(run.key, (error as Error).message); return ''; }
    if (!this.alive || this.runs.get(run.key) !== run) return '';
    const label = str(descriptor.label) || hostOf(run.origin) || 'This environment';
    run.label = label;
    if (!this.jobs[run.key]?.label) this.setJob(run.key, { label });
    if (str(descriptor.environmentId) !== run.environment) { this.fail(run.key, `${label} is a different environment now. Pair with it again.`); return ''; }
    // A connected server's config names its capabilities; an outdated host's descriptor does.
    const capabilities = run.connected && Object.keys(run.capabilities).length ? run.capabilities : obj(descriptor.capabilities);
    if (!canSelfUpdate({ capabilities })) { this.fail(run.key, `Update T3 Code on ${label} manually; it cannot update itself.`); return ''; }
    run.capabilities = capabilities; run.selfUpdate = str(capabilities.serverSelfUpdate);
    const from = str(descriptor.serverVersion);
    if (from && this.jobs[run.key]?.fromVersion === run.target) this.setJob(run.key, { fromVersion: from });
    const token = this.hooks.catalog.token(run.origin, run.environment);
    if (!token) { this.fail(run.key, `Pair with ${label} again to update it.`); return ''; }
    let ticket = '';
    try { ticket = str((await this.http('/api/auth/websocket-ticket', run.origin, { method: 'POST', body: '', bearer: token })).ticket); }
    catch (error) { if (letGoAbort(error)) throw error; this.fail(run.key, (error as Error).message); return ''; }
    if (!this.alive || this.runs.get(run.key) !== run || run.step !== step) return '';
    if (!ticket) { this.fail(run.key, 'The server did not issue a socket ticket.'); return ''; }
    // The bare socket: no orchestrationProtocol query (resolver.ts prepareForUpdate); a connected server refuses one that does not name it.
    run.url = socketURL(run.origin, ticket, run.connected);
    run.phase = 'socket'; run.openDeadline = this.hooks.now() + SOCKET_OPEN_TIMEOUT_MS + 1_000;
    this.hooks.changed();
    return run.url;
  }

  /** One event of a job's socket (its mapper): the update RPC, its progress, the commit and the restart. */
  socketEvent(slot: number, url: string, event: SocketEvent, handle: SocketHandle | undefined): void {
    const key = this.slots[slot];
    const run = key ? this.runs.get(key) : undefined;
    if (!run || run.phase !== 'socket' || run.url !== url) return;
    const send = (frame: Obj) => { try { handle?.send(encode(frame)); } catch { /* the socket is gone */ } };
    const request = (id: string, method: string, payload: Obj) => send(run.connected ? wireRequest(id, method, payload) : { _tag: 'Request', id, tag: method, payload, headers: [] });
    if (event.type === 'open') {
      run.openDeadline = 0;
      const input: Obj = { targetVersion: run.target, ...run.extras };
      request('1', run.capabilities.serverSelfUpdateProgress === true ? 'server.updateServerWithProgress' : 'server.updateServer', input);
      return;
    }
    if (event.type === 'close' || event.type === 'error') { this.outcome(run, { dropped: event.message || `The server connection closed (${event.code ?? 1006}).` }, request); return; }
    let frames;
    try { frames = decode(typeof event.data === 'string' ? event.data : new TextDecoder().decode(event.data as ArrayBuffer)); }
    catch { this.outcome(run, { failure: 'The server sent an invalid RPC frame.' }, request); return; }
    for (const frame of frames) {
      const tag = str(frame._tag);
      if (tag === 'Ping') { send({ _tag: 'Pong' }); continue; }
      if (tag === 'Pong') continue;
      if (tag === 'Defect' || tag === 'ClientProtocolError') { this.outcome(run, { failure: wireFailure(frame).message }, request); return; }
      const id = identifier(frame.requestId);
      if (tag === 'Chunk') {
        if (id === '1') for (const value of Array.isArray(frame.values) ? frame.values : []) {
          const event = obj(value);
          if (event.type === 'complete' && typeof event.result === 'object') run.terminal = obj(event.result);
          else if (event.type === 'progress' && ['downloading', 'installing'].includes(str(event.stage))) this.setJob(run.key, { stage: str(event.stage) });
        }
        send({ _tag: 'Ack', requestId: frame.requestId });
        continue;
      }
      if (tag === 'Exit' && (id === '1' || id === '2')) {
        const exit = obj(frame.exit);
        if (exit._tag === 'Success') this.outcome(run, { success: exit.value ?? null, id }, request);
        else this.outcome(run, { failure: updateFailure(exit), id }, request);
        return;
      }
    }
  }

  private outcome(run: Run, result: { success?: unknown; failure?: string; dropped?: string; id?: string }, request: (id: string, method: string, payload: Obj) => void): void {
    if (result.id === '2') {
      // The commit relaunches the desktop app, so a dropped socket is success.
      if (result.failure !== undefined) return this.fail(run.key, result.failure);
      return this.resume(run);
    }
    if (result.dropped !== undefined && run.phase === 'socket' && run.commitSent) return this.resume(run);
    const finish = (value: Obj) => {
      const commitToken = str(value.desktopUpdateToken);
      if (value.method !== 'desktop-app' || !commitToken) return this.resume(run);
      run.commitSent = true;
      request('2', 'server.commitDesktopUpdate', { requestId: commitToken });
    };
    if (run.capabilities.serverSelfUpdateProgress === true) {
      // resolveServerUpdateProgressResult: the terminal event wins over a lost handoff.
      if (result.failure !== undefined) return this.fail(run.key, result.failure);
      if (run.terminal) return finish(run.terminal);
      if (result.dropped !== undefined) return this.fail(run.key, result.dropped);
      return this.fail(run.key, `The t3@${run.target} update ended before the server accepted the restart.`);
    }
    if (result.failure !== undefined) return this.fail(run.key, result.failure);
    if (result.dropped !== undefined) {
      // Older servers can drop the socket before acknowledging the restart.
      if (run.selfUpdate === 'boot-service' || run.selfUpdate === 'respawn') return finish({ targetVersion: run.target, method: run.selfUpdate });
      return this.fail(run.key, result.dropped);
    }
    finish(Object.keys(obj(result.success)).length ? obj(result.success) : { targetVersion: run.target, method: run.selfUpdate });
  }

  private resume(run: Run): void {
    run.phase = 'resume'; run.url = '';
    run.deadline = this.hooks.now() + (run.connected ? this.connectedRestartTimeout : this.restartTimeout);
    run.nextPollAt = this.hooks.now(); run.step += 1;
    this.setJob(run.key, { stage: 'resuming' });
  }

  private async poll(run: Run): Promise<void> {
    let descriptor: Obj | null = null;
    try { descriptor = await this.descriptor(run.origin); } catch (error) { if (letGoAbort(error)) throw error; descriptor = null; }
    if (!this.alive || this.runs.get(run.key) !== run || this.jobs[run.key]?.status !== 'running') return;
    // A connected server resumed once it reports the target version (matchesServerUpdateReadyEvent).
    if (run.connected) {
      if (descriptor && str(descriptor.serverVersion) === run.target) { this.end(run.key); this.setJob(run.key, { status: 'done', resultVersion: run.target }); return; }
      if (this.hooks.now() >= run.deadline) { this.fail(run.key, `The server did not resume on t3@${run.target}.`); return; }
    } else {
      if (descriptor && protocolOf(descriptor) === PROTOCOL_VERSION) {
        this.hooks.catalog.remember(run.origin, descriptor, this.hooks.wall(this.hooks.now()));
        this.hooks.catalog.setEnabled(run.environment, true);
        this.end(run.key);
        this.setJob(run.key, { status: 'done', resultVersion: str(descriptor.serverVersion) });
        return;
      }
      if (this.hooks.now() >= run.deadline) { this.fail(run.key, `${run.label || 'This environment'} did not come back on a compatible T3 Code version.`); return; }
    }
    run.nextPollAt = this.hooks.now() + this.pollInterval;
  }

  /** The tick: a socket that did not open, and the next poll. */
  tick(now: number): void {
    for (const run of [...this.runs.values()]) {
      if (run.phase === 'socket' && run.openDeadline && now >= run.openDeadline) { this.fail(run.key, 'The server WebSocket did not open.'); continue; }
      if (run.phase === 'resume' && !run.stepRunning && run.nextPollAt && now >= run.nextPollAt) { run.nextPollAt = 0; run.step += 1; }
    }
  }
  get busy(): boolean { return this.runs.size > 0; }
}

export function splitKey(key: string): [string, string] {
  const at = key.indexOf('\n');
  return at < 0 ? [key, ''] : [key.slice(0, at), key.slice(at + 1)];
}
const hostOf = (origin: string) => { try { return new URL(origin).hostname; } catch { return ''; } };

/** ServerSelfUpdateError's message is a getter (`Server update failed: ${reason}`), not a wire field. */
export function updateFailure(exit: Obj): string {
  const failure = wireFailure(exit);
  if (failure.kind === 'ServerSelfUpdateError' && failure.reason && failure.message.startsWith('The server refused')) return `Server update failed: ${failure.reason}`;
  return failure.message;
}
