// The auth fixture server's verifier of @exact/attestation's tokens (LLP
// 1133.028 §8.1): what an app's own server does with them, for the fixture,
// not a library for apps (D2). It keeps each attested key and its counter.
//
// - `apple-appattest`: Apple's validation steps, offline
//   (https://developer.apple.com/documentation/devicecheck/validating-apps-that-connect-to-your-server):
//   the certificate chain to the App Attestation root; the nonce,
//   SHA-256(authData ‖ SHA-256(clientData)), in the credential certificate's
//   extension 1.2.840.113635.100.8.2; the key id as the SHA-256 of the
//   certificate's public key; the RP ID hash of `<team>.<bundle id>`; a
//   counter of 0; the `aaguid` of either environment; the credential id.
// - `apple-appattest-assertion`: the stored key's signature over the nonce,
//   the RP ID hash, and a counter greater than the last.
// - `play-integrity`: Google's `decodeIntegrityToken`, with the service
//   account whose key file `EXACT_PLAY_INTEGRITY_KEY` names, then the
//   request hash. The verdicts are Google's to give.
// - `exact-substitute`: the agent path's token (§7), only when the server
//   was started with `--substitute`.
import { X509Certificate, createSign, verify as verifySignature } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { authData, cbor } from '../../../apps/auth-fixture/relying-party.mjs';

/** Apple App Attestation Root CA (https://www.apple.com/certificateauthority/Apple_App_Attestation_Root_CA.pem). */
export const APPLE_ROOT = `-----BEGIN CERTIFICATE-----
MIICITCCAaegAwIBAgIQC/O+DvHN0uD7jG5yH2IXmDAKBggqhkjOPQQDAzBSMSYw
JAYDVQQDDB1BcHBsZSBBcHAgQXR0ZXN0YXRpb24gUm9vdCBDQTETMBEGA1UECgwK
QXBwbGUgSW5jLjETMBEGA1UECAwKQ2FsaWZvcm5pYTAeFw0yMDAzMTgxODMyNTNa
Fw00NTAzMTUwMDAwMDBaMFIxJjAkBgNVBAMMHUFwcGxlIEFwcCBBdHRlc3RhdGlv
biBSb290IENBMRMwEQYDVQQKDApBcHBsZSBJbmMuMRMwEQYDVQQIDApDYWxpZm9y
bmlhMHYwEAYHKoZIzj0CAQYFK4EEACIDYgAERTHhmLW07ATaFQIEVwTtT4dyctdh
NbJhFs/Ii2FdCgAHGbpphY3+d8qjuDngIN3WVhQUBHAoMeQ/cLiP1sOUtgjqK9au
Yen1mMEvRq9Sk3Jm5X8U62H+xTD3FE9TgS41o0IwQDAPBgNVHRMBAf8EBTADAQH/
MB0GA1UdDgQWBBSskRBTM72+aEH/pwyp5frq5eWKoTAOBgNVHQ8BAf8EBAMCAQYw
CgYIKoZIzj0EAwMDaAAwZQIwQgFGnByvsiVbpTKwSga0kP0e8EeDS4+sQmTvb7vn
53O5+FRXgeLhpJ06ysC5PrOyAjEAp5U4xDgEgllF7En3VcE3iexZZtKeYnpqtijV
oyFraWVIyd/dganmrduC1bmTBGwD
-----END CERTIFICATE-----`;

const NONCE_OID = Buffer.from('06092a864886f763640802', 'hex');
// Apple's pages disagree on the sandbox's (D2), so both spellings of each are taken.
const AAGUIDS = { appattestdevelop: 'development', appattest: 'production' };

const sha256 = (bytes) => new Uint8Array(new Bun.CryptoHasher('sha256').update(bytes).digest());
const b64url = (bytes) => Buffer.from(bytes).toString('base64url');
// Node's base64 reads both alphabets: Apple's key ids are base64, tokens base64url.
const unb64 = (text) => new Uint8Array(Buffer.from(String(text ?? ''), 'base64'));
const same = (a, b) => a.length === b.length && a.every((x, i) => x === b[i]);
const utf8 = (text) => new TextEncoder().encode(text);

/** The DER item at `at`: its tag, where its contents start, and where it ends. */
function der(bytes, at) {
  const tag = bytes[at];
  let length = bytes[at + 1], start = at + 2;
  if (length & 0x80) {
    const n = length & 0x7f;
    length = 0;
    for (let i = 0; i < n; i++) length = length * 256 + bytes[start + i];
    start += n;
  }
  return { tag, start, end: start + length };
}

/** The 32 bytes App Attest's nonce extension carries: `SEQUENCE { [1] { OCTET STRING nonce } }`. */
export function nonceOf(certDer) {
  const bytes = new Uint8Array(certDer), at = Buffer.from(bytes).indexOf(NONCE_OID);
  if (at < 0) return null;
  let item = der(bytes, at + NONCE_OID.length);
  if (item.tag === 0x01) item = der(bytes, item.end); // `critical`
  if (item.tag !== 0x04) return null;
  const seq = der(bytes, item.start), tagged = der(bytes, seq.start), octets = der(bytes, tagged.start);
  return octets.tag === 0x04 ? bytes.slice(octets.start, octets.end) : null;
}

/** The uncompressed EC point of a certificate's P-256 key. */
function point(cert) {
  const { x, y } = cert.publicKey.export({ format: 'jwk' });
  return new Uint8Array([4, ...Buffer.from(x, 'base64url'), ...Buffer.from(y, 'base64url')]);
}

/** A verifier for one app: `appId` is `<team>.<bundle id>`; `roots` the trusted roots' PEM (Apple's by default). */
export function verifier({ appId, packageName, substitute = false, roots = [APPLE_ROOT], now = () => new Date(), fetch = globalThis.fetch,
  playKey = process.env.EXACT_PLAY_INTEGRITY_KEY } = {}) {
  const keys = new Map(), log = { attested: 0, asserted: 0, refused: [] };
  const no = (error) => { log.refused.push(error); return { ok: false, error }; };
  const trusted = roots.map((pem) => new X509Certificate(pem));

  function chain(x5c) {
    const [leaf, ...rest] = x5c.map((c) => new X509Certificate(Buffer.from(c)));
    const at = now();
    for (const c of [leaf, ...rest]) if (new Date(c.validFrom) > at || new Date(c.validTo) < at) throw 'certificate dates';
    let child = leaf;
    for (const parent of rest) {
      if (!child.checkIssued(parent) || !child.verify(parent.publicKey)) throw 'chain';
      child = parent;
    }
    if (!trusted.some((root) => child.fingerprint256 === root.fingerprint256 || (child.checkIssued(root) && child.verify(root.publicKey)))) throw 'root';
    return leaf;
  }

  function attestation({ token, keyId, clientData }) {
    const [object] = cbor(unb64(token));
    if (object.get('fmt') !== 'apple-appattest') return no('fmt');
    const statement = object.get('attStmt'), raw = object.get('authData');
    let leaf;
    try { leaf = chain(statement.get('x5c')); } catch (why) { return no(String(why)); }
    const nonce = sha256(new Uint8Array([...raw, ...sha256(utf8(clientData))]));
    const carried = nonceOf(leaf.raw);
    if (!carried || !same(carried, nonce)) return no('nonce');
    const id = unb64(keyId), key = point(leaf);
    if (!same(sha256(key), id)) return no('key id');
    const data = authData(raw);
    if (!same(data.rpIdHash, sha256(utf8(appId)))) return no('rpIdHash');
    if (data.signCount !== 0) return no('counter');
    const aaguid = new TextDecoder().decode(raw.slice(37, 53)).replace(/\0+$/, '');
    if (!AAGUIDS[aaguid]) return no(`aaguid ${aaguid}`);
    if (!same(data.credentialId, id)) return no('credential id');
    keys.set(b64url(id), { kind: 'apple', key: leaf.publicKey, counter: 0, environment: AAGUIDS[aaguid], receipt: statement.get('receipt') });
    log.attested++;
    return { ok: true, kind: 'attested', provider: 'App Attest', environment: AAGUIDS[aaguid] };
  }

  function assertion({ token, keyId, clientData }) {
    const stored = keys.get(b64url(unb64(keyId)));
    if (!stored || stored.kind !== 'apple') return no('unknown-key');
    const [object] = cbor(unb64(token));
    const raw = object.get('authenticatorData'), signature = object.get('signature');
    const nonce = sha256(new Uint8Array([...raw, ...sha256(utf8(clientData))]));
    if (!verifySignature('sha256', nonce, stored.key, Buffer.from(signature))) return no('signature');
    const data = authData(raw);
    if (!same(data.rpIdHash, sha256(utf8(appId)))) return no('rpIdHash');
    if (data.signCount <= stored.counter) return no('counter');
    stored.counter = data.signCount;
    log.asserted++;
    return { ok: true, kind: 'asserted', provider: 'App Attest', environment: stored.environment, counter: data.signCount };
  }

  function substituted({ token, keyId, clientData }) {
    if (!substitute) return no('substitute tokens are refused: the server was not started with --substitute');
    const body = JSON.parse(Buffer.from(token, 'base64url').toString('utf8'));
    if (body.clientDataHash !== b64url(sha256(utf8(clientData)))) return no('nonce');
    if (body.keyId !== keyId) return no('key id');
    const stored = keys.get(keyId);
    if (body.kind === 'attestation') {
      if (body.counter !== 0) return no('counter');
      keys.set(keyId, { kind: 'substitute', counter: 0 });
      log.attested++;
      return { ok: true, kind: 'attested', provider: 'the agent', environment: 'development' };
    }
    if (!stored || stored.kind !== 'substitute') return no('unknown-key');
    if (body.counter <= stored.counter) return no('counter');
    stored.counter = body.counter;
    log.asserted++;
    return { ok: true, kind: 'asserted', provider: 'the agent', counter: body.counter };
  }

  // A service account's access token (RFC 7523's JWT bearer grant), then
  // Google's decode; its verdicts as they come.
  async function play({ token, clientData }) {
    if (!playKey) return no('play-integrity needs EXACT_PLAY_INTEGRITY_KEY, a service account key file');
    const account = JSON.parse(readFileSync(playKey, 'utf8'));
    const iat = Math.floor(now().getTime() / 1000);
    const part = (v) => Buffer.from(JSON.stringify(v)).toString('base64url');
    const input = `${part({ alg: 'RS256', typ: 'JWT' })}.${part({ iss: account.client_email, scope: 'https://www.googleapis.com/auth/playintegrity',
      aud: account.token_uri, iat, exp: iat + 600 })}`;
    const jwt = `${input}.${createSign('RSA-SHA256').update(input).sign(account.private_key).toString('base64url')}`;
    const grant = await (await fetch(account.token_uri, { method: 'POST', headers: { 'content-type': 'application/x-www-form-urlencoded' },
      body: `grant_type=${encodeURIComponent('urn:ietf:params:oauth:grant-type:jwt-bearer')}&assertion=${jwt}` })).json();
    if (!grant.access_token) return no(`play: no access token (${grant.error ?? 'refused'})`);
    const decoded = await (await fetch(`https://playintegrity.googleapis.com/v1/${packageName}:decodeIntegrityToken`, {
      method: 'POST', headers: { authorization: `Bearer ${grant.access_token}`, 'content-type': 'application/json' },
      body: JSON.stringify({ integrity_token: token }) })).json();
    return playVerdict(decoded, clientData);
  }

  /** What a decoded Play token says, once its request hash is this call's (standard) or its nonce (classic). */
  function playVerdict(decoded, clientData) {
    const payload = decoded?.tokenPayloadExternal;
    if (!payload) return no(`play: ${decoded?.error?.message ?? 'no payload'}`);
    const expected = b64url(sha256(utf8(clientData))), details = payload.requestDetails ?? {};
    if (details.requestHash !== expected && details.nonce !== expected) return no('play: request hash');
    if (packageName && details.requestPackageName !== packageName) return no('play: package');
    log.attested++;
    return { ok: true, kind: 'play', provider: 'Play Integrity', app: payload.appIntegrity?.appRecognitionVerdict ?? 'UNEVALUATED',
      device: payload.deviceIntegrity?.deviceRecognitionVerdict ?? [], licensing: payload.accountDetails?.appLicensingVerdict ?? 'UNEVALUATED',
      testing: payload.testingDetails?.isTestingResponse === true };
  }

  return {
    log,
    playVerdict,
    /** `{format, token, keyId, clientData}` as the app posts it. */
    async verify(proof) {
      try {
        switch (proof?.format) {
          case 'apple-appattest': return attestation(proof);
          case 'apple-appattest-assertion': return assertion(proof);
          case 'exact-substitute': return substituted(proof);
          case 'play-integrity': return await play(proof);
          default: return no(`format ${proof?.format}`);
        }
      } catch (error) {
        return no(`malformed: ${error?.message ?? error}`);
      }
    },
    /** The server forgets every key (a drive's: its database lost them). */
    reset() { keys.clear(); Object.assign(log, { attested: 0, asserted: 0, refused: [] }); },
  };
}

/** The line the fixture's row shows for a verdict (§8.1): what the server concluded, never the token. */
export function said(verdict) {
  if (!verdict.ok) return verdict.error === 'unknown-key' ? 'Unknown key' : `Refused: ${verdict.error}`;
  if (verdict.kind === 'play') {
    const app = { PLAY_RECOGNIZED: 'recognized', UNRECOGNIZED_VERSION: 'unrecognized version', UNEVALUATED: 'unevaluated' }[verdict.app] ?? verdict.app;
    const device = verdict.device.includes('MEETS_DEVICE_INTEGRITY') ? 'device integrity' : 'no device integrity';
    return `Play: ${app}, ${device}${verdict.testing ? ' (a test response)' : ''}`;
  }
  const how = verdict.kind === 'attested' ? 'Attested' : `Asserted (counter ${verdict.counter})`;
  return `${how}: ${verdict.provider}${verdict.environment ? `, ${verdict.environment} build` : ''}`;
}
