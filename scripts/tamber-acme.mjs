// A relay session's certificate by ACME (LLP 1121 §5 stage 10c R2; rt1's
// plan, ~/x2theme/rt1-s10c-plan.md, theme2's 20:45 approval): one wildcard
// a relay account (`*.<acct>.<relay>`), by DNS-01, its key made here and
// never sent anywhere, kept with the account's key under
// `~/.exact/relay/<acct>/` (0600), got again 30 days before it ends (Let's
// Encrypt's are 90: renewed at 60). The challenge's TXT record is
// published by whoever answers for the name: the relay, only under the
// connector's own account (R3's rule); or, with a domain of the
// developer's own, their DNS, by a command of theirs run here, so their
// credentials never pass through the relay (theme2's condition 2).
// RFC 8555, as much of it as one order needs.

import { createHash, createPrivateKey, createPublicKey, generateKeyPairSync, sign, X509Certificate } from 'node:crypto';
import { chmodSync, existsSync, mkdirSync, readFileSync, renameSync, statSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const DAY = 86_400_000;
/** A certificate is got again this long before it ends. */
export const RENEW_MS = 30 * DAY;

// DER, as much as a certificate request needs (X.690).
const len = n => n < 0x80 ? Buffer.from([n]) : n < 0x100 ? Buffer.from([0x81, n]) : Buffer.from([0x82, n >> 8, n & 0xff]);
const tlv = (tag, ...parts) => { const body = Buffer.concat(parts); return Buffer.concat([Buffer.from([tag]), len(body.length), body]); };
const seq = (...parts) => tlv(0x30, ...parts);
function oid(dotted) {
  const [a, b, ...rest] = dotted.split('.').map(Number), out = [40 * a + b];
  for (let v of rest) { const bytes = [v & 0x7f]; while ((v >>= 7)) bytes.unshift(0x80 | (v & 0x7f)); out.push(...bytes); }
  return tlv(0x06, Buffer.from(out));
}
const b64u = b => Buffer.from(b).toString('base64url');

/** A PKCS#10 request for `names` (DNS names) on `key` (a KeyObject),
 * signed ECDSA SHA-256, as DER. */
export function csr(names, key) {
  const spki = createPublicKey(key).export({ type: 'spki', format: 'der' });
  const subject = seq(tlv(0x31, seq(oid('2.5.4.3'), tlv(0x0c, Buffer.from(names[0])))));
  const san = seq(seq(oid('2.5.29.17'), tlv(0x04, seq(...names.map(n => tlv(0x82, Buffer.from(n)))))));
  const attributes = tlv(0xa0, seq(oid('1.2.840.113549.1.9.14'), tlv(0x31, san)));
  const info = seq(tlv(0x02, Buffer.from([0])), subject, spki, attributes);
  return seq(info, seq(oid('1.2.840.10045.4.3.2')), tlv(0x03, Buffer.from([0]), sign('sha256', info, key)));
}

/** The account key's JWK (P-256), and its RFC 7638 thumbprint. */
function jwkOf(key) {
  const { crv, kty, x, y } = createPublicKey(key).export({ format: 'jwk' });
  return { jwk: { crv, kty, x, y }, thumbprint: b64u(createHash('sha256').update(JSON.stringify({ crv, kty, x, y })).digest()) };
}

/** A certificate chain for `names` from the ACME server at `directory`:
 * `publish(record, value)` puts the DNS-01 TXT record up (and resolves
 * once it is up), `unpublish(record)` takes it down after. `accountKey` and
 * `key` are PEM (P-256); `key` is what the certificate certifies. Throws
 * with the server's words when an order fails. */
export async function acmeCertificate({ directory, names, accountKey, key, publish, unpublish = async () => {}, fetch = globalThis.fetch, wait = ms => new Promise(ok => setTimeout(ok, ms)), patience = 120_000 }) {
  const account = createPrivateKey(accountKey), { jwk, thumbprint } = jwkOf(account);
  const dir = await (await fetch(directory)).json();
  let nonce = (await fetch(dir.newNonce, { method: 'HEAD' })).headers.get('replay-nonce');
  let kid = null;
  /** A signed POST (`payload` null: POST-as-GET); one retry on a bad nonce. */
  const post = async (url, payload, tries = 2) => {
    const protect = b64u(JSON.stringify({ alg: 'ES256', nonce, url, ...(kid ? { kid } : { jwk }) }));
    const body = payload === null ? '' : b64u(JSON.stringify(payload));
    const signature = b64u(sign('sha256', Buffer.from(`${protect}.${body}`), { key: account, dsaEncoding: 'ieee-p1363' }));
    const res = await fetch(url, { method: 'POST', headers: { 'content-type': 'application/jose+json' }, body: JSON.stringify({ protected: protect, payload: body, signature }) });
    nonce = res.headers.get('replay-nonce') ?? nonce;
    if (!res.ok) {
      const problem = await res.json().catch(() => ({}));
      if (problem.type === 'urn:ietf:params:acme:error:badNonce' && tries > 1) return post(url, payload, tries - 1);
      throw new Error(`ACME ${new URL(url).pathname}: ${String(problem.detail ?? res.status).slice(0, 200)}`);
    }
    return res;
  };
  const created = await post(dir.newAccount, { termsOfServiceAgreed: true });
  kid = created.headers.get('location');
  const ordered = await post(dir.newOrder, { identifiers: names.map(value => ({ type: 'dns', value })) });
  const orderUrl = ordered.headers.get('location');
  let order = await ordered.json();
  const published = [];
  const until = async (url, done) => {
    const end = Date.now() + patience;
    for (let delay = 250; ; delay = Math.min(delay * 2, 5000)) {
      const got = await (await post(url, null)).json();
      if (done(got)) return got;
      if (got.status === 'invalid') throw new Error(`ACME: ${String(got.error?.detail ?? got.challenges?.find(c => c.error)?.error?.detail ?? 'invalid').slice(0, 200)}`);
      if (Date.now() > end) throw new Error(`ACME: not ${url.includes('authz') ? 'authorized' : 'issued'} within ${patience / 1000} s`);
      await wait(delay);
    }
  };
  try {
    for (const at of order.authorizations) {
      const authz = await (await post(at, null)).json();
      if (authz.status === 'valid') continue;
      const challenge = authz.challenges.find(c => c.type === 'dns-01');
      if (!challenge) throw new Error(`ACME: no dns-01 challenge for ${authz.identifier.value}`);
      // The record for `*.x` is `_acme-challenge.x`'s, the wildcard's base name.
      const record = `_acme-challenge.${authz.identifier.value.replace(/^\*\./, '')}`;
      const value = b64u(createHash('sha256').update(`${challenge.token}.${thumbprint}`).digest());
      await publish(record, value);
      published.push(record);
      await post(challenge.url, {});
      await until(at, a => a.status === 'valid');
    }
    const certKey = createPrivateKey(key);
    await post(order.finalize, { csr: b64u(csr(names, certKey)) });
    order = await until(orderUrl, o => o.status === 'valid');
    const chain = await (await post(order.certificate, null)).text();
    // (the certificate certifies this key, and the names asked for)
    const leaf = new X509Certificate(chain);
    if (!leaf.publicKey.equals(createPublicKey(certKey))) throw new Error('ACME: the certificate is not for this key');
    for (const n of names) if (!leaf.checkHost(n.replace(/^\*\./, 'x.'))) throw new Error(`ACME: the certificate does not cover ${n}`);
    return chain;
  } finally {
    for (const r of published) await unpublish(r).catch(() => {});
  }
}

/** A PEM P-256 key, kept at `path` (0600, written by a rename so a link
 * there is replaced, never followed); made when there is none. */
function keyAt(path) {
  if (existsSync(path)) {
    if (statSync(path).mode & 0o077) throw new Error(`${path} may be read by others: chmod 600 it`);
    return readFileSync(path, 'utf8');
  }
  const pem = generateKeyPairSync('ec', { namedCurve: 'P-256' }).privateKey.export({ type: 'pkcs8', format: 'pem' });
  keep(path, pem);
  return pem;
}
function keep(path, text) {
  const tmp = `${path}.${process.pid}.tmp`;
  writeFileSync(tmp, text, { mode: 0o600, flag: 'wx' });
  chmodSync(tmp, 0o600);
  renameSync(tmp, path);
}

/** The account's certificate for `names`, from `dir` (`~/.exact/relay/<acct>`,
 * made 0700): the one kept there while it covers them and has more than
 * `RENEW_MS` left, else a new one by ACME (`acme` gives the rest of
 * `acmeCertificate`'s options), kept. `{cert, key, fingerprint, spki, renewed}`. */
export async function relayCertificate(dir, names, acme, { now = Date.now(), get = acmeCertificate } = {}) {
  mkdirSync(dir, { recursive: true, mode: 0o700 });
  if (statSync(dir).mode & 0o077) throw new Error(`${dir} may be read by others: chmod 700 it`);
  const key = keyAt(join(dir, 'key.pem')), certPath = join(dir, 'cert.pem');
  const fits = pem => {
    try {
      const x = new X509Certificate(pem);
      return x.publicKey.equals(createPublicKey(key)) && names.every(n => x.checkHost(n.replace(/^\*\./, 'x.'))) && Date.parse(x.validTo) - now > RENEW_MS && Date.parse(x.validFrom) <= now;
    } catch { return false; }
  };
  let cert = existsSync(certPath) ? readFileSync(certPath, 'utf8') : null, renewed = false;
  if (!cert || !fits(cert)) {
    cert = await get({ ...acme, names, accountKey: keyAt(join(dir, 'account-key.pem')), key });
    keep(certPath, cert);
    renewed = true;
  }
  const x = new X509Certificate(cert);
  const spki = createHash('sha256').update(x.publicKey.export({ type: 'spki', format: 'der' })).digest('base64');
  return { cert, key, fingerprint: x.fingerprint256, spki, renewed };
}
