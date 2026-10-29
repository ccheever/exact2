// LLP 1121 §5 stage 10c R4's lines, on loopback (rt1's R3 plan, theme2's 14:34
// approval): the relay service (scripts/tamber-relay-server.mjs), two accounts'
// design servers dialling into it with the real connector, each wildcard got
// by ACME through the relay's own TXT records (the ACME stand-in), and a
// client. Each line printed MET or FAILED with its evidence. The deployed R4
// runs the same lines against the real host (deployment waits on hosting).
// Run: bun scripts/tamber-relay-gate.mjs   (exits 1 when a line fails)
// Deployed (R4): the same lines against a running relay, its two accounts' tokens in ~/.exact/relay/<label>/token:
//   RELAY_GATE_DOMAIN=<domain> [RELAY_GATE_RELAY=<host[:port]>, default the domain:443]
//   [RELAY_GATE_ACCOUNTS=<a>,<b>, default acct,other] [RELAY_GATE_CA=<PEM>, a CA beside the system's (a test relay)]
//   [RELAY_GATE_CERTS=<dir>, <label>.cert.pem and <label>.key.pem for the sessions' names instead of ACME]
//   [RELAY_GATE_ACME=<directory URL>, default Let's Encrypt's]  bun scripts/tamber-relay-gate.mjs
import { createHash, randomBytes } from 'node:crypto';
import { EventEmitter } from 'node:events';
import { mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { connect as connectTCP, createServer as createTCP } from 'node:net';
import { homedir, tmpdir } from 'node:os';
import { join } from 'node:path';
import { PassThrough } from 'node:stream';
import { connect as connectTLS, rootCertificates } from 'node:tls';
import { relayServer } from './tamber-relay-server.mjs';
import { ACME, relayTxt } from './tamber-relay.mjs';
import { openRemote, selfSignedCertificate } from './tamber-remote.mjs';
import { fakeAcme } from './tamber-acme-stub.mjs';
import { tamberServer } from './tamber-server.mjs';
import { openRelayCanvas } from './tamber-canvas.mjs';
import { createServer as createHTTP } from 'node:http';

const LOCAL_DOMAIN = 'relay.test';

/** The deployed relay the lines run against, from the environment (above); null on loopback. */
export function deployedRelay(env = process.env, home = homedir()) {
  if (!env.RELAY_GATE_DOMAIN) return null;
  const domain = env.RELAY_GATE_DOMAIN.toLowerCase(), at = env.RELAY_GATE_RELAY ?? domain;
  // host, host:port, [v6] or [v6]:port (a bare v6 address is a host alone)
  const [, host, port] = /^\[(.+)\](?::(\d+))?$/.exec(at) ?? (at.split(':').length === 2 ? /^(.+):(\d+)$/.exec(at) : [null, at]);
  const labels = (env.RELAY_GATE_ACCOUNTS ?? 'acct,other').split(',');
  if (labels.length !== 2 || labels[0] === labels[1]) throw new Error('RELAY_GATE_ACCOUNTS: two labels, a,b');
  const read = path => readFileSync(path, 'utf8');
  // (the tokens as the connector keeps them: readable by their owner alone)
  for (const l of labels) { const f = join(home, '.exact', 'relay', l, 'token'); if (statSync(f).mode & 0o077) throw new Error(`${f} may be read by others: chmod 600 it`); }
  return { domain, host, port: Number(port ?? 443), labels, tokens: Object.fromEntries(labels.map(l => [l, read(join(home, '.exact', 'relay', l, 'token')).trim()])),
    ca: env.RELAY_GATE_CA ? read(env.RELAY_GATE_CA) : undefined,
    certs: env.RELAY_GATE_CERTS ? Object.fromEntries(labels.map(l => [l, { cert: read(join(env.RELAY_GATE_CERTS, `${l}.cert.pem`)), key: read(join(env.RELAY_GATE_CERTS, `${l}.key.pem`)) }])) : null,
    acme: env.RELAY_GATE_ACME ?? ACME };
}

/** A stand-in for `contract tamber serve` that answers anything. */
function fakeServe() {
  const child = Object.assign(new EventEmitter(), { stdin: new PassThrough(), stdout: new PassThrough(), stderr: new PassThrough(), exitCode: null, signalCode: null });
  child.kill = () => { child.exitCode = 0; child.emit('exit', 0); };
  let buf = '';
  child.stdin.on('data', chunk => {
    buf += chunk; const lines = buf.split('\n'); buf = lines.pop();
    for (const line of lines) child.stdout.write(`${JSON.stringify({ id: JSON.parse(line).id, ok: { files: {} } })}\n`);
  });
  return child;
}

/** Every R4 line on loopback: `[{gate, met, evidence}]`. `say(line)` prints as it goes. */
export async function gate({ say = console.log, deployed = null } = {}) {
  const DOMAIN = deployed?.domain ?? LOCAL_DOMAIN, [A, B] = deployed?.labels ?? ['acct', 'other'];
  const results = [], cleanup = [];
  const verdict = (gate, met, evidence) => { results.push({ gate, met, evidence }); say(`${met ? 'MET' : 'FAILED'} ${gate}`); for (const e of [evidence].flat()) say(`    ${e}`); };
  const until = async (f, ms = 15_000) => { const t = Date.now(); while (!(await f())) { if (Date.now() - t > ms) throw new Error('timed out'); await new Promise(ok => setTimeout(ok, 20)); } };
  try {
    const work = mkdtempSync(join(tmpdir(), 'exact-relay-gate-'));
    cleanup.push(() => rmSync(work, { recursive: true, force: true }));
    // On loopback: a relay here, its two accounts, an ACME stand-in reading its TXT records. Deployed: the relay
    // there, the accounts' tokens as the relay gave them, its certificate by the system's roots (or RELAY_GATE_CA).
    const txt = new Map();
    let relayCert = null, tokens = deployed?.tokens, port = deployed?.port, target = deployed?.host ?? '127.0.0.1';
    if (!deployed) {
      relayCert = selfSignedCertificate([DOMAIN]);
      tokens = { acct: randomBytes(24).toString('base64url'), other: randomBytes(24).toString('base64url') };
      const sha = t => createHash('sha256').update(t).digest('hex');
      writeFileSync(join(work, 'accounts.json'), JSON.stringify({ accounts: { acct: { token_sha256: sha(tokens.acct) }, other: { token_sha256: sha(tokens.other) } }, revoked: [] }));
      const relay = relayServer({ domain: DOMAIN, listen: '127.0.0.1:0', tls: relayCert, accounts: join(work, 'accounts.json'), dns: {
        publish: async (r, v) => { txt.set(r, [...(txt.get(r) ?? []), v]); },
        clear: async (r, v) => { const left = (txt.get(r) ?? []).filter(x => x !== v); if (left.length) txt.set(r, left); else txt.delete(r); },
      } }, { log: () => {} });
      port = await relay.listen();
      cleanup.push(() => relay.close());
    }
    // What a packet capture at the relay's public side holds: every byte both ways.
    const seen = [];
    // Each accepted connection's own record, by its client port (a new one replaces an old one's): whether the tap
    // carried that very connection through to the relay.
    const accepted = new Map();
    const tap = createTCP(c => { const rec = { reached: false }; accepted.set(c.remotePort, rec); const r = connectTCP({ host: target, port }, () => { rec.reached = true; }); c.on('data', d => { seen.push(d); r.write(d); }); r.on('data', d => { seen.push(d); c.write(d); }); c.on('close', () => r.destroy()); r.on('close', () => c.destroy()); c.on('error', () => {}); r.on('error', () => {}); });
    await new Promise(ok => tap.listen(0, '127.0.0.1', ok));
    cleanup.push(() => tap.close());
    const pub = tap.address().port;
    const acme = deployed ? null : fakeAcme({ dns: txt });
    // The roots a client and a connector trust here: the stand-ins' on loopback; deployed, the system's (and RELAY_GATE_CA).
    const roots = deployed?.ca ? [...rootCertificates, deployed.ca] : undefined;
    const clientCa = deployed ? roots : acme.caPem;
    // (the connectors' and TXT requests' connections through the capture too: the connector token is in it, enciphered)
    const toRelay = opts => connectTLS({ ...opts, host: '127.0.0.1', port: pub, ca: deployed ? roots : relayCert.cert });
    // The app's dev loop, for the canvas gateway (R1b) to proxy.
    const loop = createHTTP((req, res) => res.end('<!doctype html><p>canvas</p>'));
    await new Promise(ok => loop.listen(0, '127.0.0.1', ok));
    cleanup.push(() => loop.close());
    /** One account's design server on the relay, its local token, and the requests that reached it. */
    const serve = async label => {
      let doors = null;
      const logged = [];
      const w = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: l => logged.push(l), remote: () => doors?.list() ?? [] });
      // The events requests the server holds now (a poll waiting): what revocation must cut is seen to be there.
      let polling = 0;
      const handle = w.handle;
      w.handle = async (req, res, url, access) => { const poll = url.pathname === '/__tamber/events'; if (poll) polling++; try { return await handle(req, res, url, access); } finally { if (poll) polling--; } };
      const how = deployed?.certs ? deployed.certs[label] : { acme: deployed ? deployed.acme : acme.directory, hook: null, dir: join(work, label) };
      doors = await openRemote(w, { relay: { host: DOMAIN, port, account: label, token: tokens[label], names: null, ...how } }, { connect: toRelay, say: () => {}, warn: () => {}, ...(deployed ? {} : { acme: { fetch: acme.fetch, wait: async () => {} } }) });
      cleanup.push(async () => { await doors.close(); await w.close(); });
      openRelayCanvas(doors, w, { loop: `http://127.0.0.1:${loop.address().port}`, say: () => {} });
      await until(() => doors.relay() !== null, deployed ? 180_000 : 15_000); // (deployed: a real ACME order and DNS)
      let token = null;
      await w.handle({ method: 'GET', headers: {} }, { writeHead() {}, setHeader() {}, end(t) { token = JSON.parse(t).token; } }, new URL('http://127.0.0.1/__tamber/token'), { allowed: true, local: true });
      const local = async body => {
        let text = '';
        const req = Object.assign(new PassThrough(), { method: 'POST', headers: { 'x-exact-tamber-token': token }, socket: { remoteAddress: '127.0.0.1' } });
        queueMicrotask(() => req.end(JSON.stringify(body)));
        await w.handle(req, { writeHead() {}, setHeader() {}, write(t) { text += t; }, end(t) { text += t ?? ''; } }, new URL('http://127.0.0.1/__tamber'), { allowed: true, local: true });
        return JSON.parse(text);
      };
      let canvas = null;
      doors.onRelay(n => { canvas = n.canvas; });
      return { w, doors, logged, local, tool: new URL(doors.relay().origin).hostname, canvas, polling: () => polling };
    };
    /** A client's HTTPS request through the relay (by the capture) to `to`. */
    // The certificate each client's TLS ended on, by name: the design server's own (its fingerprint, as pair-code gives
    // it) if the relay passed the TLS through, anything else if the relay ended it.
    // Every client connection checks it before sending a byte (a pairing code, a token, a design): one that ended
    // anywhere else sends nothing, and is kept in `intercepted`.
    const saw = new Map(), expected = new Map(), intercepted = [];
    const ours = (to, s) => {
      const got = s.getPeerCertificate().fingerprint256;
      saw.set(to, got);
      if (got === expected.get(to)) return true;
      intercepted.push(`${to}: ${got}`); s.destroy(); return false;
    };
    const INTERCEPTED = 'a client connection ended on a certificate not its design server\'s';
    const call = (to, path, body, headers = '', method = 'POST') => new Promise((ok, fail) => {
      // (one that ended elsewhere fails the run: nothing was sent, and no line may pass on an empty answer)
      const s = connectTLS({ host: '127.0.0.1', port: pub, servername: to, ca: clientCa }, () => { if (!ours(to, s)) { fail(new Error(`${INTERCEPTED}: ${intercepted.at(-1)}`)); return; } s.write(`${method} ${path} HTTP/1.1\r\nHost: ${to}\r\nContent-Type: application/json\r\n${headers}Content-Length: ${Buffer.byteLength(body)}\r\nConnection: close\r\n\r\n${body}`); });
      let out = ''; s.on('data', d => { out += d; }); s.on('end', () => ok(out)); s.on('close', () => ok(out)); s.on('error', e => (out ? ok(out) : fail(e)));
    });
    const status = out => Number(out.split(' ')[1]);
    const json = out => { const b = out.slice(out.indexOf('\r\n\r\n') + 4); return JSON.parse(b.slice(b.indexOf('{'), b.lastIndexOf('}') + 1)); };

    const a = await serve(A), b = await serve(B);
    // (one wildcard per account: its tool's and its canvas's names end on the same certificate, its server's own)
    for (const x of [a, b]) for (const n of [x.tool, x.canvas]) expected.set(n, x.doors.relay().fingerprint);
    // Line 1 (stage 10): a paired propose client reads and proposes; it cannot write.
    const minted = (await a.local({ method: 'pair-code', scope: 'propose', name: 'laptop' })).ok;
    const paired = await call(a.tool, '/__tamber/pair', JSON.stringify({ code: minted.code, name: 'laptop' }));
    const pt = json(paired).ok?.token;
    const hello = await call(a.tool, '/__tamber', '{"method":"hello"}', `x-exact-tamber-token: ${pt}\r\n`);
    const write = await call(a.tool, '/__tamber', JSON.stringify({ method: 'transaction', client: 'laptop', files: {}, ops: [{ op: 'set-token', name: 'x', value: '1px' }] }), `x-exact-tamber-token: ${pt}\r\n`);
    const own = minted.remote?.find(r => r.relay)?.fingerprint;
    verdict('a paired propose client through the relay reads, and cannot write (its scope named); its TLS ends at the design server, not the relay', status(paired) === 200 && status(hello) === 200 && status(write) === 403 && /propose/.test(JSON.stringify(json(write))) && !!own && own === expected.get(a.tool) && saw.get(a.tool) === own && intercepted.length === 0,
      [`pair-code's relay entry: ${JSON.stringify(minted.remote?.find(r => r.relay))}`, `the certificate the client's TLS ended on: ${saw.get(a.tool)} (${saw.get(a.tool) === own ? 'the design server\'s own' : 'NOT the design server\'s'})`, `pair ${status(paired)}, hello ${status(hello)}, a set-token transaction ${status(write)}: ${JSON.stringify(json(write).error ?? {}).slice(0, 140)}`]);
    // Line 2 (stage 10): revocation cuts the client's pending long poll through the relay, and its next request.
    const first = await call(a.tool, '/__tamber/events', '{"wait":false}', `x-exact-tamber-token: ${pt}\r\n`);
    const { seq, epoch } = json(first);
    const poll = { text: '' };
    const s = connectTLS({ host: '127.0.0.1', port: pub, servername: a.tool, ca: clientCa }, () => { if (!ours(a.tool, s)) return; const body = JSON.stringify({ after: seq, epoch, wait: true }); s.write(`POST /__tamber/events HTTP/1.1\r\nHost: ${a.tool}\r\nContent-Type: application/json\r\nContent-Length: ${body.length}\r\nx-exact-tamber-token: ${pt}\r\n\r\n${body}`); });
    s.on('data', d => { poll.text += d; });
    s.on('error', () => {});
    // Held at the server before the revoke (over a real network, a quiet 300 ms could be a request not yet there).
    await until(() => a.polling() > 0, 15_000).catch(() => {});
    const pending = poll.text === '' && a.polling() > 0;
    await a.local({ method: 'revoke', client: json(paired).ok.client });
    await until(() => poll.text.startsWith('HTTP/1.1 401'), 5000).catch(() => {});
    const after = await call(a.tool, '/__tamber', '{"method":"hello"}', `x-exact-tamber-token: ${pt}\r\n`);
    verdict('revoking the paired client cuts its pending long poll through the relay, and refuses its next request', pending && poll.text.startsWith('HTTP/1.1 401') && status(after) === 401, [`pending before: ${pending}; after revoke: ${poll.text.split('\r\n')[0]}; next hello ${status(after)}`]);
    // Line 3 (stage 10): an unencrypted connection is refused: plain HTTP to the relay is closed, nothing answered.
    // (closed at the relay, not by a forward that never reached it: the tap carried this very connection through)
    let mine = null;
    const plain = await new Promise(ok => { const c = connectTCP({ host: '127.0.0.1', port: pub }, () => { mine = c.localPort; c.write(`POST /__tamber/pair HTTP/1.1\r\nHost: ${a.tool}\r\nContent-Length: 2\r\n\r\n{}`); }); let got = ''; const t = setTimeout(() => { c.destroy(); ok(got || null); }, 10_000); c.on('data', d => { got += d; }); c.on('close', () => { clearTimeout(t); ok(got); }); c.on('error', () => {}); });
    const reached = mine !== null && accepted.get(mine)?.reached === true;
    verdict('plain HTTP to a session\'s name (the pairing endpoint) is closed at the relay, nothing answered', plain === '' && reached, [`answered ${plain === null ? 'nothing, but held open 10 s' : `${plain.length} bytes`}; this connection reached the relay: ${reached}`]);
    // Line 4: the capture holds no token and no design text in clear.
    const design = 'space.gate-secret = 17px', secondPair = (await a.local({ method: 'pair-code', scope: 'read', name: 'cap' })).ok;
    const p2 = await call(a.tool, '/__tamber/pair', JSON.stringify({ code: secondPair.code, name: 'cap' }));
    const t2 = json(p2).ok?.token;
    await call(a.tool, '/__tamber', JSON.stringify({ method: 'propose', files: {}, ops: [{ op: 'set-token', name: 'space.gate-secret', value: '17px' }], note: design }), `x-exact-tamber-token: ${t2}\r\n`);
    const captured = Buffer.concat(seen).toString('latin1');
    const leaks = [pt, t2, secondPair.code, design, tokens[A], tokens[B]].filter(x => x && captured.includes(x));
    // (the connector tokens did cross it: every connector and TXT connection came through the capture, enciphered)
    verdict('a capture at the relay holds no token, pairing code or design text in clear', leaks.length === 0 && captured.length > 0, [`${captured.length} bytes captured both ways; found in clear: ${leaks.length ? leaks.map(l => l.slice(0, 6) + '…').join(', ') : 'none'}`]);
    // Line 5 (cross-account): one account's names reach only its own server; the other's TXT names are refused.
    const gotA = a.logged.length, gotB = b.logged.length;
    const bMinted = (await b.local({ method: 'pair-code', scope: 'read', name: 'b' })).ok;
    const crossed = await call(a.tool, '/__tamber/pair', JSON.stringify({ code: bMinted.code, name: 'b' }));
    const bPaired = await call(b.tool, '/__tamber/pair', JSON.stringify({ code: bMinted.code, name: 'b' }));
    let txtRefused = '';
    await relayTxt({ host: DOMAIN, port, account: B, token: tokens[B] }, `_acme-challenge.${A}.${DOMAIN}`, 'A'.repeat(43), { connect: toRelay }).catch(e => { txtRefused = e.message; });
    verdict('cross-account: one account\'s code fails at the other\'s name (each name reaches its own server only); a TXT record for the other\'s names is refused', status(crossed) === 403 && status(bPaired) === 200 && /not your account's name/.test(txtRefused),
      [`${B}'s code at ${A}'s name ${status(crossed)}, at its own ${status(bPaired)}; logs grew ${A} +${a.logged.length - gotA}, ${B} +${b.logged.length - gotB}`, `TXT for _acme-challenge.${A}.${DOMAIN} as ${B}: ${txtRefused}`]);
    // Line 6 (cross-account): a canvas's gateway cookie opens that account's canvas only. Sent to the other account's
    // canvas (as a browser would one set with Domain=relay.test), or to any tool, it grants nothing.
    const minted2 = json(await call(a.tool, '/__tamber', '{"method":"canvas"}', `x-exact-tamber-token: ${t2}\r\n`)).ok;
    const entry = await call(a.canvas, new URL(minted2.url).pathname, JSON.stringify({ grant: minted2.grant }), `Origin: https://${a.canvas}\r\n`);
    const cookie = /set-cookie: ([^;\r\n]+)/i.exec(entry)?.[1] ?? '';
    const home = await call(a.canvas, '/', '', `Cookie: ${cookie}\r\n`, 'GET');
    const away = await call(b.canvas, '/', '', `Cookie: ${cookie}\r\n`, 'GET');
    const atTool = await call(b.tool, '/__tamber', '{"method":"hello"}', `Cookie: ${cookie}; tamber=${t2}\r\n`);
    const atOwnTool = await call(a.tool, '/__tamber', '{"method":"hello"}', `Cookie: ${cookie}; tamber=${t2}\r\n`);
    verdict('cross-account: one account\'s canvas cookie opens its own canvas only: at the other\'s canvas, or at any tool, it grants nothing', status(entry) === 204 && !!cookie && status(home) === 200 && status(away) === 404 && [401, 404].includes(status(atTool)) && [401, 404].includes(status(atOwnTool)),
      [`entered ${A}'s canvas ${status(entry)} (${cookie.split('=')[0]}=…); with that cookie: ${A}'s canvas ${status(home)}, ${B}'s canvas ${status(away)}, ${B}'s tool ${status(atTool)}, ${A}'s own tool ${status(atOwnTool)}`]);
    // Line 7 (cross-account): flooding one session's pairing leaves the other's at 0 refusals.
    const flood = [];
    for (let i = 0; i < 11; i++) flood.push(status(await call(a.tool, '/__tamber/pair', '{"code":"AAAAA-AAAAA"}')));
    const bCode = (await b.local({ method: 'pair-code', scope: 'read', name: 'b2' })).ok.code;
    const bAfter = await call(b.tool, '/__tamber/pair', JSON.stringify({ code: bCode, name: 'b2' }));
    verdict('cross-account: one session\'s pairing flooded to its limit leaves the other\'s untouched', flood.includes(429) && status(bAfter) === 200, [`${A}'s pairs: ${flood.join(' ')}; ${B}'s next pair ${status(bAfter)}`]);
    // Every client connection, the long poll's too, ended on its own design server's certificate.
    if (intercepted.length) verdict(INTERCEPTED, false, intercepted);
  } catch (e) {
    verdict('the run itself', false, String(e?.stack ?? e).split('\n').slice(0, 4));
  } finally {
    for (const c of cleanup.reverse()) await c();
  }
  return results;
}

if (import.meta.main) {
  const results = await gate({ deployed: deployedRelay() });
  console.log(`${results.filter(r => r.met).length}/${results.length} lines met`);
  process.exit(results.every(r => r.met) ? 0 : 1);
}
