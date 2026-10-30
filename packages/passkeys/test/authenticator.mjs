// A software authenticator for drives and tests (LLP 1133.011 §9), outside
// the runner: given a `credential` ticket's `args`, it makes the response a
// platform authenticator would, which the drive types into the ticket.
// Its credentials are ES256 keys kept in a file the drive names; each is
// discoverable (resident) and every ceremony is user-present and
// user-verified, with `none` attestation.
//
//   bun packages/passkeys/test/authenticator.mjs --keys <file> [--origin <o>] '<ticket args JSON>'
//
// prints the `RegistrationResponseJSON` or `AuthenticationResponseJSON`;
// exits 2 with `excluded` when `create`'s `excludeCredentials` names a
// credential it holds (the drive taps `excluded`), and with `none` when
// `get` finds no credential for the RP ID (the drive taps `none`).
// `--origin` stands for the ticket's `clientOrigin` where the target's
// platform writes one the substitute cannot know (Android's).
import { existsSync, readFileSync, writeFileSync } from 'node:fs';

const b64 = bytes => Buffer.from(bytes).toString('base64url');
const unb64 = text => new Uint8Array(Buffer.from(String(text), 'base64url'));
const sha256 = async bytes => new Uint8Array(await crypto.subtle.digest('SHA-256', bytes));
const text = s => new TextEncoder().encode(s);

/** CBOR (RFC 8949) of integers, byte strings, text and maps: what attestation and COSE keys use. */
function cbor(v) {
  const head = (major, n) => n < 24 ? [major << 5 | n] : n < 256 ? [major << 5 | 24, n] : [major << 5 | 25, n >> 8, n & 255];
  if (typeof v === 'number') return new Uint8Array(v >= 0 ? head(0, v) : head(1, -1 - v));
  if (v instanceof Uint8Array) return new Uint8Array([...head(2, v.length), ...v]);
  if (typeof v === 'string') { const t = text(v); return new Uint8Array([...head(3, t.length), ...t]); }
  if (v instanceof Map) return new Uint8Array([...head(5, v.size), ...[...v].flatMap(([k, x]) => [...cbor(k), ...cbor(x)])]);
  throw new Error(`cbor: ${typeof v}`);
}

/** WebCrypto's raw `r || s` as the DER ECDSA signature WebAuthn carries (§6.5.6). */
function der(raw) {
  const int = b => { let i = 0; while (i < b.length - 1 && b[i] === 0) i++; const t = b.slice(i); return t[0] & 128 ? [0, ...t] : [...t]; };
  const r = int(raw.slice(0, 32)), s = int(raw.slice(32));
  return new Uint8Array([0x30, r.length + s.length + 4, 2, r.length, ...r, 2, s.length, ...s]);
}

const counter = n => [n >>> 24 & 255, n >>> 16 & 255, n >>> 8 & 255, n & 255];
const UP = 1, UV = 4, AT = 64;

/** `args`'s response, with the credentials in `keys` (a path): `{response}`, or `{refuse: "excluded"|"none"}`. */
export async function respond(args, keys, origin = args.clientOrigin) {
  if (!origin) throw new Error('the ticket names no clientOrigin: pass --origin');
  const held = existsSync(keys) ? JSON.parse(readFileSync(keys, 'utf8')) : { credentials: [] };
  const save = () => writeFileSync(keys, JSON.stringify(held, null, 2));
  const options = args.options ?? {};
  const rpId = args.rpId ?? options.rp?.id ?? options.rpId;
  const clientData = type => text(JSON.stringify({ type, challenge: options.challenge, origin, crossOrigin: false }));
  if (args.ceremony === 'create') {
    const excluded = new Set((options.excludeCredentials ?? []).map(c => c.id));
    if (held.credentials.some(c => c.rpId === rpId && excluded.has(c.id))) return { refuse: 'excluded' };
    const pair = await crypto.subtle.generateKey({ name: 'ECDSA', namedCurve: 'P-256' }, true, ['sign', 'verify']);
    const jwk = await crypto.subtle.exportKey('jwk', pair.privateKey);
    const id = crypto.getRandomValues(new Uint8Array(16));
    const cose = cbor(new Map([[1, 2], [3, -7], [-1, 1], [-2, unb64(jwk.x)], [-3, unb64(jwk.y)]]));
    const authData = new Uint8Array([...await sha256(text(rpId)), UP | UV | AT, ...counter(0), ...new Uint8Array(16), id.length >> 8, id.length & 255, ...id, ...cose]);
    const attestationObject = cbor(new Map([['fmt', 'none'], ['attStmt', new Map()], ['authData', authData]]));
    const spki = new Uint8Array(await crypto.subtle.exportKey('spki', pair.publicKey));
    held.credentials.push({ id: b64(id), rpId, userHandle: options.user?.id, name: options.user?.name, jwk, signCount: 0 });
    save();
    return { response: {
      id: b64(id), rawId: b64(id), type: 'public-key', authenticatorAttachment: 'platform',
      clientExtensionResults: { credProps: { rk: true } },
      response: { clientDataJSON: b64(clientData('webauthn.create')), attestationObject: b64(attestationObject), transports: ['internal', 'hybrid'],
        publicKeyAlgorithm: -7, publicKey: b64(spki), authenticatorData: b64(authData) },
    } };
  }
  const allowed = new Set((options.allowCredentials ?? []).map(c => c.id));
  const credential = held.credentials.filter(c => c.rpId === rpId && (!allowed.size || allowed.has(c.id))).at(-1);
  if (!credential) return { refuse: 'none' };
  credential.signCount += 1;
  save();
  const authData = new Uint8Array([...await sha256(text(rpId)), UP | UV, ...counter(credential.signCount)]);
  const data = clientData('webauthn.get');
  const key = await crypto.subtle.importKey('jwk', credential.jwk, { name: 'ECDSA', namedCurve: 'P-256' }, false, ['sign']);
  const signature = new Uint8Array(await crypto.subtle.sign({ name: 'ECDSA', hash: 'SHA-256' }, key, new Uint8Array([...authData, ...await sha256(data)])));
  return { response: {
    id: credential.id, rawId: credential.id, type: 'public-key', authenticatorAttachment: 'platform', clientExtensionResults: {},
    response: { clientDataJSON: b64(data), authenticatorData: b64(authData), signature: b64(der(signature)), userHandle: credential.userHandle },
  } };
}

if (import.meta.main) {
  const argv = process.argv.slice(2), flag = name => argv.includes(name) ? argv[argv.indexOf(name) + 1] : undefined;
  const keys = flag('--keys'), origin = flag('--origin'), json = argv.filter((a, i) => !a.startsWith('--') && !argv[i - 1]?.startsWith('--')).at(-1);
  if (!keys || !json) { console.error("usage: authenticator.mjs --keys <file> [--origin <origin>] '<ticket args JSON>'"); process.exit(64); }
  const args = JSON.parse(json);
  const out = await respond(args, keys, origin ?? args.clientOrigin);
  if (out.refuse) { console.log(out.refuse); process.exit(2); }
  console.log(JSON.stringify(out.response));
}
