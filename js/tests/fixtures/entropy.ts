// LLP 1069.005: a source's Web Crypto, the same on every executor. Each
// draw of randomness inside an answer counts as a device read; a draw
// during module initialization refuses (D2, D3). A digest is pure and
// allowed anywhere; every other `subtle` member refuses by name (D1).
const draws: Record<string, () => unknown> = {
  uuid: () => crypto.randomUUID(),
  bytes: () => crypto.getRandomValues(new Uint8Array(4)).length,
  digest: () => typeof crypto.subtle.digest("SHA-256", new Uint8Array(0)).then,
};
const atInit: Record<string, string> = {};
for (const name of Object.keys(draws)) {
  try { atInit[name] = "ran: " + String(draws[name]()); }
  catch (e) { atInit[name] = String(e.message); }
}

function errorName(call: () => unknown): string {
  try { call(); return "UNGUARDED"; }
  catch (e) { return e.name; }
}

async function rejection(call: () => Promise<unknown>): Promise<string> {
  try { await call(); return "RESOLVED"; }
  catch (e) { return e.name; }
}

const hex = (buffer: ArrayBuffer) =>
  Array.from(new Uint8Array(buffer), b => b.toString(16).padStart(2, "0")).join("");

// The vectors: empty, `abc`, and 1 MiB of `i % 251`, under each SHA-2 size.
function inputs(): Uint8Array[] {
  const large = new Uint8Array(1 << 20);
  for (let i = 0; i < large.length; i++) large[i] = i % 251;
  return [new Uint8Array(0), new TextEncoder().encode("abc"), large];
}

async function digests(): Promise<string> {
  const lines: string[] = [];
  for (const name of ["SHA-256", "SHA-384", "SHA-512"]) {
    for (const data of inputs()) lines.push(hex(await crypto.subtle.digest(name, data)));
  }
  // The web's other spellings: any case, an algorithm object, a view's own
  // bytes (not its whole buffer), and an ArrayBuffer.
  const abc = new TextEncoder().encode("xabcx").subarray(1, 4);
  lines.push(hex(await crypto.subtle.digest({ name: "sha-256" }, abc)));
  lines.push(hex(await crypto.subtle.digest("Sha-256", abc.slice().buffer)));
  return lines.join("\n");
}

async function digestRefusals(): Promise<string> {
  const subtle = crypto.subtle as any;
  return [
    await rejection(() => subtle.digest("SHA-1", new Uint8Array(0))),
    await rejection(() => subtle.digest("MD5", new Uint8Array(0))),
    await rejection(() => subtle.digest("SHA-256", "abc")),
    await rejection(() => subtle.digest.call({}, "SHA-256", new Uint8Array(0))),
    await rejection(() => subtle.encrypt({ name: "AES-GCM" }, null, new Uint8Array(0))),
    await rejection(() => subtle.deriveBits({ name: "HKDF" }, null, 8)),
  ].join("/");
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
      Object.prototype.toString.call(crypto.subtle),
    ].join("/");
    case "digests": return digests();
    case "abcDigest": return crypto.subtle.digest("SHA-256", new TextEncoder().encode("abc")).then(hex);
    // What a LAN dev page (no secure context) still has: SHA-256 only.
    case "lan": return (async () => [
      hex(await crypto.subtle.digest("SHA-256", new TextEncoder().encode("abc"))),
      await rejection(() => crypto.subtle.digest("SHA-384", new Uint8Array(0))),
    ].join("/"))();
    case "digestRefusals": return digestRefusals();
    case "digestLater":
      return fetch("https://fixture.exact.test/value")
        .then(() => crypto.subtle.digest("SHA-256", new TextEncoder().encode("abc")))
        .then(hex);
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
