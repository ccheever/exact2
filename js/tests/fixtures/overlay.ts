// An overlay runs synchronously with no effects. Its calls below are tried in
// order; `refusals` answers how each went, one line per call. `capture` keeps
// the store and storage an answer is given, so the overlay can try them.
let captured: { store: any; storage: any } | null = null;
let results: string[] = [];
let pending: Promise<unknown>[] = [];

function attempt(call: () => unknown): void {
  const i = results.push("") - 1;
  let value: unknown;
  try { value = call(); } catch (e) { results[i] = "throws " + (e as Error).message; return; }
  if (!value || typeof (value as Promise<unknown>).then !== "function") { results[i] = "runs"; return; }
  pending.push((value as Promise<unknown>).then(() => { results[i] = "resolves"; }, e => { results[i] = "rejects " + e.message; }));
}

function overlay(): undefined {
  const subtle = crypto.subtle as any;
  const store = captured!.store;
  attempt(() => crypto.getRandomValues(new Uint8Array(1)));
  attempt(() => crypto.randomUUID());
  for (const m of ["digest", "generateKey", "sign", "verify", "importKey", "exportKey", "encrypt", "decrypt", "deriveBits"]) {
    attempt(() => subtle[m]("SHA-256", new Uint8Array(0)));
  }
  attempt(() => fetch("https://fixture.exact.test/value"));
  attempt(() => store.get("x"));
  attempt(() => store.set("x", "y"));
  attempt(() => store.forget("x"));
  attempt(() => store.keepKey("x", null));
  attempt(() => store.key("x"));
  attempt(() => captured!.storage.fs.readFile("app:/x"));
  return undefined;
}

function answer(source: string, _args: unknown[], store: unknown, storage: unknown): unknown {
  if (source === "capture") { captured = { store, storage }; return "captured"; }
  if (source === "refusals") {
    const done = Promise.all(pending);
    pending = [];
    return done.then(() => { const lines = results.join("\n"); results = []; return lines; });
  }
  if (source === "items") return [];
  throw new Error("unknown source " + source);
}

(globalThis as any).exact = {
  abi: 1,
  appId: "test.overlay",
  grants: "net.fetch https://fixture.exact.test\nfs.read app:/x\n",
  answer,
  overlay,
};
