// A resource's two-way socket (LLP 1016.000 D7): the mapper keeps its
// handle by the resource's id, and a mutation sends on it.

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

function feed(id: string): unknown {
  const controller = new AbortController();
  controllers.set(id, controller);
  return fetch("wss://echo.test/" + id, {
    signal: controller.signal,
    exactStream(e: Event, socket?: Socket) {
      if (socket) sockets.set(id, socket);
      if (e.type === "close") return `close ${e.code} ${e.reason} ${e.wasClean}`;
      if (e.type === "error") return `error ${e.kind}: ${e.message}`;
      let mapper = "";
      try { socket?.send("from the mapper"); } catch (x) { mapper = ` (${(x as Error).message})`; }
      const data = typeof e.data === "string" ? e.data : "bytes " + Array.from(new Uint8Array(e.data)).join(",");
      return `${e.type} ${data} ${socket?.readyState}${mapper}`;
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
