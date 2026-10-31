// The terminal page's own server socket: T3 Code 74541bab17's attached terminal session (MIT reference, see
// LICENSE-T3: packages/client-runtime/src/state/terminal.ts `attach`, terminalSession.ts
// applyTerminalAttachStreamEvent, apps/web/src/components/ThreadTerminalDrawer.tsx TerminalSessionViewport:
// synchronizeTerminalStatus, writeSystemMessage, the write and resize commands; apps/server/src/terminal/
// OutputProtocol.ts, one acknowledgement per chunk).
// Changes from the reference, for exact2:
// - The page opens the socket itself, with a ticket the app mints for the terminal's environment (`ticket` →
//   `socket`), so output reaches the surface without passing the app. A lost socket asks for a new ticket and
//   attaches again with `resume: true` once the server has answered, as the reference's durable subscription does.
// - The session lives as long as the page, and the surface is its buffer: output goes to it as it comes, with
//   no retained copy (the reference keeps one so a remounted viewport can replay it; a remounted page gets the
//   history back from the attach snapshot instead). Status, error and version fold as applyTerminalAttachStreamEvent
//   folds them.
// - The PTY grid the server shares (terminalSession.ts `grid`) goes to the surface as it changes.
// - Provider sign-in input (ProviderAuthTerminal.tsx) goes over the same socket, one `provider.auth.respond` at a
//   time in 4,096-unit slices, as T3 Code's serial queue sends it.
import { decode, encode, failure, identifier, request, type Frame } from "../../src/transport/transport-wire";
import { nextGrid, type TerminalAttachStreamEvent, type TerminalBufferState, type TerminalGrid } from "../../src/terminal/terminal-session";
import { shouldHandleTerminalExit } from "../../src/terminal/terminal-drawer";

/** What a terminal session's `terminal.attach` names (TerminalAttachInput). */
export interface AttachTarget {
  readonly threadId: string;
  readonly terminalId: string;
  readonly cwd?: string;
  readonly worktreePath?: string | null;
  readonly env?: Record<string, string>;
  readonly providerInstanceId?: string;
}

/** A provider sign-in terminal's flow (provider.auth.respond). */
export interface AuthTarget {
  readonly instanceId: string;
  readonly flowId: string;
  readonly interactionId: string;
}

export interface SessionSink {
  write(data: string): void;
  resetAndWrite(data: string): void;
  /** writeSystemMessage: `\r\n[terminal] <message>\r\n`. */
  system(message: string): void;
  /** The session printed its exit; the host closes the tab on the next tick (onSessionExited). */
  exited(): void;
  /** The socket is gone or not yet open: ask the app for a URL with a fresh ticket. */
  wantSocket(): void;
  /** Sign-in input failed (true) or recovered (false). */
  authFailed(failed: boolean): void;
  /** Bytes of output written, for the page's stats. */
  received(units: number): void;
  /** The attach stream answered (its first chunk since the socket opened). */
  answered(): void;
  /** The shared PTY grid changed (snapshots and activity carry it): the surface draws it (setSharedGrid). */
  grid(grid: TerminalGrid | null): void;
}

/** TerminalWriteInput's bound (contracts terminal.ts `isMaxLength(65_536)`, UTF-16 units). */
export const MAX_WRITE_LENGTH = 65_536;
/** What the reference shows for a write its client refuses to encode (effect's sync encode, SchemaParser.ts). */
export const SCHEMA_REFUSAL = "Schema validation failed";
const AUTH_SLICE = 4096;

type Socket = Pick<WebSocket, "send" | "close" | "readyState"> & {
  onopen: ((event: Event) => void) | null;
  onclose: ((event: CloseEvent) => void) | null;
  onerror: ((event: Event) => void) | null;
  onmessage: ((event: MessageEvent) => void) | null;
};
export type SocketFactory = (url: string) => Socket;

/** Every line end of a system message as CRLF, so a server's multi-line message starts each line at the left edge. */
export const crlf = (text: string): string => text.replace(/\r\n/g, "\n").replace(/\n/g, "\r\n");

/** Splits sign-in input into slices below the server's bound, never between a surrogate pair. */
export function authSlices(data: string): string[] {
  const slices: string[] = [];
  let index = 0;
  do {
    let end = Math.min(index + AUTH_SLICE, data.length);
    if (end < data.length && end > index && /[\ud800-\udbff]/.test(data[end - 1]!) && /[\udc00-\udfff]/.test(data[end]!)) end -= 1;
    slices.push(data.slice(index, end));
    index = end;
  } while (index < data.length);
  return slices;
}

export class TerminalSocket {
  private socket: Socket | null = null;
  private serial = 0;
  private open = false;
  private disposed = false;
  private attachId = "";
  /** The server answered this subscription; the next attach after a lost socket resumes. */
  private replied = false;
  /** TerminalBufferState without its output: the surface holds that. */
  private version = 0;
  private error: string | null = null;
  /** A chunk's output, written as one (a chunk carries several output events under load). */
  private output = "";
  private synchronized: TerminalBufferState["status"] = "closed";
  private handledExit = false;
  private shownError: string | null = null;
  private readonly pending = new Map<string, (failed: string | null) => void>();
  /** The server's PTY grid (snapshots and activity), and the one the surface was last given. */
  private grid: TerminalGrid | null = null;
  private shownGrid: TerminalGrid | null = null;
  private size: { cols: number; rows: number } | null = null;
  private sent: { cols: number; rows: number } | null = null;
  private resizing = false;
  private authQueue: string[] = [];
  private authSending = false;
  private authFailedState = false;
  status: TerminalBufferState["status"] = "closed";
  chunks = 0;
  /** Milliseconds spent handling the socket's messages, the largest chunk (UTF-16 units of its frame) and the
   * messages that carried a full window at once (OutputProtocol.ts: 8 chunks or 64 KiB unacknowledged), for the
   * page's stats. */
  busy = 0;
  largest = 0;
  fullWindows = 0;
  /** The surface shows an earlier stream's output: a snapshot replaces it, even an empty one. */
  private shown = false;
  private answeredSinceOpen = false;

  constructor(
    private readonly sink: SessionSink,
    private readonly connect: SocketFactory,
    private readonly target: { attach?: AttachTarget; auth?: AuthTarget; readOnly: boolean },
  ) {}

  get attached(): boolean { return this.open && this.attachId !== ""; }

  /** Opens the socket at `url` (the app's `socket` reply). */
  start(url: string): void {
    if (this.disposed || this.socket) return;
    let socket: Socket;
    try { socket = this.connect(url); } catch { this.sink.wantSocket(); return; }
    this.socket = socket;
    socket.onopen = () => {
      if (this.socket !== socket) return;
      this.open = true;
      this.answeredSinceOpen = false;
      if (this.target.attach) this.attach();
      this.drainAuth();
    };
    socket.onmessage = (event) => {
      if (this.socket !== socket) return;
      const start = performance.now();
      this.receive(String(event.data));
      this.busy += performance.now() - start;
    };
    socket.onerror = () => undefined;
    socket.onclose = () => { if (this.socket === socket) this.lost(); };
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    const socket = this.socket;
    this.socket = null;
    if (socket && this.attachId) { try { socket.send(encode({ _tag: "Interrupt", requestId: this.attachId })); } catch { /* closing */ } }
    try { socket?.close(); } catch { /* already closed */ }
  }

  /** handleData: one `terminal.write` per input; a failure is written into the terminal. */
  write(data: string): void {
    if (this.target.readOnly || !data || !this.target.attach) return;
    if (data.length > MAX_WRITE_LENGTH) return this.sink.system(SCHEMA_REFUSAL);
    if (!this.open) return this.sink.system("The server is not connected.");
    const { threadId, terminalId } = this.target.attach;
    this.call("terminal.write", { threadId, terminalId, data }, (message) => {
      if (message !== null) this.sink.system(message || "Terminal write failed");
    });
  }

  /** onResize → `terminal.resize`, latest-wins while one is in flight, once the session runs (canResizeTerminal). */
  resize(cols: number, rows: number): void {
    if (cols <= 0 || rows <= 0) return;
    this.size = { cols: Math.min(cols, 1000), rows: Math.min(rows, 500) };
    this.sendSize();
  }

  /** Sign-in input or grid size, queued and sent one request at a time. */
  authInput(data: string, size?: { cols: number; rows: number }): void {
    const auth = this.target.auth;
    if (this.target.readOnly || !auth || !auth.flowId || !auth.interactionId) return;
    for (const slice of authSlices(data)) {
      const response: Record<string, unknown> = { type: "terminal", data: slice };
      if (size) response.size = { cols: Math.max(1, Math.min(500, size.cols)), rows: Math.max(1, Math.min(200, size.rows)) };
      this.authQueue.push(JSON.stringify({ instanceId: auth.instanceId, flowId: auth.flowId, interactionId: auth.interactionId, response }));
    }
    this.drainAuth();
  }

  private drainAuth(): void {
    if (this.authSending || !this.authQueue.length) return;
    if (!this.open) { this.sink.wantSocket(); return; }
    const payload = JSON.parse(this.authQueue.shift()!) as Record<string, unknown>;
    this.authSending = true;
    this.call("provider.auth.respond", payload, (message) => {
      this.authSending = false;
      if (message !== null) {
        this.authQueue = [];
        this.authFailedState = true;
        this.sink.authFailed(true);
        return;
      }
      if (this.authFailedState) { this.authFailedState = false; this.sink.authFailed(false); }
      this.drainAuth();
    });
  }

  private sendSize(): void {
    const attach = this.target.attach, size = this.size;
    if (!attach || this.target.readOnly || !size || !this.open || this.resizing) return;
    if (this.version === 0 || this.status !== "running") return;
    if (this.sent && this.sent.cols === size.cols && this.sent.rows === size.rows) return;
    this.resizing = true;
    this.call("terminal.resize", { threadId: attach.threadId, terminalId: attach.terminalId, ...size }, (message) => {
      this.resizing = false;
      if (message === null) this.sent = size;
      this.sendSize();
    });
  }

  private attach(): void {
    const attach = this.target.attach!;
    this.attachId = this.nextId();
    this.version = 0; this.error = null; this.status = "closed"; this.synchronized = "closed";
    this.sent = null;
    const payload: Record<string, unknown> = this.target.readOnly
      ? { threadId: attach.threadId, terminalId: attach.terminalId }
      : {
          threadId: attach.threadId, terminalId: attach.terminalId, cwd: attach.cwd ?? "",
          worktreePath: attach.worktreePath ?? null,
          ...(attach.env && Object.keys(attach.env).length ? { env: attach.env } : {}),
          ...(attach.providerInstanceId ? { providerInstanceId: attach.providerInstanceId } : {}),
          ...(this.replied ? { resume: true } : {}),
        };
    this.send(request(this.attachId, this.target.readOnly ? "terminal.observe" : "terminal.attach", payload));
  }

  private call(method: string, payload: unknown, done: (failed: string | null) => void): void {
    const id = this.nextId();
    this.pending.set(id, done);
    if (!this.send(request(id, method, payload))) { this.pending.delete(id); done("The server is not connected."); }
  }

  private send(frame: Frame): boolean {
    if (!this.socket || !this.open) return false;
    try { this.socket.send(encode(frame)); return true; } catch { return false; }
  }

  private nextId(): string { this.serial += 1; return String(this.serial); }

  private receive(text: string): void {
    let frames: Frame[];
    try { frames = decode(text); } catch { return; }
    const chunks = frames.filter((frame) => frame._tag === "Chunk").length;
    if (chunks >= 8 || (chunks > 0 && text.length >= 64 * 1024)) this.fullWindows += 1;
    if (chunks > 0) this.largest = Math.max(this.largest, Math.round(text.length / chunks));
    for (const frame of frames) {
      const tag = frame._tag;
      if (tag === "Ping") { this.send({ _tag: "Pong" }); continue; }
      if (tag === "Pong") continue;
      const id = identifier(frame.requestId);
      if (id === null) continue;
      if (tag === "Chunk") {
        if (id === this.attachId) {
          if (!this.answeredSinceOpen) { this.answeredSinceOpen = true; this.sink.answered(); }
          this.replied = true;
          this.chunks += 1;
          for (const value of (Array.isArray(frame.values) ? frame.values : []) as TerminalAttachStreamEvent[]) this.apply(value);
          this.show();
        }
        // Acknowledged once the surface holds it: the server sends at most 8 chunks or 64 KiB ahead.
        this.send({ _tag: "Ack", requestId: frame.requestId as string });
        continue;
      }
      if (tag !== "Exit") continue;
      const exit = (frame.exit ?? {}) as Record<string, unknown>;
      const success = exit._tag === "Success";
      if (id === this.attachId) {
        // The stream ended or failed: it is not resumed. An attach error shows as the session's error
        // (useAttachedTerminalSession).
        this.attachId = "";
        this.replied = false;
        if (!success) this.apply({ type: "error", threadId: "", terminalId: "", message: failure(exit).message } as TerminalAttachStreamEvent);
        this.show();
        continue;
      }
      const done = this.pending.get(id);
      this.pending.delete(id);
      done?.(success ? null : failure(exit).message);
    }
  }

  /** applyTerminalAttachStreamEvent's status, error and version, with the output written as it comes. */
  private apply(event: TerminalAttachStreamEvent): void {
    if (!event || typeof event !== "object") return;
    switch (event.type) {
      case "snapshot": case "restarted": {
        const history = String(event.snapshot?.history ?? "");
        // A first snapshot writes only a history there is (TerminalSessionViewport's setup); any later one, a
        // resumed or new stream's included, replaces what the surface shows, as the reference's new buffer does.
        this.output = "";
        if (this.shown || history !== "") { this.sink.resetAndWrite(history); this.sink.received(history.length); }
        this.shown = true;
        this.status = event.snapshot?.status ?? "running"; this.error = null; this.version += 1;
        this.grid = nextGrid(this.grid, event.snapshot ?? {});
        return;
      }
      case "activity": this.grid = nextGrid(this.grid, event); return;
      case "output": {
        this.output += String(event.data ?? "");
        if (this.status === "closed") this.status = "running";
        this.error = null; this.version += 1;
        return;
      }
      case "cleared": this.output = ""; this.sink.resetAndWrite(""); this.shown = true; this.error = null; this.version += 1; return;
      case "exited": this.status = "exited"; this.error = null; this.version += 1; return;
      case "closed": this.status = "closed"; this.error = null; this.version += 1; return;
      case "error": this.status = "error"; this.error = event.message; this.version += 1; return;
      default: return;
    }
  }

  /** TerminalSessionViewport's session effect after a chunk: the error, then exit handling. */
  private show(): void {
    if (this.output) { this.sink.write(this.output); this.sink.received(this.output.length); this.output = ""; }
    if (this.error !== null && this.error !== this.shownError) this.sink.system(this.error);
    this.shownError = this.error;
    const status = this.status, version = this.version;
    if (version > 0 && status === "running") this.handledExit = false;
    else if (shouldHandleTerminalExit(status, this.synchronized, this.handledExit, version)) {
      this.handledExit = true;
      this.sink.system(status === "closed" ? "Terminal closed" : "Process exited");
      setTimeout(() => { if (this.handledExit && !this.disposed) this.sink.exited(); }, 0);
    }
    if (version > 0) this.synchronized = status;
    // TerminalSessionViewport: every renderer of a shared terminal draws the server's grid.
    if (this.grid !== this.shownGrid) { this.shownGrid = this.grid; this.sink.grid(this.grid); }
    // A fit before the snapshot could not resize; the grid goes again once the session runs.
    this.sendSize();
  }

  private lost(): void {
    this.socket = null;
    this.open = false;
    this.attachId = "";
    this.resizing = false;
    this.authSending = false;
    for (const done of this.pending.values()) done("The server is not connected.");
    this.pending.clear();
    if (!this.disposed) this.sink.wantSocket();
  }
}

/** How long an unanswered ticket request waits before the page asks again. */
export const TICKET_TIMEOUT_MS = 5000;
const SOCKET_RETRY_MS = [500, 1000, 2000, 5000, 10000];

/**
 * The page's requests for a socket URL: the first at once, each failure (no URL, a lost socket) after a growing
 * delay, an unanswered request again after TICKET_TIMEOUT_MS. A stream that answers starts the delays over. Each
 * request carries a serial its answer names, so a late answer to an earlier request (its ticket may have lapsed)
 * is not taken for a later one.
 */
export class SocketRequests {
  private attempts = 0;
  private wanted = false;
  private serial = 0;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private deadline: ReturnType<typeof setTimeout> | null = null;

  // Wrapped: a browser's setTimeout called as another object's method throws "Illegal invocation".
  constructor(private readonly ask: (serial: number) => void, private readonly timers: { set: typeof setTimeout; clear: typeof clearTimeout } =
    { set: ((run: () => void, ms?: number) => setTimeout(run, ms)) as typeof setTimeout, clear: ((id?: ReturnType<typeof setTimeout>) => clearTimeout(id)) as typeof clearTimeout }) {}

  /** A socket is wanted: ask, unless a request is already waiting. */
  want(): void {
    if (this.wanted) return;
    this.wanted = true;
    const delay = this.attempts === 0 ? 0 : SOCKET_RETRY_MS[Math.min(this.attempts - 1, SOCKET_RETRY_MS.length - 1)]!;
    this.attempts += 1;
    this.timer = this.timers.set(() => {
      this.timer = null;
      this.serial += 1;
      this.ask(this.serial);
      this.deadline = this.timers.set(() => { this.deadline = null; this.wanted = false; this.want(); }, TICKET_TIMEOUT_MS);
    }, delay);
  }

  /** The app answered request `serial`: true when it sent a URL to open. Only the latest request's answer counts. */
  answered(url: string, serial: number): boolean {
    if (!this.wanted || serial !== this.serial) return false;
    // A re-ask may be waiting its delay: the answer to the request it would replace stands for it.
    if (this.timer) { this.timers.clear(this.timer); this.timer = null; }
    if (this.deadline) { this.timers.clear(this.deadline); this.deadline = null; }
    this.wanted = false;
    if (!url) { this.want(); return false; }
    return true;
  }

  /** The stream answered: the next loss asks at once again. */
  reset(): void { this.attempts = 0; }

  dispose(): void {
    if (this.timer) this.timers.clear(this.timer);
    if (this.deadline) this.timers.clear(this.deadline);
    this.timer = this.deadline = null;
  }
}
