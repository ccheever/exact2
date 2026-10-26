// The terminal page's own socket (session.ts) against a fake WebSocket: the attach and its resume, the
// acknowledgement per chunk, what the surface is written, the write and resize commands, the exit, the
// read-only observe and the sign-in input's serial queue.
import { describe, expect, test } from "bun:test";
import { authSlices, crlf, MAX_WRITE_LENGTH, SCHEMA_REFUSAL, SocketRequests, TerminalSocket, TICKET_TIMEOUT_MS, type SessionSink } from "./session";

class FakeSocket {
  sent: Record<string, unknown>[] = [];
  readyState = 1;
  onopen: ((event: Event) => void) | null = null;
  onclose: ((event: CloseEvent) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  constructor(readonly url: string) {}
  send(text: string) { this.sent.push(JSON.parse(text) as Record<string, unknown>); }
  close() { this.onclose?.({} as CloseEvent); }
  open() { this.onopen?.({} as Event); }
  frame(value: unknown) { this.onmessage?.({ data: JSON.stringify(value) } as MessageEvent); }
  requests(tag?: string) { return this.sent.filter((frame) => frame._tag === "Request" && (!tag || frame.tag === tag)); }
}

function harness(target: ConstructorParameters<typeof TerminalSocket>[2]) {
  const sockets: FakeSocket[] = [];
  const log: string[] = [];
  const sink: SessionSink = {
    write: (data) => log.push(`write ${data}`),
    resetAndWrite: (data) => log.push(`reset ${data}`),
    system: (message) => log.push(`system ${message}`),
    exited: () => log.push("exited"),
    wantSocket: () => log.push("wantSocket"),
    authFailed: (failed) => log.push(`authFailed ${failed}`),
    received: () => undefined,
    answered: () => log.push("answered"),
  };
  const session = new TerminalSocket(sink, (url) => { const socket = new FakeSocket(url); sockets.push(socket); return socket; }, target);
  return { session, sockets, log };
}

const attach = { threadId: "thread-1", terminalId: "term-1", cwd: "/repo", worktreePath: null, env: { T3CODE_PROJECT_ROOT: "/repo" } };
const snapshot = (history: string, status = "running") => ({ type: "snapshot", snapshot: { threadId: "thread-1", terminalId: "term-1", cwd: "/repo",
  worktreePath: null, status, pid: 1, history, exitCode: null, exitSignal: null, label: "zsh", updatedAt: "2026-10-23T00:00:00.000Z" } });
const tick = () => new Promise((resolve) => setTimeout(resolve, 1));

describe("the page's attach stream", () => {
  test("attaches on open, writes the snapshot then the output, and acknowledges each chunk", () => {
    const { session, sockets, log } = harness({ attach, readOnly: false });
    session.start("ws://host/ws?wsTicket=a");
    const socket = sockets[0]!;
    socket.open();
    expect(socket.requests("terminal.attach")).toEqual([{ _tag: "Request", id: "1", tag: "terminal.attach",
      payload: { threadId: "thread-1", terminalId: "term-1", cwd: "/repo", worktreePath: null, env: { T3CODE_PROJECT_ROOT: "/repo" } },
      headers: [["x-t3-orchestration-protocol", "2"]] }]);
    socket.frame({ _tag: "Chunk", requestId: "1", values: [snapshot("$ ls\r\n")] });
    socket.frame([{ _tag: "Chunk", requestId: "1", values: [{ type: "output", threadId: "thread-1", terminalId: "term-1", data: "a" }, { type: "output", threadId: "thread-1", terminalId: "term-1", data: "b" }] }]);
    socket.frame({ _tag: "Ping" });
    expect(log).toEqual(["answered", "reset $ ls\r\n", "write ab"]);
    expect(socket.sent.filter((frame) => frame._tag !== "Request")).toEqual([{ _tag: "Ack", requestId: "1" }, { _tag: "Ack", requestId: "1" }, { _tag: "Pong" }]);
  });

  test("a lost socket asks for a new ticket and attaches again with resume, the history coming back from the snapshot", () => {
    const { session, sockets, log } = harness({ attach, readOnly: false });
    session.start("ws://host/ws?wsTicket=a");
    sockets[0]!.open();
    sockets[0]!.frame({ _tag: "Chunk", requestId: "1", values: [snapshot("one\r\n")] });
    sockets[0]!.close();
    expect(log.at(-1)).toBe("wantSocket");
    session.start("ws://host/ws?wsTicket=b");
    sockets[1]!.open();
    expect(sockets[1]!.requests("terminal.attach")[0]!.payload).toMatchObject({ resume: true });
    sockets[1]!.frame({ _tag: "Chunk", requestId: sockets[1]!.requests("terminal.attach")[0]!.id, values: [snapshot("one\r\ntwo\r\n")] });
    expect(log.at(-1)).toBe("reset one\r\ntwo\r\n");
    expect(log.filter((line) => line === "answered")).toHaveLength(2);
  });

  test("a resumed stream whose history is empty clears what the surface showed", () => {
    const { session, sockets, log } = harness({ attach, readOnly: false });
    session.start("ws://host/ws?wsTicket=a");
    sockets[0]!.open();
    sockets[0]!.frame({ _tag: "Chunk", requestId: "1", values: [snapshot("old\r\n")] });
    sockets[0]!.close();
    session.start("ws://host/ws?wsTicket=b");
    sockets[1]!.open();
    sockets[1]!.frame({ _tag: "Chunk", requestId: sockets[1]!.requests("terminal.attach")[0]!.id, values: [snapshot("")] });
    expect(log.filter((line) => line.startsWith("reset"))).toEqual(["reset old\r\n", "reset "]);
  });

  test("the stats count the largest chunk and the messages that carried a full window", () => {
    const { session, sockets } = harness({ attach, readOnly: false });
    session.start("ws://host/ws?wsTicket=a");
    sockets[0]!.open();
    const output = (data: string) => ({ _tag: "Chunk", requestId: "1", values: [{ type: "output", threadId: "thread-1", terminalId: "term-1", data }] });
    sockets[0]!.frame(output("x"));
    sockets[0]!.frame(Array.from({ length: 8 }, () => output("y")));
    expect(session.fullWindows).toBe(1);
    expect(session.largest).toBeGreaterThan(0);
  });

  test("a failed attach shows its error as a system line", () => {
    const { session, sockets, log } = harness({ attach, readOnly: false });
    session.start("ws://host/ws?wsTicket=a");
    sockets[0]!.open();
    sockets[0]!.frame({ _tag: "Exit", requestId: "1", exit: { _tag: "Failure", cause: { _tag: "Fail", error: { _tag: "TerminalCwdError", message: "Terminal cwd does not exist: /gone" } } } });
    expect(log).toEqual(["system Terminal cwd does not exist: /gone"]);
  });

  test("an exit prints once, then tells the page's owner on the next tick", async () => {
    const { session, sockets, log } = harness({ attach, readOnly: false });
    session.start("ws://host/ws?wsTicket=a");
    sockets[0]!.open();
    sockets[0]!.frame({ _tag: "Chunk", requestId: "1", values: [snapshot("")] });
    sockets[0]!.frame({ _tag: "Chunk", requestId: "1", values: [{ type: "exited", threadId: "thread-1", terminalId: "term-1", exitCode: 0, exitSignal: null }] });
    sockets[0]!.frame({ _tag: "Chunk", requestId: "1", values: [{ type: "activity", threadId: "thread-1", terminalId: "term-1", hasRunningSubprocess: false, label: "zsh" }] });
    expect(log).toEqual(["answered", "system Process exited"]);
    await tick();
    expect(log).toEqual(["answered", "system Process exited", "exited"]);
  });
});

describe("the page's socket requests", () => {
  function fakeTimers() {
    const pending: { at: number; run: () => void; id: number }[] = [];
    let now = 0, serial = 0;
    const set = ((run: () => void, ms = 0) => { pending.push({ at: now + ms, run, id: ++serial }); return serial; }) as unknown as typeof setTimeout;
    const clear = ((id: number) => { const index = pending.findIndex((entry) => entry.id === id); if (index >= 0) pending.splice(index, 1); }) as unknown as typeof clearTimeout;
    const advance = (ms: number) => {
      const end = now + ms;
      for (;;) { pending.sort((a, b) => a.at - b.at); const next = pending[0]; if (!next || next.at > end) break; pending.shift(); now = next.at; next.run(); }
      now = end;
    };
    return { set, clear, advance };
  }

  test("asks at once, backs off after failures, and starts over once a stream answers", () => {
    const timers = fakeTimers(), asked: number[] = [];
    let t = 0, last = 0;
    const requests = new SocketRequests((serial) => { asked.push(t); last = serial; }, timers);
    const step = (ms: number) => { t += ms; timers.advance(ms); };
    requests.want(); step(0);
    expect(asked).toEqual([0]);
    for (let loss = 0; loss < 6; loss++) { expect(requests.answered("", last)).toBe(false); step(10_000); }
    // The fifth and later losses wait 10 s; an answered stream makes the next ask immediate again.
    expect(requests.answered("ws://x", last)).toBe(true);
    requests.reset();
    requests.want(); step(0);
    expect(asked.at(-1)).toBe(t);
  });

  test("an unanswered request is asked again after the ticket timeout", () => {
    const timers = fakeTimers();
    let asked = 0;
    const requests = new SocketRequests(() => { asked += 1; }, timers);
    requests.want(); timers.advance(0);
    expect(asked).toBe(1);
    timers.advance(TICKET_TIMEOUT_MS - 1);
    expect(asked).toBe(1);
    timers.advance(1 + 500);
    expect(asked).toBe(2);
    // The first request's late answer is not the second's.
    expect(requests.answered("ws://late", 1)).toBe(false);
    expect(requests.answered("ws://x", 2)).toBe(true);
    timers.advance(60_000);
    expect(asked).toBe(2);
  });

  test("an answer that comes while a re-ask waits its delay ends the asking", () => {
    const timers = fakeTimers();
    let asked = 0;
    const requests = new SocketRequests(() => { asked += 1; }, timers);
    requests.want(); timers.advance(0);
    timers.advance(TICKET_TIMEOUT_MS + 100);
    expect(asked).toBe(1);
    expect(requests.answered("ws://x", 1)).toBe(true);
    timers.advance(60_000);
    expect(asked).toBe(1);
  });
});

describe("the page's input", () => {
  test("one terminal.write per input; a refusal is a system line, and an oversized one never leaves", () => {
    const { session, sockets, log } = harness({ attach, readOnly: false });
    session.write("x");
    expect(log).toEqual(["system The server is not connected."]);
    session.start("ws://host/ws?wsTicket=a");
    sockets[0]!.open();
    session.write("ls\r");
    const write = sockets[0]!.requests("terminal.write")[0]!;
    expect(write.payload).toEqual({ threadId: "thread-1", terminalId: "term-1", data: "ls\r" });
    sockets[0]!.frame({ _tag: "Exit", requestId: write.id, exit: { _tag: "Failure", cause: { _tag: "Fail", error: { _tag: "TerminalNotRunningError", message: "Terminal is not running" } } } });
    session.write("y".repeat(MAX_WRITE_LENGTH + 1));
    expect(sockets[0]!.requests("terminal.write")).toHaveLength(1);
    expect(log.slice(1)).toEqual(["system Terminal is not running", `system ${SCHEMA_REFUSAL}`]);
  });

  test("the grid goes once the session runs, latest-wins while one is in flight, clamped", () => {
    const { session, sockets } = harness({ attach, readOnly: false });
    session.start("ws://host/ws?wsTicket=a");
    const socket = sockets[0]!;
    socket.open();
    session.resize(80, 24);
    expect(socket.requests("terminal.resize")).toEqual([]);
    socket.frame({ _tag: "Chunk", requestId: "1", values: [snapshot("")] });
    expect(socket.requests("terminal.resize").map((frame) => frame.payload)).toEqual([{ threadId: "thread-1", terminalId: "term-1", cols: 80, rows: 24 }]);
    session.resize(100, 30);
    session.resize(2000, 900);
    expect(socket.requests("terminal.resize")).toHaveLength(1);
    socket.frame({ _tag: "Exit", requestId: socket.requests("terminal.resize")[0]!.id, exit: { _tag: "Success", value: null } });
    expect(socket.requests("terminal.resize").map((frame) => frame.payload).at(-1)).toEqual({ threadId: "thread-1", terminalId: "term-1", cols: 1000, rows: 500 });
  });

  test("a read-only terminal observes and sends nothing", () => {
    const { session, sockets } = harness({ attach, readOnly: true });
    session.start("ws://host/ws?wsTicket=a");
    sockets[0]!.open();
    expect(sockets[0]!.requests().map((frame) => [frame.tag, frame.payload])).toEqual([["terminal.observe", { threadId: "thread-1", terminalId: "term-1" }]]);
    session.write("ls\r");
    sockets[0]!.frame({ _tag: "Chunk", requestId: "1", values: [snapshot("")] });
    session.resize(80, 24);
    expect(sockets[0]!.requests()).toHaveLength(1);
  });

  test("sign-in input goes one provider.auth.respond at a time; a failure drops the queue until one succeeds", () => {
    const { session, sockets, log } = harness({ auth: { instanceId: "codex", flowId: "flow-1", interactionId: "i-1" }, readOnly: false });
    session.authInput("", { cols: 600, rows: 300 });
    expect(log).toEqual(["wantSocket"]);
    session.start("ws://host/ws?wsTicket=a");
    const socket = sockets[0]!;
    socket.open();
    session.authInput("abc");
    const responds = () => socket.requests("provider.auth.respond");
    expect(responds().map((frame) => frame.payload)).toEqual([{ instanceId: "codex", flowId: "flow-1", interactionId: "i-1",
      response: { type: "terminal", data: "", size: { cols: 500, rows: 200 } } }]);
    socket.frame({ _tag: "Exit", requestId: responds()[0]!.id, exit: { _tag: "Success", value: null } });
    expect(responds()).toHaveLength(2);
    session.authInput("d");
    socket.frame({ _tag: "Exit", requestId: responds()[1]!.id, exit: { _tag: "Failure", cause: { _tag: "Fail", error: { message: "gone" } } } });
    expect(log.at(-1)).toBe("authFailed true");
    expect(responds()).toHaveLength(2);
    session.authInput("e");
    socket.frame({ _tag: "Exit", requestId: responds()[2]!.id, exit: { _tag: "Success", value: null } });
    expect(log.at(-1)).toBe("authFailed false");
  });

  test("sign-in slices stay under 4,096 units and never split a surrogate pair; system lines end in CRLF", () => {
    const text = `${"a".repeat(4095)}😀b`;
    const slices = authSlices(text);
    expect(slices.map((slice) => slice.length)).toEqual([4095, 3]);
    expect(slices.join("")).toBe(text);
    expect(authSlices("")).toEqual([""]);
    expect(crlf("one\ntwo\r\nthree")).toBe("one\r\ntwo\r\nthree");
  });
});
