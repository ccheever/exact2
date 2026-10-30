// A child process as a stream resource (LLP 1133.007 D2): the mapper keeps
// each incarnation's handle and writes its stdin; a mutation writes, ends
// and signals through the kept handle; a one-shot `execFile` and the
// permission fact.

type Child = {
  readonly pid: number | null;
  readonly writableLength: number;
  write(data: string | ArrayBuffer | ArrayBufferView): boolean;
  end(): void;
  kill(signal?: string): boolean;
};
type Event = { type: string; data?: string | ArrayBuffer; pid?: number; restarts?: number; code?: number | null; signal?: string | null; drained?: boolean; kind?: string; message?: string };
const cp = (globalThis as any).__exact_child_process;

let child: Child | null = null;

function server(id: string): unknown {
  return cp.spawn("path:t3", ["--bootstrap-fd", "0", id], {
    encoding: "utf8",
    exactRestart: { when: "always", delay: 500 },
    exactStream(e: Event, handle: Child) {
      child = handle;
      if (e.type === "spawn") { handle.write("envelope " + id + "\n"); handle.end(); return `spawn ${e.pid} ${e.restarts}`; }
      if (e.type === "stdout" || e.type === "stderr") return `${e.type} ${e.data}`;
      if (e.type === "exit") return `exit ${e.code} ${e.signal}`;
      if (e.type === "close") {
        let late = "";
        try { handle.write("late"); } catch (x) { late = ` (${(x as Error).name})`; }
        return `close ${e.code} ${e.drained}${late}`;
      }
      if (e.type === "error") return `error ${e.kind}: ${e.message}`;
      return e.type;
    },
  });
}

function poke(what: string): unknown {
  if (!child) return "no child";
  try {
    switch (what) {
      case "write": return String(child.write("x".repeat(70000))) + " " + child.writableLength;
      case "kill": return String(child.kill("SIGINT"));
      case "bad signal": return String(child.kill("SIGSEGV"));
      case "end": child.end(); return "ended";
    }
  } catch (x) { return `${(x as Error).name}: ${(x as Error).message}`; }
  return "?";
}

async function version(): Promise<string> {
  try {
    const r = await cp.execFile("path:t3", ["--version"], { encoding: "utf8", timeout: 1500 });
    return `${r.code} ${r.stdout.trim()}`;
  } catch (x) { return `${(x as any).code}: ${(x as Error).message}`; }
}

// Two one-shots in one answer: the second is asked by the turn that
// answers the first, which keeps the origin of the turn that asked.
async function twice(): Promise<string> {
  const a = await cp.execFile("path:t3", ["--version"], { encoding: "utf8" });
  const b = await cp.execFile("path:t3", ["--help"], { encoding: "utf8" });
  return `${a.stdout.trim()} ${b.stdout.trim()}`;
}

// A one-shot its `signal` aborts (D3): another answer's turn aborts it, and
// the answer that awaited it resumes with the rejection.
let stopping: AbortController | null = null;
async function held(): Promise<string> {
  stopping = new AbortController();
  try {
    const r = await cp.execFile("path:t3", ["--wait"], { encoding: "utf8", signal: stopping.signal });
    return `${r.code}`;
  } catch (x) { return (x as Error).name; }
}

function answer(source: string, args: unknown[]): unknown {
  switch (source) {
    case "server": return server(String(args[0]));
    case "poke": return poke(String(args[0]));
    case "version": return version();
    case "twice": return twice();
    case "held": return held();
    case "halt": if (stopping) stopping.abort(); return "halted";
    case "status": return JSON.stringify(cp.processStatus());
    case "refused": return cp.spawn("path:t3", [], { shell: true, exactStream() { return ""; } }).catch((x: Error) => x.message);
    default: throw new Error("unknown source " + source);
  }
}

(globalThis as any).exact = {
  abi: 1,
  appId: "test.process",
  grants: "process.spawn path:t3\n",
  answer,
};
