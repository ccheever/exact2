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

// Frames whose answer is a large string, and how many of them saw an
// earlier frame's sentinel finalized: at most one per collection.
let frames = 0;
let collectedFrames = 0;
let clearedBefore = 0;

// Answers that each await a fetch and reply with a large string, two in
// flight at once; each reply counts as `large` does. The first allocates
// garbage enough to make a collection due while it waits.
let overlapReplies = 0;
let overlapCollected = 0;
let overlapClearedBefore = 0;

async function overlap(n: number): Promise<string> {
  if (n === 0) garbage(1 << 20);
  await fetch(`https://heap.test/${n}`);
  overlapReplies++;
  registry.register({}, `overlap ${n}`);
  if (cleared.size !== overlapClearedBefore) {
    overlapClearedBefore = cleared.size;
    overlapCollected++;
  }
  return "o".repeat(65536) + n;
}

function answer(source: string, args: unknown[]): unknown {
  if (source === "overlap") return overlap(args[0] as number);
  if (source === "overlapCollected") return String(overlapCollected);
  if (source === "overlapCleared") return state(`overlap ${args[0]}`);
  if (source === "large") {
    frames++;
    registry.register({}, `frame ${frames}`);
    if (cleared.size !== clearedBefore) {
      clearedBefore = cleared.size;
      collectedFrames++;
    }
    return "l".repeat(65536) + frames;
  }
  if (source === "collections") return String(collectedFrames);
  if (source === "heldLarge") {
    // A large string the answer holds past the collection its garbage makes
    // due after the call, sent at the settle that follows.
    const value = { text: "h".repeat(70000) };
    registry.register(value, "heldLarge");
    garbage(1 << 20);
    return value;
  }
  if (source === "heldLargeState") return state("heldLarge");
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
  grants: "net.fetch https://heap.test",
  answer,
};
