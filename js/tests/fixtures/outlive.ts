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

// A write the app can abort (LLP 1146 D2): the first call to ask starts it,
// beside the ordered lane, and its answer, let go while the POST is held, has
// it kept. Its continuation's catch writes the store. `cancel` aborts it in a
// reply's turn and answers outside its shape, refusing that turn's commit;
// `cancelAwait` aborts it and awaits its end in the same answer.
let cancelling: AbortController | null = null;
let abortable: Promise<string> | null = null;

async function writeOnce(url: string, store: Store, signal: AbortSignal): Promise<string> {
  try {
    const init = { method: 'POST', body: 'w', signal, exactIndependentHttp: { maxResponseBytes: 65536 } };
    const reply = await fetch(url, init as RequestInit);
    return await reply.text();
  } catch (e) {
    const name = (e as { name?: string }).name;
    store.set(KEY, `aborted ${name}`);
    return `aborted ${name}`;
  }
}

async function write(n: unknown, url: unknown, store: Store): Promise<{ text: string }> {
  if (!abortable) {
    cancelling = new AbortController();
    abortable = writeOnce(String(url), store, cancelling.signal);
  }
  return { text: `write ${n} ${await abortable}` };
}

async function cancel(n: unknown): Promise<unknown> {
  if (Number(n) < 2 || !cancelling) return { text: 'idle' };
  await fetch('https://outlive.test/ping');
  cancelling.abort();
  return { wrong: true };
}

async function cancelAwait(n: unknown): Promise<{ text: string }> {
  if (Number(n) < 2 || !cancelling || !abortable) return { text: 'idle' };
  cancelling.abort();
  return { text: `awaited ${await abortable}` };
}

// Two writes, the second made before the first is awaited: the answer's
// settle hands out the first, and the second stays unclaimed.
async function twice(): Promise<{ text: string }> {
  const first = fetch('https://outlive.test/first', { method: 'POST' });
  const second = fetch('https://outlive.test/second', { method: 'POST' });
  return { text: `${await (await first).text()} ${await (await second).text()}` };
}

function answer(source: string, args: unknown[], store: Store): unknown {
  switch (source) {
    case 'token': return { text: store.get(KEY) ?? 'none' };
    case 'read': return read(store);
    case 'late': return late(store);
    case 'renewed': return renewed(args[0], store);
    case 'stands': return stands(store);
    case 'tagged': return tagged();
    case 'write': return write(args[0], args[1], store);
    case 'cancel': return cancel(args[0]);
    case 'cancelAwait': return cancelAwait(args[0]);
    case 'twice': return twice();
    default: throw Object.assign(new Error(source), { kind: 'UnknownSource' });
  }
}
(globalThis as any).exact = {
  abi: 1,
  appId: "test.outlive",
  grants: "net.fetch https://outlive.test\nnet.fetch local-network development\nsecret.keep outlive.token\n",
  answer,
};
