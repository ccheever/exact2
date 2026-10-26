// The auth fixture's passkey relying party (LLP 1133.011 §10), beside its
// OAuth server in `fixture.mjs`: registration and sign-in verified as a
// real server verifies them (WebAuthn Level 3 §7.1 and §7.2), with
// WebCrypto and a CBOR reader of the few types WebAuthn uses. Each
// challenge is single-use with an expiry. Registration asks for a
// discoverable ES256 credential with user verification, since conditional
// mediation offers only discoverable ones.
//
//   POST /passkey/register/options  {username, rpId}  → {publicKey}
//   POST /passkey/register          {response}        → {ok, userId, handle, credentialId} | {ok: false, error}
//   POST /passkey/sign-in/options   {rpId}            → {publicKey}
//   POST /passkey/sign-in           {response}        → {ok, userId, handle} | {ok: false, error}
//   POST /passkey/accepted          {userId}          → {ids}
//   POST /passkey/forget            {credentialId}    → {ok}   (a drive's: the server forgets a passkey)

const RP_IDS = ['auth-fixture.exact.test', 'localhost'];
// The origins `clientDataJSON` may carry (§7.1 step 9), per target (D6): a
// loopback development page for `localhost`; the app's origin for Apple,
// the value AuthenticationServices is expected to write, which stage 2
// records from a real ceremony; Android's per signing certificate, which
// `EXACT_ANDROID_ORIGINS` lists (`android:apk-key-hash:…`, comma-separated).
const origin = (rpId, value) => rpId === 'localhost'
  ? /^http:\/\/(localhost|127\.0\.0\.1)(:\d+)?$/.test(value)
  : value === `https://${rpId}` || (process.env.EXACT_ANDROID_ORIGINS ?? '').split(',').filter(Boolean).includes(value);
const TTL = 5 * 60_000;

const b64 = bytes => Buffer.from(bytes).toString('base64url');
const unb64 = text => new Uint8Array(Buffer.from(String(text ?? ''), 'base64url'));
const sha256 = async bytes => new Uint8Array(await crypto.subtle.digest('SHA-256', bytes));
const same = (a, b) => a.length === b.length && a.every((x, i) => x === b[i]);

/** One CBOR item at `at` (RFC 8949): unsigned and negative integers, byte and text strings, arrays, maps. */
export function cbor(bytes, at = 0) {
  const head = bytes[at], major = head >> 5, info = head & 31;
  let n, next = at + 1;
  if (info < 24) n = info;
  else if (info === 24) { n = bytes[next]; next += 1; }
  else if (info === 25) { n = (bytes[next] << 8) | bytes[next + 1]; next += 2; }
  else if (info === 26) { n = new DataView(bytes.buffer, bytes.byteOffset + next, 4).getUint32(0); next += 4; }
  else throw new Error(`CBOR: an item this reader does not take (${head})`);
  switch (major) {
    case 0: return [n, next];
    case 1: return [-1 - n, next];
    case 2: return [bytes.slice(next, next + n), next + n];
    case 3: return [new TextDecoder().decode(bytes.slice(next, next + n)), next + n];
    case 4: { const out = []; for (let i = 0; i < n; i++) { const [v, end] = cbor(bytes, next); out.push(v); next = end; } return [out, next]; }
    case 5: { const out = new Map(); for (let i = 0; i < n; i++) { const [k, a] = cbor(bytes, next); const [v, b] = cbor(bytes, a); out.set(k, v); next = b; } return [out, next]; }
    default: throw new Error(`CBOR: major type ${major}`);
  }
}

/** Authenticator data (§6.1): the RP ID hash, flags, sign count, and the attested credential where `AT` is set. */
export function authData(bytes) {
  const flags = bytes[32], out = { rpIdHash: bytes.slice(0, 32), up: !!(flags & 1), uv: !!(flags & 4), at: !!(flags & 64),
    signCount: new DataView(bytes.buffer, bytes.byteOffset + 33, 4).getUint32(0) };
  if (out.at) {
    const length = (bytes[53] << 8) | bytes[54];
    out.credentialId = bytes.slice(55, 55 + length);
    [out.publicKey] = cbor(bytes, 55 + length);
  }
  return out;
}

/** A DER ECDSA signature (§6.5.6) as WebCrypto's raw `r || s`. */
function raw(der) {
  let at = 2;
  const int = () => { if (der[at] !== 2) throw new Error('signature: not DER'); const n = der[at + 1]; const v = der.slice(at + 2, at + 2 + n); at += 2 + n; return v; };
  const fit = v => { const t = v[0] === 0 ? v.slice(1) : v; const out = new Uint8Array(32); out.set(t, 32 - t.length); return out; };
  const r = int(), s = int(), out = new Uint8Array(64);
  out.set(fit(r)); out.set(fit(s), 32);
  return out;
}

export function relyingParty(log) {
  const users = new Map(), handles = new Map(), credentials = new Map(), challenges = new Map();
  log.passkey = { registered: 0, assertions: 0, refused: [] };
  const no = error => { log.passkey.refused.push(error); return { ok: false, error }; };
  const challenge = (kind, rpId, extra = {}) => { const c = b64(crypto.getRandomValues(new Uint8Array(32))); challenges.set(c, { kind, rpId, expires: Date.now() + TTL, ...extra }); return c; };
  // `clientDataJSON` (§7.1 steps 5–12, §7.2 steps 10–16): its type, its challenge (spent here whatever follows) and its origin.
  const client = (response, kind) => {
    const data = JSON.parse(new TextDecoder().decode(unb64(response?.response?.clientDataJSON)));
    const issued = challenges.get(data.challenge);
    challenges.delete(data.challenge);
    if (data.type !== `webauthn.${kind}`) throw `type ${data.type}`;
    if (!issued || issued.kind !== kind || issued.expires < Date.now()) throw 'challenge';
    if (!origin(issued.rpId, data.origin)) throw `origin ${data.origin}`;
    return issued;
  };
  const routes = {
    async '/passkey/register/options'({ username, rpId }) {
      if (!RP_IDS.includes(rpId)) return no(`rpId ${rpId}`);
      const handle = String(username || 'fixture.test');
      const userId = handles.get(handle) ?? b64(crypto.getRandomValues(new Uint8Array(16)));
      const exclude = [...credentials].filter(([, c]) => c.userId === userId && c.rpId === rpId).map(([id]) => ({ type: 'public-key', id }));
      return { publicKey: {
        rp: { id: rpId, name: 'Auth Fixture' }, user: { id: userId, name: handle, displayName: handle },
        challenge: challenge('create', rpId, { userId, handle }), pubKeyCredParams: [{ type: 'public-key', alg: -7 }],
        authenticatorSelection: { residentKey: 'required', userVerification: 'required' }, attestation: 'none',
        excludeCredentials: exclude, timeout: 60_000,
      } };
    },
    async '/passkey/register'({ response }) {
      if (response?.type !== 'public-key') return no('type');
      let issued;
      try { issued = client(response, 'create'); } catch (why) { return no(String(why)); }
      const [attestation] = cbor(unb64(response.response.attestationObject));
      if (attestation.get('fmt') !== 'none' || attestation.get('attStmt')?.size !== 0) return no('attestation');
      const data = authData(attestation.get('authData'));
      if (!same(data.rpIdHash, await sha256(new TextEncoder().encode(issued.rpId)))) return no('rpIdHash');
      if (!data.up || !data.uv || !data.at) return no('flags');
      if (!same(data.credentialId, unb64(response.rawId)) || b64(data.credentialId) !== response.id) return no('credential id');
      const key = data.publicKey;
      if (key.get(1) !== 2 || key.get(3) !== -7 || key.get(-1) !== 1) return no('not an ES256 P-256 key');
      if (credentials.has(response.id)) return no('credential registered');
      const jwk = { kty: 'EC', crv: 'P-256', x: b64(key.get(-2)), y: b64(key.get(-3)) };
      await crypto.subtle.importKey('jwk', jwk, { name: 'ECDSA', namedCurve: 'P-256' }, false, ['verify']);
      credentials.set(response.id, { userId: issued.userId, rpId: issued.rpId, jwk, signCount: data.signCount });
      users.set(issued.userId, issued.handle);
      handles.set(issued.handle, issued.userId);
      log.passkey.registered++;
      return { ok: true, userId: issued.userId, handle: issued.handle, credentialId: response.id };
    },
    async '/passkey/sign-in/options'({ rpId }) {
      if (!RP_IDS.includes(rpId)) return no(`rpId ${rpId}`);
      return { publicKey: { challenge: challenge('get', rpId), rpId, userVerification: 'required', allowCredentials: [], timeout: 60_000 } };
    },
    async '/passkey/sign-in'({ response }) {
      if (response?.type !== 'public-key') return no('type');
      let issued;
      try { issued = client(response, 'get'); } catch (why) { return no(String(why)); }
      const stored = credentials.get(response.id);
      if (!stored || stored.rpId !== issued.rpId) return no('unknown-credential');
      if (b64(unb64(response.response.userHandle)) !== stored.userId) return no('userHandle');
      const authenticator = unb64(response.response.authenticatorData), data = authData(authenticator);
      if (!same(data.rpIdHash, await sha256(new TextEncoder().encode(issued.rpId)))) return no('rpIdHash');
      if (!data.up || !data.uv) return no('flags');
      const signed = new Uint8Array([...authenticator, ...await sha256(unb64(response.response.clientDataJSON))]);
      const key = await crypto.subtle.importKey('jwk', stored.jwk, { name: 'ECDSA', namedCurve: 'P-256' }, false, ['verify']);
      if (!await crypto.subtle.verify({ name: 'ECDSA', hash: 'SHA-256' }, key, raw(unb64(response.response.signature)), signed)) return no('signature');
      // §7.2 step 21: a counter that did not grow signals a cloned authenticator.
      if ((data.signCount || stored.signCount) && data.signCount <= stored.signCount) return no('signCount');
      stored.signCount = data.signCount;
      log.passkey.assertions++;
      return { ok: true, userId: stored.userId, handle: users.get(stored.userId) };
    },
    async '/passkey/accepted'({ userId }) {
      return { ids: [...credentials].filter(([, c]) => c.userId === userId).map(([id]) => id) };
    },
    async '/passkey/forget'({ credentialId }) {
      return { ok: credentials.delete(credentialId) };
    },
  };
  return {
    route: path => routes[path],
    reset() {
      users.clear(); handles.clear(); credentials.clear(); challenges.clear();
      log.passkey = { registered: 0, assertions: 0, refused: [] };
    },
  };
}
