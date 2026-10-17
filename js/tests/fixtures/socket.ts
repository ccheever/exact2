// A resource's two-way socket (LLP 1016.000 D7): the mapper keeps its
// handle by the resource's id and replies to frames on it, and a mutation
// sends on it.

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
  // A resource's source cannot send on a handle the module kept.
  for (const kept of sockets.values()) try { kept.send("from a resource"); } catch (x) { refused(x); }
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
      const said = socket ? reply(socket, e.type, data) : "";
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
      case "async": socket.send("never"); return Promise.resolve("later");
      case "later": return Promise.resolve().then(() => { socket.send("never"); return "sent"; });
      default: socket.send(what); socket.send(what + " again"); return "sent";
    }
  } catch (e) {
    if ((e as Error).message === "the source threw") throw e;
    return `${(e as Error).name}: ${(e as Error).message}`;
  }
}

function answer(source: string, args: unknown[]): unknown {
  switch (source) {
    case "feed": return feed(String(args[0]));
    case "say": return say(String(args[0]), String(args[1]));
    default: throw new Error("unknown source " + source);
  }
}

(globalThis as any).exact = {
  abi: 1,
  appId: "test.socket",
  grants: "net.websocket wss://echo.test\n",
  answer,
};
