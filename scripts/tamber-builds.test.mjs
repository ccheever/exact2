// The build channel (LLP 1122 D4, LLP 1121 stage 4 commit 5): registration
// by nonce and the build list, a report without the nonce refused, `pin` on
// one build only and its effective facts reported, a client's pins cleared
// when it goes and replayed to a build that registers again, a command
// never read as a plan push, and a polled stream (Linux, the resident loop's
// relay).
import { expect, test } from 'bun:test';
import { buildChannel } from './tamber-builds.mjs';

const stream = channel => {
  const frames = [];
  const opened = channel.open(f => frames.push(f));
  return { frames, ...opened };
};
const report = (channel, body) => channel.report(JSON.stringify(body));

test('builds register by nonce, report up, and take pins one build at a time', () => {
  const events = [];
  const channel = buildChannel({ send: e => events.push(e) });
  const a = stream(channel), b = stream(channel);
  // No nonce, another's, or a stream gone: refused.
  expect(report(channel, { kind: 'register', id: 'web-1', host: 'web', capabilities: ['pin'] }).status).toBe(403);
  expect(report(channel, { nonce: 'nope', kind: 'register', id: 'web-1', host: 'web', capabilities: ['pin'] }).status).toBe(403);
  // A report before registering asks for it.
  expect(report(channel, { nonce: a.nonce, kind: 'status' }).status).toBe(409);
  expect(report(channel, { nonce: a.nonce, kind: 'register', id: 'web-1', host: 'web', capabilities: ['pin'], plan: 'p', design: 'd' }).status).toBe(200);
  expect(report(channel, { nonce: b.nonce, kind: 'register', id: 'ios-1', host: 'ios', capabilities: ['pin'] }).status).toBe(200);
  expect(channel.list().map(x => x.id)).toEqual(['web-1', 'ios-1']);
  expect(events.at(-1).event).toBe('builds');
  // A pin on one build goes to it alone, as a command frame (never a plan push).
  const done = channel.pin('tool', 'ios-1', { scheme: 'dark', mode: { 'dark-style': 'dim' } });
  expect(done.builds).toEqual(['ios-1']);
  expect(a.frames).toEqual([]);
  expect(b.frames).toEqual([{ kind: 'pin', revision: done.revision, pin: { scheme: 'dark', mode: { 'dark-style': 'dim' } } }]);
  expect(b.frames[0].epoch === undefined && b.frames[0].generation === undefined).toBe(true);
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
  report(channel, { nonce: again.nonce, kind: 'register', id: 'ios-1', host: 'ios', capabilities: ['pin'] });
  expect(again.frames).toEqual([{ kind: 'pin', revision: done.revision, pin: { scheme: 'dark', mode: { 'dark-style': 'dim' } } }]);
  // The client goes: its pins are handed back on every build.
  channel.clear('tool');
  expect(again.frames.at(-1)).toMatchObject({ kind: 'pin', pin: null });
  expect(channel.list().find(x => x.id === 'ios-1').pin).toBe(null);
});

test('a polled stream queues its frames until it polls', () => {
  const channel = buildChannel();
  const { nonce } = channel.openPolled();
  expect(report(channel, { nonce, kind: 'register', id: 'linux-1', host: 'linux', capabilities: ['pin'] }).status).toBe(200);
  channel.pin('tool', 'linux-1', { rootFontSize: 20 });
  expect(channel.poll(nonce)).toEqual([{ kind: 'pin', revision: 1, pin: { rootFontSize: 20 } }]);
  expect(channel.poll(nonce)).toEqual([]);
  expect(channel.poll('other')).toBe(null);
});

// The resident loop (`dev.mjs --serve-as`) relays the main loop's commands to
// the devices on its stream: a nonce from the main loop's channel, its frames
// polled and written on.
test('the resident loop relays the main loop\'s commands onto its own stream', async () => {
  const { createServer } = await import('node:http');
  const { relayChannel } = await import('./tamber-builds.mjs');
  const channel = buildChannel();
  const main = createServer((req, res) => {
    const url = new URL(req.url, 'http://x');
    const answer = url.pathname === '/__dev/hello' ? { hello: true, nonce: channel.openPolled().nonce } : channel.poll(url.searchParams.get('nonce'));
    res.writeHead(answer ? 200 : 403, { 'content-type': 'application/json' });
    res.end(JSON.stringify(answer));
  });
  await new Promise(ok => main.listen(0, '127.0.0.1', ok));
  const frames = [];
  const relayed = await relayChannel(`http://127.0.0.1:${main.address().port}`, f => frames.push(f), 10);
  // The device registers with the main loop, as its origin is that loop's.
  expect(report(channel, { nonce: relayed.nonce, kind: 'register', id: 'ios-sim', host: 'ios', capabilities: ['pin'] }).status).toBe(200);
  channel.pin('tool', 'ios-sim', { scheme: 'light' });
  const end = Date.now() + 3000;
  while (!frames.length && Date.now() < end) await Bun.sleep(10);
  expect(frames).toEqual([{ kind: 'pin', revision: 1, pin: { scheme: 'light' } }]);
  relayed.close();
  main.close();
});
