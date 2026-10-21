// Many answers that each await one shared load before their own fetch, as
// the Bluesky clone's sources await `loadModeration()` (LLP 1041 §8.4,
// proposed amendment, 2026-10-09): the load is one promise, made by the
// first answer to ask, and it performs two fetches in a row through the
// host; every answer then fetches its own row.

type Row = { text: string };

const API = "https://shared-load.test";

let loading: Promise<string> | null = null;

function loadShared(): Promise<string> {
  if (!loading) {
    loading = (async () => {
      const prefs = await (await fetch(`${API}/prefs`)).text();
      const labelers = await (await fetch(`${API}/labelers`)).text();
      return `${prefs}+${labelers}`;
    })();
  }
  return loading;
}

// How many times each row's answer began: a host that restarted a source
// instead of asking the same call again would show more than one.
const begun = new Map<number, number>();

async function row(i: number): Promise<Row> {
  begun.set(i, (begun.get(i) ?? 0) + 1);
  const shared = await loadShared();
  const own = await (await fetch(`${API}/row/${i}`)).text();
  return { text: `${own} after ${shared} #${begun.get(i)}` };
}

// An answer that awaits the shared load and fetches nothing of its own:
// many of these wait on it without asking the network for more.
async function quiet(i: number): Promise<Row> {
  begun.set(-1 - i, (begun.get(-1 - i) ?? 0) + 1);
  const shared = await loadShared();
  return { text: `quiet-${i} after ${shared} #${begun.get(-1 - i)}` };
}

function answer(source: string, args: unknown[]): unknown {
  if (source === "row" && typeof args[0] === "number") return row(args[0]);
  if (source === "quiet" && typeof args[0] === "number") return quiet(args[0]);
  throw { kind: "UnknownSource", message: source };
}

(globalThis as any).exact = {
  abi: 1,
  appId: "test.shared-load",
  grants: "net.fetch https://shared-load.test\n",
  answer,
};
