// A local listener (LLP 1133.005 D1): the mapper keeps each connection's
// handle and answers a bad line at once; a mutation answers a line later.

type Connection = { readonly readyState: number; send(line: string): void; end(): void };
type Event = { type: string; id?: number; data?: string; reason?: string; address?: string; port?: number; kind?: string; message?: string };
declare function exactListen(address: string, options: unknown): Promise<unknown>;

const connections = new Map<number, Connection>();
const said = (x: unknown) => ` (${(x as Error).name}: ${(x as Error).message})`;

function control(running: boolean): unknown {
  if (!running) return "off";
  return exactListen("unix:/tmp/exact-listen-js/app.sock", {
    takeover: true,
    firstLineTimeout: 5000,
    exactStream(e: Event, c?: Connection) {
      if (e.type === "listening") return `listening ${e.address} ${e.port}`;
      if (e.type === "error") return `error ${e.kind}: ${e.message}`;
      if (e.type === "connection") { connections.set(e.id!, c!); c!.send("hello"); return `connection ${e.id} ${c!.readyState}`; }
      if (e.type === "end") {
        connections.delete(e.id!);
        let late = "";
        try { c!.send("late"); } catch (x) { late = said(x); }
        return `end ${e.id} ${e.reason} ${c!.readyState}${late}`;
      }
      let note = "";
      try {
        switch (e.data) {
          case "bad": c!.send("refused"); c!.end(); break;
          case "two": c!.send("a\nb"); break;
          case "other": connections.get(1)!.send("stray"); break;
          case "after end": c!.end(); c!.send("never"); break;
        }
      } catch (x) { note = said(x); }
      return `message ${e.id} ${e.data}${note}`;
    },
  });
}

function reply(id: number, text: string): unknown {
  const c = connections.get(id);
  if (!c) return "no connection " + id;
  try {
    if (text === "end") c.end(); else c.send(text);
    return "sent";
  } catch (x) {
    return said(x).trim();
  }
}

function answer(source: string, args: unknown[]): unknown {
  switch (source) {
    case "control": return control(args[0] === true);
    case "reply": return reply(Number(args[0]), String(args[1]));
    case "detect": return String("exactListen" in globalThis);
    case "listenNow":
      try { exactListen("unix:/tmp/exact-listen-js/m.sock", { exactStream: () => "" }); return "listened"; } catch (x) { return said(x).trim(); }
    default: throw new Error("unknown source " + source);
  }
}

(globalThis as any).exact = {
  abi: 1,
  appId: "test.listen",
  grants: "net.listen unix:/tmp/exact-listen-js\n",
  answer,
};
