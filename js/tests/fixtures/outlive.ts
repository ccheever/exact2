// Work that outlives its answer (LLP 1146 D2): a renewal that one answer
// starts and others await, the way @exact/oauth refreshes, with no lock. The
// renewal is a POST, which runs to its end when the answer that started it
// is let go; what its continuation writes to the store stands.
type Store = { get(name: string): string | null; set(name: string, value: string): void };
const KEY = 'outlive.token';

let renewing: Promise<string> | null = null;

async function renewOnce(store: Store): Promise<string> {
  const reply = await fetch('https://outlive.test/token', { method: 'POST', body: store.get(KEY) ?? '0' });
  const next = await reply.text();
  store.set(KEY, next);
  return next;
}

// One renewal at a time: a caller awaits the one in flight.
function renew(store: Store): Promise<string> {
  renewing ??= renewOnce(store).finally(() => { renewing = null; });
  return renewing;
}

async function read(store: Store): Promise<{ text: string }> {
  // Only a missing token is renewed, as a refresh follows only a refusal.
  const token = store.get(KEY) ?? await renew(store);
  // A live answer resumes in its own turn: its read and its fetch are its own.
  const kept = store.get(KEY);
  const reply = await fetch(`https://outlive.test/me?token=${token}`);
  return { text: `read ${await reply.text()} kept ${kept}` };
}

// The store is read only after a reply, in a later turn of the answer.
async function late(store: Store): Promise<{ text: string }> {
  await fetch('https://outlive.test/ping');
  return { text: `late ${store.get(KEY) ?? 'none'}` };
}

// Once `go` is set, a missing token is renewed.
async function renewed(go: unknown, store: Store): Promise<{ text: string }> {
  return { text: go ? `renewed ${store.get(KEY) ?? await renew(store)}` : 'idle' };
}

// A renewal whose answer then fails its shape, refusing the commit that
// lands it: what the reply's turn wrote stands.
async function stands(store: Store): Promise<unknown> {
  await renew(store);
  return { wrong: true };
}

// Each call numbers itself, so a test can tell which call took a reply.
let calls = 0;
async function tagged(): Promise<{ text: string }> {
  const call = ++calls;
  const reply = await fetch('https://outlive.test/tagged', { method: 'POST' });
  return { text: `call ${call} ${await reply.text()}` };
}

function answer(source: string, args: unknown[], store: Store): unknown {
  switch (source) {
    case 'token': return { text: store.get(KEY) ?? 'none' };
    case 'read': return read(store);
    case 'late': return late(store);
    case 'renewed': return renewed(args[0], store);
    case 'stands': return stands(store);
    case 'tagged': return tagged();
    default: throw Object.assign(new Error(source), { kind: 'UnknownSource' });
  }
}
(globalThis as any).exact = {
  abi: 1,
  appId: "test.outlive",
  grants: "net.fetch https://outlive.test\nsecret.keep outlive.token\n",
  answer,
};
