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

async function row(i: number): Promise<Row> {
  const shared = await loadShared();
  const own = await (await fetch(`${API}/row/${i}`)).text();
  return { text: `${own} after ${shared}` };
}

function answer(source: string, args: unknown[]): unknown {
  if (source === "row" && typeof args[0] === "number") return row(args[0]);
  throw { kind: "UnknownSource", message: source };
}

(globalThis as any).exact = {
  abi: 1,
  appId: "test.shared-load",
  grants: "net.fetch https://shared-load.test\n",
  answer,
};
