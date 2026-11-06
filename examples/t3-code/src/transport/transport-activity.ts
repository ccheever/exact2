// backgroundActivityReporter.ts (T3 Code 1e2ecbd975, MIT, see LICENSE-T3): one reporter owns
// the debounce, cadence and scopes for every connection, and sends
// `server.reportClientActivity` to each connected environment. `visible` and `focused` are the
// page's (`exactPage()` visibilityState and hasFocus), handed over when they change
// (`activityFacts`); when the reader last pointed, typed or scrolled is the window's
// (app.contract `interacted`, the reference's window listeners), to the second. A data module's timer
// lives only inside an answer: the transport's tick runs the debounce and the 25 s cadence on its clock.

export const REPORT_INTERVAL_MS = 25_000;
export const LEASE_TTL_MS = 45_000;
export const DEBOUNCE_MS = 250;

export type Scope = Record<string, unknown>;
type Sender = (payload: Record<string, unknown>, done: () => void) => void;

export const recentlyInteracted = (last: number, now: number) => last <= now && now - last <= LEASE_TTL_MS;

/** The scopes subscriptions retain (diagnostics, a workspace's VCS status), counted per environment. */
export class ActivityScopes {
  private entries = new Map<string, { environment: string; scope: Scope; count: number }>();
  retain(environment: string, method: string, payload: Record<string, unknown>): (() => void) | null {
    let scope: Scope;
    if (method === 'subscribeResourceTelemetry') scope = { type: 'diagnostics' };
    // A passive observer (`includeRemote: false`) reads cached remote status and retains no refresh loop.
    else if (method === 'subscribeVcsStatus' && typeof payload.cwd === 'string' && payload.includeRemote !== false) scope = { type: 'vcs-status', cwd: payload.cwd };
    else return null;
    const key = JSON.stringify([environment, scope.type, scope.cwd ?? null]);
    const entry = this.entries.get(key) ?? { environment, scope, count: 0 };
    entry.count += 1; this.entries.set(key, entry);
    let released = false;
    return () => {
      const current = this.entries.get(key);
      if (released || !current) return;
      released = true; current.count -= 1;
      if (current.count === 0) this.entries.delete(key);
    };
  }
  values(environment: string): Scope[] {
    return [...this.entries.keys()].sort().map(key => this.entries.get(key)!).filter(entry => entry.environment === environment).map(entry => entry.scope);
  }
}

export class ActivityReporter {
  private readonly scopes = new ActivityScopes();
  private connections = new Map<object, { environment: string; send: Sender }>();
  private releases = new Map<object, () => void>();
  /** When the debounced report is due; `asked` while no clock has said when it was asked. */
  private dueAt: number | null = null;
  private asked = false;
  private nextCadenceAt = 0;
  private reporting = false;
  private reportAgain = false;
  private remaining = 0;
  /** The reporter's start until the window reports an interaction, as the reference's starts at its clock. */
  private lastInteraction: number | null = null;
  private visible = false;
  private focused = false;
  /** A reporter whose facts come from the page holds its first report until they arrive. */
  private factsKnown = false;

  constructor(private readonly clientId: () => string, private readonly wall: (now: number) => string) {}

  connect(id: object, environment: string, send: Sender): void {
    this.connections.set(id, { environment, send });
    this.requestReport();
  }
  disconnect(id: object): void { if (this.connections.delete(id)) this.requestReport(); }
  changed(): void { this.requestReport(); }
  facts(visible: boolean, focused: boolean): void {
    if (this.factsKnown && visible === this.visible && focused === this.focused) return;
    this.visible = visible; this.focused = focused; this.factsKnown = true;
    this.requestReport();
  }
  /** The window's last interaction, on the tick's clock: becoming recent again is reported, as the reference's listeners do. */
  interaction(at: number): void {
    if (this.lastInteraction !== null && !(at > this.lastInteraction)) return;
    const recent = this.lastInteraction !== null && recentlyInteracted(this.lastInteraction, at);
    this.lastInteraction = at;
    if (!recent) this.requestReport();
  }
  retain(id: object, environment: string, method: string, payload: Record<string, unknown>): void {
    this.releases.get(id)?.();
    const release = this.scopes.retain(environment, method, payload);
    if (release) { this.releases.set(id, release); this.requestReport(); } else this.releases.delete(id);
  }
  release(id: object): void {
    const release = this.releases.get(id);
    if (!release) return;
    this.releases.delete(id); release(); this.requestReport();
  }

  private requestReport(): void { this.asked = true; }

  /** Whether the tick should come back within a second: a debounced report waits. */
  get soon(): boolean { return this.asked || this.dueAt !== null; }
  get live(): boolean { return this.connections.size > 0; }

  /** The tick: the debounce, then the cadence. */
  tick(now: number): void {
    this.lastInteraction ??= now;
    if (this.asked) { this.asked = false; this.dueAt = now + DEBOUNCE_MS; }
    if (this.connections.size && now >= this.nextCadenceAt) {
      if (this.nextCadenceAt) this.dueAt = this.dueAt ?? now;
      this.nextCadenceAt = now + REPORT_INTERVAL_MS;
    }
    if (!this.connections.size) this.nextCadenceAt = 0;
    if (this.dueAt !== null && now >= this.dueAt) { this.dueAt = null; this.report(now); }
  }

  private report(now: number): void {
    if (!this.factsKnown) return;
    if (this.reporting) { this.reportAgain = true; return; }
    if (!this.connections.size) return;
    const clientId = this.clientId();
    const seen = new Set<string>();
    const targets = [...this.connections.values()].filter(entry => !seen.has(entry.environment) && (seen.add(entry.environment), true));
    this.reporting = true;
    this.remaining = targets.length;
    for (const entry of targets) {
      entry.send({ environmentId: entry.environment, clientId, clientKind: 'desktop-renderer', visible: this.visible, focused: this.focused,
        recentlyInteracted: recentlyInteracted(this.lastInteraction ?? now, now), appState: this.visible ? 'active' : 'background', ttlMs: LEASE_TTL_MS,
        observedAt: this.wall(now), scopes: [{ type: 'provider-status' }, ...this.scopes.values(entry.environment)] }, () => {
        this.remaining -= 1;
        if (this.remaining > 0) return;
        this.reporting = false;
        if (this.reportAgain) { this.reportAgain = false; this.requestReport(); }
      });
    }
  }
}
