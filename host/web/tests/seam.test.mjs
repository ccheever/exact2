// A big answer crossing the JS target's data seam is said once per resource
// or mutation (host/web-js/seam.js), whichever module answers and whenever
// it installs its `answer`; a bounded one is not.
import { expect, test } from 'bun:test';
import { ROWS, rows, watch } from '../../web-js/seam.js';

// The heavy-list bench's feed, cut down: a record of messages, each with
// paragraphs of runs and reactions (names.js `sourceTypes` form).
const Message = { id: 's', paragraphs: ['[', { id: 's', runs: ['[', { t: 's' }] }], photos: ['?', { n: 'n' }], reactions: ['[', { emoji: 's', count: 'n' }] };
const types = { feed: [['s'], { rows: ['[', Message] }], react: [['s', 's'], { rows: ['[', Message] }], ack: [[], { ok: 'b' }] };
const message = i => [`m${i}`, [[`p${i}`, [['a'], ['b']]]], null, [['🔥', 2]]];
const feed = n => [Array.from({ length: n }, (_, i) => message(i))];

const page = () => {
  const lines = [], data = { answer: () => null, parse: null };
  watch(data, { types, say: line => lines.push(line), owner: args => args.holder });
  return { lines, data };
};

test('rows counts the longest list at any depth, by type', () => {
  expect(rows(feed(3), types.feed[1])).toBe(3);
  // One message whose paragraph list is the longest list in the answer.
  expect(rows([[['m', Array.from({ length: 9 }, () => ['p', []]), null, []]]], types.feed[1])).toBe(9);
  expect(rows([true], types.ack[1])).toBe(0);
  expect(rows(null, ['?', ['[', 's']])).toBe(0);
});

test('a whole-list answer is said once per target, naming it and the bounded form', () => {
  const { lines, data } = page();
  // A Rust module installs its answer after the watch (rust-data.js after first pixel).
  data.answer = (source, args) => ({ v: source === 'ack' ? [true] : feed(args[0] === 'all' ? ROWS + 1 : 200) });
  expect(data.answer('feed', ['window'], null, 'feed').v[0]).toHaveLength(200);
  expect(lines).toEqual([]);
  data.answer('react', ['all'], null, 'changed');
  data.answer('react', ['all'], null, 'changed');
  data.answer('feed', ['all'], null, 'initial');
  expect(lines).toHaveLength(2);
  expect(lines[0]).toStartWith(`big answer: changed (source react) carries ${ROWS + 1} list rows`);
  expect(lines[0]).toContain('LLP 1027.004');
  expect(lines[0]).toContain('docs/agent-pitfalls.md "A tap that changes one row of a long list"');
  expect(lines[1]).toStartWith('big answer: initial (source feed)');
});

test("a TypeScript module's promise and a parsed reply are watched too", async () => {
  const { lines, data } = page();
  data.answer = () => ({ promise: Promise.resolve(feed(ROWS + 5)), store: true });
  const a = data.answer('feed', ['all'], null, 'later');
  expect(a.store).toBe(true);
  expect((await a.promise)[0]).toHaveLength(ROWS + 5);
  data.parse = () => ({ v: feed(ROWS + 1) });
  data.parse('react', { holder: 'changed' }, {}, null);
  expect(lines.map(l => l.split(' carries')[0])).toEqual(['big answer: later (source feed)', 'big answer: changed (source react)']);
});
