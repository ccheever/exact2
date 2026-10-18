export type ComposerFieldIdentity = { generation: number; snapshotOwner: string; owner: string };
export type ComposerFieldSnapshot = ComposerFieldIdentity & {
  value: string; start: number; end: number; composing: boolean; live: boolean;
};
export type ComposerFieldEdit = ComposerFieldIdentity & {
  value: string; start: number; end: number; text: string;
  selectionStart: number; selectionEnd: number;
};
export type ComposerFieldEffects = { focus: boolean; then: '' | 'models' | 'mode-plan' | 'mode-default'; history?: number; citation?: number };
export type ComposerFieldCommand = ComposerFieldEdit & ComposerFieldEffects & { seq: number; ready: boolean; history: number; citation: number };
export type ComposerFieldResult = { applied: boolean; reason: string; value: string; start: number; end: number };
type Issued = { command: ComposerFieldCommand; delivered: boolean; observedInput: boolean; resolve: (result: ComposerFieldResult) => void };
type Waiting = { after: number; identity: ComposerFieldIdentity; resolve: (command: ComposerFieldCommand) => void };
const identityMatches = (a: ComposerFieldIdentity, b: ComposerFieldIdentity) => a.generation === b.generation
  && a.snapshotOwner === b.snapshotOwner && a.owner === b.owner;
const rangeValid = (value: string, start: number, end: number) => Number.isSafeInteger(start)
  && Number.isSafeInteger(end) && start >= 0 && end >= start && end <= value.length;
const nextValue = (edit: ComposerFieldEdit) => edit.value.slice(0, edit.start) + edit.text + edit.value.slice(edit.end);

/** App edit requests wait for Contract's shared field command and its actual
 *  input acknowledgement. This owns no native text, history or timing loop. */
export class ComposerFieldCommands {
  private snapshot: ComposerFieldSnapshot | null = null;
  private requests: Issued[] = [];
  private waiting: Waiting | null = null;
  private sequence = 0;
  private disposed = false;

  read(): ComposerFieldSnapshot | null { return this.snapshot ? { ...this.snapshot } : null; }

  observe(snapshot: ComposerFieldSnapshot, input: boolean): void {
    const previous = this.snapshot;
    this.snapshot = { ...snapshot };
    if (!snapshot.live || previous && !identityMatches(previous, snapshot)) {
      this.rejectAll(!snapshot.live ? 'field unavailable' : 'field owner changed');
      return;
    }
    // An issued edit may publish its expected input before the ACK source runs.
    for (const request of [...this.requests]) {
      if (input && request.delivered && !snapshot.composing && snapshot.value === nextValue(request.command)) request.observedInput = true;
      if (!request.observedInput && (input || !request.delivered) && (snapshot.composing || snapshot.value !== request.command.value)) {
        this.reject(request, snapshot.composing ? 'field composing' : 'field source changed');
      }
    }
    this.deliver();
  }

  request(edit: ComposerFieldEdit, effects: ComposerFieldEffects = { focus: true, then: '' }): Promise<ComposerFieldResult> {
    const snapshot = this.snapshot;
    const refused = (reason: string) => Promise.resolve(this.result(false, reason));
    if (this.disposed || !snapshot?.live) return refused('field unavailable');
    if (snapshot.composing) return refused('field composing');
    if (!identityMatches(edit, snapshot) || edit.value !== snapshot.value) return refused('field source changed');
    const value = nextValue(edit);
    if (!rangeValid(edit.value, edit.start, edit.end) || !rangeValid(value, edit.selectionStart, edit.selectionEnd)) return refused('field range invalid');
    if (this.requests.length >= 16) return refused('field request queue full');
    return new Promise(resolve => {
      this.requests.push({ command: { ...edit, ...effects, history: effects.history ?? 0, citation: effects.citation ?? 0, seq: ++this.sequence, ready: true }, delivered: false, observedInput: false, resolve });
      this.deliver();
    });
  }

  wait(after: number, identity: ComposerFieldIdentity): ComposerFieldCommand | Promise<ComposerFieldCommand> {
    if (this.disposed || !this.snapshot?.live || !identityMatches(identity, this.snapshot)) return this.empty(after, identity);
    const next = this.available(after, identity);
    if (next) { next.delivered = true; return next.command; }
    // A replaced resource has no further deliveries. Retire its Promise too.
    if (this.waiting) this.waiting.resolve(this.empty(this.waiting.after, this.waiting.identity));
    return new Promise(resolve => { this.waiting = { after, identity: { ...identity }, resolve }; });
  }

  acknowledge(seq: number, identity: ComposerFieldIdentity, observed: { value: string; start: number; end: number; composing: boolean }, applied: boolean): ComposerFieldResult {
    const request = this.requests.find(item => item.command.seq === seq), snapshot = this.snapshot;
    if (!request || !snapshot || !identityMatches(identity, request.command) || !identityMatches(identity, snapshot)) return this.result(false, 'field acknowledgement stale');
    const successful = applied && request.observedInput && snapshot.live && observed.value === snapshot.value
      && rangeValid(observed.value, observed.start, observed.end);
    const result: ComposerFieldResult = { applied: successful, reason: successful ? '' : 'field edit refused',
      value: observed.value, start: observed.start, end: observed.end };
    request.resolve(result);
    this.requests = this.requests.filter(item => item !== request);
    this.deliver();
    return result;
  }

  dispose(): void { this.disposed = true; this.rejectAll('field disposed'); }

  private result(applied: boolean, reason: string): ComposerFieldResult {
    return { applied, reason, value: this.snapshot?.value ?? '', start: this.snapshot?.start ?? 0, end: this.snapshot?.end ?? 0 };
  }
  private empty(seq: number, identity: ComposerFieldIdentity): ComposerFieldCommand {
    return { ...identity, seq, ready: false, focus: false, then: '', history: 0, citation: 0, value: '', start: 0, end: 0, text: '', selectionStart: 0, selectionEnd: 0 };
  }
  private reject(request: Issued, reason: string): void {
    request.resolve(this.result(false, reason));
    this.requests = this.requests.filter(item => item !== request);
  }
  private rejectAll(reason: string): void {
    for (const request of [...this.requests]) this.reject(request, reason);
    if (this.waiting) { this.waiting.resolve(this.empty(this.waiting.after, this.waiting.identity)); this.waiting = null; }
  }
  private available(after: number, identity: ComposerFieldIdentity): Issued | undefined {
    // Advancing the resource cursor happens before the field's generated input.
    // Keep later requests undelivered until the active edit is acknowledged.
    const active = this.requests.find(request => request.delivered);
    const matches = (request: Issued) => request.command.seq > after && identityMatches(request.command, identity);
    return active ? matches(active) ? active : undefined : this.requests.find(matches);
  }
  private deliver(): void {
    const waiting = this.waiting;
    if (!waiting) return;
    const next = this.available(waiting.after, waiting.identity);
    if (next) { next.delivered = true; this.waiting = null; waiting.resolve(next.command); }
  }
}
