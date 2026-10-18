// A resource's two-way socket (LLP 1016.000 D7): the mapper keeps its
// handle by the resource's id and replies to frames on it, and a mutation
// and a resource's answer send on it.

type Socket = {
  readonly readyState: number;
  readonly bufferedAmount: number;
  send(data: string | ArrayBuffer | ArrayBufferView): void;
  close(code?: number, reason?: string): void;
};
type Event = {
  type: string; data: string | ArrayBuffer; code?: number; reason?: string;
  wasClean?: boolean; kind?: string; message?: string;
};

const sockets = new Map<string, Socket>();
const controllers = new Map<string, AbortController>();
// What refused outside a mapper's own turn, for the `report` frame.
const refusals: string[] = [];
const refused = (x: unknown) => refusals.push((x as Error).message);

// The mapper's reply to one frame of its own socket; what refused, if anything.
function reply(socket: Socket, type: string, data: string): string {
  try {
    if (type === "open") { socket.send("subscribe"); return ""; }
    if (data.startsWith("frame ")) { socket.send("Ack " + data.slice(6)); return ""; }
    switch (data) {
      case "mapper close": socket.close(4001, "done"); return "";
      case "mapper probe": { const before = socket.bufferedAmount; socket.send("abc"); return ` ${before}:${socket.bufferedAmount}`; }
      case "mapper other": sockets.get("side")!.send("stray"); return "";
      case "mapper later": Promise.resolve().then(() => socket.send("never")).catch(refused); return "";
      case "mapper fetch": fetch("https://echo.test/x").catch(refused); return "";
      case "mapper too many": for (let i = 0; i <= 256; i++) socket.send("x"); return "";
      case "mapper too big": socket.send("x".repeat(1048577)); return "";
      case "mapper throws": socket.send("never"); throw new Error("the mapper threw");
      case "report": return " " + refusals.join("; ");
    }
    return "";
  } catch (x) {
    if ((x as Error).message === "the mapper threw") throw x;
    return ` (${(x as Error).message})`;
  }
}

function feed(id: string): unknown {
  const controller = new AbortController();
  controllers.set(id, controller);
  return fetch("wss://echo.test/" + id, {
    signal: controller.signal,
    exactStream(e: Event, socket?: Socket) {
      if (socket) sockets.set(id, socket);
      let late = "";
      if (e.type === "close" || e.type === "error") try { socket?.send("late"); } catch (x) { late = ` (${(x as Error).message})`; }
      if (e.type === "close") return `close ${e.code} ${e.reason} ${e.wasClean}${late}`;
      if (e.type === "error") return `error ${e.kind}: ${e.message}${late}`;
      const data = typeof e.data === "string" ? e.data : "bytes " + Array.from(new Uint8Array(e.data)).join(",");
      // A server's reply to an answer's request, settled by `deliver`'s turn, or (`direct`) by this mapper.
      if (data.startsWith("reply ")) { const [, n, ...rest] = data.split(" "); queue.push(() => waiters.get(n)?.(rest.join(" "))); }
      if (data.startsWith("direct ")) { const [, n, ...rest] = data.split(" "); waiters.get(n)?.(rest.join(" ")); }
      const said = socket && id !== "quiet" ? reply(socket, e.type, data) : "";
      if (data === "mapper misshapen") { socket?.send("never"); return 123; }
      return `${e.type} ${data} ${socket?.readyState}${said}`;
    },
  } as RequestInit);
}

function say(id: string, what: string): unknown {
  const socket = sockets.get(id);
  if (!socket) throw new Error("no socket " + id);
  try {
    switch (what) {
      case "binary": {
        const bytes = new Uint8Array([9, 1, 2, 3]);
        socket.send(bytes.subarray(1));
        bytes[1] = 7; // copied at the call
        return "sent";
      }
      case "close": socket.close(4001, "finished"); return "closed";
      case "probe": { const before = socket.bufferedAmount; socket.send("abc"); return `${before}:${socket.bufferedAmount}`; }
      case "read": return String(socket.bufferedAmount);
      case "big": socket.send("x".repeat(200000)); return "sent";
      case "send then fetch": socket.send("a"); fetch("https://echo.test/x").catch(() => {}); return "sent";
      case "send, await, fetch": socket.send("a"); return Promise.resolve().then(() => fetch("https://echo.test/x").catch(() => "refused"));
      case "send then fetch later": socket.send("a"); Promise.resolve().then(() => fetch("https://echo.test/x").catch(() => {})); return "sent";
      case "fetch then send": fetch("https://echo.test/x").catch(() => {}); try { socket.send("a"); } catch {} return "sent";
      case "abort": controllers.get(id)!.abort(); socket.send("x"); return "sent";
      case "after close": socket.close(); socket.send("x"); return "sent";
      case "bad code": socket.close(1001); return "closed";
      case "too many": for (let i = 0; i <= 256; i++) socket.send("x"); return "sent";
      case "too big": socket.send("x".repeat(1048577)); return "sent";
      case "throws": socket.send("never"); throw new Error("the source threw");
      case "async": socket.send("early"); return Promise.resolve("later");
      case "later": return Promise.resolve().then(() => { socket.send("after"); return "sent"; });
      case "then send": Promise.resolve().then(() => socket.send("never")).catch(refused); return "now";
      default: socket.send(what); socket.send(what + " again"); return "sent";
    }
  } catch (e) {
    if ((e as Error).message === "the source threw") throw e;
    return `${(e as Error).name}: ${(e as Error).message}`;
  }
}

// A resource's answer (the amendment of 2026-10-18): it sends a request and
// awaits the reply; what its code sends after an await is the turn's that
// settled it.
const waiters = new Map<string, (data: string) => void>();
const queue: Array<() => void> = [];
const waitFor = (n: string) => new Promise<string>(resolve => waiters.set(n, resolve));

async function ask(id: string, q: string): Promise<string> {
  if (!q) return "";
  const socket = sockets.get(id)!, [what, n] = q.split(" ");
  try {
    switch (what) {
      case "sync": socket.send("request " + n); return "got " + await waitFor(n);
      case "chain": { socket.send("request " + n); const got = await waitFor(n); socket.send("thanks " + n); return "chained " + got; }
      case "fetch": socket.send("request " + n); await fetch("https://echo.test/x"); socket.send("after fetch " + n); return "fetched";
      case "abandon":
        socket.send("request " + n);
        try { await fetch("https://echo.test/slow"); } catch { try { socket.send("cancel " + n); } catch (x) { refused(x); } }
        return "abandoned";
      case "split":
        for (let i = 0; i < 200; i++) socket.send("x");
        await fetch("https://echo.test/x");
        for (let i = 0; i < 100; i++) socket.send("y");
        return "sent";
      case "probe": { const before = socket.bufferedAmount; socket.send("abc"); return `${before}:${socket.bufferedAmount}`; }
      case "big": socket.send("x".repeat(200000)); return "sent";
      case "many": for (let i = 0; i <= 256; i++) socket.send("x"); return "sent";
      case "huge": socket.send("x".repeat(1048577)); return "sent";
      case "close": socket.close(4001, "done"); socket.send("after"); return "sent";
      case "quiet": sockets.get("quiet")!.send("x"); return "sent";
      case "throws": socket.send("request throws"); throw new Error("the answer threw");
    }
    return "unknown " + q;
  } catch (x) {
    if ((x as Error).message === "the answer threw") throw x;
    return `${(x as Error).name}: ${(x as Error).message}`;
  }
}

// Runs the replies the mapper queued, in this answer's turn: the code that
// awaited them resumes here, as the T3 Code clone's transport does.
function deliver(side: string): string {
  for (const work of queue.splice(0)) work();
  return side;
}

function answer(source: string, args: unknown[]): unknown {
  switch (source) {
    case "feed": return feed(String(args[0]));
    case "say": return say(String(args[0]), String(args[1]));
    case "ask": return ask(String(args[0]), String(args[1]));
    case "deliver": return deliver(String(args[0]));
    default: throw new Error("unknown source " + source);
  }
}

(globalThis as any).exact = {
  abi: 1,
  appId: "test.socket",
  grants: "net.websocket wss://echo.test\n",
  answer,
};
