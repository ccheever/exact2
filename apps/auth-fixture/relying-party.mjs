// The auth fixture's passkey relying party (LLP 1133.011 §10): registration
// and sign-in verified as a real server verifies them (WebAuthn Level 3 §7.1
// and §7.2), with WebCrypto and a CBOR reader of the few types WebAuthn uses.
// Registration asks for a discoverable ES256 credential with user
// verification, since conditional mediation offers only discoverable ones.
//
// It keeps nothing between requests, so the same file runs behind
// `fixture.mjs` and as the public origin's worker routes (`origin.mjs`),
// where requests land on isolates that share no memory. A challenge carries
// what the server needs back, under an HMAC: its kind, RP ID, expiry and, for
// a registration, the account. A registered passkey is a signed record (its
// id, account, RP ID, public key and sign count) that the app keeps and sends
// back at sign-in; the server forgets a passkey when the app drops its record.
// Spent challenges are remembered in memory, so a challenge is single-use
// within one process (all of `fixture.mjs`; one isolate of the worker).
//
//   POST /passkey/register/options  {username, rpId, records}  → {publicKey}
//   POST /passkey/register          {response}                 → {ok, userId, handle, credentialId, record} | {ok: false, error}
//   POST /passkey/sign-in/options   {rpId}                     → {publicKey}
//   POST /passkey/sign-in           {response, record}         → {ok, userId, handle, record} | {ok: false, error}
//   POST /passkey/accepted          {userId, records}          → {ids}
//
// `records` is every record the app holds; the server reads only those its
// key signed.

const TTL = 5 * 60_000;
const encoder = new TextEncoder(), decoder = new TextDecoder();

export const b64 = bytes => {
  let text = '';
  for (const byte of bytes) text += String.fromCharCode(byte);
  return btoa(text).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
};
export const unb64 = text => {
  const s = String(text ?? '').replace(/-/g, '+').replace(/_/g, '/');
  return Uint8Array.from(atob(s + '='.repeat((4 - s.length % 4) % 4)), c => c.charCodeAt(0));
};
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
    case 3: return [decoder.decode(bytes.slice(next, next + n)), next + n];
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

/** The origin Android writes into `clientDataJSON` for an app signed by the certificate with this SHA-256
 * fingerprint (`AB:CD:…`, as `app.json`'s `host.android.certificates` lists it). */
export const androidOrigin = fingerprint => `android:apk-key-hash:${b64(Uint8Array.from(fingerprint.split(':'), h => parseInt(h, 16)))}`;

/** The loopback page origins a `localhost` RP ID accepts. */
export const LOOPBACK = /^http:\/\/(localhost|127\.0\.0\.1)(:\d+)?$/;

/**
 * A relying party. `rps` maps each RP ID it serves to the origins `clientDataJSON` may carry for it (§7.1 step 9,
 * D6): strings, or a RegExp. `key` is the HMAC key's text (at least 32 characters); without one, each instance and
 * each `reset()` makes a random key, which makes every earlier challenge and record unknown. `log.passkey` counts
 * what it verified and lists what it refused.
 */
export function relyingParty(log, { rps, key } = {}) {
  if (key !== undefined && String(key).length < 32) throw new Error('relying party: the record key must be at least 32 characters');
  let hmac, spent;
  const start = () => {
    hmac = crypto.subtle.importKey('raw', key !== undefined ? encoder.encode(String(key)) : crypto.getRandomValues(new Uint8Array(32)),
      { name: 'HMAC', hash: 'SHA-256' }, false, ['sign', 'verify']);
    spent = new Map();
    log.passkey = { registered: 0, assertions: 0, refused: [] };
  };
  start();
  const mac = async (kind, bytes) => new Uint8Array(await crypto.subtle.sign('HMAC', await hmac, new Uint8Array([...encoder.encode(`${kind}\n`), ...bytes])));
  const verified = async (kind, bytes, tag) => crypto.subtle.verify('HMAC', await hmac, tag, new Uint8Array([...encoder.encode(`${kind}\n`), ...bytes]));
  const no = error => { log.passkey.refused.push(error); return { ok: false, error }; };
  const allowed = (rpId, origin) => [rps[rpId]].flat().some(o => (o instanceof RegExp ? o.test(origin) : o === origin));

  // A challenge is the HMAC (32 bytes) and then the JSON it covers.
  const challenge = async fields => {
    const body = encoder.encode(JSON.stringify({ ...fields, exp: Date.now() + TTL, n: b64(crypto.getRandomValues(new Uint8Array(16))) }));
    return b64([...await mac('challenge', body), ...body]);
  };
  const issued = async text => {
    const bytes = unb64(text), body = bytes.slice(32);
    if (bytes.length <= 32 || !await verified('challenge', body, bytes.slice(0, 32))) return null;
    return JSON.parse(decoder.decode(body));
  };
  // A record is base64url JSON, a dot, and its HMAC.
  const record = async fields => { const body = encoder.encode(JSON.stringify(fields)); return `${b64(body)}.${b64(await mac('record', body))}`; };
  const opened = async text => {
    const [body, tag] = String(text ?? '').split('.');
    try {
      const bytes = unb64(body);
      return tag && await verified('record', bytes, unb64(tag)) ? JSON.parse(decoder.decode(bytes)) : null;
    } catch { return null; }
  };
  const held = async records => (await Promise.all((Array.isArray(records) ? records : []).map(opened))).filter(Boolean);
  // The account's id: the same for a username under the same key.
  const userId = async handle => b64((await mac('user', encoder.encode(handle))).slice(0, 16));

  // `clientDataJSON` (§7.1 steps 5–12, §7.2 steps 10–16): its type, its challenge (a genuine one is spent here
  // whatever follows) and its origin.
  const client = async (response, kind) => {
    const data = JSON.parse(decoder.decode(unb64(response?.response?.clientDataJSON)));
    const fields = await issued(data.challenge).catch(() => null), now = Date.now();
    for (const [c, exp] of spent) if (exp < now) spent.delete(c);
    const fresh = fields && !spent.has(data.challenge);
    if (fields) spent.set(data.challenge, fields.exp);
    if (data.type !== `webauthn.${kind}`) throw `type ${data.type}`;
    if (!fresh || fields.k !== kind || fields.exp < now) throw 'challenge';
    if (!rps[fields.rp] || !allowed(fields.rp, data.origin)) throw `origin ${data.origin}`;
    return fields;
  };
  const routes = {
    async '/passkey/register/options'({ username, rpId, records }) {
      if (!rps[rpId]) return no(`rpId ${rpId}`);
      const handle = String(username || 'fixture.test'), user = await userId(handle);
      const exclude = (await held(records)).filter(r => r.userId === user && r.rpId === rpId).map(r => ({ type: 'public-key', id: r.id }));
      return { publicKey: {
        rp: { id: rpId, name: 'Auth Fixture' }, user: { id: user, name: handle, displayName: handle },
        challenge: await challenge({ k: 'create', rp: rpId, u: user, h: handle }), pubKeyCredParams: [{ type: 'public-key', alg: -7 }],
        authenticatorSelection: { residentKey: 'required', userVerification: 'required' }, attestation: 'none',
        excludeCredentials: exclude, timeout: 60_000,
      } };
    },
    async '/passkey/register'({ response }) {
      if (response?.type !== 'public-key') return no('type');
      let fields;
      try { fields = await client(response, 'create'); } catch (why) { return no(String(why)); }
      const [attestation] = cbor(unb64(response.response.attestationObject));
      if (attestation.get('fmt') !== 'none' || attestation.get('attStmt')?.size !== 0) return no('attestation');
      const data = authData(attestation.get('authData'));
      if (!same(data.rpIdHash, await sha256(encoder.encode(fields.rp)))) return no('rpIdHash');
      if (!data.up || !data.uv || !data.at) return no('flags');
      if (!same(data.credentialId, unb64(response.rawId)) || b64(data.credentialId) !== response.id) return no('credential id');
      const key = data.publicKey;
      if (key.get(1) !== 2 || key.get(3) !== -7 || key.get(-1) !== 1) return no('not an ES256 P-256 key');
      const jwk = { kty: 'EC', crv: 'P-256', x: b64(key.get(-2)), y: b64(key.get(-3)) };
      await crypto.subtle.importKey('jwk', jwk, { name: 'ECDSA', namedCurve: 'P-256' }, false, ['verify']);
      log.passkey.registered++;
      return { ok: true, userId: fields.u, handle: fields.h, credentialId: response.id,
        record: await record({ id: response.id, userId: fields.u, handle: fields.h, rpId: fields.rp, x: jwk.x, y: jwk.y, signCount: data.signCount }) };
    },
    async '/passkey/sign-in/options'({ rpId }) {
      if (!rps[rpId]) return no(`rpId ${rpId}`);
      return { publicKey: { challenge: await challenge({ k: 'get', rp: rpId }), rpId, userVerification: 'required', allowCredentials: [], timeout: 60_000 } };
    },
    async '/passkey/sign-in'({ response, record: kept }) {
      if (response?.type !== 'public-key') return no('type');
      let fields;
      try { fields = await client(response, 'get'); } catch (why) { return no(String(why)); }
      const stored = await opened(kept);
      if (!stored || stored.id !== response.id || stored.rpId !== fields.rp) return no('unknown-credential');
      if (b64(unb64(response.response.userHandle)) !== stored.userId) return no('userHandle');
      const authenticator = unb64(response.response.authenticatorData), data = authData(authenticator);
      if (!same(data.rpIdHash, await sha256(encoder.encode(fields.rp)))) return no('rpIdHash');
      if (!data.up || !data.uv) return no('flags');
      const signed = new Uint8Array([...authenticator, ...await sha256(unb64(response.response.clientDataJSON))]);
      const key = await crypto.subtle.importKey('jwk', { kty: 'EC', crv: 'P-256', x: stored.x, y: stored.y }, { name: 'ECDSA', namedCurve: 'P-256' }, false, ['verify']);
      if (!await crypto.subtle.verify({ name: 'ECDSA', hash: 'SHA-256' }, key, raw(unb64(response.response.signature)), signed)) return no('signature');
      // §7.2 step 21: a counter that did not grow signals a cloned authenticator. The app holds the record, so it
      // could send an older one; a fixture's server does not defend against its own app.
      if ((data.signCount || stored.signCount) && data.signCount <= stored.signCount) return no('signCount');
      log.passkey.assertions++;
      return { ok: true, userId: stored.userId, handle: stored.handle, record: await record({ ...stored, signCount: data.signCount }) };
    },
    async '/passkey/accepted'({ userId: user, records }) {
      return { ids: (await held(records)).filter(r => r.userId === user).map(r => r.id) };
    },
  };
  return { route: path => routes[path], reset: start };
}
