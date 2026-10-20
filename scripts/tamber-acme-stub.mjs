// An ACME stand-in for the relay's tests (LLP 1121 §5 stage 10c R2): RFC
// 8555's one-order path, every JWS checked, DNS-01 read from `dns` (a Map
// of record → values, or a function of the record), certificates issued by
// a CA of its own (`caPem`, for a client to trust as WebPKI would a real
// one), every order counted. Test code only.
import { createHash, createPublicKey, generateKeyPairSync, randomBytes, sign, verify } from 'node:crypto';

const DAY = 86_400_000;
// DER for the stand-in's own certificates, and a reader for a request's.
const len = n => n < 0x80 ? Buffer.from([n]) : n < 0x100 ? Buffer.from([0x81, n]) : Buffer.from([0x82, n >> 8, n & 0xff]);
const tlv = (tag, ...parts) => { const body = Buffer.concat(parts); return Buffer.concat([Buffer.from([tag]), len(body.length), body]); };
const seq = (...p) => tlv(0x30, ...p);
const oid = dotted => { const [a, b, ...rest] = dotted.split('.').map(Number), out = [40 * a + b]; for (let v of rest) { const bytes = [v & 0x7f]; while ((v >>= 7)) bytes.unshift(0x80 | (v & 0x7f)); out.push(...bytes); } return tlv(0x06, Buffer.from(out)); };
const time = ms => tlv(0x17, Buffer.from(new Date(ms).toISOString().replace(/[-:T]|\.\d+/g, '').slice(2)));
const name = cn => seq(tlv(0x31, seq(oid('2.5.4.3'), tlv(0x0c, Buffer.from(cn)))));
const ECDSA = seq(oid('1.2.840.10045.4.3.2'));
/** One DER element at `at`: its tag, its body's start and its end. */
function read(der, at) {
  const tag = der[at];
  let n = der[at + 1], head = 2;
  if (n & 0x80) { const k = n & 0x7f; n = 0; for (let i = 0; i < k; i++) n = n * 256 + der[at + 2 + i]; head = 2 + k; }
  return { tag, start: at + head, end: at + head + n };
}
const pem = der => `-----BEGIN CERTIFICATE-----\n${der.toString('base64').match(/.{1,64}/g).join('\n')}\n-----END CERTIFICATE-----\n`;

/** The ACME stand-in: an issuing CA of its own, `dns` the TXT records its
 * DNS-01 check reads (`record → [values]`), every order counted. */
export function fakeAcme({ dns, now = () => Date.now(), days = 90, badNonceOnce = false } = {}) {
  const ca = generateKeyPairSync('ec', { namedCurve: 'P-256' });
  const nonces = new Set(), accounts = new Map(), orders = [], authzs = [];
  let refusedOnce = !badNonceOnce;
  const origin = 'https://acme.test';
  const nonce = () => { const n = randomBytes(12).toString('base64url'); nonces.add(n); return n; };
  const reply = (status, body, headers = {}) => new Response(typeof body === 'string' ? body : JSON.stringify(body), { status, headers: { 'replay-nonce': nonce(), ...headers } });
  const problem = (type, detail, status = 400) => reply(status, { type: `urn:ietf:params:acme:error:${type}`, detail });
  const issue = (spki, names) => {
    const tbs = seq(tlv(0xa0, tlv(0x02, Buffer.from([2]))), tlv(0x02, Buffer.from([0x40, ...randomBytes(15)])), ECDSA, name('acme stand-in CA'), seq(time(now() - 60_000), time(now() + days * DAY)), name(names[0]), spki,
      tlv(0xa3, seq(seq(oid('2.5.29.17'), tlv(0x04, seq(...names.map(n => tlv(0x82, Buffer.from(n)))))))));
    return pem(seq(tbs, ECDSA, tlv(0x03, Buffer.from([0]), sign('sha256', tbs, ca.privateKey))));
  };
  const fetch = async (url, init = {}) => {
    const u = new URL(url), path = u.pathname;
    if (path === '/dir') return reply(200, { newNonce: `${origin}/nonce`, newAccount: `${origin}/acct`, newOrder: `${origin}/order` });
    if (path === '/nonce') return reply(200, '');
    const jws = JSON.parse(init.body);
    const head = JSON.parse(Buffer.from(jws.protected, 'base64url'));
    if (!nonces.delete(head.nonce)) return problem('badNonce', 'no such nonce');
    if (!refusedOnce) { refusedOnce = true; return problem('badNonce', 'try another'); }
    if (head.url !== url) return problem('unauthorized', 'url');
    const jwk = head.jwk ?? accounts.get(head.kid);
    if (!jwk) return problem('accountDoesNotExist', 'no account');
    if (!verify('sha256', Buffer.from(`${jws.protected}.${jws.payload}`), { key: createPublicKey({ key: jwk, format: 'jwk' }), dsaEncoding: 'ieee-p1363' }, Buffer.from(jws.signature, 'base64url'))) return problem('unauthorized', 'bad signature');
    const payload = jws.payload ? JSON.parse(Buffer.from(jws.payload, 'base64url')) : null;
    if (path === '/acct') { const kid = `${origin}/acct/${accounts.size + 1}`; accounts.set(kid, jwk); return reply(201, { status: 'valid' }, { location: kid }); }
    const thumb = Buffer.from(createHash('sha256').update(JSON.stringify({ crv: jwk.crv, kty: jwk.kty, x: jwk.x, y: jwk.y })).digest()).toString('base64url');
    if (path === '/order') {
      const o = { names: payload.identifiers.map(i => i.value), status: 'pending', authz: [] };
      for (const value of o.names) { authzs.push({ value, status: 'pending', token: randomBytes(16).toString('base64url'), thumb }); o.authz.push(authzs.length - 1); }
      orders.push(o);
      return reply(201, { status: 'pending', authorizations: o.authz.map(i => `${origin}/authz/${i}`), finalize: `${origin}/finalize/${orders.length - 1}` }, { location: `${origin}/order/${orders.length - 1}` });
    }
    let m;
    if ((m = /^\/authz\/(\d+)$/.exec(path))) { const a = authzs[m[1]]; return reply(200, { status: a.status, identifier: { type: 'dns', value: a.value }, challenges: [{ type: 'dns-01', url: `${origin}/chal/${m[1]}`, token: a.token, ...(a.error ? { error: { detail: a.error } } : {}) }] }); }
    if ((m = /^\/chal\/(\d+)$/.exec(path))) {
      const a = authzs[m[1]], record = `_acme-challenge.${a.value.replace(/^\*\./, '')}`;
      const want = Buffer.from(createHash('sha256').update(`${a.token}.${a.thumb}`).digest()).toString('base64url');
      a.status = (typeof dns === 'function' ? dns(record) : dns.get(record) ?? []).includes(want) ? 'valid' : 'invalid';
      if (a.status === 'invalid') a.error = `no TXT record ${want} at ${record}`;
      return reply(200, { status: 'processing' });
    }
    if ((m = /^\/finalize\/(\d+)$/.exec(path))) {
      const o = orders[m[1]];
      if (!o.authz.every(i => authzs[i].status === 'valid')) return problem('orderNotReady', 'not authorized');
      // The request's own key, read from its DER, its signature checked.
      const der = Buffer.from(payload.csr, 'base64url'), outer = read(der, 0), info = read(der, outer.start);
      let at = info.start;
      for (let i = 0; i < 2; i++) at = read(der, at).end;
      const spkiEl = read(der, at), spki = der.subarray(at, spkiEl.end);
      const sig = read(der, read(der, info.end).end);
      if (!verify('sha256', der.subarray(outer.start, info.end), createPublicKey({ key: spki, format: 'der', type: 'spki' }), der.subarray(sig.start + 1, sig.end))) return problem('badCSR', 'bad signature');
      o.cert = issue(spki, o.names);
      o.status = 'valid';
      return reply(200, { status: 'processing' });
    }
    if ((m = /^\/order\/(\d+)$/.exec(path))) { const o = orders[m[1]]; return reply(200, { status: o.status, ...(o.cert ? { certificate: `${origin}/cert/${m[1]}` } : {}) }); }
    if ((m = /^\/cert\/(\d+)$/.exec(path))) return reply(200, orders[m[1]].cert, { 'content-type': 'application/pem-certificate-chain' });
    return problem('malformed', `no ${path}`, 404);
  };
  // Its own certificate, a CA's (basicConstraints CA:TRUE, keyCertSign), for a client to trust.
  const caTbs = seq(tlv(0xa0, tlv(0x02, Buffer.from([2]))), tlv(0x02, Buffer.from([0x40, ...randomBytes(15)])), ECDSA, name('acme stand-in CA'), seq(time(Date.now() - 86_400_000), time(Date.now() + 3650 * DAY)), name('acme stand-in CA'),
    ca.publicKey.export({ type: 'spki', format: 'der' }),
    tlv(0xa3, seq(seq(oid('2.5.29.19'), tlv(0x01, Buffer.from([0xff])), tlv(0x04, seq(tlv(0x01, Buffer.from([0xff]))))), seq(oid('2.5.29.15'), tlv(0x01, Buffer.from([0xff])), tlv(0x04, tlv(0x03, Buffer.from([0x01, 0x06])))))));
  const caPem = pem(seq(caTbs, ECDSA, tlv(0x03, Buffer.from([0]), sign('sha256', caTbs, ca.privateKey))));
  return { fetch, orders, directory: `${origin}/dir`, caPem };
}

