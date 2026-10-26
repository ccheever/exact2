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
import { helloName, keyOf, loadAccounts, masked, relayServer } from './tamber-relay-server.mjs';
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
    expect(readFileSync(join(dir, 'labels.seen'), 'utf8')).toBe('a\nb\nold\nrevoked old\n');
    // Revoked and an account again; dropped from both; or a seen label reissued after a revoke's removal: refused.
    write({ accounts: { a: { token_sha256: t }, b: { token_sha256: t }, old: { token_sha256: t } }, revoked: ['old'] });
    expect(() => loadAccounts(path, DOMAIN)).toThrow('old is revoked: a label is never given out again');
    write({ accounts: { a: { token_sha256: t } }, revoked: ['old'] });
    expect(() => loadAccounts(path, DOMAIN)).toThrow('b was an account and is neither one nor revoked');
    // Taken off the revoked list and given out again, under another token (round 1): labels.seen remembers.
    write({ accounts: { a: { token_sha256: t }, b: { token_sha256: t }, old: { token_sha256: hash('new owner') } }, revoked: [] });
    expect(() => loadAccounts(path, DOMAIN)).toThrow('old is revoked: a label is never given out again');
    // One account's names another's, or under or over them; or under the relay's own domain: refused.
    write({ accounts: { a: { token_sha256: t, names: ['example.com'] }, b: { token_sha256: t, names: ['tamber.example.com'] } }, revoked: ['old'] });
    expect(() => loadAccounts(path, DOMAIN)).toThrow('one account\'s names may not be, or be under or over, another\'s');
    write({ accounts: { a: { token_sha256: t }, b: { token_sha256: t, names: ['x.relay.test'] } }, revoked: ['old'] });
    expect(() => loadAccounts(path, DOMAIN)).toThrow('is the relay\'s own domain, under it or over it');
    write({ accounts: { a: { token_sha256: t }, b: { token_sha256: t, names: ['relay.test'] } }, revoked: ['old'] });
    expect(() => loadAccounts(path, 'edge.relay.test')).toThrow('is the relay\'s own domain, under it or over it');
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
  const { NODE_TLS_REJECT_UNAUTHORIZED: _tls, ...env } = process.env;
  const run = (...args) => spawnSync('bun', [join(import.meta.dir, 'tamber-relay-server.mjs'), ...args, '--config', config], { encoding: 'utf8', env });
  const added = run('accounts', 'add', 'newbie');
  expect(added.status).toBe(0);
  const token = /shown once: (\S+)$/m.exec(added.stdout)[1];
  const file = JSON.parse(readFileSync(join(dir, 'accounts.json'), 'utf8'));
  expect(file.accounts.newbie.token_sha256).toBe(hash(token));
  expect(readFileSync(join(dir, 'accounts.json'), 'utf8').includes(token)).toBe(false);
  expect(run('accounts', 'revoke', 'newbie').status).toBe(0);
  const again = run('accounts', 'add', 'newbie');
  expect([again.status, again.stderr.replace(/\x1b\[[0-9;]*m/g, '').trim()]).toEqual([1, 'newbie is taken, or was: a label is never given out again']);
  // Taken off the revoked list by hand, the label is still refused (labels.seen), said plainly, nothing left behind.
  const edited = JSON.parse(readFileSync(join(dir, 'accounts.json'), 'utf8'));
  edited.revoked = [];
  writeFileSync(join(dir, 'accounts.json'), JSON.stringify(edited));
  const sneaked = run('accounts', 'add', 'newbie');
  expect([sneaked.status, sneaked.stderr.replace(/\x1b\[[0-9;]*m/g, '').trim()]).toEqual([1, 'newbie is revoked: a label is never given out again']);
  const { readdirSync } = await import('node:fs');
  expect(readdirSync(dir).filter(f => f.endsWith('.tmp') || f.endsWith('.lock'))).toEqual([]);
  const help = spawnSync('bun', [join(import.meta.dir, 'tamber-relay-server.mjs'), '--help'], { encoding: 'utf8' }).stdout;
  for (const key of ['domain', 'listen', 'tls.cert', 'tls.key', 'accounts', 'dns.command', 'perIpPerSession', 'streamsPerSession', 'idlePerSession', 'sessionsPerAccount', 'txtPerAccount', 'txtInFlight', 'pendingPerIp', 'pendingTotal', 'greetingsPerIp', 'waitMs', 'graceMs', 'helloMs']) expect(help).toContain(key);
  expect(CONFIG_HELP.length).toBe(14);
});

test('R4\'s lines on loopback (scripts/tamber-relay-gate.mjs): every one met', async () => {
  const { gate } = await import('./tamber-relay-gate.mjs');
  const said = [];
  const results = await gate({ say: l => said.push(l) });
  expect(results.filter(r => !r.met).map(r => r.gate), said.join('\n')).toEqual([]);
  expect(results.length).toBe(7);
}, 120_000);

test('the gate against a deployed relay (R4) reads it from the environment: the tokens from their files, two labels', async () => {
  const { deployedRelay } = await import('./tamber-relay-gate.mjs');
  expect(deployedRelay({})).toBe(null);
  const home = mkdtempSync(join(tmpdir(), 'exact-gate-home-'));
  try {
    const { mkdirSync } = await import('node:fs');
    for (const l of ['alpha', 'beta']) { mkdirSync(join(home, '.exact', 'relay', l), { recursive: true }); writeFileSync(join(home, '.exact', 'relay', l, 'token'), `${l}-token\n`, { mode: 0o600 }); }
    const d = deployedRelay({ RELAY_GATE_DOMAIN: 'Relay.Example', RELAY_GATE_RELAY: '127.0.0.1:18443', RELAY_GATE_ACCOUNTS: 'alpha,beta' }, home);
    expect([d.domain, d.host, d.port, d.labels, d.tokens, d.certs, d.ca]).toEqual(['relay.example', '127.0.0.1', 18443, ['alpha', 'beta'], { alpha: 'alpha-token', beta: 'beta-token' }, null, undefined]);
    expect(deployedRelay({ RELAY_GATE_DOMAIN: 'relay.example', RELAY_GATE_ACCOUNTS: 'alpha,beta' }, home).port).toBe(443);
    expect(() => deployedRelay({ RELAY_GATE_DOMAIN: 'relay.example', RELAY_GATE_ACCOUNTS: 'alpha,alpha' }, home)).toThrow('two labels');
    expect(() => deployedRelay({ RELAY_GATE_DOMAIN: 'relay.example' }, home)).toThrow(/acct/);
    expect(deployedRelay({ RELAY_GATE_DOMAIN: 'relay.example', RELAY_GATE_RELAY: '[::1]:8443', RELAY_GATE_ACCOUNTS: 'alpha,beta' }, home)).toMatchObject({ host: '::1', port: 8443 });
    const { chmodSync } = await import('node:fs');
    chmodSync(join(home, '.exact', 'relay', 'beta', 'token'), 0o644);
    expect(() => deployedRelay({ RELAY_GATE_DOMAIN: 'relay.example', RELAY_GATE_ACCOUNTS: 'alpha,beta' }, home)).toThrow('chmod 600');
  } finally { rmSync(home, { recursive: true, force: true }); }
});

test('the gate\'s deployed mode, against a relay of its own with certificates from a CA of its own: every line met', async () => {
  const { deployedRelay, gate } = await import('./tamber-relay-gate.mjs');
  const { spawnSync } = await import('node:child_process');
  const { mkdirSync } = await import('node:fs');
  const dir = mkdtempSync(join(tmpdir(), 'exact-gate-deployed-'));
  let relay = null;
  try {
    const ssl = (...a) => { const r = spawnSync('openssl', a, { cwd: dir, encoding: 'utf8' }); if (r.status !== 0) throw new Error(r.stderr); };
    ssl('req', '-x509', '-newkey', 'ec', '-pkeyopt', 'ec_paramgen_curve:P-256', '-nodes', '-days', '2', '-subj', '/CN=gate CA', '-keyout', 'ca.key', '-out', 'ca.pem');
    const issue = (name, san) => {
      ssl('req', '-newkey', 'ec', '-pkeyopt', 'ec_paramgen_curve:P-256', '-nodes', '-subj', `/CN=${san}`, '-keyout', `${name}.key.pem`, '-out', `${name}.csr`);
      writeFileSync(join(dir, `${name}.ext`), `subjectAltName=DNS:${san}\n`);
      ssl('x509', '-req', '-in', `${name}.csr`, '-CA', 'ca.pem', '-CAkey', 'ca.key', '-CAcreateserial', '-days', '2', '-extfile', `${name}.ext`, '-out', `${name}.cert.pem`);
    };
    issue('relay', 'relay.example'); issue('alpha', '*.alpha.relay.example'); issue('beta', '*.beta.relay.example');
    const home = join(dir, 'home'), tokens = { alpha: 'alpha-token-0123456789abcdef', beta: 'beta-token-0123456789abcdef' };
    for (const l of ['alpha', 'beta']) { mkdirSync(join(home, '.exact', 'relay', l), { recursive: true }); writeFileSync(join(home, '.exact', 'relay', l, 'token'), tokens[l], { mode: 0o600 }); }
    const sha = t => createHash('sha256').update(t).digest('hex');
    writeFileSync(join(dir, 'accounts.json'), JSON.stringify({ accounts: { alpha: { token_sha256: sha(tokens.alpha) }, beta: { token_sha256: sha(tokens.beta) } }, revoked: [] }));
    relay = relayServer({ domain: 'relay.example', listen: '127.0.0.1:0', tls: { cert: readFileSync(join(dir, 'relay.cert.pem'), 'utf8'), key: readFileSync(join(dir, 'relay.key.pem'), 'utf8') }, accounts: join(dir, 'accounts.json'), dns: { publish: async () => {}, clear: async () => {} } }, { log: () => {} });
    const port = await relay.listen();
    const deployed = deployedRelay({ RELAY_GATE_DOMAIN: 'relay.example', RELAY_GATE_RELAY: `127.0.0.1:${port}`, RELAY_GATE_ACCOUNTS: 'alpha,beta', RELAY_GATE_CA: join(dir, 'ca.pem'), RELAY_GATE_CERTS: dir }, home);
    const said = [];
    const results = await gate({ say: l => said.push(l), deployed });
    expect(results.filter(r => !r.met).map(r => r.gate), said.join('\n')).toEqual([]);
    expect(results.length).toBe(7);
  } finally { await relay?.close(); rmSync(dir, { recursive: true, force: true }); }
}, 120_000);

test('round 1: a hostile hello, cut and flipped anywhere, is never read past its record and never throws', async () => {
  const named = await helloTo('s1-tool.acct.relay.test');
  let seed = 7;
  const rand = n => { seed = (seed * 1103515245 + 12345) & 0x7fffffff; return seed % n; };
  for (let i = 0; i < 5000; i++) {
    const b = Buffer.from(named);
    for (let k = rand(4) + 1; k > 0; k--) b[rand(b.length)] = rand(256);
    const tail = Buffer.from('after');
    const h = helloName(Buffer.concat([b.subarray(0, rand(b.length) + 1), tail]));
    expect(['ok', 'bad', 'more']).toContain(h.state);
    if (h.state === 'ok') expect(h.read).toBeLessThanOrEqual(5 + 16_384);
  }
});

test('round 1: revocation cuts the clients waiting for a connection and the account\'s TXT requests too; a parent moved to another account drops the session on it', async () => {
  const { relay, port, path } = await startRelay({ limits: { waitMs: 60_000 } });
  // A session with no idle connection: a client waits.
  const c = await greet(port, hello('acct', TOKENS.acct, [`run: ${RUN}`]));
  const tool = c.line.split(' ')[1];
  const first = await helloTo(tool);
  const busy = connectTCP({ host: '127.0.0.1', port }, () => busy.write(first)); // (takes the one idle connection)
  busy.on('error', () => {});
  await until(() => [...relay.sessions.values()][0]?.carried.size === 1);
  const waiting = connectTCP({ host: '127.0.0.1', port }, () => waiting.write(first));
  const waited = new Promise(ok => waiting.on('close', ok));
  waiting.on('error', () => {});
  await until(() => [...relay.sessions.values()][0]?.waiting.size === 1);
  // A BYO session on `other`'s parent.
  const byo = await greet(port, hello('other', TOKENS.other, [`run: ${RUN}`, 'names: tool.tamber.example.com canvas.tamber.example.com']));
  expect(byo.line).toBe('READY tool.tamber.example.com canvas.tamber.example.com');
  const file = JSON.parse(readFileSync(path, 'utf8'));
  delete file.accounts.acct;
  file.revoked.push('acct');
  file.accounts.other.names = [];
  file.accounts.third = { token_sha256: hash('third'), names: ['tamber.example.com'] };
  writeFileSync(path, JSON.stringify(file));
  expect(relay.reload()).toBe(true);
  await waited;
  await until(() => byo.conn.destroyed);
  expect([...relay.sessions.values()].map(s => s.tool)).toEqual([]);
});

test('round 1: a session\'s idle connections and its carried and waiting clients are capped; a refused peer that holds its end open is cut', async () => {
  const { relay, port } = await startRelay({ limits: { idlePerSession: 2, streamsPerSession: 1, waitMs: 60_000 } });
  const a = await greet(port, hello('acct', TOKENS.acct, [`run: ${RUN}`]));
  await greet(port, hello('acct', TOKENS.acct, [`run: ${RUN}`]));
  const third = await greet(port, hello('acct', TOKENS.acct, [`run: ${RUN}`]));
  expect(third.line).toBeNull(); // (closed, nothing said: the connector waits)
  const first = await helloTo(a.line.split(' ')[1]);
  const open = () => new Promise(ok => { const s = connectTCP({ host: '127.0.0.1', port }, () => s.write(first)); let lived = false; const t = setTimeout(() => { lived = true; ok(true); }, 300); s.on('close', () => { clearTimeout(t); if (!lived) ok(false); }); s.on('error', () => {}); closing.push(() => s.destroy()); });
  expect([await open(), await open()]).toEqual([true, false]);
  expect([...relay.sessions.values()][0].carried.size + [...relay.sessions.values()][0].waiting.size).toBe(1);
  // A refused greeting whose peer never closes its side: destroyed all the same.
  const held = await greet(port, hello('acct', 'wrong', [`run: ${RUN}`]));
  expect(held.line).toBe('REFUSED no such account');
  await until(() => held.conn.destroyed, 3000);
});

test('round 1: a run whose session lapsed (or whose relay restarted) asks for its names again and gets them back; another account, or another run, can\'t take them', async () => {
  const { relay, port } = await startRelay({ limits: { graceMs: 50 } });
  const a = await greet(port, hello('acct', TOKENS.acct, [`run: ${RUN}`]));
  const [, tool, canvas] = a.line.split(' ');
  a.conn.destroy();
  await until(() => relay.sessions.size === 0);
  expect((await greet(port, hello('other', TOKENS.other, [`run: ${RUN}`, `names: ${tool} ${canvas}`]))).line).toBe('REFUSED names not of your account');
  // Another run of the same account can't claim them either (round 2): a run's names are of its own run id.
  expect((await greet(port, hello('acct', TOKENS.acct, [`run: ${'e'.repeat(32)}`, `names: ${tool} ${canvas}`]))).line).toBe('REFUSED names not of your account');
  expect((await greet(port, hello('acct', TOKENS.acct, [`run: ${RUN}`, `names: ${tool} ${canvas}`]))).line).toBe(`READY ${tool} ${canvas}`);
});

test('round 1: the accounts file replaced by a rename, again and again, is read each time', async () => {
  const { relay, path } = await startRelay();
  const { renameSync } = await import('node:fs');
  const replace = file => { writeFileSync(`${path}.tmp`, JSON.stringify(file)); renameSync(`${path}.tmp`, path); };
  const file = JSON.parse(readFileSync(path, 'utf8'));
  delete file.accounts.acct;
  file.revoked.push('acct');
  replace(file);
  await until(() => relay.revoked.has('acct'), 5000);
  delete file.accounts.other;
  file.revoked.push('other');
  replace(file);
  await until(() => relay.revoked.has('other'), 5000);
});

test('round 2: connections not yet a session\'s or authenticated are capped per address and in all; a cut account\'s TXT request in flight is stopped', async () => {
  const { port, txt } = await startRelay({ limits: { pendingPerIp: 3 } });
  const silent = () => new Promise(ok => { const s = connectTCP({ host: '127.0.0.1', port }, () => setTimeout(() => ok(s), 50)); s.gone = false; s.on('close', () => { s.gone = true; }); s.on('error', () => {}); closing.push(() => s.destroy()); });
  const held = [await silent(), await silent(), await silent()];
  const fourth = await silent();
  await until(() => fourth.gone, 2000);
  expect(held.map(s => s.gone)).toEqual([false, false, false]);
  for (const s of held) s.destroy();
  // A TXT request whose DNS work hangs: the account revoked, the work told to stop, the connection cut.
  let stopped = false;
  const { relayServer: make } = await import('./tamber-relay-server.mjs');
  const dir = mkdtempSync(join(tmpdir(), 'exact-relay-txtcut-'));
  closing.push(() => rmSync(dir, { recursive: true, force: true }));
  const accounts = join(dir, 'accounts.json');
  writeFileSync(accounts, JSON.stringify({ accounts: { acct: { token_sha256: hash(TOKENS.acct) } }, revoked: [] }));
  const slow = make({ domain: DOMAIN, listen: '127.0.0.1:0', tls: relayCert, accounts, dns: { publish: (r, v, signal) => new Promise((_, fail) => signal.addEventListener('abort', () => { stopped = true; fail(new Error('stopped')); })), clear: async () => {} } }, { log: () => {} });
  const slowPort = await slow.listen();
  closing.push(() => slow.close());
  const asked = relayTxt({ host: DOMAIN, port: slowPort, account: 'acct', token: TOKENS.acct }, '_acme-challenge.acct.relay.test', 'A'.repeat(43), { connect: toRelay(slowPort) }).then(() => 'ok', e => e.message);
  await new Promise(ok => setTimeout(ok, 200));
  writeFileSync(accounts, JSON.stringify({ accounts: {}, revoked: ['acct'] }));
  expect(slow.reload()).toBe(true);
  expect(await asked).not.toBe('ok');
  expect(stopped).toBe(true);
  expect(txt.size).toBe(0);
});

test('round 2: a log shows an address\'s network only: an IPv4\'s /24, an IPv6\'s /64 (compressed or not, mapped or not)', () => {
  expect(masked('203.0.113.9')).toBe('203.0.113.x');
  expect(masked('::ffff:203.0.113.9')).toBe('203.0.113.x');
  expect(masked('2001:db8::1234')).toBe('2001:db8:0:0::/64');
  expect(masked('2001:db8:aa:bb:cc:dd:ee:ff')).toBe('2001:db8:aa:bb::/64');
  expect(masked('::1')).toBe('0:0:0:0::/64');
  expect(masked('nonsense')).toBe('unknown');
});

test('round 2: the relay\'s certificate names its domain alone', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-relay-cert2-'));
  closing.push(() => rmSync(dir, { recursive: true, force: true }));
  const accounts = join(dir, 'accounts.json');
  writeFileSync(accounts, JSON.stringify({ accounts: {}, revoked: [] }));
  expect(() => relayServer({ domain: DOMAIN, tls: selfSignedCertificate([DOMAIN, 'also.example.com']), accounts })).toThrow(`tls.cert does not name ${DOMAIN} alone`);
});

test('round 3: the parser touches no byte past the first record (instrumented), and the relay hands it no more than that record', async () => {
  const named = await helloTo('s1-tool.acct.relay.test');
  const end = 5 + named.readUInt16BE(3);
  const whole = Buffer.concat([named, Buffer.from('later records, never read')]);
  let furthest = -1;
  const watched = new Proxy(whole, { get: (t, k) => { if (typeof k === 'string' && /^\d+$/.test(k)) furthest = Math.max(furthest, Number(k)); const v = t[k]; return typeof v === 'function' ? v.bind(t) : v; } });
  expect(helloName(watched)).toMatchObject({ state: 'ok', sni: 's1-tool.acct.relay.test', read: end });
  expect(furthest).toBe(end - 1);
});

test('round 3: a label ends in a letter or digit, and its sessions\' names must fit under the domain', () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-relay-label-'));
  try {
    const path = join(dir, 'accounts.json');
    writeFileSync(path, JSON.stringify({ accounts: { 'bad-': { token_sha256: hash('x') } }, revoked: [] }));
    expect(() => loadAccounts(path, DOMAIN)).toThrow('is no label');
    writeFileSync(path, JSON.stringify({ accounts: { ['a'.repeat(63)]: { token_sha256: hash('x') } }, revoked: [] }));
    expect(() => loadAccounts(path, `${'d'.repeat(60)}.${'e'.repeat(60)}.${'f'.repeat(60)}.test`)).toThrow('would be too long');
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('round 3 (Grok): a restarted server (a new run) takes its own names back from its lapsed session at once; a live one keeps them', async () => {
  const { port } = await startRelay({ limits: { graceMs: 60_000 } });
  const byo = 'names: tool.tamber.example.com canvas.tamber.example.com';
  const first = await greet(port, hello('other', TOKENS.other, [`run: ${RUN}`, byo]));
  expect(first.line).toBe('READY tool.tamber.example.com canvas.tamber.example.com');
  // Live: another run is refused.
  expect((await greet(port, hello('other', TOKENS.other, [`run: ${'b'.repeat(32)}`, byo]))).line).toBe('REFUSED names in use by another session');
  // The first run's process gone (its connections closed), within the grace: the new run gets them.
  first.conn.destroy();
  await new Promise(ok => setTimeout(ok, 100));
  expect((await greet(port, hello('other', TOKENS.other, [`run: ${'c'.repeat(32)}`, byo]))).line).toBe('READY tool.tamber.example.com canvas.tamber.example.com');
});

test('round 3 (Grok): an account\'s TXT requests under way at once are capped; an authenticated refusal never silences its address; limits count an IPv6 /64 as one', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-relay-inflight-'));
  closing.push(() => rmSync(dir, { recursive: true, force: true }));
  const accounts = join(dir, 'accounts.json');
  writeFileSync(accounts, JSON.stringify({ accounts: { acct: { token_sha256: hash(TOKENS.acct) } }, revoked: [] }));
  const hang = [];
  const slow = relayServer({ domain: DOMAIN, listen: '127.0.0.1:0', tls: relayCert, accounts, limits: { txtInFlight: 1, greetingsPerIp: 2 }, dns: { publish: () => new Promise(ok => hang.push(ok)), clear: async () => {} } }, { log: () => {} });
  const port = await slow.listen();
  closing.push(() => slow.close());
  const as = { host: DOMAIN, port, account: 'acct', token: TOKENS.acct };
  const via = { connect: toRelay(port) };
  const first = relayTxt(as, '_acme-challenge.acct.relay.test', 'A'.repeat(43), via);
  await until(() => hang.length === 1);
  await expect(relayTxt(as, '_acme-challenge.acct.relay.test', 'B'.repeat(43), via)).rejects.toThrow('more than 1 TXT requests at once');
  await expect(relayTxt(as, '_acme-challenge.acct.relay.test', 'C'.repeat(43), via)).rejects.toThrow('more than 1 TXT requests at once');
  // (two authenticated refusals, past greetingsPerIp: the address still answered)
  expect((await greet(port, hello('acct', TOKENS.acct, [`run: ${RUN}`]))).line).toMatch(/^READY /);
  hang[0]();
  await first;
  expect(keyOf('2001:db8::1')).toBe(keyOf('2001:db8:0:0:ffff::2'));
  expect(keyOf('::ffff:203.0.113.9')).toBe('203.0.113.9');
});

test('round 3 re-check (Astra): a client waiting on a lapsed session, or the account\'s session cap, never keeps a restarted server from its names', async () => {
  const { port } = await startRelay({ limits: { graceMs: 60_000, sessionsPerAccount: 1, waitMs: 30_000 } });
  const byo = 'names: tool.tamber.example.com canvas.tamber.example.com';
  const first = await greet(port, hello('other', TOKENS.other, [`run: ${RUN}`, byo]));
  expect(first.line).toMatch(/^READY /);
  first.conn.destroy();
  await new Promise(ok => setTimeout(ok, 100));
  // A stranger's client arrives for the name meanwhile, and waits.
  const hello1 = await helloTo('tool.tamber.example.com');
  const stranger = connectTCP({ host: '127.0.0.1', port }, () => stranger.write(hello1));
  const gone = new Promise(ok => stranger.on('close', ok));
  stranger.on('error', () => {});
  await new Promise(ok => setTimeout(ok, 100));
  // The restarted server (a new run), at the account's cap of one: its names back; the waiting client closed.
  expect((await greet(port, hello('other', TOKENS.other, [`run: ${'f'.repeat(32)}`, byo]))).line).toBe('READY tool.tamber.example.com canvas.tamber.example.com');
  await gone;
});
