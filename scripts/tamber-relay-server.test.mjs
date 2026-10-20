// The hosted relay (LLP 1121 §5 stage 10c R3) on loopback: the ClientHello's
// name read and nothing past it, the control endpoint on the relay's only
// certificate, sessions by run, accounts as hashes (a label never given out
// again, revocation cutting every stream), TXT records for the connector's
// own account only, the limits; and end to end with the real connector, an
// ACME certificate through the relay's TXT records, and two accounts.
import { afterEach, expect, test } from 'bun:test';
import { createHash } from 'node:crypto';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { connect as connectTCP, createServer as createTCP } from 'node:net';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { connect as connectTLS } from 'node:tls';
import { helloName, loadAccounts, relayServer } from './tamber-relay-server.mjs';
import { relayTxt } from './tamber-relay.mjs';
import { openRemote, selfSignedCertificate } from './tamber-remote.mjs';
import { fakeAcme } from './tamber-acme-stub.mjs';

const DOMAIN = 'relay.test';
const relayCert = selfSignedCertificate([DOMAIN]);
const TOKENS = { acct: 'acct-token-0123456789abcdef', other: 'other-token-0123456789abcdef' };
const hash = t => createHash('sha256').update(t).digest('hex');
const closing = [];
afterEach(async () => { for (const c of closing.splice(0).reverse()) await c(); });
const until = async (f, ms = 5000) => { const t = Date.now(); while (!(await f())) { if (Date.now() - t > ms) throw new Error('timed out'); await new Promise(ok => setTimeout(ok, 20)); } };

/** A first record as a client sends it, to `servername` (or none). */
function helloTo(servername) {
  return new Promise(ok => {
    const server = createTCP(s => { s.once('data', d => { ok(d); s.destroy(); server.close(); }); });
    server.listen(0, '127.0.0.1', () => {
      const c = connectTLS({ host: '127.0.0.1', port: server.address().port, ...(servername ? { servername } : {}), rejectUnauthorized: false });
      c.on('error', () => {});
    });
  });
}

/** A relay on loopback with accounts `acct` and `other` (and `revoked`), an in-process TXT store. */
async function startRelay({ accounts = null, limits = {}, now } = {}) {
  const dir = mkdtempSync(join(tmpdir(), 'exact-relay-server-'));
  closing.push(() => rmSync(dir, { recursive: true, force: true }));
  const path = join(dir, 'accounts.json');
  writeFileSync(path, JSON.stringify(accounts ?? { accounts: { acct: { token_sha256: hash(TOKENS.acct) }, other: { token_sha256: hash(TOKENS.other), names: ['tamber.example.com'] } }, revoked: ['gone'] }));
  const txt = new Map(), logs = [];
  const dns = {
    publish: async (r, v) => { txt.set(r, [...(txt.get(r) ?? []), v]); },
    clear: async (r, v) => { const left = (txt.get(r) ?? []).filter(x => x !== v); if (left.length) txt.set(r, left); else txt.delete(r); },
  };
  const relay = relayServer({ domain: DOMAIN, listen: '127.0.0.1:0', tls: relayCert, accounts: path, dns, limits: { graceMs: 200, ...limits } }, { log: l => logs.push(l), ...(now ? { now } : {}) });
  const port = await relay.listen();
  closing.push(() => relay.close());
  return { relay, port, txt, logs, dir, path };
}
/** `connect` aimed at the relay, trusting its certificate as WebPKI would a real one. */
const toRelay = port => opts => connectTLS({ ...opts, host: '127.0.0.1', port, ca: relayCert.cert });
/** A connector's greeting by hand, its first line back and the connection. */
function greet(port, lines, { servername = DOMAIN } = {}) {
  return new Promise(ok => {
    const c = connectTLS({ host: '127.0.0.1', port, servername, ca: relayCert.cert }, () => c.write(`${lines.join('\r\n')}\r\n\r\n`));
    let buf = Buffer.alloc(0), done = false;
    const finish = line => { if (!done) { done = true; ok({ line, conn: c, rest: () => buf }); } };
    c.on('data', d => { buf = Buffer.concat([buf, d]); const nl = buf.indexOf(10); if (nl >= 0 && !done) { const line = buf.subarray(0, nl).toString(); buf = buf.subarray(nl + 1); finish(line); } });
    c.on('close', () => finish(null));
    c.on('error', () => finish(null));
    closing.push(() => c.destroy());
  });
}
const RUN = 'a'.repeat(32);
const hello = (label, token, extra = []) => [`EXACT-RELAY/1 ${label}`, `authorization: Bearer ${token}`, ...extra];
/** Whether a raw connection to the relay with `first` bytes is closed with nothing said. */
function closedSilently(port, first) {
  return new Promise(ok => {
    const s = connectTCP({ host: '127.0.0.1', port }, () => s.write(first));
    let got = 0;
    s.on('data', d => { got += d.length; });
    s.on('close', () => ok(got === 0));
    s.on('error', () => {});
  });
}

test('the ClientHello\'s name, and nothing past its first record; anything else is no hello', async () => {
  const named = await helloTo('s1-tool.acct.relay.test');
  const h = helloName(Buffer.concat([named, Buffer.from('application bytes after it')]));
  expect(h).toMatchObject({ state: 'ok', sni: 's1-tool.acct.relay.test' });
  // (what it read: the first record, its header and body, never the bytes after)
  expect(h.read).toBe(5 + named.readUInt16BE(3));
  expect(helloName(named.subarray(0, 40))).toEqual({ state: 'more' });
  expect(helloName(await helloTo(null))).toMatchObject({ state: 'ok', sni: null });
  // Not a handshake record; a record past 16 KiB; a ClientHello longer than its record (split over records).
  expect(helloName(Buffer.from('GET / HTTP/1.1\r\n\r\n'))).toEqual({ state: 'bad' });
  expect(helloName(Buffer.from([0x16, 0x03, 0x01, 0x40, 0x01]))).toEqual({ state: 'bad' });
  const split = Buffer.from(named);
  split.writeUInt16BE(100, 3);
  expect(helloName(split.subarray(0, 105))).toEqual({ state: 'bad' });
  // A field running past its bound, at every cut (a truncated hello whose record says it is whole).
  for (let cut = 6; cut < named.length; cut += 7) {
    const short = Buffer.from(named.subarray(0, cut));
    short.writeUInt16BE(cut - 5, 3);
    expect(['bad', 'ok']).toContain(helloName(short).state);
  }
});

test('the accounts: a label never given out again, nor one account\'s names another\'s', () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-relay-accounts-'));
  try {
    const path = join(dir, 'accounts.json');
    const write = file => writeFileSync(path, JSON.stringify(file));
    const t = hash('x');
    write({ accounts: { a: { token_sha256: t }, b: { token_sha256: t, names: ['tamber.example.com'] } }, revoked: ['old'] });
    expect([...loadAccounts(path, DOMAIN).accounts.keys()]).toEqual(['a', 'b']);
    expect(readFileSync(join(dir, 'labels.seen'), 'utf8')).toBe('a\nb\nold\n');
    // Revoked and an account again; dropped from both; or a seen label reissued after a revoke's removal: refused.
    write({ accounts: { a: { token_sha256: t }, b: { token_sha256: t }, old: { token_sha256: t } }, revoked: ['old'] });
    expect(() => loadAccounts(path, DOMAIN)).toThrow('old is revoked: a label is never given out again');
    write({ accounts: { a: { token_sha256: t } }, revoked: ['old'] });
    expect(() => loadAccounts(path, DOMAIN)).toThrow('b was an account and is neither one nor revoked');
    // One account's names another's, or under or over them; or under the relay's own domain: refused.
    write({ accounts: { a: { token_sha256: t, names: ['example.com'] }, b: { token_sha256: t, names: ['tamber.example.com'] } }, revoked: ['old'] });
    expect(() => loadAccounts(path, DOMAIN)).toThrow('one account\'s names may not be, or be under or over, another\'s');
    write({ accounts: { a: { token_sha256: t }, b: { token_sha256: t, names: ['x.relay.test'] } }, revoked: ['old'] });
    expect(() => loadAccounts(path, DOMAIN)).toThrow('under the relay\'s own domain');
    write({ accounts: { a: { token_sha256: 'nope' }, b: { token_sha256: t } }, revoked: ['old'] });
    expect(() => loadAccounts(path, DOMAIN)).toThrow('token_sha256 is not 64 hex digits');
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('the relay starts only on its own certificate: one naming its domain, valid now, its key\'s', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-relay-cert-'));
  closing.push(() => rmSync(dir, { recursive: true, force: true }));
  const accounts = join(dir, 'accounts.json');
  writeFileSync(accounts, JSON.stringify({ accounts: {}, revoked: [] }));
  const other = selfSignedCertificate(['elsewhere.test']);
  expect(() => relayServer({ domain: DOMAIN, tls: other, accounts })).toThrow(`tls.cert does not name ${DOMAIN}`);
  expect(() => relayServer({ domain: DOMAIN, tls: relayCert, accounts }, { now: () => Date.now() + 30 * 86_400_000 })).toThrow('tls.cert is not valid now');
  expect(() => relayServer({ domain: DOMAIN, tls: { cert: relayCert.cert, key: selfSignedCertificate([DOMAIN]).key }, accounts })).toThrow('tls.key is not tls.cert\'s key');
});

test('greetings: a wrong token or label refused alike; a run\'s connections one session; names of the account\'s own parents only', async () => {
  const { port } = await startRelay();
  expect((await greet(port, hello('acct', 'wrong', [`run: ${RUN}`]))).line).toBe('REFUSED no such account');
  expect((await greet(port, hello('nobody', TOKENS.acct, [`run: ${RUN}`]))).line).toBe('REFUSED no such account');
  expect((await greet(port, hello('gone', TOKENS.acct, [`run: ${RUN}`]))).line).toBe('REFUSED no such account');
  const a = await greet(port, hello('acct', TOKENS.acct, [`run: ${RUN}`]));
  const b = await greet(port, hello('acct', TOKENS.acct, [`run: ${RUN}`]));
  expect(a.line).toMatch(/^READY [a-f0-9]{12}-tool\.acct\.relay\.test [a-f0-9]{12}-canvas\.acct\.relay\.test$/);
  expect(b.line).toBe(a.line);
  // Another run: another session.
  expect((await greet(port, hello('acct', TOKENS.acct, [`run: ${'b'.repeat(32)}`]))).line).not.toBe(a.line);
  // Names of its own parent only, never another account's.
  expect((await greet(port, hello('other', TOKENS.other, [`run: ${RUN}`, 'names: tool.tamber.example.com canvas.tamber.example.com']))).line).toBe('READY tool.tamber.example.com canvas.tamber.example.com');
  expect((await greet(port, hello('acct', TOKENS.acct, [`run: ${'c'.repeat(32)}`, 'names: tool.tamber.example.com canvas.tamber.example.com']))).line).toBe('REFUSED names not of your account');
  // The relay's own name is the only one it ends TLS on: a session's name's handshake goes down to the connector,
  // the relay answering none of it (it holds no key to).
  const down = new Promise(ok => { for (const c of [a, b]) c.conn.once('data', d => ok(d.toString('latin1'))); });
  const tried = await new Promise(ok => { const c = connectTLS({ host: '127.0.0.1', port, servername: a.line.split(' ')[1], ca: relayCert.cert }, () => ok('handshake')); c.on('error', () => ok('refused')); setTimeout(() => { c.destroy(); ok('unanswered'); }, 500); });
  expect(tried).toBe('unanswered');
  expect((await down).startsWith(`CLIENT ${a.line.split(' ')[1]} 127.0.0.1\n\x16\x03`)).toBe(true);
});

test('a client\'s bytes go down as they came (hello and all after it), after one CLIENT line; an unknown name, no name or no hello is closed, nothing said', async () => {
  const { port } = await startRelay();
  const c = await greet(port, hello('acct', TOKENS.acct, [`run: ${RUN}`]));
  const tool = c.line.split(' ')[1];
  const first = await helloTo(tool), after = Buffer.from('bytes after the hello, never read by the relay');
  const got = new Promise(ok => { let buf = c.rest(); c.conn.on('data', d => { buf = Buffer.concat([buf, d]); if (buf.length >= first.length + after.length + 30) ok(buf); }); });
  const client = connectTCP({ host: '127.0.0.1', port }, () => { client.write(first); setTimeout(() => client.write(after), 50); });
  closing.push(() => client.destroy());
  const down = await got;
  const nl = down.indexOf(10);
  expect(down.subarray(0, nl).toString()).toBe(`CLIENT ${tool} 127.0.0.1`);
  expect(down.subarray(nl + 1).equals(Buffer.concat([first, after]))).toBe(true);
  // Both ways: what the connector writes reaches the client.
  const back = new Promise(ok => client.once('data', d => ok(d.toString())));
  c.conn.write('from the server');
  expect(await back).toBe('from the server');
  expect(await closedSilently(port, await helloTo('s9-tool.acct.relay.test'))).toBe(true);
  expect(await closedSilently(port, await helloTo(null))).toBe(true);
  expect(await closedSilently(port, Buffer.from('GET / HTTP/1.1\r\nHost: relay.test\r\n\r\n'))).toBe(true);
});

test('TXT records: only the connector\'s own account\'s, ACME-shaped, at most 10 a minute', async () => {
  const { port, txt } = await startRelay({ limits: { txtPerAccount: 3 } });
  const as = label => ({ host: DOMAIN, port, account: label, token: TOKENS[label] });
  const via = { connect: toRelay(port) };
  await relayTxt(as('acct'), '_acme-challenge.acct.relay.test', 'A'.repeat(43), via);
  expect(txt.get('_acme-challenge.acct.relay.test')).toEqual(['A'.repeat(43)]);
  // Another account's record, the apex's, or a name of the account's own parent: refused.
  await expect(relayTxt(as('other'), '_acme-challenge.acct.relay.test', 'B'.repeat(43), via)).rejects.toThrow("not your account's name");
  await expect(relayTxt(as('acct'), '_acme-challenge.relay.test', 'B'.repeat(43), via)).rejects.toThrow("not your account's name");
  await expect(relayTxt(as('other'), '_acme-challenge.tamber.example.com', 'B'.repeat(43), via)).rejects.toThrow("not your account's name");
  // A clear takes down its own value only.
  await relayTxt(as('acct'), '_acme-challenge.acct.relay.test', 'C'.repeat(43), via);
  await expect(relayTxt(as('acct'), '_acme-challenge.acct.relay.test', 'A'.repeat(43), { ...via, clear: true })).rejects.toThrow('more than 3 TXT requests a minute');
  expect(txt.get('_acme-challenge.acct.relay.test')).toEqual(['A'.repeat(43), 'C'.repeat(43)]);
  // (another account's own allowance is its own)
  expect(txt.size).toBe(1);
});

test('limits: clients a minute from one address to one session; refused greetings then silence', async () => {
  const { port } = await startRelay({ limits: { perIpPerSession: 2, greetingsPerIp: 3 } });
  const c = await greet(port, hello('acct', TOKENS.acct, [`run: ${RUN}`]));
  await greet(port, hello('acct', TOKENS.acct, [`run: ${RUN}`]));
  await greet(port, hello('acct', TOKENS.acct, [`run: ${RUN}`]));
  const tool = c.line.split(' ')[1], first = await helloTo(tool);
  const opened = () => new Promise(ok => { const s = connectTCP({ host: '127.0.0.1', port }, () => s.write(first)); let lived = false; const t = setTimeout(() => { lived = true; s.destroy(); }, 300); s.on('close', () => { clearTimeout(t); ok(lived); }); s.on('error', () => {}); });
  expect([await opened(), await opened(), await opened()]).toEqual([true, true, false]);
  for (let i = 0; i < 3; i++) expect((await greet(port, hello('acct', 'wrong', [`run: ${RUN}`]))).line).toBe('REFUSED no such account');
  // (past the limit, answered nothing: closed)
  expect((await greet(port, hello('acct', TOKENS.acct, [`run: ${'d'.repeat(32)}`]))).line).toBeNull();
});

test('revoked: every connection and stream of the account cut at once; the label refused for good', async () => {
  const { relay, port, path, logs } = await startRelay();
  const idle = await greet(port, hello('acct', TOKENS.acct, [`run: ${RUN}`]));
  const carrying = await greet(port, hello('acct', TOKENS.acct, [`run: ${RUN}`]));
  const otherConn = await greet(port, hello('other', TOKENS.other, [`run: ${RUN}`]));
  const first = await helloTo(idle.line.split(' ')[1]);
  const client = connectTCP({ host: '127.0.0.1', port }, () => client.write(first));
  const clientClosed = new Promise(ok => client.on('close', ok));
  client.on('error', () => {});
  await new Promise(ok => setTimeout(ok, 100));
  const file = JSON.parse(readFileSync(path, 'utf8'));
  delete file.accounts.acct;
  file.revoked.push('acct');
  writeFileSync(path, JSON.stringify(file));
  expect(relay.reload()).toBe(true);
  await clientClosed;
  await until(() => idle.conn.destroyed && carrying.conn.destroyed);
  expect(otherConn.conn.destroyed).toBe(false);
  expect(logs.some(l => l === 'relay: acct: revoked: every connection cut')).toBe(true);
  expect((await greet(port, hello('acct', TOKENS.acct, [`run: ${RUN}`]))).line).toBe('REFUSED no such account');
  // Given out again: the file is not taken, the accounts before it kept.
  file.accounts.acct = { token_sha256: hash('new owner') };
  writeFileSync(path, JSON.stringify(file));
  expect(relay.reload()).toBe(false);
  expect(relay.revoked.has('acct')).toBe(true);
});

test('end to end: two accounts\' design servers dial in, each wildcard got by ACME through the relay\'s own TXT records, each client served by its own; the relay sees no token or design in clear', async () => {
  const { port, txt } = await startRelay();
  const acme = fakeAcme({ dns: txt });
  // The client's side of the relay, captured: what a packet capture there would hold.
  const seen = [];
  const tap = createTCP(c => { const r = connectTCP({ host: '127.0.0.1', port }); c.on('data', d => { seen.push(d); r.write(d); }); r.on('data', d => { seen.push(d); c.write(d); }); c.on('close', () => r.destroy()); r.on('close', () => c.destroy()); c.on('error', () => {}); r.on('error', () => {}); });
  await new Promise(ok => tap.listen(0, '127.0.0.1', ok));
  closing.push(() => tap.close());
  const home = mkdtempSync(join(tmpdir(), 'exact-relay-e2e-'));
  closing.push(() => rmSync(home, { recursive: true, force: true }));
  const serve = async label => {
    const got = [];
    const w = { got, handle: async (req, res, url) => { let body = ''; for await (const c of req) body += c; got.push({ token: req.headers['x-exact-tamber-token'], body }); res.end('{"ok":true,"design":"brand.ink = #123456"}'); return true; } };
    const args = { host: DOMAIN, port, account: label, token: TOKENS[label], names: null, acme: acme.directory, hook: null, dir: join(home, label) };
    const doors = await openRemote(w, { relay: args }, { connect: toRelay(port), say: () => {}, warn: () => {}, acme: { fetch: acme.fetch, wait: async () => {} } });
    closing.push(() => doors.close());
    await until(() => doors.relay() !== null, 15_000);
    return { doors, got, tool: new URL(doors.relay().origin).hostname };
  };
  const a = await serve('acct'), b = await serve('other');
  expect(a.tool).toMatch(/\.acct\.relay\.test$/);
  expect(b.tool).toMatch(/\.other\.relay\.test$/);
  expect(acme.orders.map(o => o.names)).toEqual([['*.acct.relay.test'], ['*.other.relay.test']]);
  expect(txt.size).toBe(0); // (each taken down once issued)
  const call = (to, token, body) => new Promise((ok, fail) => {
    const s = connectTLS({ host: '127.0.0.1', port: tap.address().port, servername: to, ca: acme.caPem }, () => s.write(`POST /__tamber HTTP/1.1\r\nHost: ${to}\r\nx-exact-tamber-token: ${token}\r\nContent-Length: ${Buffer.byteLength(body)}\r\nConnection: close\r\n\r\n${body}`));
    let out = ''; s.on('data', d => { out += d; }); s.on('end', () => ok(out)); s.on('error', fail);
  });
  const secret = 'pairing-token-' + 'z'.repeat(24), design = 'space.secret-padding = 13px';
  expect(await call(a.tool, secret, `{"set":"${design}"}`)).toContain('"ok":true');
  expect(await call(b.tool, 'b-token', '{}')).toContain('"ok":true');
  // Each account's client reached its own server, never the other's.
  expect(a.got.map(g => g.token)).toEqual([secret]);
  expect(b.got.map(g => g.token)).toEqual(['b-token']);
  // Nothing in clear at the relay: the token, the design text, the server's answer.
  const all = Buffer.concat(seen).toString('latin1');
  for (const clear of [secret, design, 'brand.ink']) expect(all.includes(clear)).toBe(false);
}, 60_000);

test('accounts add prints a token once and keeps its hash; revoke is for good; --help names every config key', async () => {
  const { spawnSync } = await import('node:child_process');
  const { CONFIG_HELP } = await import('./tamber-relay-server.mjs');
  const dir = mkdtempSync(join(tmpdir(), 'exact-relay-cli-'));
  closing.push(() => rmSync(dir, { recursive: true, force: true }));
  const config = join(dir, 'relay.json');
  writeFileSync(config, JSON.stringify({ domain: DOMAIN, accounts: 'accounts.json' }));
  const run = (...args) => spawnSync('bun', [join(import.meta.dir, 'tamber-relay-server.mjs'), ...args, '--config', config], { encoding: 'utf8' });
  const added = run('accounts', 'add', 'newbie');
  expect(added.status).toBe(0);
  const token = /shown once: (\S+)$/m.exec(added.stdout)[1];
  const file = JSON.parse(readFileSync(join(dir, 'accounts.json'), 'utf8'));
  expect(file.accounts.newbie.token_sha256).toBe(hash(token));
  expect(readFileSync(join(dir, 'accounts.json'), 'utf8').includes(token)).toBe(false);
  expect(run('accounts', 'revoke', 'newbie').status).toBe(0);
  const again = run('accounts', 'add', 'newbie');
  expect([again.status, again.stderr.trim()]).toEqual([1, 'newbie is taken, or was: a label is never given out again']);
  const help = spawnSync('bun', [join(import.meta.dir, 'tamber-relay-server.mjs'), '--help'], { encoding: 'utf8' }).stdout;
  for (const key of ['domain', 'listen', 'tls.cert', 'tls.key', 'accounts', 'dns.command', 'perIpPerSession', 'streamsPerSession', 'sessionsPerAccount', 'txtPerAccount', 'greetingsPerIp', 'waitMs', 'graceMs', 'helloMs']) expect(help).toContain(key);
  expect(CONFIG_HELP.length).toBe(11);
});

test('R4\'s lines on loopback (scripts/tamber-relay-gate.mjs): every one met', async () => {
  const { gate } = await import('./tamber-relay-gate.mjs');
  const said = [];
  const results = await gate({ say: l => said.push(l) });
  expect(results.filter(r => !r.met).map(r => r.gate), said.join('\n')).toEqual([]);
  expect(results.length).toBe(7);
}, 120_000);
