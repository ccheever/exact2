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
import { chmodSync, lstatSync, mkdirSync, readFileSync, renameSync, unlinkSync, writeFileSync } from 'node:fs';
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

/** A DNS name an order may name: lower-case labels, a leading `*.` for a
 * wildcard, at most 253 characters (never a space, a newline or a quote). */
export const ACME_NAME = /^(?=.{3,253}$)(\*\.)?[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)+$/;
/** An answer's bytes, at most (a certificate chain is a few KiB). */
const BODY = 256 * 1024;

/** A certificate's DNS names, as its subjectAltName lists them. */
export const sansOf = x => (x.subjectAltName ?? '').split(', ').filter(n => n.startsWith('DNS:')).map(n => n.slice(4));
/** Whether `x` names every one of `names` itself (a wildcard as the
 * wildcard, never one host under it), and is valid at `now`. */
export const certifies = (x, names, now = Date.now()) => names.every(n => sansOf(x).includes(n)) && Date.parse(x.validFrom) <= now && Date.parse(x.validTo) > now;

/** A certificate chain for `names` from the ACME server at `directory`:
 * `publish(record, value)` puts the DNS-01 TXT record up (and resolves
 * once it is up), `unpublish(record, value)` takes that value down after.
 * `accountKey` and `key` are PEM (P-256); `key` is what the certificate
 * certifies. Every URL the server gives must be https on the directory's
 * own origin, every request has `ms` and every answer `BODY` bytes, and
 * each authorization must be for one of `names`, checked before its record
 * goes anywhere. Throws with the server's words when an order fails. */
export async function acmeCertificate({ directory, names, accountKey, key, publish, unpublish = async () => {}, fetch = globalThis.fetch, wait = ms => new Promise(ok => setTimeout(ok, ms)), patience = 120_000, ms = 30_000, warn = () => {}, account: told = () => {}, now = Date.now() }) {
  for (const n of names) if (!ACME_NAME.test(n)) throw new Error(`ACME: ${JSON.stringify(n).slice(0, 80)} is no name to order`);
  const origin = new URL(directory).origin;
  if (!directory.startsWith('https://')) throw new Error('ACME: the directory is not https');
  /** A URL the server gave: https on the directory's origin, or refused. */
  const ours = url => {
    let u;
    try { u = new URL(String(url)); } catch { throw new Error('ACME: the server gave no URL'); }
    if (u.origin !== origin) throw new Error(`ACME: the server named another origin (${u.origin.slice(0, 80)})`);
    return u.href;
  };
  /** A request with its deadline, never following a redirect; its body read to at most `BODY` bytes. */
  const call = async (url, init = {}) => {
    const res = await fetch(ours(url), { ...init, redirect: 'error', signal: AbortSignal.timeout(ms) });
    const reader = res.body?.getReader?.();
    let got = Buffer.alloc(0);
    if (reader) for (;;) { const { done, value } = await reader.read(); if (done) break; got = Buffer.concat([got, Buffer.from(value)]); if (got.length > BODY) { reader.cancel?.(); throw new Error('ACME: an answer over 256 KiB'); } }
    const text = got.toString('utf8');
    return { ok: res.ok, status: res.status, headers: res.headers, text, json: () => JSON.parse(text) };
  };
  const account = createPrivateKey(accountKey), { jwk, thumbprint } = jwkOf(account);
  const dir = (await call(directory)).json();
  let nonce = (await call(dir.newNonce, { method: 'HEAD' })).headers.get('replay-nonce');
  let kid = null;
  /** A signed POST (`payload` null: POST-as-GET); one retry on a bad nonce. */
  const post = async (url, payload, tries = 2) => {
    const protect = b64u(JSON.stringify({ alg: 'ES256', nonce, url: ours(url), ...(kid ? { kid } : { jwk }) }));
    const body = payload === null ? '' : b64u(JSON.stringify(payload));
    const signature = b64u(sign('sha256', Buffer.from(`${protect}.${body}`), { key: account, dsaEncoding: 'ieee-p1363' }));
    const res = await call(url, { method: 'POST', headers: { 'content-type': 'application/jose+json' }, body: JSON.stringify({ protected: protect, payload: body, signature }) });
    nonce = res.headers.get('replay-nonce') ?? nonce;
    if (!res.ok) {
      let problem = {};
      try { problem = res.json(); } catch { /* none */ }
      if (problem.type === 'urn:ietf:params:acme:error:badNonce' && tries > 1) return post(url, payload, tries - 1);
      throw new Error(`ACME ${new URL(url).pathname.slice(0, 60)}: ${String(problem.detail ?? res.status).replace(/[^\x20-\x7e]/g, '?').slice(0, 200)}`);
    }
    return res;
  };
  const created = await post(dir.newAccount, { termsOfServiceAgreed: true });
  kid = ours(created.headers.get('location'));
  told(kid);
  const ordered = await post(dir.newOrder, { identifiers: names.map(value => ({ type: 'dns', value })) });
  const orderUrl = ours(ordered.headers.get('location'));
  let order = ordered.json();
  const published = [];
  const said = text => String(text ?? 'invalid').replace(/[^\x20-\x7e]/g, '?').slice(0, 200);
  const until = async (url, done) => {
    const end = Date.now() + patience;
    for (let delay = 250; ; delay = Math.min(delay * 2, 5000)) {
      const got = (await post(url, null)).json();
      if (done(got)) return got;
      if (got.status === 'invalid') throw new Error(`ACME: ${said(got.error?.detail ?? got.challenges?.find(c => c.error)?.error?.detail)}`);
      if (Date.now() > end) throw new Error(`ACME: not ${url.includes('authz') ? 'authorized' : 'issued'} within ${patience / 1000} s`);
      await wait(delay);
    }
  };
  try {
    for (const at of order.authorizations ?? []) {
      const authz = (await post(at, null)).json();
      // For one of the names asked for, and only that: before its record goes anywhere.
      const asked = (authz.wildcard ? `*.${authz.identifier?.value}` : authz.identifier?.value);
      const value = names.find(n => n === asked || n === authz.identifier?.value);
      if (authz.identifier?.type !== 'dns' || !value) throw new Error(`ACME: an authorization for a name not asked for (${said(authz.identifier?.value).slice(0, 80)})`);
      if (authz.status === 'valid') continue;
      const challenge = (authz.challenges ?? []).find(c => c.type === 'dns-01');
      if (!challenge || !/^[A-Za-z0-9_-]{16,256}$/.test(String(challenge.token))) throw new Error(`ACME: no dns-01 challenge for ${value}`);
      // The record for `*.x` is `_acme-challenge.x`'s, the wildcard's base name.
      const record = `_acme-challenge.${value.replace(/^\*\./, '')}`;
      const txt = b64u(createHash('sha256').update(`${challenge.token}.${thumbprint}`).digest());
      await publish(record, txt);
      published.push([record, txt]);
      await post(challenge.url, {});
      await until(at, a => a.status === 'valid');
    }
    const certKey = createPrivateKey(key);
    await post(order.finalize, { csr: b64u(csr(names, certKey)) });
    order = await until(orderUrl, o => o.status === 'valid');
    const chain = (await post(order.certificate, null)).text;
    // (this key, these names as themselves, valid now)
    const leaf = new X509Certificate(chain);
    if (!leaf.publicKey.equals(createPublicKey(certKey))) throw new Error('ACME: the certificate is not for this key');
    if (!certifies(leaf, names, now)) throw new Error(`ACME: the certificate does not name ${names.join(', ')} itself, or is not valid now`);
    return chain;
  } finally {
    for (const [r, v] of published) await unpublish(r, v).catch(e => warn(`tamber: the ACME record ${r} could not be taken down: ${String(e?.message ?? e).slice(0, 200)}`));
  }
}

/** A path that must be a regular file if it is anything (never a link),
 * and its contents, or null when there is none. */
function regular(path) {
  let st;
  try { st = lstatSync(path); } catch (e) { if (e.code === 'ENOENT') return null; throw e; }
  if (!st.isFile()) throw new Error(`${path} is not a regular file (a link?): remove it`);
  if (st.mode & 0o077) throw new Error(`${path} may be read by others: chmod 600 it`);
  return readFileSync(path, 'utf8');
}

/** A PEM P-256 key, kept at `path` (0600, written by a rename so a link
 * there is replaced, never followed); made when there is none. */
function keyAt(path) {
  const had = regular(path);
  if (had !== null) return had;
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

/** One process at a time gets an account's certificate: a lock file made
 * with O_EXCL, its holder's pid in it, taken over when that process is
 * gone or the lock is older than `stale`. */
async function locked(dir, f, { stale = 600_000, wait = ms => new Promise(ok => setTimeout(ok, ms)) } = {}) {
  const lock = join(dir, 'lock');
  for (let tries = 0; ; tries++) {
    try { writeFileSync(lock, String(process.pid), { flag: 'wx', mode: 0o600 }); break; } catch (e) {
      if (e.code !== 'EEXIST') throw e;
      let holder = NaN, age = 0;
      try { holder = Number(readFileSync(lock, 'utf8')); age = Date.now() - lstatSync(lock).mtimeMs; } catch { continue; }
      let alive = true;
      try { process.kill(holder, 0); } catch { alive = false; }
      if (!alive || age > stale) { try { unlinkSync(lock); } catch { /* raced */ } continue; }
      if (tries > 600) throw new Error(`another process holds ${lock}`);
      await wait(500);
    }
  }
  try { return await f(); } finally { try { unlinkSync(lock); } catch { /* gone */ } }
}

/** The account's certificate for `names`, from `dir` (`~/.exact/relay/<acct>`,
 * made 0700): the one kept there while it covers them and has more than
 * `RENEW_MS` left, else a new one by ACME (`acme` gives the rest of
 * `acmeCertificate`'s options), kept. `{cert, key, fingerprint, spki, renewed}`. */
export async function relayCertificate(dir, names, acme, { now = Date.now(), get = acmeCertificate, wait } = {}) {
  mkdirSync(dir, { recursive: true, mode: 0o700 });
  const st = lstatSync(dir);
  if (!st.isDirectory()) throw new Error(`${dir} is not a directory (a link?): remove it`);
  if (st.mode & 0o077) throw new Error(`${dir} may be read by others: chmod 700 it`);
  return locked(dir, async () => {
    const key = keyAt(join(dir, 'key.pem')), certPath = join(dir, 'cert.pem');
    // Kept while it is this key's, names them as themselves, and has more than RENEW_MS left.
    const fits = pem => {
      try {
        const x = new X509Certificate(pem);
        return x.publicKey.equals(createPublicKey(key)) && certifies(x, names, now) && Date.parse(x.validTo) - now > RENEW_MS;
      } catch { return false; }
    };
    let cert = regular(certPath), renewed = false;
    if (!cert || !fits(cert)) {
      cert = await get({ ...acme, names, accountKey: keyAt(join(dir, 'account-key.pem')), key, now });
      keep(certPath, cert);
      renewed = true;
    }
    const x = new X509Certificate(cert);
    const spki = createHash('sha256').update(x.publicKey.export({ type: 'spki', format: 'der' })).digest('base64');
    return { cert, key, fingerprint: x.fingerprint256, spki, renewed };
  }, wait ? { wait } : {});
}
