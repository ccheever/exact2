// The screen reader announcers (announce.ts). Ported from T3 Code 7c3b3b3605ef (MIT, see LICENSE-T3):
// components/chat/threadActivityAnnouncement.test.ts and components/backgroundThreadAnnouncement.test.ts under their own
// names (run ids are strings; announcedRun takes the latest executed run). Clone rows (marked) cover the regions the
// window draws, the open thread read from the client, and lib/announce.ts's queue.
import { describe, expect, it } from 'bun:test';
import {
  CLEAR_AFTER_MS, announce, announcedRun, announcements, announcing, cancelAnnouncement, nextBackgroundThreadAnnouncement, nextThreadAnnouncement, resolveBackgroundThreadRequest,
  type BackgroundThreadMemory, type BackgroundThreadSnapshot, type ThreadActivitySnapshot, type ThreadAnnouncementMemory,
} from '../../src/shell/announce';
import { EnvironmentFleet } from '../../src/connections/settings-b-fleet';
import type { T3Client } from '../../src/client/client';
import { str, type Obj } from '../../src/client/domain';

const firstRun = "run-1";
const secondRun = "run-2";

const idle = (
  runId: string | null,
  runStatus: ThreadActivitySnapshot["runStatus"],
): ThreadActivitySnapshot => ({ status: "idle", requestId: null, runId, runStatus });
const busy = (
  status: Exclude<ThreadActivitySnapshot["status"], "idle"> = "working",
  runId: string | null = secondRun,
  runStatus: ThreadActivitySnapshot["runStatus"] = "running",
  requestId: string | null = status === "working" ? null : "request-1",
): ThreadActivitySnapshot => ({ status, requestId, runId, runStatus });

function speak(snapshots: ReadonlyArray<ThreadActivitySnapshot>) {
  let memory: ThreadAnnouncementMemory | null = null;
  const spoken: string[] = [];
  for (const snapshot of snapshots) {
    const result = nextThreadAnnouncement(memory, snapshot);
    memory = result.memory;
    if (result.announcement !== null) spoken.push(result.announcement);
  }
  return spoken;
}

describe("nextThreadAnnouncement", () => {
  it("follows a turn from start to finish", () => {
    expect(
      speak([idle(firstRun, "completed"), busy(), busy(), idle(secondRun, "completed")]),
    ).toEqual(["Working", "Done"]);
  });

  it("says Working once when a send looks idle before its run appears", () => {
    expect(
      speak([idle(null, null), busy(), idle(null, null), busy(), idle(secondRun, "completed")]),
    ).toEqual(["Working", "Done"]);
  });

  it("announces a queued message that starts as the last turn ends", () => {
    const thirdRun = "run-3";
    expect(
      speak([
        idle(firstRun, "completed"),
        busy("working", firstRun, "completed"),
        busy(),
        busy("working", secondRun, "completed"),
        busy("working", thirdRun, "starting"),
        idle(thirdRun, "completed"),
      ]),
    ).toEqual(["Working", "Working", "Done"]);
    // The last turn's run is still settling when the follow-up is sent.
    expect(
      speak([
        idle(firstRun, "running"),
        busy("working", firstRun, "running"),
        busy("working", secondRun, "starting"),
      ]),
    ).toEqual(["Working"]);
    expect(
      speak([
        idle(firstRun, "running"),
        busy("working", firstRun, "running"),
        idle(firstRun, "completed"),
        busy("working", secondRun, "starting"),
        idle(secondRun, "completed"),
      ]),
    ).toEqual(["Working", "Done"]);
    expect(
      speak([
        busy("working", firstRun, "running"),
        idle(firstRun, "running"),
        busy("working", firstRun, "running"),
        busy("working", secondRun, "starting"),
      ]),
    ).toEqual(["Working"]);
  });

  it("treats a run that starts while the thread looks idle as the next turn", () => {
    const thirdRun = "run-3";
    // A resume or a turn started from another device.
    expect(
      speak([
        idle(firstRun, "completed"),
        idle(secondRun, "starting"),
        busy("working", secondRun, "running"),
        idle(secondRun, "completed"),
      ]),
    ).toEqual(["Working", "Done"]);
    // A queued message waiting to start after the last turn.
    expect(
      speak([
        idle(firstRun, "completed"),
        busy(),
        idle(thirdRun, "queued"),
        idle(thirdRun, "starting"),
        busy("working", thirdRun, "running"),
        idle(thirdRun, "completed"),
      ]),
    ).toEqual(["Working", "Working", "Done"]);
  });

  it("stays silent when a thread opens mid-turn or after one", () => {
    expect(speak([idle(firstRun, "completed"), idle(firstRun, "completed")])).toEqual([]);
    expect(speak([busy(), busy()])).toEqual([]);
  });

  it("announces the end of a turn that was already running when the thread opened", () => {
    expect(speak([busy(), idle(secondRun, "failed")])).toEqual(["Failed"]);
  });

  it("says how the turn ended when a queued message was cancelled before it started", () => {
    // The clone hands announcedRun the latest executed run (composer-controls.ts latestExecutedRun) for `runs`.
    const ended = announcedRun({ runId: "run-3", status: "cancelled", startedAt: null }, { runId: secondRun, status: "completed" });
    expect(speak([busy(), idle(ended?.runId ?? null, ended?.status ?? null)])).toEqual(["Done"]);
  });

  it("waits for the run to settle before announcing how the turn ended", () => {
    expect(
      speak([
        idle(firstRun, "completed"),
        busy(),
        idle(secondRun, "running"),
        idle(secondRun, "interrupted"),
      ]),
    ).toEqual(["Working", "Stopped"]);
  });

  it("does not report the previous turn's ending when work stops before a new run exists", () => {
    expect(
      speak([
        idle(firstRun, "completed"),
        busy("working", firstRun, "completed"),
        idle(firstRun, "completed"),
        busy(),
        idle(secondRun, "completed"),
      ]),
    ).toEqual(["Working", "Working", "Done"]);
    // The previous run finished while this client looked idle.
    expect(
      speak([
        idle(firstRun, "completed"),
        idle(secondRun, "completed"),
        busy("working", secondRun, "completed"),
        idle(secondRun, "completed"),
      ]),
    ).toEqual(["Working"]);
  });

  it("lets the next turn start after one is rolled back", () => {
    expect(
      speak([
        idle(firstRun, "completed"),
        busy(),
        idle(secondRun, "rolled_back"),
        busy("working", "run-3"),
      ]),
    ).toEqual(["Working", "Working"]);
  });

  it("announces each request once without repeating that work resumed", () => {
    expect(
      speak([
        busy(),
        busy("approval"),
        busy("approval"),
        busy(),
        busy("input"),
        idle(secondRun, "completed"),
      ]),
    ).toEqual(["Needs approval", "Needs your input", "Done"]);
  });

  it("announces a second request of the same kind once the first is answered", () => {
    expect(
      speak([
        busy(),
        busy("approval", secondRun, "running", "request-1"),
        busy("approval", secondRun, "running", "request-2"),
      ]),
    ).toEqual(["Needs approval", "Needs approval"]);
  });
});

const bgFirst = "run-1";
const bgSecond = "run-2";
function bgThread(
  key: string,
  title: string,
  state: Partial<Omit<BackgroundThreadSnapshot, "key" | "title">> = {},
): BackgroundThreadSnapshot {
  return {
    key,
    title,
    request: null,
    requestId: null,
    runId: bgFirst,
    runStatus: "running",
    activeRunId: null,
    snoozed: false,
    ...state,
  };
}
const login = (state?: Partial<BackgroundThreadSnapshot>) =>
  bgThread("env:login", "Fix login redirect", state);
const docs = (state?: Partial<BackgroundThreadSnapshot>) =>
  bgThread("env:docs", "Update docs", state);

function speakBackground(
  frames: ReadonlyArray<{
    readonly threads: ReadonlyArray<BackgroundThreadSnapshot>;
    readonly open?: string;
  }>,
) {
  let memory: BackgroundThreadMemory = new Map();
  const spoken: string[] = [];
  for (const frame of frames) {
    const result = nextBackgroundThreadAnnouncement(memory, frame.threads, frame.open ?? null);
    memory = result.memory;
    if (result.announcement !== null) spoken.push(result.announcement);
  }
  return spoken;
}

describe("nextBackgroundThreadAnnouncement", () => {
  it("names the thread with the same words the open thread uses", () => {
    expect(
      speakBackground([
        { threads: [login()] },
        { threads: [login({ request: "approval" })] },
        { threads: [login()] },
        { threads: [login({ request: "input" })] },
        { threads: [login({ runStatus: "completed" })] },
        { threads: [login({ runId: bgSecond, runStatus: "failed" })] },
        { threads: [login({ runId: "run-3", runStatus: "interrupted" })] },
      ]),
    ).toEqual([
      "Needs approval: Fix login redirect",
      "Needs your input: Fix login redirect",
      "Done: Fix login redirect",
      "Failed: Fix login redirect",
      "Stopped: Fix login redirect",
    ]);
  });

  it("announces each run's ending once", () => {
    expect(
      speakBackground([
        { threads: [login()] },
        { threads: [login({ runStatus: "completed" })] },
        // Marking the thread unread or renaming it changes nothing about the run.
        { threads: [login({ runStatus: "completed", title: "Fix the login redirect" })] },
        // The next run can finish between two updates, so it never looks busy here.
        { threads: [login({ runId: bgSecond, runStatus: "completed" })] },
        { threads: [login({ runId: "run-3", runStatus: "rolled_back" })] },
      ]),
    ).toEqual(["Done: Fix login redirect", "Done: Fix login redirect"]);
  });

  it("announces a second request of the same kind", () => {
    expect(
      speakBackground([
        { threads: [login()] },
        { threads: [login({ request: "approval", requestId: "request-1" })] },
        { threads: [login({ request: "approval", requestId: "request-2" })] },
      ]),
    ).toEqual(["Needs approval: Fix login redirect", "Needs approval: Fix login redirect"]);
  });

  it("doesn't call a thread stopped when a queued message is cancelled while it works", () => {
    expect(
      speakBackground([
        { threads: [login({ activeRunId: bgFirst })] },
        {
          threads: [login({ runId: bgSecond, runStatus: "cancelled", activeRunId: bgFirst })],
        },
        { threads: [login({ runId: bgSecond, runStatus: "cancelled" })] },
      ]),
    ).toEqual([]);
  });

  it("stays quiet for threads it sees for the first time, like on launch or reconnect", () => {
    expect(
      speakBackground([
        { threads: [login({ runStatus: "completed" }), docs({ request: "approval" })] },
        { threads: [login({ runStatus: "completed" }), docs({ request: "approval" })] },
        // The environment drops out while it resyncs, then comes back changed.
        { threads: [] },
        {
          threads: [
            login({ runId: bgSecond, runStatus: "failed" }),
            docs({ runStatus: "completed" }),
          ],
        },
      ]),
    ).toEqual([]);
  });

  it("leaves the open thread to its own announcer", () => {
    expect(
      speakBackground([
        { threads: [login(), docs()], open: "env:login" },
        { threads: [login({ runStatus: "completed" }), docs()], open: "env:login" },
        // Switching away from a thread doesn't replay its ending.
        { threads: [login({ runStatus: "completed" }), docs()], open: "env:docs" },
        {
          threads: [login({ runStatus: "completed" }), docs({ request: "input" })],
          open: "env:login",
        },
      ]),
    ).toEqual(["Needs your input: Update docs"]);
  });

  it("keeps a snoozed thread quiet, even after it wakes", () => {
    expect(
      speakBackground([
        { threads: [login({ snoozed: true })] },
        { threads: [login({ snoozed: true, runStatus: "completed" })] },
        { threads: [login({ runStatus: "completed" })] },
      ]),
    ).toEqual([]);
  });

  it("speaks threads that change together in one announcement", () => {
    expect(
      speakBackground([
        { threads: [login(), docs()] },
        { threads: [login({ runStatus: "completed" }), docs({ request: "approval" })] },
      ]),
    ).toEqual(["Done: Fix login redirect. Needs approval: Update docs"]);
  });
});

describe("resolveBackgroundThreadRequest", () => {
  // The clone's shell carries the request as `pendingRuntimeRequest` and a hidden subagent's as `pendingSubagentRequest`.
  const childApproval = (parentQuestion: string | null) => resolveBackgroundThreadRequest({
    pendingSubagentRequest: "approval", pendingRuntimeRequest: parentQuestion === null ? null : { id: parentQuestion, kind: "user_input" },
  });
  it("raises a subagent's request on the parent without borrowing the parent's request id", () => {
    expect(childApproval(null)).toEqual({ request: "approval", requestId: null });
    expect(
      speakBackground([
        { threads: [login()] },
        { threads: [login(childApproval(null))] },
        { threads: [login(childApproval("question-1"))] },
        { threads: [login(childApproval("question-2"))] },
        { threads: [login(childApproval(null))] },
      ]),
    ).toEqual(["Needs approval: Fix login redirect"]);
  });
});

// ── The regions (clone) ──
function client(projection: Obj, threads: Obj[] = [{ id: 't1', title: 'Fix login', latestRunId: 'run-1', status: 'running', activityRunStatus: 'running', activeRunId: 'run-1' }]) {
  return {
    environmentId: 'env', threadId: 't1', projectId: 'p1', ready: true, draftKey: 'env:t1', busy: false, pending: undefined, projection,
    shell: { threads, projects: [] }, local: {}, config: {},
  } as unknown as T3Client & { projection: Obj; shell: { threads: Obj[] } };
}
const run = (id: string, status: string, ordinal: number) => ({ id, status, ordinal, startedAt: '2026-10-27T00:00:00Z', completedAt: status === 'running' ? null : '2026-10-27T00:01:00Z' });
const fleet = new EnvironmentFleet();
describe('the window\'s regions (clone)', () => {
  it('the open thread\'s region says Working, then Done, from the client\'s thread, and clears 7 s later', () => {
    const c = client({ runs: [run('run-1', 'running', 1)], runtimeRequests: [] });
    expect(announcements(c, 1_000, fleet).thread).toBe(''); // the baseline is silent
    c.projection = { runs: [run('run-1', 'completed', 1)], runtimeRequests: [] };
    c.shell.threads[0] = { ...c.shell.threads[0], status: 'completed', activityRunStatus: 'completed' };
    expect(announcements(c, 2_000, fleet).thread).toBe('Done');
    expect(announcing(c)).toBe(true);
    c.projection = { runs: [run('run-1', 'completed', 1), run('run-2', 'running', 2)], runtimeRequests: [] };
    c.shell.threads[0] = { ...c.shell.threads[0], latestRunId: 'run-2', status: 'running', activityRunStatus: 'running' };
    expect(announcements(c, 3_000, fleet).thread).toBe('Working');
    announcements(c, 3_500, fleet); // the clock's first tick after the write
    expect(announcements(c, 3_500 + CLEAR_AFTER_MS - 1, fleet).thread).toBe('Working');
    expect(announcements(c, 3_500 + CLEAR_AFTER_MS, fleet).thread).toBe('');
    expect(announcing(c)).toBe(false);
  });
  it('a detail behind its shell is not read', () => {
    const c = client({ runs: [run('run-1', 'running', 1)], runtimeRequests: [] });
    announcements(c, 1_000, fleet);
    c.shell.threads[0] = { ...c.shell.threads[0], status: 'completed', activityRunStatus: 'completed' }; // the shell is ahead
    expect(announcements(c, 2_000, fleet).thread).toBe('');
  });
  it('a background thread\'s ending is spoken in its own region; the same words again get a no-break space', () => {
    const c = client({ runs: [], runtimeRequests: [] }, [{ id: 't2', title: 'Update docs', latestRunId: 'run-1', status: 'running' }]);
    (c as unknown as { threadId: string }).threadId = '';
    expect(announcements(c, 1_000, fleet).background).toBe('');
    c.shell.threads[0] = { ...c.shell.threads[0], status: 'completed' };
    expect(announcements(c, 2_000, fleet).background).toBe('Done: Update docs');
    c.shell.threads[0] = { ...c.shell.threads[0], latestRunId: 'run-2', status: 'completed' };
    expect(announcements(c, 3_000, fleet).background).toBe('Done: Update docs\u00a0');
  });
  it('announce waits for the focus to settle, keeps one message per source, and alternates between its two regions', () => {
    const c = client({ runs: [], runtimeRequests: [] });
    announce(c, 'Model: GPT-5', 'model-picker');
    announce(c, 'Model: o4', 'model-picker');
    announce(c, 'Opened in Zed');
    // The answer the command brought carries the clock's last reading; the next tick speaks them together.
    expect(announcements(c, 1_000, fleet)).toMatchObject({ first: '', second: '' });
    expect(announcements(c, 1_500, fleet)).toMatchObject({ first: 'Model: o4. Opened in Zed', second: '' });
    announce(c, 'Opened in Zed');
    announcements(c, 2_000, fleet);
    expect(announcements(c, 2_500, fleet)).toMatchObject({ first: '', second: 'Opened in Zed' });
    announce(c, 'Archived 1 thread, ⌘Z to undo', 'thread-undo-notice');
    cancelAnnouncement(c, 'thread-undo-notice');
    expect(announcements(c, 3_000, fleet)).toMatchObject({ first: '', second: 'Opened in Zed' });
    // lib/announce.ts never clears its pair: the text stays, and nothing is left for the clock to do.
    expect(announcements(c, 3_000 + CLEAR_AFTER_MS + 1_000, fleet)).toMatchObject({ first: '', second: 'Opened in Zed' });
    expect(announcing(c)).toBe(false);
  });
  it('a write that is not a send, like a model change, leaves an idle thread silent', () => {
    const c = client({ runs: [run('run-1', 'completed', 1)], runtimeRequests: [] }, [{ id: 't1', title: 'Fix login', latestRunId: 'run-1', status: 'completed', activityRunStatus: 'completed', activeRunId: null }]);
    const write = c as unknown as { busy: boolean; pending: Obj | undefined };
    expect(announcements(c, 1_000, fleet).thread).toBe('');
    write.busy = true;
    write.pending = { method: 'orchestration.dispatchCommand', threadId: 't1', payload: { type: 'thread.model-selection.set' } };
    expect(announcements(c, 1_500, fleet).thread).toBe('');
    write.busy = false; write.pending = undefined;
    expect(announcements(c, 2_000, fleet).thread).toBe('');
  });
  // useLocalDispatchState's callers: an unstarted thread's first message into a new worktree (useComposerSend) and
  // Implement in a new thread (useProposedPlanSend) are sends of the open thread too.
  it('a launch that reuses the open thread, or starts a new one from it, is the open thread\'s send', () => {
    for (const payload of [{ threadId: 't1', reuseExistingThread: true }, { threadId: 't9' }]) {
      const c = client({ runs: [], runtimeRequests: [] }, [{ id: 't1', title: 'Plan', latestRunId: null, status: 'idle' }]);
      const write = c as unknown as { busy: boolean; pending: Obj | undefined };
      announcements(c, 1_000, fleet);
      write.busy = true;
      write.pending = { method: 'orchestration.launchThread', threadId: str(payload.threadId), payload };
      expect(announcements(c, 1_500, fleet).thread).toBe('Working');
    }
  });
  // ChatView.logic.ts hasServerAcknowledgedLocalDispatch: a send stays in flight past its reply until the thread moves.
  it('a shell answer between the send\'s reply and its run\'s first event does not say Working again', () => {
    const c = client({ runs: [run('r1', 'completed', 1)], runtimeRequests: [], messages: [{ id: 'm1', role: 'user' }] },
      [{ id: 't1', title: 'B', latestRunId: 'r1', status: 'completed', activityRunStatus: 'completed', activeRunId: null, updatedAt: '2026-10-27T00:01:00Z' }]);
    const write = c as unknown as { busy: boolean; pending: Obj | undefined };
    // Each change of the region's text is a new announcement (a repeat gets a no-break space).
    const spoken: string[] = [];
    let at = 1_000, last = '';
    const step = () => { const text = announcements(c, at += 600, fleet).thread; if (text && text !== last) spoken.push(text); last = text; };
    step();
    write.busy = true; write.pending = { method: 'orchestration.dispatchCommand', threadId: 't1', payload: { type: 'message.dispatch' } }; step();
    // The reply: the request is done, but the shell still shows the last settled run.
    write.busy = false; write.pending = undefined; step(); step();
    c.projection = { runs: [run('r1', 'completed', 1), run('r2', 'running', 2)], runtimeRequests: [], messages: [{ id: 'm1', role: 'user' }, { id: 'm2', role: 'user' }] };
    c.shell.threads[0] = { id: 't1', title: 'B', latestRunId: 'r2', status: 'running', activityRunStatus: 'running', activeRunId: 'r2', latestRunStartedAt: '2026-10-27T00:02:00Z' }; step();
    c.projection = { runs: [run('r1', 'completed', 1), run('r2', 'completed', 2)], runtimeRequests: [], messages: c.projection.messages };
    c.shell.threads[0] = { ...c.shell.threads[0], status: 'completed', activityRunStatus: 'completed', activeRunId: null }; step();
    expect(spoken).toEqual(['Working', 'Done']);
  });
  it('another thread\'s send in flight is not this thread\'s work: its first turn says Working, then Done', () => {
    const c = client({ runs: [], runtimeRequests: [] }, [{ id: 't1', title: 'B', latestRunId: null, status: 'idle' }]);
    const write = c as unknown as { busy: boolean; pending: Obj | undefined };
    write.busy = true;
    write.pending = { method: 'orchestration.dispatchCommand', threadId: 't0', payload: { type: 'message.dispatch' } };
    const spoken: string[] = [];
    let at = 1_000;
    const step = () => { const text = announcements(c, at += 600, fleet).thread; if (text && spoken.at(-1) !== text) spoken.push(text); };
    step(); // B opens while thread t0's send is in flight: the baseline
    write.busy = false; write.pending = undefined; step();
    write.busy = true; write.pending = { method: 'orchestration.dispatchCommand', threadId: 't1', payload: { type: 'message.dispatch' } }; step();
    write.busy = false; write.pending = undefined;
    c.projection = { runs: [run('r1', 'running', 1)], runtimeRequests: [] };
    c.shell.threads[0] = { id: 't1', title: 'B', latestRunId: 'r1', status: 'running', activityRunStatus: 'running', activeRunId: 'r1' }; step();
    c.projection = { runs: [run('r1', 'completed', 1)], runtimeRequests: [] };
    c.shell.threads[0] = { id: 't1', title: 'B', latestRunId: 'r1', status: 'completed', activityRunStatus: 'completed', activeRunId: null }; step();
    expect(spoken).toEqual(['Working', 'Done']);
  });
});
