// Objects whose collection the executor's between-calls collections bring
// about (js/src/shim.cc, collect_if_due), each made beside garbage enough to
// make a collection due. A FinalizationRegistry reports each, at the next
// microtask drain after its collection; a WeakRef would not do, since its
// target is kept until that drain.

type Answer = string;

const cleared = new Set<string>();
const registry = new FinalizationRegistry((name: string) => {
  cleared.add(name);
});

// About `bytes` of short-lived arrays.
function garbage(bytes: number): number {
  let n = 0;
  for (let i = 0; i < bytes / 64; i++) n += [i, i + 1, i + 2].length;
  return n;
}

function watched(name: string): object {
  const object = { name };
  registry.register(object, name);
  return object;
}

// Module initialization allocates past the first collection's threshold.
watched("atInit");
garbage(1 << 20);

function state(name: string): Answer {
  return cleared.has(name) ? "cleared" : "held";
}

function answer(source: string): unknown {
  if (source === "big") {
    // Allocates past the young generation with the object reachable, so
    // Hermes' own collections within the call move it to the old generation.
    let object: object | null = watched("big");
    garbage(8 << 20);
    const held = object !== null;
    object = null;
    return held ? state("big") : "lost";
  }
  return state(source === "inCall" ? "big" : source);
}

(globalThis as any).exact = {
  abi: 1,
  appId: "test.heap",
  grants: "",
  answer,
};
