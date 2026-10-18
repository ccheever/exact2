// The build channel (LLP 1122 D4, LLP 1121 stage 4 commit 5): registration
// by nonce and the build list, a report without the nonce refused, `pin` on
// one build only and its effective facts reported, a client's pins cleared
// when it goes and replayed to a build that registers again, a command
// never read as a plan push, a stream that is one build, and a polled stream
// (Linux, the resident loop's relay), which expires, is bounded, answers a
// poll within the hosts' limits and keeps an answer until it is
// acknowledged; a report's fields typed, and its body read safely.
import { expect, test } from 'bun:test';
import { buildChannel, relayChannel, serveCommands, serveReport, streamFrame, BEHIND } from './tamber-builds.mjs';

const stream = channel => {
  const frames = [];
  const opened = channel.open(f => frames.push(f));
  return { frames, ...opened };
};
const report = (channel, body) => channel.report(JSON.stringify(body));
const register = (channel, nonce, id, host = 'web') => report(channel, { nonce, kind: 'register', id, host, capabilities: ['pin'] });
// A polled stream's frames, each answer acknowledged as a client does.
const poller = (channel, nonce) => { let ack = 0; return () => { const a = channel.poll(nonce, ack); if (!a) return null; ack = a.seq; return a.frames; }; };

test('builds register by nonce, report up, and take pins one build at a time', () => {
  const events = [];
  const channel = buildChannel({ send: e => events.push(e) });
  const a = stream(channel), b = stream(channel);
  // An unguessable nonce.
  expect(a.nonce).toMatch(/^[0-9a-f]{32}$/);
  // No nonce, another's, or a stream gone: refused.
  expect(report(channel, { kind: 'register', id: 'web-1', host: 'web', capabilities: ['pin'] }).status).toBe(403);
  expect(report(channel, { nonce: 'nope', kind: 'register', id: 'web-1', host: 'web', capabilities: ['pin'] }).status).toBe(403);
  // A report before registering asks for it.
  expect(report(channel, { nonce: a.nonce, kind: 'status' }).status).toBe(409);
  expect(report(channel, { nonce: a.nonce, kind: 'register', id: 'web-1', host: 'web', capabilities: ['pin'], plan: 'p', design: 'd' }).status).toBe(200);
  expect(register(channel, b.nonce, 'ios-1', 'ios').status).toBe(200);
  expect(channel.list().map(x => x.id)).toEqual(['web-1', 'ios-1']);
  expect(events.at(-1).event).toBe('builds');
  // A registration is told the pin the server holds for it: none yet.
  expect(a.frames).toEqual([{ kind: 'pin', revision: 0, pin: null }]);
  // A pin on one build goes to it alone, as a command frame (never a plan push).
  const done = channel.pin('tool', 'ios-1', { scheme: 'dark', mode: { 'dark-style': 'dim' } });
  expect(done.builds).toEqual(['ios-1']);
  expect(a.frames.length).toBe(1);
  expect(b.frames.at(-1)).toEqual({ kind: 'pin', revision: done.revision, pin: { scheme: 'dark', mode: { 'dark-style': 'dim' } } });
  expect(b.frames.at(-1).epoch === undefined && b.frames.at(-1).generation === undefined).toBe(true);
  // Its effective facts come back as its status.
  expect(report(channel, { nonce: b.nonce, kind: 'status', scheme: 'dark', contrast: 'no-preference', mode: { 'dark-style': 'dim' }, rootFontSize: 16, revision: done.revision }).status).toBe(204);
  expect(events.at(-1)).toMatchObject({ event: 'build-status', build: { id: 'ios-1', status: { scheme: 'dark', revision: done.revision }, pin: { scheme: 'dark' } } });
  // A command it cannot do comes back by name.
  expect(report(channel, { nonce: a.nonce, kind: 'unsupported', what: 'pin.contrast' }).status).toBe(204);
  expect(events.at(-1)).toMatchObject({ event: 'unsupported', build: 'web-1', what: 'pin.contrast' });
  // Refused: a fact it names wrong, a build none such.
  expect(channel.pin('tool', 'ios-1', { scheme: 'sepia' }).error).toContain('light or dark');
  expect(channel.pin('tool', 'none', { scheme: 'dark' }).error).toContain('no registered build');
  // The build reconnects (a reload is a new stream): its pin is replayed.
  b.close();
  expect(channel.list().map(x => x.id)).toEqual(['web-1']);
  const again = stream(channel);
  register(channel, again.nonce, 'ios-1', 'ios');
  expect(again.frames).toEqual([{ kind: 'pin', revision: done.revision, pin: { scheme: 'dark', mode: { 'dark-style': 'dim' } } }]);
  // The client goes: its pins are handed back on every build.
  channel.clear('tool');
  expect(again.frames.at(-1)).toMatchObject({ kind: 'pin', pin: null });
  expect(channel.list().find(x => x.id === 'ios-1').pin).toBe(null);
  // A pin cleared while its build was away is cleared when it comes back.
  channel.pin('tool', 'ios-1', { scheme: 'light' });
  again.close();
  channel.clear('tool');
  const back = stream(channel);
  register(channel, back.nonce, 'ios-1', 'ios');
  expect(back.frames).toEqual([expect.objectContaining({ kind: 'pin', pin: null })]);
});

test('each listed build names the revision of the last pin frame it was sent, whoever sent it, so a client sees when its status catches up', () => {
  const events = [];
  const channel = buildChannel({ send: e => events.push(e) });
  const a = stream(channel), b = stream(channel);
  register(channel, a.nonce, 'web-1');
  register(channel, b.nonce, 'ios-1', 'ios');
  const row = id => events.findLast(e => e.event === 'builds').builds.find(x => x.id === id);
  expect(row('web-1')).toMatchObject({ pin: null, pinRevision: 0, status: null });
  // One client pins one build: that build's revision moves, announced with
  // the pin, before its status says it applied it.
  const one = channel.pin('tool', 'web-1', { scheme: 'dark' });
  expect(row('web-1')).toMatchObject({ pin: { scheme: 'dark' }, pinRevision: one.revision });
  expect(row('ios-1').pinRevision).toBe(0);
  // Another client's pin on every build: each names it.
  const all = channel.pin('agent', null, { scheme: 'light' });
  expect([row('web-1').pinRevision, row('ios-1').pinRevision]).toEqual([all.revision, all.revision]);
  // A hand-back is a frame too, its revision the clear's.
  channel.clear('agent');
  const cleared = row('web-1').pinRevision;
  expect(cleared).toBeGreaterThan(all.revision);
  expect(row('web-1').pin).toBe(null);
  expect(a.frames.at(-1)).toEqual({ kind: 'pin', revision: cleared, pin: null });
  // A reload is told its pin again, at that frame's revision, its status
  // not yet reported.
  channel.pin('tool', 'ios-1', { scheme: 'dark' });
  const held = row('ios-1').pinRevision;
  b.close();
  const back = stream(channel);
  register(channel, back.nonce, 'ios-1', 'ios');
  expect(row('ios-1')).toMatchObject({ pinRevision: held, status: null, pin: { scheme: 'dark' } });
  expect(back.frames).toEqual([{ kind: 'pin', revision: held, pin: { scheme: 'dark' } }]);
});

test('a build\'s last refusal is on its record, the pin\'s it reports next, until its status moves to another revision', () => {
  const events = [];
  const channel = buildChannel({ send: e => events.push(e) });
  const a = stream(channel);
  register(channel, a.nonce, 'web-1');
  const row = () => channel.list()[0];
  const status = revision => report(channel, { nonce: a.nonce, kind: 'status', scheme: 'dark', contrast: 'no-preference', mode: {}, rootFontSize: 16, revision });
  // Contrast pinned at 1, a scheme at 2 sent before the web answers 1.
  const one = channel.pin('tool', 'web-1', { contrast: 'more' }).revision;
  const two = channel.pin('tool', 'web-1', { scheme: 'dark' }).revision;
  report(channel, { nonce: a.nonce, kind: 'unsupported', what: 'pin.contrast', detail: 'no' });
  expect(row().unsupported).toEqual({ what: 'pin.contrast', detail: 'no' });
  // Its status for 1 follows: the refusal is 1's, and stays while it is.
  status(one);
  expect(events.at(-1).build.unsupported).toEqual({ what: 'pin.contrast', detail: 'no' });
  status(one);
  expect(row().unsupported).not.toBe(null);
  // It applies 2: the refusal was not 2's.
  status(two);
  expect(row().unsupported).toBe(null);
  expect(events.at(-1).build.unsupported).toBe(null);
  // A late status (1 again) cannot rewind the row; a late refusal for 1
  // is old news.
  status(one);
  expect(row().status.revision).toBe(two);
  report(channel, { nonce: a.nonce, kind: 'unsupported', what: 'pin.contrast', revision: one });
  expect(row().unsupported).toBe(null);
  // A refusal that names its revision is that frame's, whichever status
  // lands first.
  const three = channel.pin('tool', 'web-1', { contrast: 'less' }).revision;
  status(three);
  report(channel, { nonce: a.nonce, kind: 'unsupported', what: 'pin.contrast', detail: 'no', revision: three });
  expect(row().unsupported).toEqual({ what: 'pin.contrast', detail: 'no' });
  // A revision this registration was never sent acknowledges nothing (a
  // report from before a server restart, say).
  status(three + 5);
  expect(row().status.revision).toBe(null);
});

test('a build with no pin is told so at revision 0, and a pin to no build sends nothing', () => {
  const channel = buildChannel();
  const a = stream(channel);
  register(channel, a.nonce, 'web-1');
  channel.pin('tool', 'web-1', { scheme: 'dark' });
  channel.clear('tool');
  // A later build has no pin: revision 0, already caught up when it says so.
  const b = stream(channel);
  register(channel, b.nonce, 'ios-1', 'ios');
  expect(b.frames).toEqual([{ kind: 'pin', revision: 0, pin: null }]);
  expect(channel.list().find(x => x.id === 'ios-1').pinRevision).toBe(0);
  a.close(); b.close();
  const before = channel.pin('tool', null, null).revision;
  expect(channel.pin('tool', null, { scheme: 'dark' })).toEqual({ builds: [], revision: before });
});

test('a polled stream queues its frames, expires unpolled, and is bounded', () => {
  const realNow = Date.now;
  let now = 1_000_000;
  Date.now = () => now;
  try {
    const events = [];
    const channel = buildChannel({ send: e => events.push(e) });
    const { nonce } = channel.openPolled(), poll = poller(channel, nonce);
    expect(register(channel, nonce, 'linux-1', 'linux').status).toBe(200);
    expect(poll()).toEqual([{ kind: 'pin', revision: 0, pin: null }]);
    channel.pin('tool', 'linux-1', { rootFontSize: 20 });
    expect(poll()).toEqual([{ kind: 'pin', revision: 1, pin: { rootFontSize: 20 } }]);
    expect(poll()).toEqual([]);
    expect(channel.poll('other')).toBe(null);
    // Unpolled for 5 s: gone with nothing else on the channel. The list
    // says so, and the server's timer (`sweep`) tells the clients.
    now += 4_000;
    expect(channel.list().map(b => b.id)).toEqual(['linux-1']);
    now += 1_001;
    channel.sweep();
    expect(events.at(-1)).toEqual({ event: 'builds', builds: [] });
    expect(channel.list()).toEqual([]);
    expect(report(channel, { nonce, kind: 'status' }).status).toBe(403);
    // Hellos nobody polls are bounded: the oldest go first.
    const first = channel.openPolled().nonce;
    for (let i = 0; i < 100; i++) { now += 1; channel.openPolled(); }
    expect(channel.poll(first)).toBe(null);
    expect(channel.list()).toEqual([]);
  } finally { Date.now = realNow; }
});

test('a poll\'s answer is kept until acknowledged: a lost response loses no command', () => {
  const channel = buildChannel();
  const { nonce } = channel.openPolled();
  register(channel, nonce, 'linux-1', 'linux');
  // The first answer is seq 1; a poll that does not acknowledge it (its
  // response lost: `ack` still 0, or none) gets it again, the same.
  const first = channel.poll(nonce, 0);
  expect(first).toEqual({ seq: 1, frames: [{ kind: 'pin', revision: 0, pin: null }] });
  channel.pin('tool', 'linux-1', { scheme: 'dark' });
  expect(channel.poll(nonce)).toEqual(first);
  expect(channel.poll(nonce, '0')).toEqual(first);
  expect(channel.poll(nonce, 'junk')).toEqual(first);
  // Acknowledged: the next frames, seq 2, kept in turn.
  const second = channel.poll(nonce, 1);
  expect(second).toEqual({ seq: 2, frames: [{ kind: 'pin', revision: 1, pin: { scheme: 'dark' } }] });
  expect(channel.poll(nonce, 1)).toEqual(second);
  expect(channel.poll(nonce, 2)).toEqual({ seq: 3, frames: [] });
});

test('a long poll (`wait`) is held until a frame comes, or its wait passes; a newer one answers the older at once', async () => {
  const channel = buildChannel();
  const { nonce } = channel.openPolled();
  register(channel, nonce, 'linux-1', 'linux');
  // Its pin frame is queued: answered at once.
  const first = await channel.waitPoll(nonce, 0, 4000);
  expect(first.frames).toEqual([{ kind: 'pin', revision: 0, pin: null }]);
  // Nothing queued: held until the pin comes.
  const t0 = Date.now();
  const held = channel.waitPoll(nonce, first.seq, 4000);
  setTimeout(() => channel.pin('tool', 'linux-1', { scheme: 'dark' }), 30);
  const second = await held;
  expect(second.frames).toEqual([{ kind: 'pin', revision: 1, pin: { scheme: 'dark' } }]);
  expect(Date.now() - t0).toBeLessThan(1000);
  // Its wait passes: an answer with no frames.
  expect((await channel.waitPoll(nonce, second.seq, 40)).frames).toEqual([]);
  // One held per stream: a newer poll answers the older now.
  const older = channel.waitPoll(nonce, 3, 4000);
  const newer = channel.waitPoll(nonce, 3, 50);
  expect((await older).frames).toEqual([]);
  await newer;
  // Its stream gone: null at once.
  expect(await channel.waitPoll('none', 0, 4000)).toBe(null);
});

test('a polled queue is bounded: one pin queued, live frames collapsed into a notice', () => {
  const channel = buildChannel();
  const { nonce } = channel.openPolled();
  // Registrations again and again (each told its pin), and pins: only the
  // latest pin is queued.
  for (let i = 0; i < 10_000; i++) register(channel, nonce, 'linux-1', 'linux');
  for (let i = 0; i < 100; i++) channel.pin('tool', 'linux-1', { rootFontSize: 10 + (i % 20) });
  expect(channel.poll(nonce).frames).toEqual([{ kind: 'pin', revision: 100, pin: { rootFontSize: 29 } }]);
  // Batches past the bound: one notice of the latest design, beside the pin.
  const batch = i => ({ kind: 'tamber-batch', structure: 's', base: `d${i}`, design: `d${i + 1}`, tokens: [{ name: 't', value: 'x'.repeat(4000), slices: [] }] });
  for (let i = 0; i < 1000; i++) channel.broadcast(batch(i));
  channel.pin('tool', 'linux-1', null);
  const frames = [];
  for (let ack = 1, a; (a = channel.poll(nonce, ack)).frames.length; ack = a.seq) frames.push(...a.frames);
  expect(frames.length).toBeLessThan(64);
  expect(frames.filter(f => f.kind === 'pin')).toEqual([{ kind: 'pin', revision: 101, pin: null }]);
  // The batches before the last collapse are its notice; those after follow
  // on from it, up to the latest.
  const lives = frames.filter(f => f.kind !== 'pin');
  expect(lives[0].tokens).toEqual([]);
  expect(lives[0].base).toBe(lives[0].design);
  for (let i = 1; i < lives.length; i++) expect(lives[i].base).toBe(lives[i - 1].design);
  expect(lives.at(-1).design).toBe('d1000');
  // Under the bound, batches go as they are, in order.
  for (let i = 0; i < 3; i++) channel.broadcast(batch(i));
  expect(channel.poll(nonce, 999).frames).toEqual([0, 1, 2].map(batch));
});

test('streams are capped: past 256 none has a nonce, a hello is 503, polled at most 64', () => {
  const channel = buildChannel();
  const hello = () => { const out = {}; serveCommands(channel, new URL('http://x/__dev/hello'), { writeHead: (s) => { out.status = s; }, end: (b) => { out.body = JSON.parse(b); } }); return out; };
  // Polled hellos past 64: the oldest gives way.
  const polled = Array.from({ length: 100 }, () => hello().body.nonce);
  expect(polled.filter(n => channel.poll(n)).length).toBe(64);
  const pages = Array.from({ length: 300 }, () => channel.open(() => {}));
  expect(pages.filter(Boolean).length).toBe(256 - 64);
  expect(pages.at(-1)).toBe(null);
  // So connected builds are capped: every stream registered, and no more.
  const nonces = [...polled.filter(n => channel.poll(n)), ...pages.filter(Boolean).map(p => p.nonce)];
  nonces.forEach((n, i) => expect(register(channel, n, `b-${i}`).status).toBe(200));
  expect(channel.list().length).toBe(256);
  // Full of pages: a hello is 503, and one closed makes room.
  const full = buildChannel(), held = Array.from({ length: 256 }, () => full.open(() => {}));
  const refused = {};
  serveCommands(full, new URL('http://x/__dev/hello'), { writeHead: (s) => { refused.status = s; }, end() {} });
  expect(refused.status).toBe(503);
  held[0].close();
  expect(full.openPolled()).not.toBe(null);
  // A gone nonce's poll: 404, `null`.
  const gone = {};
  serveCommands(channel, new URL('http://x/__dev/commands?nonce=gone&ack=3'), { writeHead: (s) => { gone.status = s; }, end: (b) => { gone.body = b; } });
  expect(gone).toEqual({ status: 404, body: 'null' });
});

test('a report\'s fields are typed and bounded: a hostile one is refused, never coerced', () => {
  const events = [];
  const channel = buildChannel({ send: e => events.push(e), log: () => {} });
  const { nonce } = channel.open(() => {});
  const hostile = { toString: null, valueOf: null };
  for (const body of [
    [1], null, 'x', { nonce, kind: hostile }, { nonce, kind: 'register', id: 'w', host: hostile, capabilities: [] },
    { nonce, kind: 'register', id: 'w', host: 'web', capabilities: [hostile] }, { nonce, kind: 'register', id: 'w', host: 'web', capabilities: [], plan: hostile },
  ]) expect(report(channel, body).status).toBe(400);
  expect(register(channel, nonce, 'web-1').status).toBe(200);
  for (const body of [
    { kind: 'unsupported', what: hostile }, { kind: 'unsupported', what: 'x', detail: { a: 1 } }, { kind: 'unsupported', what: 'x'.repeat(300) },
    { kind: 'status', scheme: hostile }, { kind: 'status', mode: { a: hostile } }, { kind: 'status', mode: [] }, { kind: 'status', rootFontSize: '16' },
    { kind: 'status', revision: -1 }, { kind: 'status', design: 7 },
  ]) expect(report(channel, { nonce, ...body }).status).toBe(400);
  expect(events.filter(e => e.event === 'unsupported' || e.event === 'build-status')).toEqual([]);
  expect(report(channel, { nonce, kind: 'unsupported', what: 'pin.contrast', detail: null }).status).toBe(204);
  expect(report(channel, { nonce, kind: 'status', scheme: 'dark', contrast: null, mode: { a: 'b' }, rootFontSize: 16, revision: 0 }).status).toBe(204);
});

test('a stream is one build: registering another id replaces it, so the cap holds', () => {
  const channel = buildChannel();
  const a = stream(channel);
  expect(register(channel, a.nonce, 'web-0').status).toBe(200);
  channel.pin('tool', 'web-0', { scheme: 'dark' });
  for (let i = 1; i <= 1000; i++) register(channel, a.nonce, `web-${i}`);
  expect(channel.list().map(b => b.id)).toEqual(['web-1000']);
  // The build it was is gone, its pin with it: registering it again has none.
  expect(channel.owners()).toEqual([]);
  // The cap still forgets the oldest disconnected builds.
  for (let i = 0; i < 300; i++) { const s = stream(channel); register(channel, s.nonce, `b-${i}`); s.close(); }
  expect(register(channel, stream(channel).nonce, 'b-0').status).toBe(200);
  expect(channel.list().map(b => b.id)).toEqual(['web-1000', 'b-0']);
});

test('a poll answers at most 32 KiB: a frame over it as a notice, the rest queued', () => {
  const channel = buildChannel();
  const { nonce } = channel.openPolled();
  const token = (i, n) => ({ name: `t${i}`, value: 'x'.repeat(n), slices: [] });
  const big = { kind: 'tamber-batch', structure: 's', base: 'd0', design: 'd1', tokens: [token(0, 40_000)] };
  channel.broadcast(big);
  expect(channel.poll(nonce).frames).toEqual([{ kind: 'tamber-batch', structure: 's', base: 'd1', design: 'd1', tokens: [] }]);
  const mid = i => ({ kind: 'tamber-batch', structure: 's', base: `d${i}`, design: `d${i + 1}`, tokens: [token(i, 12_000)] });
  for (let i = 0; i < 5; i++) channel.broadcast(mid(i));
  const polls = [], poll = poller(channel, nonce);
  poll();
  for (let frames; (frames = poll()).length;) polls.push(frames);
  for (const frames of polls) expect(Buffer.byteLength(JSON.stringify(frames))).toBeLessThanOrEqual(32 * 1024);
  expect(polls.flat()).toEqual([0, 1, 2, 3, 4].map(mid));
  expect(polls.length).toBe(3);
});

test('a report\'s body: read within its bound, an abort or an oversize body survived', async () => {
  const { createServer, request } = await import('node:http');
  const channel = buildChannel();
  const server = createServer((req, res) => serveReport(channel, req, res));
  await new Promise(ok => server.listen(0, '127.0.0.1', ok));
  const at = `http://127.0.0.1:${server.address().port}/__dev/report`;
  const send = (body, abort = false) => new Promise((ok) => {
    const out = request(at, { method: 'POST', headers: { 'content-length': String(1 << 20) } }, (res) => { res.resume(); ok(res.statusCode); });
    out.on('error', () => ok('reset'));
    out.write(body);
    if (abort) setTimeout(() => { out.destroy(); ok('aborted'); }, 20);
  });
  // Aborted midway: the server goes on.
  expect(await send('{"nonce":', true)).toBe('aborted');
  // Over 64 KiB: its socket destroyed.
  expect(await send('x'.repeat(70 * 1024))).toBe('reset');
  const { nonce } = channel.open(() => {});
  const answer = await fetch(at, { method: 'POST', body: JSON.stringify({ nonce, kind: 'register', id: 'web-1', host: 'web', capabilities: [] }) });
  expect(answer.status).toBe(200);
  // A hostile report is 400, and one that throws in its handling 500: the
  // server goes on either way.
  const hostile = await fetch(at, { method: 'POST', body: JSON.stringify({ nonce, kind: 'unsupported', what: { toString: null, valueOf: null } }) });
  expect(hostile.status).toBe(400);
  const throwing = createServer((req, res) => serveReport({ report: () => { throw new Error('boom'); } }, req, res));
  await new Promise(ok => throwing.listen(0, '127.0.0.1', ok));
  expect((await fetch(`http://127.0.0.1:${throwing.address().port}/__dev/report`, { method: 'POST', body: '{}' })).status).toBe(500);
  throwing.close();
  server.close();
});

// The resident loop (`dev.mjs --serve-as`) relays the main loop's commands to
// the devices on its stream: a nonce from the main loop's channel, its frames
// polled and written on; the nonce gone, the device's stream ends.
test('the resident loop relays the main loop\'s commands onto its own stream', async () => {
  const { createServer } = await import('node:http');
  const channel = buildChannel();
  // The first answer with frames is lost on the way (the connection reset
  // after the server answered): the relay asks again and loses nothing.
  // Each poll is a long one (`wait`), held until a frame comes.
  let lost = false, waits = 0;
  const main = createServer(async (req, res) => {
    const url = new URL(req.url, 'http://x');
    if (url.searchParams.has('wait')) waits++;
    if (!lost && url.pathname === '/__dev/commands') {
      const answer = await channel.waitPoll(url.searchParams.get('nonce'), url.searchParams.get('ack'), url.searchParams.get('wait'));
      if (answer?.frames.length) { lost = true; res.destroy(); return; }
      res.writeHead(answer ? 200 : 404, { 'content-type': 'application/json' });
      res.end(JSON.stringify(answer));
      return;
    }
    serveCommands(channel, url, res);
  });
  await new Promise(ok => main.listen(0, '127.0.0.1', ok));
  const frames = [];
  let ended = false;
  const relayed = await relayChannel(`http://127.0.0.1:${main.address().port}`, f => frames.push(f), 10, () => { ended = true; });
  // The device registers with the main loop, as its origin is that loop's.
  expect(register(channel, relayed.nonce, 'ios-sim', 'ios').status).toBe(200);
  channel.pin('tool', 'ios-sim', { scheme: 'light' });
  const until = async ok => { const end = Date.now() + 3000; while (!ok() && Date.now() < end) await Bun.sleep(10); };
  await until(() => frames.at(-1)?.revision === 1);
  // (A pin replaced before it was polled is not sent: only the latest is.)
  expect(lost).toBe(true);
  expect(waits).toBeGreaterThan(0);
  expect(frames.at(-1)).toEqual({ kind: 'pin', revision: 1, pin: { scheme: 'light' } });
  // The main loop forgets the nonce: the relay ends the device's stream.
  channel.poll('x');
  for (const b of channel.list()) expect(b.id).toBe('ios-sim');
  const realNow = Date.now;
  Date.now = () => realNow() + 60_000;
  // Swept then: the poll held there is answered with the stream gone.
  channel.poll('x');
  try { await until(() => ended); } finally { Date.now = realNow; }
  expect(ended).toBe(true);
  relayed.close();
  main.close();
});

test('inspect goes to the builds that can, refresh asks for the selection again, and it is turned off when its client goes', () => {
  const channel = buildChannel();
  const a = stream(channel), b = stream(channel);
  report(channel, { nonce: a.nonce, kind: 'register', id: 'web-1', host: 'web', capabilities: ['pin', 'inspect'] });
  register(channel, b.nonce, 'ios-1', 'ios');
  expect(channel.inspect('tool', null, { on: true })).toEqual({ builds: ['web-1'], unsupported: ['ios-1'] });
  expect(a.frames.at(-1)).toEqual({ kind: 'inspect', on: true, seq: 1 });
  expect(b.frames.some(f => f.kind === 'inspect')).toBe(false);
  expect(channel.owners()).toEqual(['tool']);
  expect(channel.inspect('tool', 'web-1', { refresh: true }).builds).toEqual(['web-1']);
  expect(a.frames.at(-1)).toEqual({ kind: 'inspect', refresh: true });
  expect(channel.inspect('tool', 'none', { on: true }).error).toContain('no registered build');
  channel.clear('tool');
  expect(a.frames.at(-1)).toEqual({ kind: 'inspect', on: false, seq: 2 });
  expect(channel.owners()).toEqual([]);
  // A page's own exit (a selection of none) drops the owner only for the
  // command it left: a later owner's stays.
  channel.inspect('tool', 'web-1', { on: true });
  channel.inspect('agent', 'web-1', { on: true });
  expect(report(channel, { nonce: a.nonce, kind: 'selected', selected: null, inspect: 3 }).status).toBe(204);
  expect(channel.owners()).toEqual(['agent']);
  report(channel, { nonce: a.nonce, kind: 'selected', selected: null, inspect: 4 });
  expect(channel.owners()).toEqual([]);
  // Registering again, a build is told inspect mode as the server holds it.
  channel.inspect('tool', 'web-1', { on: true });
  const again = stream(channel);
  report(channel, { nonce: again.nonce, kind: 'register', id: 'web-1', host: 'web', capabilities: ['pin', 'inspect'] });
  expect(again.frames).toContainEqual({ kind: 'inspect', on: true, seq: 5 });
  // Turned off everywhere while the build is away: it registers again
  // told off, not on.
  again.close();
  expect(channel.inspect('tool', null, { on: false }).builds).toEqual([]);
  const back = stream(channel);
  report(channel, { nonce: back.nonce, kind: 'register', id: 'web-1', host: 'web', capabilities: ['pin', 'inspect'] });
  expect(back.frames).toContainEqual({ kind: 'inspect', on: false, seq: 6 });
  expect(channel.owners()).toEqual([]);
});

test('inspect mode\'s owners are published as they move: a command, another client taking over, a page\'s own exit, the client gone', () => {
  const events = [];
  const channel = buildChannel({ send: e => events.push(e) });
  const a = stream(channel), b = stream(channel);
  report(channel, { nonce: a.nonce, kind: 'register', id: 'web-1', host: 'web', capabilities: ['pin', 'inspect'] });
  report(channel, { nonce: b.nonce, kind: 'register', id: 'web-2', host: 'web', capabilities: ['pin', 'inspect'] });
  const told = () => events.splice(0).filter(e => e.event === 'inspect');
  told();
  channel.inspect('tool', null, { on: true });
  expect(told()).toEqual([{ event: 'inspect', client: 'tool', on: true, builds: ['web-1', 'web-2'] }]);
  // Again, nothing moved: nothing said.
  channel.inspect('tool', null, { on: true });
  expect(told()).toEqual([]);
  // Another client takes one.
  channel.inspect('agent', 'web-2', { on: true });
  expect(told()).toEqual([{ event: 'inspect', client: 'tool', on: false, builds: ['web-2'], to: 'agent' }, { event: 'inspect', client: 'agent', on: true, builds: ['web-2'] }]);
  // The page leaves inspect mode itself.
  const seq = a.frames.filter(f => f.kind === 'inspect').at(-1).seq;
  report(channel, { nonce: a.nonce, kind: 'selected', selected: null, inspect: seq });
  expect(told()).toEqual([{ event: 'inspect', client: 'tool', on: false, builds: ['web-1'], to: null }]);
  // The client gone (a presence lapse): its builds are off, said.
  channel.clear('agent');
  expect(told()).toEqual([{ event: 'inspect', client: 'agent', on: false, builds: ['web-2'], to: null }]);
  // Each record names its owner; off everywhere is off on a build away too.
  channel.inspect('tool', null, { on: true });
  told();
  expect(channel.list().map(b => [b.id, b.inspector])).toEqual([['web-1', 'tool'], ['web-2', 'tool']]);
  b.close();
  channel.inspect('tool', null, { on: false });
  expect(told()).toEqual([{ event: 'inspect', client: 'tool', on: false, builds: ['web-1', 'web-2'], to: null }]);
  // A refresh moves nothing.
  channel.inspect('tool', null, { refresh: true });
  expect(told()).toEqual([]);
});

test('a build says the digest of the plan it runs, on registering and in its statuses', () => {
  const channel = buildChannel();
  const a = stream(channel);
  expect(report(channel, { nonce: a.nonce, kind: 'register', id: 'web-1', host: 'web', capabilities: ['pin'], planDigest: 'not hex' }).status).toBe(400);
  report(channel, { nonce: a.nonce, kind: 'register', id: 'web-1', host: 'web', capabilities: ['pin'], planDigest: '1'.repeat(64) });
  expect(channel.list()[0].planDigest).toBe('1'.repeat(64));
  report(channel, { nonce: a.nonce, kind: 'status', scheme: 'light', revision: 0, planDigest: '2'.repeat(64) });
  expect(channel.list()[0].planDigest).toBe('2'.repeat(64));
});

test('a scrub frame collapsed or over the bound keeps its mark in the notice (*1.11*)', () => {
  const channel = buildChannel();
  const { nonce } = channel.openPolled();
  register(channel, nonce, 'linux-1', 'linux');
  channel.poll(nonce);
  const frame = i => ({ kind: 'tamber-batch', structure: 's', base: `d${i}`, design: `d${i + 1}`, tokens: [{ name: 't', value: 'x'.repeat(40_000), slices: [] }], scrub: { id: 'scrub-1', rev: i + 1, run: 'r' } });
  channel.broadcast(frame(0));
  const [f] = channel.poll(nonce, 1).frames.filter(f => f.kind === 'tamber-batch');
  expect(f.tokens).toEqual([]);
  expect(f.scrub).toEqual({ id: 'scrub-1', rev: 1, run: 'r' });
});

test('a stream behind takes no frames until it drains, then the latest design\'s notice', () => {
  const written = [];
  let drain = null;
  const res = { writableLength: 0, write: t => written.push(t), once: (e, f) => { if (e === 'drain') drain = f; } };
  streamFrame(res, '{"n":1}', () => ({ n: 'latest' }));
  res.writableLength = BEHIND + 1;
  streamFrame(res, '{"n":2}', () => ({ n: 'latest' }));
  streamFrame(res, '{"n":3}', () => ({ n: 'latest' }));
  expect(written).toEqual(['data: {"n":1}\n\n']);
  res.writableLength = 0;
  drain();
  expect(written).toEqual(['data: {"n":1}\n\n', 'data: {"n":"latest"}\n\n']);
  streamFrame(res, '{"n":4}', () => null);
  expect(written.at(-1)).toBe('data: {"n":4}\n\n');
});

// LLP 1122 D8, LLP 1121 stage 8 E (*1.11*): a hold, one a build and its
// client's, sent as a command frame; refused where it is malformed and told
// where the build holds no state; its build's `held` reports go to every
// client; a build that lets it go (real input) is not held any more; replayed
// on a reconnect under the same plan, at the one corresponding site under a
// new plan (a rebuild on the same stream), else cleared with a diagnostic;
// let go when its client goes.
test('a hold is one a build, its client\'s, replayed only through one corresponding site', () => {
  const events = [];
  const moves = new Map([[`${'1'.repeat(64)}>${'2'.repeat(64)}:3`, 5]]);
  const channel = buildChannel({ send: e => events.push(e), correspond: (from, to, site) => moves.get(`${from}>${to}:${site}`) ?? null });
  const a = stream(channel), b = stream(channel);
  const plan = (n) => n.repeat(64);
  report(channel, { nonce: a.nonce, kind: 'register', id: 'web-1', host: 'web', capabilities: ['pin', 'hold'], planDigest: plan('1') });
  register(channel, b.nonce, 'term-1', 'terminal');
  // Refused: malformed; told: a build that holds no state.
  expect(channel.hold('tool', 'web-1', { site: 3, instance: [], state: 'pressed' }).error).toContain('state');
  expect(channel.hold('tool', 'web-1', { site: -1, instance: [], state: 'active' }).error).toContain('site');
  expect(channel.hold('tool', 'term-1', { site: 3, instance: [], state: 'active' }).unsupported).toContain('holds no state');
  const held = channel.hold('tool', 'web-1', { site: 3, instance: [[0, 1]], state: 'active' }, plan('1'));
  expect(a.frames.at(-1)).toEqual({ kind: 'hold', revision: held.revision, hold: { site: 3, instance: [[0, 1]], state: 'active' } });
  // A new one replaces it.
  const hover = channel.hold('other', 'web-1', { site: 3, instance: [[0, 1]], state: 'hover' }, plan('1'));
  expect(channel.list()[0].hold).toEqual({ site: 3, instance: [[0, 1]], state: 'hover' });
  expect(channel.owners()).toContain('other');
  // Its build reports it applied, to every client.
  expect(report(channel, { nonce: a.nonce, kind: 'held', state: 'hover', revision: hover.revision }).status).toBe(204);
  expect(events.at(-1)).toEqual({ event: 'held', build: 'web-1', state: 'hover', reason: null });
  // A reconnect under the same plan: replayed as it is.
  a.close();
  const a2 = stream(channel);
  report(channel, { nonce: a2.nonce, kind: 'register', id: 'web-1', host: 'web', capabilities: ['pin', 'hold'], planDigest: plan('1') });
  expect(a2.frames.at(-1)).toEqual({ kind: 'hold', revision: hover.revision, hold: { site: 3, instance: [[0, 1]], state: 'hover' } });
  // A rebuild on the same stream (a new planDigest): the one site that corresponds.
  report(channel, { nonce: a2.nonce, kind: 'status', planDigest: plan('2'), revision: 0 });
  expect(a2.frames.at(-1)).toEqual({ kind: 'hold', revision: hover.revision, hold: { site: 5, instance: [[0, 1]], state: 'hover' } });
  // Another rebuild with no single corresponding site: cleared, said so.
  report(channel, { nonce: a2.nonce, kind: 'status', planDigest: plan('3'), revision: 0 });
  expect(events.find(e => e.event === 'hold-cleared')).toMatchObject({ build: 'web-1', client: 'other' });
  expect(channel.list()[0].hold).toBe(null);
  // Real input lets it go: not held any more, nor replayed.
  const again = channel.hold('tool', 'web-1', { site: 5, instance: [], state: 'active' }, plan('3'));
  report(channel, { nonce: a2.nonce, kind: 'held', state: null, reason: 'input', revision: again.revision });
  expect(events.at(-1)).toEqual({ event: 'held', build: 'web-1', state: null, reason: 'input' });
  expect(channel.list()[0].hold).toBe(null);
  // The client goes: its hold is let go.
  channel.hold('tool', 'web-1', { site: 5, instance: [], state: 'focus-visible' }, plan('3'));
  channel.clear('tool');
  expect(a2.frames.at(-1)).toMatchObject({ kind: 'hold', hold: null });
  // A held report's fields are typed.
  expect(report(channel, { nonce: a2.nonce, kind: 'held', state: 'pressed' }).status).toBe(400);
});
