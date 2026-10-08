// The Code tab's Viewed marks (20261005-pr-code-tab): usePullRequestFilesViewed (T3 Code 1e2ecbd975,
// MIT, see LICENSE-T3: apps/web/src/components/pullRequest/usePullRequestFilesViewed.ts) as a plain
// store the panel's resource and its commands share, one per change request on one environment.
//
// Presses show at once (the overlay) and gather until the flush: the panel's resource waits the
// reference's 400 ms with the native sleep once the presses stop moving `key()` (X19: a data source
// has no timer; a press meanwhile asks the resource again and lets the wait go), and the flush sends
// one `pullRequests.setFilesViewed` of at most 500 presses, detached from the answer that sent it
// (composer-replies.ts). One write is in flight per change request (the reference's serial command
// scheduler); presses made while a write is out wait for the next flush. A press is held over the host's answers until a read that could have seen
// it comes back (`answeredFrom`); a failed write takes its own presses back and says "Could not
// update viewed files" unless nothing on screen went back or the connection went away.
import {
  countViewedFiles, isFileViewed, isStaleViewedState, revertFileViewedOverlay, settleFileViewedOverlay, toFileViewedBatch, toFileViewedStates,
  type FileViewedOverlay, type FileViewedStates, type PullRequestFileViewedState,
} from './pages-pr-code-logic';

/** MAX_FILES_VIEWED_PRESSES: what one write may carry (the contract refuses more). */
export const MAX_FILES_VIEWED_PRESSES = 500;
/** FLUSH_DELAY_MS: how long presses gather before the host is told (the root task's delay). */
export const FLUSH_DELAY_MS = 400;
const NO_OVERLAY: FileViewedOverlay = new Map();

export type ViewedBatch = { request: number; batch: { path: string; viewed: boolean }[] };

export class FilesViewedStore {
  /** The host's last answer, kept through a failed read (the boxes stay where it last put them). */
  states: FileViewedStates | null = null;
  truncated = false;
  /** Why the marks could not be read, when they could not. */
  error: string | null = null;
  /** A read is owed: the first, after a write was acknowledged, or the panel's refresh. */
  due = true;
  overlay: FileViewedOverlay = NO_OVERLAY;
  /** Presses waiting for the next flush, and which request carries each path already sent. */
  private queued = new Map<string, boolean>();
  private sentBy = new Map<string, number>();
  private requests = 0;
  /** The host's answer as it stood when a write for the path was acknowledged. */
  private answeredFrom = new Map<string, FileViewedStates | null>();
  private presses = 0;
  private flushes = 0;

  /** A read landed: adopt it, and retire the presses it answers for. */
  adopt(result: { files: readonly { path: string; state: PullRequestFileViewedState }[]; truncated?: boolean }): void {
    const states = toFileViewedStates({ files: result.files });
    this.states = states; this.truncated = result.truncated === true; this.error = null; this.due = false;
    const pending = new Set([...this.queued.keys(), ...this.sentBy.keys()]), answered = new Set<string>();
    for (const [path, from] of this.answeredFrom) {
      if (pending.has(path) || from === states) continue;
      answered.add(path); this.answeredFrom.delete(path);
    }
    this.overlay = settleFileViewedOverlay(this.overlay, states, pending, answered);
  }
  /** A read failed: the last answer stays, and the error travels with it. */
  failed(message: string): void { this.error = message; this.due = false; }
  setViewed(path: string, viewed: boolean): void {
    this.overlay = new Map(this.overlay).set(path, viewed);
    this.queued.set(path, viewed); this.presses += 1;
  }
  /** How many presses wait for a flush, and the key that re-arms the flush timer on each press. */
  queuedCount(): number { return this.queued.size; }
  key(): string { return `${this.presses}:${this.flushes}`; }
  /** Presses wait and no write is out: what the resource's wait-then-flush is for. */
  flushable(): boolean { return this.queued.size > 0 && this.sentBy.size === 0; }
  /** A write that never left (its answer was let go first): its presses wait for the next flush, unless pressed again since. */
  requeue(taken: ViewedBatch): void {
    for (const file of taken.batch) {
      if (this.sentBy.get(file.path) !== taken.request) continue;
      this.sentBy.delete(file.path);
      if (!this.queued.has(file.path)) this.queued.set(file.path, file.viewed);
    }
    this.flushes += 1;
  }
  /** The flush: up to 500 queued presses, carried by a new request from here on. */
  takeBatch(): ViewedBatch | null {
    this.flushes += 1;
    const batch = toFileViewedBatch(this.queued).slice(0, MAX_FILES_VIEWED_PRESSES);
    if (batch.length === 0) return null;
    for (const file of batch) this.queued.delete(file.path);
    const request = ++this.requests;
    for (const file of batch) this.sentBy.set(file.path, request);
    return { request, batch };
  }
  /**
   * A write answered. `ok`: from the next read on, the host answers for these presses (a read is
   * owed). Failed: the presses this request still owns go back; the result says whether anything
   * on screen did. `unknown`: the reply never came (the connection went, or the answer was let go)
   * — the presses are answered for by the next read, which says where they landed.
   */
  landed(taken: ViewedBatch, outcome: 'ok' | 'failed' | 'unknown'): { reverted: boolean } {
    const mine = taken.batch.map(file => file.path).filter(path => this.sentBy.get(path) === taken.request);
    for (const path of mine) this.sentBy.delete(path);
    if (outcome === 'failed') {
      const owned = new Set(mine.filter(path => !this.queued.has(path)));
      this.overlay = revertFileViewedOverlay(this.overlay, taken.batch, owned);
      return { reverted: owned.size > 0 };
    }
    for (const path of mine) this.answeredFrom.set(path, this.states);
    this.due = true;
    return { reverted: false };
  }
  isViewed(path: string): boolean { return isFileViewed(path, this.states, this.overlay); }
  /** Pushed to since it was cleared, and not pressed since. */
  isStale(path: string): boolean { return !this.overlay.has(path) && isStaleViewedState(this.states?.get(path)); }
  count(paths: readonly string[]): number { return countViewedFiles(paths, this.states, this.overlay); }
}
