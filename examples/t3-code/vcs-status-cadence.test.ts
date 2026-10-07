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

/** vcs.refreshStatus replies held until the answer that asked is let go; returns the count sent. */
function holdRefreshStatus(native: Fake): { sent: () => number; letGo: () => void } {
  const later = native.later.bind(native);
  let sent = 0, held: Array<(error: unknown) => void> = [];
  native.later = async (input: unknown) => {
    const request = obj(input);
    if (request.op === 'request' && request.method === 'vcs.refreshStatus') {
      sent++;
      return new Promise((_resolve, reject) => { held.push(reject); });
    }
    return later(input);
  };
  return { sent: () => sent, letGo: () => { const now = held; held = []; now.forEach(reject => reject(new FetchError())); } };
}

/** The window's answers for one tick: each answer gets its own seam, as app.ts gives it. */
function ask(client: Awaited<ReturnType<typeof connected>>['client'], native: Native, now: number) {
  noteNow(client, now);
  const settle = (work: Promise<unknown>) => work.catch(error => { if (!(error instanceof ClientError)) throw error; });
  return [settle(shellDetails(client, letGoAware(native), false, '', false, now, 0, true)), settle(composerBranches(client, letGoAware(native), false, ''))];
}

describe('git status cadence', () => {
  test('with a toast ticking and the replies held for 2 s, the strip and the card ask the server for status at most once', async () => {
    const { client, native } = await connected();
    const held = holdRefreshStatus(native);
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

  test('the strip shows the branch the status stream reports, with no vcs.refreshStatus', async () => {
    const { client, native, disk } = await connected();
    const held = holdRefreshStatus(native);
    const later = native.later.bind(native);
    native.later = async (input: unknown) => {
      const request = obj(input), reply = await later(input);
      if (request.op === 'subscribe' && request.key === VCS_STATUS_KEY) {
        native.emit(VCS_STATUS_KEY, { _tag: 'snapshot', local: { isRepo: true, refName: 'feature', hasPrimaryRemote: false, isDefaultRef: false, hasWorkingTreeChanges: false,
          workingTree: { files: [], insertions: 0, deletions: 0 } }, remote: null });
      }
      return reply;
    };
    // The card is closed: its answer keeps the stream on the strip's workspace.
    await shellDetails(client, native, false, '', false, 1_800_000_000_000, 0, true);
    await client.refresh(native, disk); // the stream's snapshot is drained
    const strip = await composerBranches(client, native, false, '');
    expect(strip).toMatchObject({ show: true, branchLabel: 'feature' });
    expect(held.sent()).toBe(0);
    expect(str(native.subs[VCS_STATUS_KEY])).not.toBe('');
  });
});
