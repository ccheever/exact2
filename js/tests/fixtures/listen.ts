// A local listener (LLP 1133.005): the mapper answers a bad line at once on
// its own connection and keeps each connection's handle, so a mutation
// answers a good one later; a mutation cannot listen.

type Connection = { readonly readyState: number; send(line: string): void; end(): void };
type Event = { type: string; id?: number; data?: string; reason?: string; address?: string; port?: number; kind?: string; message?: string };

const connections = new Map<number, Connection>();
let ended: Connection | undefined;

function control(): unknown {
  return (globalThis as any).exactListen("unix:/tmp/exact-listen-js/a.sock", {
    takeover: true,
    firstLineTimeout: 5000,
    exactStream(e: Event, connection?: Connection) {
      if (e.type === "connection") connections.set(e.id!, connection!);
      if (e.type === "end") { connections.delete(e.id!); ended = connection; }
      if (e.type === "message") {
        try {
          if (e.data === "bad") { connection!.send("refused"); connection!.end(); }
          if (e.data === "newline") connection!.send("two\nlines");
          if (e.data === "other") connections.get(1)!.send("stray");
        } catch (x) {
          return `${e.type} ${e.id} ${(x as Error).name}`;
        }
      }
      const at = e.type === "listening" ? `${e.address} ${e.port}` : e.type === "error" ? `${e.kind}` : `${e.id} ${e.data ?? e.reason ?? ""}`;
      return `${e.type} ${at} ${connection ? connection.readyState : "-"}`.trim();
    },
  });
}

function reply(id: string, text: string): string {
  const connection = connections.get(Number(id));
  if (id === "ended") {
    try { ended!.send("late"); } catch (x) { return (x as Error).name; }
  }
  if (!connection) return "gone";
  connection.send(text);
  connection.end();
  return "sent";
}

function answer(source: string, args: unknown[]): unknown {
  switch (source) {
    case "control": return control();
    case "present": return "exactListen" in globalThis ? "yes" : "no";
    case "reply": return reply(String(args[0]), String(args[1]));
    case "listenFromMutation":
      try { return control(); } catch (x) { return (x as Error).name; }
    default: throw new Error("unknown source " + source);
  }
}

(globalThis as any).exact = {
  abi: 1,
  appId: "test.listen",
  grants: "net.listen unix:/tmp/exact-listen-js\n",
  answer,
};
