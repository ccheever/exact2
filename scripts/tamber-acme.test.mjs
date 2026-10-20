// A relay session's certificate by ACME (LLP 1121 §5 stage 10c R2), against
// an ACME stand-in run here: a wildcard by DNS-01, the TXT record put up by
// whoever answers for the name and taken down after, a failed challenge
// said in the server's words, the account's certificate kept (0600) and
// got again only 30 days before it ends.
import { afterEach, expect, test } from 'bun:test';
import { createHash, createPublicKey, generateKeyPairSync, X509Certificate } from 'node:crypto';
import { chmodSync, mkdtempSync, readFileSync, rmSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { acmeCertificate, csr, relayCertificate, RENEW_MS } from './tamber-acme.mjs';
import { fakeAcme } from './tamber-acme-stub.mjs';

const DAY = 86_400_000;
const closing = [];
afterEach(() => { for (const c of closing.splice(0)) c(); });

const keyPem = () => generateKeyPairSync('ec', { namedCurve: 'P-256' }).privateKey.export({ type: 'pkcs8', format: 'pem' });
const quick = { wait: async () => {} };

test('a wildcard by DNS-01: the record put up at the base name, taken down after; the certificate for this key and these names', async () => {
  const dns = new Map(), said = [];
  const acme = fakeAcme({ dns, badNonceOnce: true });
  const key = keyPem();
  const chain = await acmeCertificate({ ...quick, directory: acme.directory, fetch: acme.fetch, names: ['*.acct.relay.test'], accountKey: keyPem(), key,
    publish: async (record, value) => { said.push(['up', record]); dns.set(record, [value]); },
    unpublish: async record => { said.push(['down', record]); dns.delete(record); } });
  expect(said).toEqual([['up', '_acme-challenge.acct.relay.test'], ['down', '_acme-challenge.acct.relay.test']]);
  const leaf = new X509Certificate(chain);
  expect(leaf.checkHost('s1-tool.acct.relay.test')).toBe('*.acct.relay.test');
  expect(leaf.checkHost('s1-tool.other.relay.test')).toBeUndefined();
  expect(leaf.publicKey.equals(createPublicKey(key))).toBe(true);
});

test('a challenge the record does not answer fails in the server\'s words, the record still taken down', async () => {
  const dns = new Map(), down = [];
  const acme = fakeAcme({ dns });
  const got = acmeCertificate({ ...quick, directory: acme.directory, fetch: acme.fetch, names: ['*.acct.relay.test'], accountKey: keyPem(), key: keyPem(),
    publish: async record => { dns.set(record, ['not-the-value']); }, unpublish: async record => { down.push(record); } });
  await expect(got).rejects.toThrow(/^ACME: no TXT record \S+ at _acme-challenge\.acct\.relay\.test$/);
  expect(down).toEqual(['_acme-challenge.acct.relay.test']);
});

test('the account\'s certificate is kept, 0600 in a 0700 directory, and got again only 30 days before it ends', async () => {
  const home = mkdtempSync(join(tmpdir(), 'exact-acme-'));
  closing.push(() => rmSync(home, { recursive: true, force: true }));
  const dir = join(home, 'relay', 'acct'), dns = new Map();
  let now = Date.now();
  const acme = fakeAcme({ dns, now: () => now });
  const options = { ...quick, directory: acme.directory, fetch: acme.fetch, publish: async (r, v) => { dns.set(r, [v]); }, unpublish: async r => { dns.delete(r); } };
  const first = await relayCertificate(dir, ['*.acct.relay.test'], options, { now });
  expect([first.renewed, acme.orders.length]).toEqual([true, 1]);
  for (const f of ['key.pem', 'account-key.pem', 'cert.pem']) expect(statSync(join(dir, f)).mode & 0o777).toBe(0o600);
  expect(statSync(dir).mode & 0o777).toBe(0o700);
  expect(first.spki).toBe(createHash('sha256').update(createPublicKey(readFileSync(join(dir, 'key.pem'), 'utf8')).export({ type: 'spki', format: 'der' })).digest('base64'));
  // Again at 59 days: kept. At 61, with 29 left: got again, the same key.
  now += 59 * DAY;
  expect((await relayCertificate(dir, ['*.acct.relay.test'], options, { now })).renewed).toBe(false);
  now += 2 * DAY;
  const again = await relayCertificate(dir, ['*.acct.relay.test'], options, { now });
  expect([again.renewed, acme.orders.length, again.spki]).toEqual([true, 2, first.spki]);
  // Names it does not cover: got again.
  expect((await relayCertificate(dir, ['*.other.relay.test'], options, { now })).renewed).toBe(true);
  // A key others may read: refused, never used.
  chmodSync(join(dir, 'key.pem'), 0o644);
  await expect(relayCertificate(dir, ['*.acct.relay.test'], options, { now })).rejects.toThrow('chmod 600');
  expect(RENEW_MS).toBe(30 * DAY);
});

test('the request names every name and is signed by the certificate\'s key', () => {
  const { privateKey } = generateKeyPairSync('ec', { namedCurve: 'P-256' });
  const der = csr(['*.acct.relay.test', 'acct.relay.test'], privateKey);
  expect(der.includes(Buffer.from('*.acct.relay.test'))).toBe(true);
  expect(der.includes(Buffer.from('acct.relay.test'))).toBe(true);
});
