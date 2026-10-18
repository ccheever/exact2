export type ComposerFieldIdentity = { generation: number; snapshotOwner: string; owner: string };
export type ComposerFieldSnapshot = ComposerFieldIdentity & {
  value: string; start: number; end: number; composing: boolean; live: boolean;
};
export type ComposerFieldEdit = ComposerFieldIdentity & {
  value: string; start: number; end: number; text: string;
  selectionStart: number; selectionEnd: number;
};
export type ComposerFieldEffects = { focus: boolean; then: '' | 'models' | 'mode-plan' | 'mode-default'; history?: number; citation?: number; completion?: number };
export type ComposerFieldCommand = ComposerFieldEdit & ComposerFieldEffects & { seq: number; ready: boolean; history: number; citation: number; completion: number };
export type ComposerFieldResult = { applied: boolean; reason: string; value: string; start: number; end: number };
export type ComposerFieldAdmission = { accepted: boolean; queued: boolean; applied: false; seq: number; reason: string };
export type ComposerFieldCompleted = { command: ComposerFieldCommand; result: ComposerFieldResult };
type Issued = { command: ComposerFieldCommand; delivered: boolean; observedInput: boolean; inputSerial: number };
const identityMatches = (a: ComposerFieldIdentity, b: ComposerFieldIdentity) => a.generation === b.generation
  && a.snapshotOwner === b.snapshotOwner && a.owner === b.owner;
const rangeValid = (value: string, start: number, end: number) => Number.isSafeInteger(start)
  && Number.isSafeInteger(end) && start >= 0 && end >= start && end <= value.length;
const nextValue = (edit: ComposerFieldEdit) => edit.value.slice(0, edit.start) + edit.text + edit.value.slice(edit.end);

/** Admission and reads finish now; independent input/ACK completes the command. */
export class ComposerFieldCommands {
  private snapshot: ComposerFieldSnapshot | null = null;
  private requests: Issued[] = [];
  private completed: ComposerFieldCompleted[] = [];
  private sequence = 0;
  private disposed = false;

  read(): ComposerFieldSnapshot | null { return this.snapshot ? { ...this.snapshot } : null; }

  observe(snapshot: ComposerFieldSnapshot, input: boolean, inputSerial = -1): void {
    const previous = this.snapshot;
    this.snapshot = { ...snapshot };
    if (!snapshot.live || previous && !identityMatches(previous, snapshot)) {
      this.rejectAll(!snapshot.live ? 'field unavailable' : 'field owner changed');
      return;
    }
    // An issued edit may publish its expected input before the ACK source runs.
    for (const request of [...this.requests]) {
      if (input && !request.observedInput && request.delivered && !snapshot.composing && snapshot.value === nextValue(request.command)) { request.observedInput = true; request.inputSerial = inputSerial; }
      if (!request.observedInput && (input || !request.delivered) && (snapshot.composing || snapshot.value !== request.command.value)) {
        this.reject(request, snapshot.composing ? 'field composing' : 'field source changed');
      }
    }
  }

  request(edit: ComposerFieldEdit, effects: ComposerFieldEffects = { focus: true, then: '' }): ComposerFieldAdmission {
    const snapshot = this.snapshot;
    const refused = (reason: string): ComposerFieldAdmission => ({ accepted: false, queued: false, applied: false, seq: 0, reason });
    if (this.disposed || !snapshot?.live) return refused('field unavailable');
    if (snapshot.composing) return refused('field composing');
    if (!identityMatches(edit, snapshot) || edit.value !== snapshot.value) return refused('field source changed');
    const value = nextValue(edit);
    if (!rangeValid(edit.value, edit.start, edit.end) || !rangeValid(value, edit.selectionStart, edit.selectionEnd)) return refused('field range invalid');
    if (this.requests.length >= 16) return refused('field request queue full');
    const command: ComposerFieldCommand = { ...edit, ...effects, history: effects.history ?? 0, citation: effects.citation ?? 0, completion: effects.completion ?? 0, seq: ++this.sequence, ready: true };
    this.requests.push({ command, delivered: false, observedInput: false, inputSerial: -1 });
    return { accepted: true, queued: true, applied: false, seq: command.seq, reason: '' };
  }

  wait(after: number, identity: ComposerFieldIdentity): ComposerFieldCommand {
    if (this.disposed || !this.snapshot?.live || !identityMatches(identity, this.snapshot)) return this.empty(after, identity);
    const next = this.available(after, identity);
    if (next) { next.delivered = true; return { ...next.command }; }
    return this.empty(after, identity);
  }

  generatedInputSerial(seq: number): number { return this.requests.find(item => item.command.seq === seq)?.inputSerial ?? -1; }
  awaitingInput(seq: number): boolean { return this.requests.some(item => item.command.seq === seq && item.delivered && !item.observedInput); }

  drain(): ComposerFieldCompleted[] { return this.completed.splice(0); }
  cancel(seq: number, reason: string): void {
    const request = this.requests.find(item => item.command.seq === seq);
    if (request) this.reject(request, reason);
  }

  acknowledge(seq: number, identity: ComposerFieldIdentity, observed: { value: string; start: number; end: number; composing: boolean }, applied: boolean): ComposerFieldResult {
    const request = this.requests.find(item => item.command.seq === seq), snapshot = this.snapshot;
    if (!request || !snapshot || !identityMatches(identity, request.command) || !identityMatches(identity, snapshot)) return this.result(false, 'field acknowledgement stale');
    const successful = applied && request.observedInput && snapshot.live && observed.value === snapshot.value
      && rangeValid(observed.value, observed.start, observed.end);
    const result: ComposerFieldResult = { applied: successful, reason: successful ? '' : 'field edit refused',
      value: observed.value, start: observed.start, end: observed.end };
    this.completed.push({ command: request.command, result });
    this.requests = this.requests.filter(item => item !== request);
    return result;
  }

  dispose(): void { this.disposed = true; this.rejectAll('field disposed'); }

  private result(applied: boolean, reason: string): ComposerFieldResult {
    return { applied, reason, value: this.snapshot?.value ?? '', start: this.snapshot?.start ?? 0, end: this.snapshot?.end ?? 0 };
  }
  private empty(seq: number, identity: ComposerFieldIdentity): ComposerFieldCommand {
    return { generation: identity.generation, snapshotOwner: identity.snapshotOwner, owner: identity.owner, seq, ready: false, focus: false, then: '', history: 0, citation: 0, completion: 0, value: '', start: 0, end: 0, text: '', selectionStart: 0, selectionEnd: 0 };
  }
  private reject(request: Issued, reason: string): void {
    this.completed.push({ command: request.command, result: this.result(false, reason) });
    this.requests = this.requests.filter(item => item !== request);
  }
  private rejectAll(reason: string): void {
    for (const request of [...this.requests]) this.reject(request, reason);
  }
  private available(after: number, identity: ComposerFieldIdentity): Issued | undefined {
    // Advancing the resource cursor happens before the field's generated input.
    // Keep later requests undelivered until the active edit is acknowledged.
    const active = this.requests.find(request => request.delivered);
    const matches = (request: Issued) => request.command.seq > after && identityMatches(request.command, identity);
    return active ? matches(active) ? active : undefined : this.requests.find(matches);
  }
}
