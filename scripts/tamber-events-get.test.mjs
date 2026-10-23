// `GET /__tamber/events` (tamber/1.17, LLP 1122 stage 14 D): the change stream's long poll for a client that re-asks
// by cursor (`exactLongPoll`), beside the POST form: an opaque cursor in each answer, a 204 heartbeat after the wait,
// the query bounded as the body's fields are, the same scope as the POST read, a client present 90 s after its
// answer (LLP 1122 D4), and never a `Retry-After` past 30 s.
import { expect, test } from 'bun:test';
import { EventEmitter } from 'node:events';
import { PassThrough } from 'node:stream';
import { developmentGate } from './install-page.mjs';
import { eventsQuery } from './tamber-events-get.mjs';
import { tamberServer } from './tamber-server.mjs';

/** A stand-in for `contract tamber serve` that answers every request. */
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

/** A request as the gate and the handler see it: its status, headers and body. */
async function ask(tamber, gate, { path, method = 'GET', headers = {}, body = '' }) {
  const req = Object.assign(new PassThrough(), { method, headers, socket: { remoteAddress: '127.0.0.1' } });
  let status = 0, sent = {}, text = '';
  const res = Object.assign(new EventEmitter(), { destroyed: false, writableEnded: false, setHeader(k, v) { sent[k.toLowerCase()] = v; }, writeHead(s, h = {}) { status = s; for (const [k, v] of Object.entries(h)) sent[k.toLowerCase()] = v; }, write(t) { text += t; }, end(t) { text += t ?? ''; this.writableEnded = true; } });
  queueMicrotask(() => req.end(body));
  await tamber.handle(req, res, new URL(path, 'http://x'), gate.check(req));
  return { status, headers: sent, body: text ? JSON.parse(text) : null };
}

const PORT = 8817;
const setup = () => {
  const gate = developmentGate([{ origin: `http://127.0.0.1:${PORT}` }], PORT);
  const tamber = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {} });
  const headers = { host: `127.0.0.1:${PORT}`, 'x-exact-tamber-token': tamber.token };
  return { gate, tamber, headers, get: (query) => ask(tamber, gate, { path: `/__tamber/events${query}`, headers }) };
};

/** The server's 25 s wait made short for a test: a timer of that length runs at once. */
async function quickWait(f) {
  const real = globalThis.setTimeout;
  globalThis.setTimeout = (fn, ms, ...rest) => real(fn, ms === 25_000 ? 20 : ms, ...rest);
  try { return await f(); } finally { globalThis.setTimeout = real; }
}

test('the GET form answers as the POST read, with an opaque cursor; nothing new in the wait is a 204 with no body', async () => {
  const { tamber, get } = setup();
  try {
    const first = await get('?client=tool');
    expect(first.status).toBe(200);
    expect(first.body).toMatchObject({ events: [], missed: false, cursor: `${first.body.seq}.${first.body.held}` });
    expect(first.body.asked).toBe(null);
    expect(Array.isArray(first.body.proposals)).toBe(true);
    // A stream's first ask opens from a cursor (a POST read's): answered at once, nothing new, that cursor asked.
    const opened = await get(`?open=${first.body.cursor}&epoch=${first.body.epoch}&client=tool`);
    expect([opened.status, opened.body.asked, opened.body.cursor, opened.body.events]).toEqual([200, first.body.cursor, first.body.cursor, []]);
    // From that cursor, nothing new: held until the wait ends, then a heartbeat.
    const quiet = await quickWait(() => get(`?after=${first.body.cursor}&epoch=${first.body.epoch}&client=tool`));
    expect([quiet.status, quiet.body]).toEqual([204, null]);
    // A held list that moved past the cursor's revision: answered at once, the list with it.
    const moved = await get(`?after=${first.body.seq}.${first.body.held + 5}&epoch=${first.body.epoch}&client=tool`);
    expect(moved.status).toBe(200);
    expect(moved.body.cursor).toBe(`${first.body.seq}.${first.body.held}`);
    expect(moved.body.asked).toBe(`${first.body.seq}.${first.body.held + 5}`);
    expect(Array.isArray(moved.body.proposals)).toBe(true);
    // An open cursor this run can't serve (past its numbers, or another run's) is answered as a gap is: missed.
    const past = await get(`?open=${first.body.seq + 1000}.0&epoch=${first.body.epoch}&client=tool`);
    expect([past.status, past.body.missed, past.body.asked]).toEqual([200, true, `${first.body.seq + 1000}.0`]);
    const elsewhere = await get(`?open=${first.body.cursor}&epoch=elsewhere&client=tool`);
    expect([elsewhere.status, elsewhere.body.missed, elsewhere.body.epoch]).toEqual([200, true, first.body.epoch]);
    // Another run's epoch: answered at once, as missed.
    const other = await get(`?after=${first.body.cursor}&epoch=elsewhere&client=tool`);
    expect([other.status, other.body.missed]).toEqual([200, true]);
    // No answer here names a Retry-After (none past 30 s on this route).
    for (const r of [first, opened, quiet, moved, other]) expect(r.headers['retry-after']).toBeUndefined();
  } finally { await tamber.close(); }
});

test("the query is bounded as the body's fields are: a cursor this server gave, each parameter once, names 1 to 256 characters", async () => {
  const { tamber, get } = setup();
  try {
    for (const query of ['?after=x', '?after=1.2.3', '?after=-1', '?after=01', '?wait=false', '?after=1&after=2', '?open=x', '?open=1&open=2', `?client=${'c'.repeat(257)}`, '?epoch=']) {
      const r = await get(query);
      expect([query, r.status]).toEqual([query, 400]);
      expect(typeof r.body.error.message).toBe('string');
    }
    expect(eventsQuery(new URL('http://x/?after=12.3&epoch=e&client=c')).request).toEqual({ after: 12, held: 3, epoch: 'e', client: 'c' });
    expect(eventsQuery(new URL('http://x/?after=12')).request).toEqual({ after: 12 });
    // `open` only for an ask with no `after` (answered at once); the host's `after` wins on every later one.
    expect(eventsQuery(new URL('http://x/?open=12.3')).request).toEqual({ after: 12, held: 3, now: true });
    expect(eventsQuery(new URL('http://x/?open=12.3&after=14')).request).toEqual({ after: 14 });
  } finally { await tamber.close(); }
});

test('the GET takes a token and the POST read\'s scope; without one it is not found, and other methods stay the POST form\'s', async () => {
  const { gate, tamber, headers } = setup();
  try {
    expect((await ask(tamber, gate, { path: '/__tamber/events', headers: { host: headers.host } })).status).toBe(404);
    expect((await ask(tamber, gate, { path: '/__tamber/events', headers: { ...headers, 'x-exact-tamber-token': 'wrong' } })).status).toBe(404);
    // A GET elsewhere on the wire is still nothing.
    expect((await ask(tamber, gate, { path: '/__tamber/subscribe', headers })).status).toBe(404);
    expect((await ask(tamber, gate, { path: '/__tamber', headers })).status).toBe(404);
  } finally { await tamber.close(); }
});

test('a client is present 90 s after its GET long poll is answered, a POST client 35 s', async () => {
  const { gate, tamber, headers, get } = setup();
  const post = body => ask(tamber, gate, { path: '/__tamber', method: 'POST', headers, body: JSON.stringify(body) });
  const frames = [];
  const build = tamber.channel.open(f => frames.push(f));
  tamber.channel.report(JSON.stringify({ nonce: build.nonce, kind: 'register', id: 'web-1', host: 'web', capabilities: ['pin'] }));
  const realNow = Date.now;
  const at = async (ms, f) => { Date.now = () => realNow() + ms; try { return await f(); } finally { Date.now = realNow; } };
  try {
    // The GET poller: still present 60 s on (past the POST form's 35 s), its pin kept; 91 s on, gone.
    expect((await get('?client=longpoll')).status).toBe(200);
    expect((await post({ method: 'pin', client: 'longpoll', build: 'web-1', pin: { scheme: 'dark' } })).status).toBe(200);
    expect((await at(60_000, () => post({ method: 'pin', client: 'longpoll', build: 'web-1', pin: { scheme: 'light' } }))).status).toBe(200);
    await at(91_000, () => post({ method: 'hello' }));
    expect(frames.at(-1)).toMatchObject({ kind: 'pin', pin: null });
    // The POST poller: gone 36 s on.
    // (Its answer has a cursor too, for a stream to open from.)
    const posted = await ask(tamber, gate, { path: '/__tamber/events', method: 'POST', headers, body: JSON.stringify({ wait: false, client: 'poster' }) });
    expect([posted.status, posted.body.cursor]).toEqual([200, `${posted.body.seq}.${posted.body.held}`]);
    expect((await at(36_000, () => post({ method: 'pin', client: 'poster', build: 'web-1', pin: { scheme: 'dark' } }))).status).toBe(409);
  } finally { await tamber.close(); }
});
