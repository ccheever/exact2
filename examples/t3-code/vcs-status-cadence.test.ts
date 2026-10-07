// Round 5: how often the clone asks the server for git status. The reference
// (T3 Code 1e2ecbd975) reads a workspace's status from the subscribeVcsStatus
// stream (BranchToolbarBranchSelector, GitActionsControl: `vcsEnvironment.status`)
// and asks vcs.refreshStatus only on window focus, on becoming visible and when a
// git menu opens. While a toast is up, the window asks the strip and the details
// card again every 500 ms (app.contract `shellTick`), and Exact lets the previous
// answers go. The strip used to ask vcs.refreshStatus until a reply arrived, so a
// slow reply (it can wait on a remote fetch) was asked for on every tick.
import { describe, expect, test } from 'bun:test';
import { obj, str } from './domain';
import { ClientError, type Native } from './protocol';
import { composerBranches } from './composer-controls-branch';
import { noteNow } from './composer-controls';
import { shellDetails } from './shell-details';
import { VCS_STATUS_KEY } from './shell-vcs';
import { letGoAware } from './let-go';
import { connected, type Fake } from './composer-controls-fixture';

// The prelude's FetchError for an answer Exact let go (js/src/prelude.js).
class FetchError extends Error {
  readonly kind = 'Aborted';
  constructor() { super('the answer was let go before this reply; the request may already have been sent'); this.name = 'FetchError'; }
}
const flush = () => new Promise(resolve => setTimeout(resolve, 0));

/** Replies to these server methods held until the answer that asked is let go; counts what was sent. */
function holdReads(native: Fake, methods = ['vcs.refreshStatus']): { sent: (method?: string) => number; letGo: () => void } {
  const later = native.later.bind(native);
  const sent = new Map<string, number>();
  let held: Array<(error: unknown) => void> = [];
  native.later = async (input: unknown) => {
    const request = obj(input), method = str(request.method);
    if (request.op === 'request' && methods.includes(method)) {
      sent.set(method, (sent.get(method) ?? 0) + 1);
      return new Promise((_resolve, reject) => { held.push(reject); });
    }
    return later(input);
  };
  return { sent: (method = 'vcs.refreshStatus') => sent.get(method) ?? 0, letGo: () => { const now = held; held = []; now.forEach(reject => reject(new FetchError())); } };
}
/** The server's subscribeVcsStatus stream: a snapshot follows each subscribe. */
function streamStatus(native: Fake, refName = 'main'): void {
  const later = native.later.bind(native);
  native.later = async (input: unknown) => {
    const request = obj(input), reply = await later(input);
    if (request.op === 'subscribe' && request.key === VCS_STATUS_KEY) {
      native.emit(VCS_STATUS_KEY, { _tag: 'snapshot', local: { isRepo: true, refName, hasPrimaryRemote: false, isDefaultRef: refName === 'main', hasWorkingTreeChanges: false,
        workingTree: { files: [], insertions: 0, deletions: 0 } }, remote: null });
    }
    return reply;
  };
}

/** The window's answers for one tick: each answer gets its own seam, as app.ts gives it. */
function ask(client: Awaited<ReturnType<typeof connected>>['client'], native: Native, now: number, cardOpen = false) {
  noteNow(client, now);
  const settle = (work: Promise<unknown>) => work.catch(error => { if (!(error instanceof ClientError)) throw error; });
  return [settle(shellDetails(client, letGoAware(native), cardOpen, '', false, now, 0, true)), settle(composerBranches(client, letGoAware(native), false, '', false, now))];
}

describe('git status cadence', () => {
  test('with a toast ticking and the replies held for 2 s, the strip and the card ask the server for status at most once', async () => {
    const { client, native } = await connected();
    const held = holdReads(native);
    const start = 1_800_000_000_000;
    for (let tick = 0; tick <= 4; tick++) {
      held.letGo(); // the previous tick's answers are let go
      ask(client, native, start + tick * 500);
      await flush();
    }
    held.letGo();
    await flush();
    expect(held.sent()).toBeLessThanOrEqual(1);
  });

  test('with the card open, its stream delivered and the replies held for 2 s, at most one status read and one ref read', async () => {
    const { client, native, disk } = await connected();
    streamStatus(native);
    const held = holdReads(native, ['vcs.refreshStatus', 'vcs.listRefs']);
    const start = 1_800_000_000_000;
    ask(client, native, start, true); // the card subscribes; its status is not drained yet
    await flush();
    await client.refresh(native, disk); // the stream's snapshot arrives
    for (let tick = 1; tick <= 4; tick++) {
      held.letGo();
      ask(client, native, start + tick * 500, true);
      await flush();
    }
    held.letGo();
    await flush();
    // At most the strip's first read, when it is asked before the card's stream follows its workspace.
    expect(held.sent('vcs.refreshStatus')).toBeLessThanOrEqual(1);
    expect(held.sent('vcs.listRefs')).toBeLessThanOrEqual(1);
  });

  test('the strip shows the branch the status stream reports, with no vcs.refreshStatus', async () => {
    const { client, native, disk } = await connected();
    const held = holdReads(native);
    streamStatus(native, 'feature');
    // The card is closed: its answer keeps the stream on the strip's workspace.
    await shellDetails(client, native, false, '', false, 1_800_000_000_000, 0, true);
    await client.refresh(native, disk); // the stream's snapshot is drained
    const strip = await composerBranches(client, native, false, '');
    expect(strip).toMatchObject({ show: true, branchLabel: 'feature' });
    expect(held.sent()).toBe(0);
    expect(str(native.subs[VCS_STATUS_KEY])).not.toBe('');
  });
});
