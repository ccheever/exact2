// LLP 1069.005 D2/D3: a source's secure randomness, the same on every
// executor. Each draw inside an answer counts as a device read; a draw
// during module initialization refuses; `subtle` waits for D1.
const draws: Record<string, () => unknown> = {
  uuid: () => crypto.randomUUID(),
  bytes: () => crypto.getRandomValues(new Uint8Array(4)).length,
};
const atInit: Record<string, string> = {};
for (const name of Object.keys(draws)) {
  try { atInit[name] = "UNGUARDED: " + String(draws[name]()); }
  catch (e) { atInit[name] = String(e.message); }
}

function errorName(call: () => unknown): string {
  try { call(); return "UNGUARDED"; }
  catch (e) { return e.name; }
}

function answer(source: string, args: any[]): unknown {
  switch (source) {
    case "atInit": return atInit[args[0]];
    case "uuid": return crypto.randomUUID();
    case "bytes": return Array.from(crypto.getRandomValues(new Uint8Array(args[0]))).join(",");
    case "uuidLater":
      return fetch("https://fixture.exact.test/value").then(() => crypto.randomUUID());
    case "refusals": return [
      errorName(() => crypto.getRandomValues(new Uint8Array(65537))),
      errorName(() => crypto.getRandomValues(new Float64Array(1) as any)),
      errorName(() => { const { randomUUID } = crypto; return randomUUID(); }),
    ].join("/");
    case "globals": return [
      typeof crypto.subtle,
      typeof (globalThis as any).SubtleCrypto,
      typeof (globalThis as any).CryptoKey,
      Object.prototype.toString.call(crypto),
      Object.keys(Object.getPrototypeOf(crypto)).sort().join(","),
    ].join("/");
    case "plain": return "no randomness";
    default: throw new Error("unknown source " + source);
  }
}

(globalThis as any).exact = {
  abi: 1,
  appId: "test.entropy",
  grants: "net.fetch https://fixture.exact.test\n",
  answer,
};
