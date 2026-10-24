// A walk over `n` items of work, the shape of a large response's parse
// (LLP 1141.001 D10): `walk(n, every)` awaits `scheduler.yield()` every
// `every` items, or never for 0, and answers the walk's checksum and how
// many times it yielded, as "sum/yields". `sharedA` and `sharedB` answer one
// walk between them, which the first to ask starts.

async function walk(n: number, every: number): Promise<string> {
  let sum = 0;
  let yields = 0;
  try {
    for (let i = 0; i < n; i++) {
      sum = (sum * 31 + i) % 1000003;
      if (every > 0 && (i + 1) % every === 0) {
        await scheduler.yield();
        yields++;
      }
    }
  } catch (e) {
    console.log(`walk ${n}: ${(e as Error).name}`);
    throw e;
  }
  return `${sum}/${yields}`;
}

let shared: { n: number; walk: Promise<string> } | null = null;
function sharedWalk(n: number, every: number): Promise<string> {
  if (shared?.n !== n) shared = { n, walk: walk(n, every) };
  return shared.walk;
}

// A yield outside an answer's turn goes on at once.
let atLoad = "pending";
scheduler.yield().then(() => {
  atLoad = "resolved";
});

function answer(source: string, args: unknown[]): unknown {
  const n = Number(args[0]);
  const every = Number(args[1]);
  if (source === "walk") return walk(n, every);
  if (source === "sharedA" || source === "sharedB") return sharedWalk(n, every);
  // After the microtasks the module's evaluation queued.
  if (source === "atLoad") return Promise.resolve().then(() => atLoad);
  if (source === "postTask") {
    try {
      (scheduler as any).postTask(() => {});
      return "ran";
    } catch (e) {
      return (e as Error).message;
    }
  }
  throw { kind: "UnknownSource", message: source };
}

(globalThis as any).exact = {
  abi: 1,
  appId: "test.walk",
  grants: "",
  answer,
};
