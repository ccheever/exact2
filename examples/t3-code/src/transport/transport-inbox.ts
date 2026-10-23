// The retained inbox of one connection: every stream value a subscription delivered, in order,
// until the reader acknowledges it. Reading never removes events, so an answer Exact lets go
// before it lands loses nothing; the next reader reads the same page. A server Ack (sent as each
// chunk arrives) means retained here; the reader's `ack` means reduced by the client.
import { utf8Length } from './transport-wire';

export interface InboxEntry { seq: number; generation: number; key: string; subscriptionId: string; value: unknown }

export class Inbox {
  latest = 0;
  floor = 0;
  bytes = 0;
  entries: Array<{ seq: number; bytes: number; entry: InboxEntry }> = [];
  /** One notification advertises all retained work until the reader acknowledges it. */
  private notificationPending = false;
  constructor(readonly limit = 4096, readonly byteLimit = 16 * 1024 * 1024) {}

  /** Retains one value; true when the reader should be told (nothing was waiting for it yet). */
  append(generation: number, key: string, subscriptionId: string, value: unknown): boolean {
    const size = utf8Length(JSON.stringify(value) ?? 'null');
    const notify = !this.notificationPending;
    this.notificationPending = true;
    this.latest += 1;
    if (this.entries.length >= this.limit || this.bytes + size > this.byteLimit) {
      // An explicit reset: the reader must ask for authoritative snapshots.
      this.entries = []; this.bytes = 0; this.floor = this.latest;
    }
    if (size > this.byteLimit) return notify;
    this.entries.push({ seq: this.latest, bytes: size, entry: { seq: this.latest, generation, key, subscriptionId, value } });
    this.bytes += size;
    return notify;
  }

  /** A page after `after`, at most about 512 KiB; `reset` when entries the reader has not seen were dropped. */
  read(after: number): { events: InboxEntry[]; latest: number; reset: boolean } {
    const events: InboxEntry[] = [];
    let used = 0;
    for (const item of this.entries) {
      if (item.seq <= after) continue;
      if (events.length && used + item.bytes + 256 > 512 * 1024) break;
      events.push(item.entry); used += item.bytes + 256;
    }
    return { events, latest: this.latest, reset: after < this.floor || after > this.latest };
  }

  acknowledge(through: number): void {
    const accepted = Math.min(Math.max(through, 0), this.latest);
    let removed = 0;
    while (removed < this.entries.length && this.entries[removed]!.seq <= accepted) { this.bytes -= this.entries[removed]!.bytes; removed++; }
    this.entries.splice(0, removed);
    this.floor = Math.max(this.floor, accepted);
    if (accepted === this.latest) this.notificationPending = false;
  }

  reset(): void { this.entries = []; this.bytes = 0; this.floor = this.latest; this.notificationPending = false; }
}
