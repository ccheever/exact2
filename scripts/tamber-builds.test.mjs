// The build channel (LLP 1122 D4, LLP 1121 stage 4 commit 5): registration
// by nonce and the build list, a report without the nonce refused, `pin` on
// one build only and its effective facts reported, a client's pins cleared
// when it goes and replayed to a build that registers again, a command
// never read as a plan push, and a polled stream (Linux, the resident loop's
// relay), which expires and is bounded.
import { expect, test } from 'bun:test';
import { buildChannel, relayChannel } from './tamber-builds.mjs';

const stream = channel => {
  const frames = [];
  const opened = channel.open(f => frames.push(f));
  return { frames, ...opened };
};
const report = (channel, body) => channel.report(JSON.stringify(body));
const register = (channel, nonce, id, host = 'web') => report(channel, { nonce, kind: 'register', id, host, capabilities: ['pin'] });

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

test('a polled stream queues its frames, expires unpolled, and is bounded', () => {
  const realNow = Date.now;
  let now = 1_000_000;
  Date.now = () => now;
  try {
    const channel = buildChannel();
    const { nonce } = channel.openPolled();
    expect(register(channel, nonce, 'linux-1', 'linux').status).toBe(200);
    expect(channel.poll(nonce)).toEqual([{ kind: 'pin', revision: 0, pin: null }]);
    channel.pin('tool', 'linux-1', { rootFontSize: 20 });
    expect(channel.poll(nonce)).toEqual([{ kind: 'pin', revision: 1, pin: { rootFontSize: 20 } }]);
    expect(channel.poll(nonce)).toEqual([]);
    expect(channel.poll('other')).toBe(null);
    // Unpolled for 30 s: gone, its reports refused, its build off the list.
    now += 31_000;
    expect(report(channel, { nonce, kind: 'status' }).status).toBe(403);
    expect(channel.list()).toEqual([]);
    // Hellos nobody polls are bounded: the oldest go first.
    const first = channel.openPolled().nonce;
    for (let i = 0; i < 100; i++) { now += 1; channel.openPolled(); }
    expect(channel.poll(first)).toBe(null);
  } finally { Date.now = realNow; }
});

// The resident loop (`dev.mjs --serve-as`) relays the main loop's commands to
// the devices on its stream: a nonce from the main loop's channel, its frames
// polled and written on; the nonce gone, the device's stream ends.
test('the resident loop relays the main loop\'s commands onto its own stream', async () => {
  const { createServer } = await import('node:http');
  const channel = buildChannel();
  const main = createServer((req, res) => {
    const url = new URL(req.url, 'http://x');
    const answer = url.pathname === '/__dev/hello' ? { hello: true, nonce: channel.openPolled().nonce } : channel.poll(url.searchParams.get('nonce'));
    res.writeHead(answer ? 200 : 403, { 'content-type': 'application/json' });
    res.end(JSON.stringify(answer ?? { message: 'gone' }));
  });
  await new Promise(ok => main.listen(0, '127.0.0.1', ok));
  const frames = [];
  let ended = false;
  const relayed = await relayChannel(`http://127.0.0.1:${main.address().port}`, f => frames.push(f), 10, () => { ended = true; });
  // The device registers with the main loop, as its origin is that loop's.
  expect(register(channel, relayed.nonce, 'ios-sim', 'ios').status).toBe(200);
  channel.pin('tool', 'ios-sim', { scheme: 'light' });
  const until = async ok => { const end = Date.now() + 3000; while (!ok() && Date.now() < end) await Bun.sleep(10); };
  await until(() => frames.length >= 2);
  expect(frames.at(-1)).toEqual({ kind: 'pin', revision: 1, pin: { scheme: 'light' } });
  // The main loop forgets the nonce: the relay ends the device's stream.
  channel.poll('x');
  for (const b of channel.list()) expect(b.id).toBe('ios-sim');
  const realNow = Date.now;
  Date.now = () => realNow() + 60_000;
  try { await until(() => ended); } finally { Date.now = realNow; }
  expect(ended).toBe(true);
  relayed.close();
  main.close();
});
