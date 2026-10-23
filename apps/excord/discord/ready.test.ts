import { expect, test } from 'bun:test';
import { startScan, step, withoutGuilds } from './ready.ts';

function scanned(text: string, budget = Infinity) {
  const scan = startScan(text);
  let calls = 0;
  while (!step(scan, budget)) calls++;
  return { spans: scan.spans, calls };
}

const tricky = {
  name: 'braces } ] { [ and "quotes" and \\ backslashes \\" and : colons',
  emoji: '🦊 日本語', nested: { guilds: [{ id: 'not this one' }], d: { guilds: [] } },
};
const ready = {
  t: 'READY', s: 1, op: 0,
  d: {
    v: 9, user: { id: '1', username: 'me "quoted" {' }, merged_members: [[{ user_id: '1', roles: ['5'] }], []],
    guilds: [
      { id: '10', name: 'first', channels: [{ id: '100', permission_overwrites: [] }], roles: [{ id: '10' }], ...tricky },
      { id: '11', name: 'second "two"', properties: { name: 'x]' }, channels: [] },
    ],
    read_state: { entries: [{ id: '100', last_message_id: '7' }] }, private_channels: [],
  },
};

test('the guilds and each one are found, whatever their text holds', () => {
  const text = JSON.stringify(ready);
  const { spans } = scanned(text);
  expect(spans).not.toBeNull();
  expect(spans!.elements.map(([from, to]) => JSON.parse(text.slice(from, to)))).toEqual(ready.d.guilds);
  expect(JSON.parse(withoutGuilds(text, spans!))).toEqual({ ...ready, d: { ...ready.d, guilds: [] } });
});

test('a scan a few characters a call finds the same spans', () => {
  const text = JSON.stringify(ready, null, 2);
  const whole = scanned(text).spans;
  const sliced = scanned(text, 5);
  expect(sliced.calls).toBeGreaterThan(50);
  expect(sliced.spans).toEqual(whole);
});

test('d may come before t, and guilds after the rest', () => {
  const { guilds, ...rest } = ready.d;
  const text = JSON.stringify({ d: { ...rest, guilds }, op: 0, s: 1, t: 'READY' });
  const { spans } = scanned(text);
  expect(spans!.elements.map(([from, to]) => JSON.parse(text.slice(from, to)).id)).toEqual(['10', '11']);
});

test('a frame without d.guilds, or one that is not JSON, has no spans', () => {
  expect(scanned(JSON.stringify({ t: 'GUILD_CREATE', s: 2, op: 0, d: { id: '10', channels: [] } })).spans).toBeNull();
  expect(scanned(JSON.stringify({ t: 'X', s: 2, op: 0, d: { inner: { guilds: [{ id: '1' }] } } })).spans).toBeNull();
  expect(scanned('{"t":"READY","d":{"guilds":[{"id":"1"}]').spans).toBeNull();
  expect(scanned('{"t":"READY","d":{"guilds":[{"id":"1}]}}').spans).toBeNull();
  expect(scanned('{"t":"READY","d":{"guilds":[{"id":"1"]}}').spans).toBeNull();
});

test('an empty guilds array has no elements', () => {
  const text = JSON.stringify({ t: 'READY', s: 1, op: 0, d: { guilds: [] } });
  expect(scanned(text).spans).toEqual({ guilds: [text.indexOf('[]'), text.indexOf('[]') + 2], elements: [] });
});
