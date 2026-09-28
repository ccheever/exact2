// LLP 1069.005 D1b: ECDSA P-256 for DPoP, the same on every executor —
// generate, sign (ES256, raw r‖s), and JWK import and export, with the web's
// errors; generating and signing are counted reads; `store.keepKey` and
// `store.key` keep a pair under a `secret.keep` grant. `js/tests/it/ecdsa.rs`
// (Hermes) and `js/web/tests/browser.rs` (both browser realms) check the
// signatures against Rust's P-256, never their bytes.
const P256 = { name: "ECDSA", namedCurve: "P-256" } as const;
const ES256 = { name: "ECDSA", hash: "SHA-256" } as const;
const hex = (buffer: ArrayBuffer) =>
  Array.from(new Uint8Array(buffer), b => b.toString(16).padStart(2, "0")).join("");
const bytes = (text: string) => new TextEncoder().encode(text);

// At module scope a key draws entropy outside an answer: refused (D3).
const initKey = crypto.subtle.generateKey(P256, false, ["sign"]).then(() => "RESOLVED", e => e.message);

async function rejection(call: () => Promise<unknown>): Promise<string> {
  try { await call(); return "RESOLVED"; }
  catch (e) { return e.name; }
}

// A fresh extractable pair: both JWKs and a signature over `proof`.
async function keypair(): Promise<string> {
  const pair = await crypto.subtle.generateKey(P256, true, ["sign", "verify"]) as CryptoKeyPair;
  const priv = await crypto.subtle.exportKey("jwk", pair.privateKey);
  const pub = await crypto.subtle.exportKey("jwk", pair.publicKey);
  const signature = hex(await crypto.subtle.sign(ES256, pair.privateKey, bytes("proof")));
  const shape = [pair.privateKey.type, pair.privateKey.extractable, pair.privateKey.usages.join(","),
    pair.publicKey.type, pair.publicKey.usages.join(","), (pair.privateKey.algorithm as any).namedCurve,
    Object.prototype.toString.call(pair.privateKey), pair.privateKey instanceof CryptoKey].join(" ");
  return JSON.stringify({ priv, pub, signature, shape });
}

// Another executor's private JWK, imported (non-extractable) and signing.
async function importSign(jwk: string): Promise<string> {
  const key = await crypto.subtle.importKey("jwk", JSON.parse(jwk), P256, false, ["sign"]);
  return hex(await crypto.subtle.sign(ES256, key, bytes("imported")));
}

// An import and an export alone: pure, no read.
async function roundTrip(jwk: string): Promise<string> {
  const key = await crypto.subtle.importKey("jwk", JSON.parse(jwk), P256, true, ["verify"]);
  const back = await crypto.subtle.exportKey("jwk", key);
  return [back.kty, back.crv, back.x, back.y, back.d === undefined].join(" ");
}

async function refusals(): Promise<string> {
  const subtle = crypto.subtle as any;
  const fixed = await subtle.generateKey(P256, false, ["sign", "verify"]);
  return [
    await rejection(() => subtle.generateKey({ name: "ECDSA", namedCurve: "P-384" }, true, ["sign"])),
    await rejection(() => subtle.generateKey({ name: "ECDH", namedCurve: "P-256" }, true, ["deriveBits"])),
    await rejection(() => subtle.generateKey(P256, true, ["encrypt"])),
    await rejection(() => subtle.exportKey("jwk", fixed.privateKey)),
    await rejection(() => subtle.exportKey("raw", fixed.publicKey)),
    await rejection(() => subtle.importKey("pkcs8", new Uint8Array(8), P256, true, ["sign"])),
    await rejection(() => subtle.sign(ES256, fixed.publicKey, bytes("x"))),
    await rejection(() => subtle.sign({ name: "ECDSA", hash: "SHA-384" }, fixed.privateKey, bytes("x"))),
    await rejection(() => subtle.verify(ES256, fixed.publicKey, new Uint8Array(64), bytes("x"))),
  ].join("/");
}

// Keep a non-extractable pair; read it back, sign with it, and name its
// public key, so a later incarnation's signature can be checked.
async function keep(): Promise<string> {
  const pair = await crypto.subtle.generateKey(P256, false, ["sign", "verify"]) as CryptoKeyPair;
  await (store as any).keepKey("dpop", pair);
  const back = await (store as any).key("dpop") as CryptoKeyPair;
  const pub = await crypto.subtle.exportKey("jwk", back.publicKey);
  const signature = hex(await crypto.subtle.sign(ES256, back.privateKey, bytes("kept")));
  return JSON.stringify({ pub, signature, extractable: back.privateKey.extractable });
}

// The kept pair, in a later incarnation: a signature under it, or "none".
async function kept(): Promise<string> {
  const back = await (store as any).key("dpop") as CryptoKeyPair | null;
  if (!back) return "none";
  return hex(await crypto.subtle.sign(ES256, back.privateKey, bytes("kept")));
}

let store: unknown;
function answer(source: string, args: any[], kept_: unknown): unknown {
  store = kept_;
  switch (source) {
    case "initKey": return initKey;
    case "keypair": return keypair();
    case "importSign": return importSign(args[0]);
    case "roundTrip": return roundTrip(args[0]);
    case "refusals": return refusals();
    case "keep": return keep();
    case "kept": return kept();
    default: throw new Error("unknown source " + source);
  }
}

(globalThis as any).exact = {
  abi: 1,
  appId: "test.ecdsa",
  grants: "secret.keep dpop\n",
  answer,
};
