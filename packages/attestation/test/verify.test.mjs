// The fixture server's verifier (verify.mjs) against synthetic App Attest
// objects: a chain of certificates made here under a root of the test's own,
// built as Apple's are (the nonce extension, the key id, the authenticator
// data), so each of Apple's validation steps is exercised and each refusal
// seen; and Play's decode against a stubbed Google. A real token needs the
// App Attest capability on a phone (test/checklist.md).
//
//   bun test packages/attestation/test/verify.test.mjs
import { expect, test } from 'bun:test';
import { X509Certificate, generateKeyPairSync, sign } from 'node:crypto';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { APPLE_ROOT, nonceOf, said, verifier } from './verify.mjs';

const APP = 'TEAMID1234.com.exact.authfixture';
const sha256 = (bytes) => new Uint8Array(new Bun.CryptoHasher('sha256').update(bytes).digest());
const utf8 = (t) => new TextEncoder().encode(t);
const cat = (...parts) => new Uint8Array(parts.flatMap((p) => [...p]));

// DER (X.690), the few forms a certificate needs.
const len = (n) => n < 128 ? [n] : n < 256 ? [0x81, n] : [0x82, n >> 8, n & 255];
const tlv = (tag, body) => cat([tag, ...len(body.length)], body);
const seq = (...items) => tlv(0x30, cat(...items));
const oid = (dotted) => {
  const [a, b, ...rest] = dotted.split('.').map(Number);
  const out = [40 * a + b];
  for (const n of rest) { const bytes = [n & 127]; for (let v = n >> 7; v; v >>= 7) bytes.unshift((v & 127) | 128); out.push(...bytes); }
  return tlv(0x06, new Uint8Array(out));
};
const name = (cn) => seq(tlv(0x31, seq(oid('2.5.4.3'), tlv(0x0c, utf8(cn)))));
const time = (d) => tlv(0x17, utf8(d.toISOString().replace(/[-:T]/g, '').slice(2, 14) + 'Z'));
const ECDSA_SHA256 = seq(oid('1.2.840.10045.4.3.2'));

/** A certificate for `key`'s public half, named `cn`, signed by `issuer` (`{cn, key}`), with `extensions`. */
function certificate(cn, key, issuer, extensions = []) {
  const spki = new Uint8Array(key.publicKey.export({ type: 'spki', format: 'der' }));
  const now = Date.now();
  const tbs = seq(tlv(0xa0, tlv(0x02, new Uint8Array([2]))), tlv(0x02, new Uint8Array([1, ...crypto.getRandomValues(new Uint8Array(7))])),
    ECDSA_SHA256, name(issuer.cn), seq(time(new Date(now - 86400e3)), time(new Date(now + 86400e3))), name(cn), spki,
    ...(extensions.length ? [tlv(0xa3, seq(...extensions))] : []));
  const signature = new Uint8Array(sign('sha256', tbs, issuer.key.privateKey));
  return seq(tbs, ECDSA_SHA256, tlv(0x03, cat([0], signature)));
}
const ca = seq(oid('2.5.29.19'), tlv(0x01, new Uint8Array([255])), tlv(0x04, seq(tlv(0x01, new Uint8Array([255])))));
const nonceExtension = (nonce) => seq(oid('1.2.840.113635.100.8.2'), tlv(0x04, seq(tlv(0xa1, tlv(0x04, nonce)))));

// CBOR (RFC 8949), the types an attestation object holds.
function cbor(v) {
  const head = (major, n) => n < 24 ? [major << 5 | n] : n < 256 ? [major << 5 | 24, n] : n < 65536 ? [major << 5 | 25, n >> 8, n & 255] : [major << 5 | 26, n >>> 24, (n >> 16) & 255, (n >> 8) & 255, n & 255];
  if (typeof v === 'number') return new Uint8Array(v >= 0 ? head(0, v) : head(1, -1 - v));
  if (typeof v === 'string') { const b = utf8(v); return cat(head(3, b.length), b); }
  if (v instanceof Uint8Array) return cat(head(2, v.length), v);
  if (Array.isArray(v)) return cat(head(4, v.length), ...v.map(cbor));
  const entries = v instanceof Map ? [...v] : Object.entries(v);
  return cat(head(5, entries.length), ...entries.flatMap(([k, x]) => [cbor(k), cbor(x)]));
}

const keys = () => generateKeyPairSync('ec', { namedCurve: 'P-256' });
const root = { cn: 'Test App Attestation Root CA', key: keys() };
const rootDer = certificate(root.cn, root.key, root);
const intermediate = { cn: 'Test App Attestation CA 1', key: keys() };
const intermediateDer = certificate(intermediate.cn, intermediate.key, root, [ca]);
const pem = (der) => `-----BEGIN CERTIFICATE-----\n${Buffer.from(der).toString('base64')}\n-----END CERTIFICATE-----`;

/** A device's key and what `attestKey` and `generateAssertion` answer for it, as Apple builds them. */
function device({ appId = APP, aaguid = 'appattestdevelop' } = {}) {
  const key = keys();
  const { x, y } = key.publicKey.export({ format: 'jwk' });
  const pointBytes = cat([4], Buffer.from(x, 'base64url'), Buffer.from(y, 'base64url'));
  const id = sha256(pointBytes), keyId = Buffer.from(id).toString('base64');
  const cose = cbor(new Map([[1, 2], [3, -7], [-1, 1], [-2, Buffer.from(x, 'base64url')], [-3, Buffer.from(y, 'base64url')]].map(([k, v]) => [k, v instanceof Buffer ? new Uint8Array(v) : v])));
  const aaguidBytes = cat(utf8(aaguid), new Uint8Array(16 - aaguid.length));
  const counter = (n) => new Uint8Array([n >>> 24, (n >> 16) & 255, (n >> 8) & 255, n & 255]);
  return {
    keyId,
    attest(clientData) {
      const authData = cat(sha256(utf8(appId)), [0x40], counter(0), aaguidBytes, [0, 32], id, cose);
      const nonce = sha256(cat(authData, sha256(utf8(clientData))));
      const leaf = certificate(keyId, key, intermediate, [nonceExtension(nonce)]);
      const object = cbor({ fmt: 'apple-appattest', attStmt: { x5c: [leaf, intermediateDer], receipt: utf8('receipt') }, authData });
      return { format: 'apple-appattest', token: Buffer.from(object).toString('base64url'), keyId, clientData };
    },
    assert(clientData, n) {
      const authenticatorData = cat(sha256(utf8(appId)), [0], counter(n));
      const signature = new Uint8Array(sign('sha256', sha256(cat(authenticatorData, sha256(utf8(clientData)))), key.privateKey));
      return { format: 'apple-appattest-assertion', token: Buffer.from(cbor({ signature, authenticatorData })).toString('base64url'), keyId, clientData };
    },
  };
}

test("Apple's root is the one Apple publishes", () => {
  expect(new X509Certificate(APPLE_ROOT).fingerprint256).toBe('1C:B9:82:3B:A2:8B:A6:AD:2D:33:A0:06:94:1D:E2:AE:4F:51:3E:F1:D4:E8:31:B9:F7:E0:FA:7B:62:42:C9:32');
});

test('an attestation, then assertions whose counter grows', async () => {
  const v = verifier({ appId: APP, roots: [pem(rootDer)] }), d = device();
  const first = await v.verify(d.attest('challenge-1'));
  expect(first).toEqual({ ok: true, kind: 'attested', provider: 'App Attest', environment: 'development' });
  expect(said(first)).toBe('Attested: App Attest, development build');
  expect(nonceOf(new X509Certificate(pem(certificate('x', keys(), intermediate, [nonceExtension(new Uint8Array(32).fill(7))]))).raw)).toEqual(new Uint8Array(32).fill(7));
  const second = await v.verify(d.assert('request-2', 1));
  expect(said(second)).toBe('Asserted (counter 1): App Attest, development build');
  expect(await v.verify(d.assert('request-3', 1))).toEqual({ ok: false, error: 'counter' });
  const forged = { ...d.assert('request-4', 2), clientData: 'request-5' };
  expect((await v.verify(forged)).error).toBe('signature');
  expect((await v.verify(d.assert('request-6', 5))).counter).toBe(5);
  v.reset();
  expect(said(await v.verify(d.assert('request-7', 6)))).toBe('Unknown key');
});

test("each of Apple's steps refuses what it checks", async () => {
  const v = verifier({ appId: APP, roots: [pem(rootDer)] });
  expect((await verifier({ appId: APP }).verify(device().attest('c'))).error).toBe('root');
  expect((await v.verify({ ...device().attest('c'), clientData: 'other' })).error).toBe('nonce');
  expect((await v.verify({ ...device().attest('c'), keyId: device().keyId })).error).toBe('key id');
  expect((await v.verify(device({ appId: 'OTHERTEAM.com.exact.authfixture' }).attest('c'))).error).toBe('rpIdHash');
  expect((await v.verify(device({ aaguid: 'appattestnope' }).attest('c'))).error).toBe('aaguid appattestnope');
  expect((await v.verify(device({ aaguid: 'appattest' }).attest('c'))).environment).toBe('production');
  expect((await v.verify({ format: 'apple-appattest', token: 'AAAA', keyId: 'x', clientData: 'c' })).error).toMatch(/^malformed|^fmt/);
});

test('substitute tokens only under --substitute, with the same lifecycle', async () => {
  const token = (body) => Buffer.from(JSON.stringify(body)).toString('base64url');
  const hash = (c) => Buffer.from(sha256(utf8(c))).toString('base64url');
  const proof = (kind, counter, clientData = 'c1') => ({ format: 'exact-substitute', keyId: 'k1', clientData,
    token: token({ clientDataHash: hash(clientData), keyId: 'k1', kind, counter }) });
  expect((await verifier({ appId: APP }).verify(proof('attestation', 0))).error).toMatch(/--substitute/);
  const v = verifier({ appId: APP, substitute: true });
  expect(said(await v.verify(proof('assertion', 1)))).toBe('Unknown key');
  expect(said(await v.verify(proof('attestation', 0)))).toBe('Attested: the agent, development build');
  expect(said(await v.verify(proof('assertion', 1, 'c2')))).toBe('Asserted (counter 1): the agent');
  expect((await v.verify(proof('assertion', 1, 'c3'))).error).toBe('counter');
  expect((await v.verify({ ...proof('assertion', 2), clientData: 'other' })).error).toBe('nonce');
});

test("Play's token is decoded by Google with the service account, then its request hash checked", async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-play-')), file = join(dir, 'key.json');
  const { privateKey } = generateKeyPairSync('rsa', { modulusLength: 2048 });
  writeFileSync(file, JSON.stringify({ client_email: 'fixture@project.iam.gserviceaccount.com', token_uri: 'https://oauth2.example/token',
    private_key: privateKey.export({ type: 'pkcs8', format: 'pem' }) }));
  const asked = [];
  const payload = (hash) => ({ tokenPayloadExternal: { requestDetails: { requestPackageName: 'com.exact.authfixture', requestHash: hash },
    appIntegrity: { appRecognitionVerdict: 'UNRECOGNIZED_VERSION' }, deviceIntegrity: {}, accountDetails: { appLicensingVerdict: 'UNLICENSED' } } });
  const fetch = async (url, init) => {
    asked.push([url, init.body]);
    return { json: async () => url.endsWith('/token') ? { access_token: 'at' } : payload(Buffer.from(sha256(utf8('c1'))).toString('base64url')) };
  };
  const v = verifier({ appId: APP, packageName: 'com.exact.authfixture', playKey: file, fetch });
  const verdict = await v.verify({ format: 'play-integrity', token: 'opaque', clientData: 'c1' });
  expect(said(verdict)).toBe('Play: unrecognized version, no device integrity');
  expect(asked[0][1]).toMatch(/^grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Ajwt-bearer&assertion=[\w-]+\.[\w-]+\.[\w-]+$/);
  expect(asked[1]).toEqual(['https://playintegrity.googleapis.com/v1/com.exact.authfixture:decodeIntegrityToken', '{"integrity_token":"opaque"}']);
  expect((await v.verify({ format: 'play-integrity', token: 'opaque', clientData: 'c2' })).error).toBe('play: request hash');
  expect(said(v.playVerdict({ tokenPayloadExternal: { requestDetails: { requestPackageName: 'com.exact.authfixture', nonce: Buffer.from(sha256(utf8('c3'))).toString('base64url') },
    appIntegrity: { appRecognitionVerdict: 'PLAY_RECOGNIZED' }, deviceIntegrity: { deviceRecognitionVerdict: ['MEETS_DEVICE_INTEGRITY'] }, testingDetails: { isTestingResponse: true } } }, 'c3')))
    .toBe('Play: recognized, device integrity (a test response)');
  expect((await verifier({ appId: APP, playKey: '' }).verify({ format: 'play-integrity', token: 't', clientData: 'c' })).error).toMatch(/EXACT_PLAY_INTEGRITY_KEY/);
});
