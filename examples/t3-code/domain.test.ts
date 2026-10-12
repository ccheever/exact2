import { describe, expect, test } from "bun:test";
import {
  applyShell, applyThread, arr, initialShell, mergeHistory, messages, obj,
  readyCheckpoint, threadSnapshot, visibleTurnItems, type Obj, type ThreadState,
} from "./domain";

const timestamp = "2026-10-03T00:00:00.000Z";

function turn(id: string, ordinal: number, changes: Obj = {}): Obj {
  return {
    id, ordinal, threadId: "thread-1", runId: "run-1", nodeId: "node-1",
    type: "assistant_message", status: "completed", messageId: `message-${id}`,
    text: `text ${id}`, streaming: false, ...changes,
  };
}
function row(item: Obj, changes: Obj = {}): Obj {
  return { position: 0, visibility: "local", sourceThreadId: item.threadId, sourceItemId: item.id, item, ...changes };
}
function snapshot(items: Obj[] = [], changes: Obj = {}): Obj {
  return {
    snapshotSequence: 10,
    projection: {
      thread: { id: "thread-1", title: "Example" }, runs: [], attempts: [], nodes: [],
      subagents: [], providerSessions: [], providerThreads: [], providerTurns: [], runtimeRequests: [],
      messages: [], plans: [], turnItems: items, checkpointScopes: [], checkpoints: [],
      contextHandoffs: [], contextTransfers: [], visibleTurnItems: items.map((item, position) => row(item, { position })),
      updatedAt: timestamp,
    },
    ...changes,
  };
}
function state(items: Obj[] = [], changes: Obj = {}): ThreadState { return threadSnapshot(snapshot(items, changes)); }
function event(type: string, payload: Obj, sequence = 11, changes: Obj = {}): Obj {
  return { kind: "event", sequence, event: { id: `event-${sequence}`, type, threadId: "thread-1", occurredAt: timestamp, payload, ...changes } };
}
function shellSnapshot(sequence: number, projects: Obj[] = [], threads: Obj[] = []): Obj {
  return { snapshotSequence: sequence, projects, threads, archivedThreads: [] };
}

describe("shell synchronization", () => {
  test("accepts sequence gaps, deduplicates replay, and replaces with lower snapshots", () => {
    const first = applyShell(initialShell(), shellSnapshot(30, [{ id: "project-1" }], [{ id: "thread-1" }]));
    const second = applyShell(first, { kind: "thread.updated", sequence: 52, location: "active", thread: { id: "thread-2" } });
    expect(second.threads.map(thread => thread.id)).toEqual(["thread-1", "thread-2"]);
    expect(applyShell(second, { kind: "thread.removed", sequence: 30, location: "active", threadId: "thread-1" })).toBe(second);
    const replaced = applyShell(second, { kind: "snapshot", snapshot: shellSnapshot(4, [], []) });
    expect(replaced).toEqual({ projects: [], threads: [], sequence: 4 });
  });

  test("enrichment updates metadata only and accepts an explicitly resolved null identity", () => {
    const first = applyShell(initialShell(), shellSnapshot(30,
      [{ id: "p", workspaceRoot: "/repo", repositoryIdentity: { remote: "git@example" }, title: "current" }],
      [{ id: "t" }]));
    const enriched = applyShell(first, {
      kind: "snapshot", resolvedRepositoryIdentityRoots: ["/repo"],
      snapshot: shellSnapshot(1, [{ id: "p", workspaceRoot: "/repo", repositoryIdentity: null, title: "old" }]),
    });
    expect(enriched.sequence).toBe(30);
    expect(enriched.threads).toEqual([{ id: "t" }]);
    expect(enriched.projects[0]).toEqual({ id: "p", workspaceRoot: "/repo", repositoryIdentity: null, title: "current" });
  });

  test("keeps resolved identity for the same root, but never carries it to another root", () => {
    const first = applyShell(initialShell(), shellSnapshot(10, [{ id: "p", workspaceRoot: "/a", repositoryIdentity: { id: "repo" } }]));
    const second = applyShell(first, { kind: "project.updated", sequence: 11, project: { id: "p", workspaceRoot: "/a", repositoryIdentity: null } });
    expect(obj(second.projects[0]?.repositoryIdentity).id).toBe("repo");
    const third = applyShell(second, { kind: "project.updated", sequence: 12, project: { id: "p", workspaceRoot: "/b", repositoryIdentity: null } });
    expect(third.projects[0]?.repositoryIdentity).toBeNull();
  });

  test("archived events remove active rows and unknown events advance the cursor", () => {
    const first = applyShell(initialShell(), shellSnapshot(10, [], [{ id: "t" }]));
    const archived = applyShell(first, { kind: "thread.updated", sequence: 20, location: "archive", thread: { id: "t" } });
    expect(archived.threads).toEqual([]);
    expect(applyShell(archived, { kind: "future.changed", sequence: 22 }).sequence).toBe(22);
  });

  test("rejects malformed known shell updates instead of clearing state", () => {
    expect(() => applyShell(initialShell(), {})).toThrow("snapshotSequence");
    expect(() => applyShell(initialShell(), { kind: "thread.updated", sequence: 2, location: "active", thread: {} })).toThrow("id");
    expect(() => applyShell(initialShell(), { kind: "project.removed", sequence: 2 })).toThrow("projectId");
  });
});

describe("thread synchronization", () => {
  test("streamed assistant text replaces an entity and its visible row", () => {
    const first = state([turn("a", 1, { text: "Hel", streaming: true })]);
    const second = applyThread(first, event("turn-item.updated", turn("a", 1, { text: "Hello", streaming: false }), 20));
    expect(second.sequence).toBe(20);
    expect(arr(second.projection.turnItems)).toHaveLength(1);
    expect(messages(second).map(item => item.body)).toEqual(["Hello"]);
    expect(messages(first).map(item => item.body)).toEqual(["Hel"]);
    expect(applyThread(second, event("turn-item.updated", turn("a", 1, { text: "old" }), 15))).toBe(second);
  });

  test("authoritative socket fallback resets lower cursor and progressive metadata", () => {
    const partial = state([turn("old", 8)], { snapshotSequence: 99, historyCursor: "old-cursor", hasMoreHistory: true, latestLocalTurnOrdinal: 8 });
    const next = applyThread(partial, { kind: "snapshot", ...snapshot([turn("new", 1)], { snapshotSequence: 2 }) });
    expect(next.sequence).toBe(2);
    expect(next.historyCursor).toBeNull();
    expect(next.hasMore).toBe(false);
    expect(next.latestLocalTurnOrdinal).toBeNull();
    expect(messages(next)[0]?.body).toBe("text new");
  });

  test("unknown events advance cursor; malformed known event payloads throw", () => {
    const first = state();
    const unknown = applyThread(first, { kind: "event", sequence: 80, event: { type: "future.event" } });
    expect(unknown.sequence).toBe(80);
    expect(unknown.projection).toBe(first.projection);
    expect(() => applyThread(first, event("turn-item.updated", { id: "broken" }))).toThrow("threadId");
    expect(() => applyThread(first, event("turn-item.updated", turn("bad-type", 1, { type: "unrecognized" })))).toThrow("unknown value");
    expect(() => applyThread(first, event("run.updated", { id: "run-1", status: 4 }))).toThrow("status");
    expect(() => threadSnapshot({ snapshotSequence: 0, projection: {} })).toThrow("projection.thread");
    expect(() => threadSnapshot({ snapshotSequence: 0, projection: { thread: { id: "t" } } })).toThrow("projection.runs");
  });

  test("updates for another thread do not change state or its cursor", () => {
    const first = state();
    expect(applyThread(first, event("run.updated", { id: "run-2", status: "running" }, 15, { threadId: "thread-2" }))).toBe(first);
    expect(() => applyThread(first, event("thread.metadata-updated", { id: "thread-2" }))).toThrow("does not match");
    expect(() => applyThread(first, event("turn-item.updated", turn("other", 1, { threadId: "thread-2" })))).toThrow("does not match");
  });

  test("all entity update families use replacement upserts", () => {
    const families: [string, string, Obj][] = [
      ["thread.metadata-updated", "thread", { id: "thread-1", title: "Renamed" }],
      ["run.created", "runs", { id: "r", status: "running" }],
      ["run-attempt.created", "attempts", { id: "a", runId: "r", rootNodeId: "n", status: "running" }],
      ["node.updated", "nodes", { id: "n" }], ["subagent.updated", "subagents", { id: "s" }],
      ["provider-session.attached", "providerSessions", { id: "session" }],
      ["provider-thread.updated", "providerThreads", { id: "pt" }],
      ["provider-turn.updated", "providerTurns", { id: "turn" }],
      ["runtime-request.updated", "runtimeRequests", { id: "request", status: "pending" }],
      ["message.updated", "messages", { id: "message", role: "assistant", text: "hello" }],
      ["plan.updated", "plans", { id: "plan" }],
      ["checkpoint-scope.created", "checkpointScopes", { id: "scope" }],
      ["checkpoint.captured", "checkpoints", { id: "checkpoint", status: "ready", runId: "r", appRunOrdinal: 1 }],
      ["context-handoff.updated", "contextHandoffs", { id: "handoff" }],
      ["context-transfer.created", "contextTransfers", { id: "transfer" }],
    ];
    let next = state();
    families.forEach(([type, family, payload], index) => {
      next = applyThread(next, event(type, payload, 20 + index));
      expect(family === "thread" ? next.projection.thread : arr(next.projection[family])[0]).toEqual(payload);
    });
    next = applyThread(next, event("provider-session.detached", { providerSessionId: "session", detachedAt: timestamp }, 99));
    expect(next.projection.providerSessions).toEqual([]);
  });

  test("provider-turn replacement retains usage omitted in later payloads", () => {
    let next = applyThread(state(), event("provider-turn.updated", { id: "pt", tokenUsage: { input: 42 } }));
    next = applyThread(next, event("provider-turn.updated", { id: "pt", status: "completed" }, 12));
    expect(arr(next.projection.providerTurns)[0]).toEqual({ id: "pt", status: "completed", tokenUsage: { input: 42 } });
  });

  test("background cancellation carries the structured restart list", () => {
    let next = applyThread(state(), event("run.created", { id: "run-1", status: "running" }));
    const work = [{ kind: "subagent", id: "child" }];
    next = applyThread(next, event("run.background-work-cancelled", { runId: "run-1", restartCancelledBackgroundWork: work }, 12));
    expect(arr(next.projection.runs)[0]?.restartCancelledBackgroundWork).toEqual(work);
  });

  test("read-state updates preserve activity timestamp", () => {
    const first = state();
    const next = applyThread(first, event("thread.visited", { id: "thread-1", lastVisitedAt: timestamp }, 11, { occurredAt: "2026-10-04T00:00:00.000Z" }));
    expect(next.projection.updatedAt).toBe(first.projection.updatedAt);
  });
});

describe("timeline visibility and bounded history", () => {
  test("rolled-back runs hide local rows and preserve inherited rows", () => {
    const own = turn("a", 1);
    const inherited = turn("a", 1, { threadId: "parent" });
    const first = snapshot([own]);
    obj(first.projection).visibleTurnItems = [row(inherited, { visibility: "inherited" }), row(own, { position: 1 })];
    const next = applyThread(threadSnapshot(first), event("run.updated", { id: "run-1", status: "rolled_back" }));
    expect(visibleTurnItems(next)).toHaveLength(1);
    expect(visibleTurnItems(next)[0]?.sourceThreadId).toBe("parent");
  });

  test("cancelled queued input never reappears as a transcript row", () => {
    const queued = turn("queued", 1, { type: "user_message", inputIntent: "queued_turn", attachments: [] });
    const next = applyThread(state([queued]), event("run.updated", { id: "run-1", status: "cancelled" }));
    expect(messages(next)).toEqual([]);
  });

  test("superseded steer results hide unless paired with an explicit stop request", () => {
    const result = turn("interrupt", 2, { type: "run_interrupt_result", message: "Stopped" });
    const attempt = { id: "attempt", runId: "run-1", rootNodeId: "node-1", status: "superseded" };
    const hidden = applyThread(state([result]), event("run-attempt.updated", attempt));
    expect(messages(hidden)).toEqual([]);
    const request = turn("request", 1, { type: "run_interrupt_request", message: "Stop" });
    const paired = applyThread(state([request, result]), event("run-attempt.updated", attempt));
    expect(messages(paired).map(item => item.body)).toEqual(["Stop", "Stopped"]);
  });

  test("partial snapshots reject missing old ordinals even with inherited-only windows", () => {
    const inherited = turn("parent-row", 12, { threadId: "parent" });
    const first = snapshot([], { historyCursor: "cursor", hasMoreHistory: true, latestLocalTurnOrdinal: 20 });
    obj(first.projection).visibleTurnItems = [row(inherited, { visibility: "inherited" })];
    const ignored = applyThread(threadSnapshot(first), event("turn-item.updated", turn("old", 18)));
    expect(ignored.sequence).toBe(11);
    expect(ignored.projection.turnItems).toEqual([]);
    const next = applyThread(ignored, event("turn-item.updated", turn("new", 21), 12));
    expect(next.latestLocalTurnOrdinal).toBe(21);
    expect(messages(next).map(item => item.body)).toEqual(["text parent-row", "text new"]);
  });

  test("older-page overlap preserves live text and uses source identity", () => {
    const first = state([turn("latest", 10, { text: "new streamed value" })], { historyCursor: "cursor", hasMoreHistory: true, latestLocalTurnOrdinal: 10 });
    const older = row(turn("old", 1));
    const sameIdOtherThread = row(turn("latest", 1, { threadId: "parent" }), { visibility: "inherited", sourceThreadId: "parent" });
    const next = mergeHistory(first, {
      snapshotSequence: 8,
      items: [older, sameIdOtherThread, row(turn("latest", 10, { text: "stale page value" }))],
      nextCursor: null, hasMoreHistory: false,
    });
    expect(messages(next).map(item => item.body)).toEqual(["text old", "text latest", "new streamed value"]);
    expect(new Set(messages(next).map(item => item.id)).size).toBe(3);
    expect(next.sequence).toBe(10);
    expect(next.latestLocalTurnOrdinal).toBe(10);
    expect(next.hasMore).toBe(false);
    expect(visibleTurnItems(next).map(item => item.position)).toEqual([0, 1, 2]);
  });

  test("history page cannot resurrect rows hidden while the request was in flight", () => {
    const item = turn("old", 1);
    const hidden = applyThread(state([item]), event("run.updated", { id: "run-1", status: "rolled_back" }));
    const next = mergeHistory(hidden, { snapshotSequence: 9, items: [row(item)], nextCursor: null, hasMoreHistory: false });
    expect(messages(next)).toEqual([]);
  });

  test("retained interrupt requests enter history using their latest control-state value", () => {
    const request = turn("stop", 1, { type: "run_interrupt_request", message: "Updated stop" });
    const first = snapshot([request], { historyCursor: "cursor", hasMoreHistory: true, latestLocalTurnOrdinal: 1 });
    obj(first.projection).visibleTurnItems = [];
    const next = mergeHistory(threadSnapshot(first), {
      snapshotSequence: 8, items: [row({ ...request, message: "Old stop" })], nextCursor: null, hasMoreHistory: false,
    });
    expect(messages(next)[0]?.body).toBe("Updated stop");
  });

  test("fully loaded histories accept missing old items again", () => {
    const first = state([turn("newer", 10)], { latestLocalTurnOrdinal: 10 });
    const next = applyThread(first, event("turn-item.updated", turn("older", 2)));
    expect(messages(next).map(item => item.body)).toEqual(["text older", "text newer"]);
  });
});

describe("presentation", () => {
  test("renders common turn types and preserves JSON tool output", () => {
    const next = state([
      turn("u", 1, { type: "user_message", inputIntent: "turn_start", attachments: [], text: "Help" }),
      turn("reason", 2, { type: "reasoning", text: "Thinking through it" }),
      turn("plan", 3, { type: "proposed_plan", markdown: "## Plan" }),
      turn("tool", 4, { type: "dynamic_tool", toolName: "read", input: { path: "a.ts" }, output: { ok: true } }),
      turn("error", 5, { type: "error", failure: { message: "Provider failed" } }),
    ]);
    expect(messages(next).map(item => item.kind)).toEqual(["user", "reasoning", "plan", "tool", "error"]);
    expect(messages(next)[3]?.body).toContain('"ok": true');
    expect(messages(next)[4]?.body).toBe("Provider failed");
  });

  test("diff ordinal comes from the latest ready checkpoint, not row count", () => {
    const first = snapshot([turn("a", 1)]);
    obj(first.projection).checkpoints = [
      { id: "c1", status: "ready", appRunOrdinal: 2, runId: "r1" },
      { id: "c2", status: "error", appRunOrdinal: 20, runId: "r2" },
      { id: "c3", status: "ready", appRunOrdinal: 12, runId: "r3" },
      { id: "baseline", status: "ready", appRunOrdinal: null, runId: null },
    ];
    expect(readyCheckpoint(threadSnapshot(first))).toBe(12);
    expect(readyCheckpoint(state())).toBeNull();
    expect(readyCheckpoint(null)).toBeNull();
  });
});
