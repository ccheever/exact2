// macos/tests/terminal/vectors.json: the shared terminal output buffer vectors, which terminal-output.test.ts
// replays against the TypeScript port (terminal-output.ts) and the terminal AppKit tests against the Swift
// one (T3TerminalOutput.swift). CASES below are committed test data: each step's inputs and what the
// buffer must hold after it. They are T3 Code 1e2ecbd975's own buffer (MIT reference, see LICENSE-T3:
// packages/client-runtime/src/state/terminalOutput.ts, SHA-256 084bd07d61dca49b7945ce98485bbd8cb81449d3
// 0cc3f82ff4630630a915bb18) over these inputs, checked against that file when they were written, and
// never computed from either port: a change to a port's behaviour fails its test.
// This generator reads no app source. It spells the inputs out and shortens long texts for the JSON
// (generate.mjs writes the file).

import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const example = join(dirname(fileURLToPath(import.meta.url)), '..', '..');

const ABOUT = 'Shared terminal output buffer vectors (terminal-output.ts, T3TerminalOutput.swift). Written from the committed test data in tools/generate/gen-vectors.mjs: the expectations are T3 Code 1e2ecbd975\'s terminalOutput.ts over these inputs, stated there, not computed from either port; strings longer than 256 UTF-16 units are {length (UTF-16 units), head and tail (64 code points each)}. An op\'s `parts` ([text, count] pairs, joined) replace its empty `data`. Each step applies `op` (`max` bytes, default 524288) `repeat` times; `readEach` reads after every repeat and joins the appended data, false skips the read, else one read follows. `staleRead` reads from the initial cursor.';

const BOM_CHUNK = `${'x'.repeat(16384)}\ufeff${'y'.repeat(20)}`;
/** 38 writes of 16 KiB: 608 KiB of ASCII, of which the 512 KiB budget keeps the last 512 KiB. */
const FLOOD = '0123456789abcdef'.repeat(38 * 1024);
const KEPT = 512 * 1024;

/**
 * The cases: a name and its steps. A step is its inputs (op, data, then max, stale, repeat, readEach,
 * parts as needed) and `expect`: the buffer's text, retained UTF-8 bytes, chunk count, next offset and
 * reset version after the step, the read that follows as [type, data] ('skipped' when readEach is false)
 * and, for `stale`, the read from the initial cursor.
 */
const CASES = [
  ['caps retained output by UTF-8 byte length', [
    { op: 'append', data: '🙂🙂', max: 4, expect: { text: '🙂', retainedBytes: 4, chunks: 1, nextOffset: 4, resetVersion: 0, read: ['reset', '🙂'] } },
  ]],
  ['reduces a snapshot then output under an 8 byte budget', [
    { op: 'reset', data: 'hello', expect: { text: 'hello', retainedBytes: 5, chunks: 1, nextOffset: 5, resetVersion: 1, read: ['reset', 'hello'] } },
    { op: 'append', data: ' world', max: 8, stale: true, expect: { text: 'lo world', retainedBytes: 8, chunks: 2, nextOffset: 11, resetVersion: 1, read: ['append', ' world'], staleRead: ['reset', 'lo world'] } },
  ]],
  ['preserves a BOM code point at a new output chunk boundary', [
    { op: 'reset', data: '', expect: { text: '', retainedBytes: 0, chunks: 0, nextOffset: 0, resetVersion: 1, read: ['reset', ''] } },
    { op: 'append', data: BOM_CHUNK, expect: { text: BOM_CHUNK, retainedBytes: 16407, chunks: 2, nextOffset: 16405, resetVersion: 1, read: ['append', BOM_CHUNK] } },
  ]],
  ['preserves a leading BOM in the retained snapshot tail', [
    { op: 'reset', data: 'discard\ufefftail', max: 7, expect: { text: '\ufefftail', retainedBytes: 7, chunks: 1, nextOffset: 5, resetVersion: 1, read: ['reset', '\ufefftail'] } },
  ]],
  ['trims whole Unicode code points from a partially retained chunk', [
    { op: 'reset', data: 'é界🙂end', max: 12, expect: { text: 'é界🙂end', retainedBytes: 12, chunks: 1, nextOffset: 7, resetVersion: 1, read: ['reset', 'é界🙂end'] } },
    { op: 'append', data: 'x', max: 12, expect: { text: '界🙂endx', retainedBytes: 11, chunks: 2, nextOffset: 8, resetVersion: 1, read: ['append', 'x'] } },
    { op: 'append', data: 'yz', max: 12, expect: { text: '🙂endxyz', retainedBytes: 10, chunks: 3, nextOffset: 10, resetVersion: 1, read: ['append', 'yz'] } },
    { op: 'append', data: 'abc', max: 12, expect: { text: 'endxyzabc', retainedBytes: 9, chunks: 4, nextOffset: 13, resetVersion: 1, read: ['append', 'abc'] } },
  ]],
  ['delivers all output when several events arrive before a renderer reads', [
    { op: 'reset', data: 'hello', expect: { text: 'hello', retainedBytes: 5, chunks: 1, nextOffset: 5, resetVersion: 1, read: ['reset', 'hello'] } },
    { op: 'append', data: ' one', readEach: false, expect: { text: 'hello one', retainedBytes: 9, chunks: 2, nextOffset: 9, resetVersion: 1, read: ['skipped', ''] } },
    { op: 'append', data: ' two', readEach: false, expect: { text: 'hello one two', retainedBytes: 13, chunks: 3, nextOffset: 13, resetVersion: 1, read: ['skipped', ''] } },
    { op: 'append', data: ' three', expect: { text: 'hello one two three', retainedBytes: 19, chunks: 4, nextOffset: 19, resetVersion: 1, read: ['append', ' one two three'] } },
  ]],
  ['discards output when the byte budget is 0', [
    { op: 'reset', data: 'hello', expect: { text: 'hello', retainedBytes: 5, chunks: 1, nextOffset: 5, resetVersion: 1, read: ['reset', 'hello'] } },
    { op: 'append', data: '', max: 0, parts: [['x', 65536]], expect: { text: '', retainedBytes: 0, chunks: 0, nextOffset: 65541, resetVersion: 2, read: ['reset', ''] } },
  ]],
  ['keeps appending while compacting metadata for many small writes', [
    { op: 'reset', data: '', expect: { text: '', retainedBytes: 0, chunks: 0, nextOffset: 0, resetVersion: 1, read: ['reset', ''] } },
    { op: 'append', data: 'x', repeat: 2500, readEach: true, expect: { text: 'x'.repeat(2500), retainedBytes: 2500, chunks: 452, nextOffset: 2500, resetVersion: 1, read: ['append', 'x'.repeat(2500)] } },
  ]],
  ['appends every unread character across a compaction boundary', [
    { op: 'reset', data: '', expect: { text: '', retainedBytes: 0, chunks: 0, nextOffset: 0, resetVersion: 1, read: ['reset', ''] } },
    { op: 'append', data: 'xé界🙂', repeat: 300, readEach: true, expect: { text: 'xé界🙂'.repeat(300), retainedBytes: 3000, chunks: 300, nextOffset: 1500, resetVersion: 1, read: ['append', 'xé界🙂'.repeat(300)] } },
  ]],
  ['trims a 600 KiB flood to the 512 KiB tail and keeps the last line', [
    { op: 'reset', data: '', expect: { text: '', retainedBytes: 0, chunks: 0, nextOffset: 0, resetVersion: 1, read: ['reset', ''] } },
    { op: 'append', data: '', repeat: 38, readEach: false, parts: [['0123456789abcdef', 1024]], expect: { text: FLOOD.slice(-KEPT), retainedBytes: KEPT, chunks: 32, nextOffset: 622592, resetVersion: 1, read: ['skipped', ''] } },
    { op: 'append', data: '\nlast line\n', expect: { text: `${FLOOD}\nlast line\n`.slice(-KEPT), retainedBytes: KEPT, chunks: 33, nextOffset: 622603, resetVersion: 1, read: ['reset', `${FLOOD}\nlast line\n`.slice(-KEPT)] } },
  ]],
  ['resets for a repeated snapshot even when the text repeats', [
    { op: 'reset', data: 'hello', expect: { text: 'hello', retainedBytes: 5, chunks: 1, nextOffset: 5, resetVersion: 1, read: ['reset', 'hello'] } },
    { op: 'reset', data: 'hello', expect: { text: 'hello', retainedBytes: 5, chunks: 1, nextOffset: 5, resetVersion: 2, read: ['reset', 'hello'] } },
    { op: 'reset', data: '', expect: { text: '', retainedBytes: 0, chunks: 0, nextOffset: 0, resetVersion: 3, read: ['reset', ''] } },
  ]],
];

/** A long string as its length and its first and last 64 code points. */
const text = (value) => {
  if (value.length <= 256) return value;
  const points = Array.from(value);
  return { length: value.length, head: points.slice(0, 64).join(''), tail: points.slice(-64).join('') };
};
const read = ([type, data]) => ({ type, data: text(data) });

export function generate() {
  const cases = CASES.map(([name, steps]) => ({
    name,
    steps: steps.map(({ expect, ...step }) => ({
      ...step,
      expect: {
        text: text(expect.text), retainedBytes: expect.retainedBytes, chunks: expect.chunks, nextOffset: expect.nextOffset, resetVersion: expect.resetVersion,
        read: read(expect.read), ...(expect.staleRead && { staleRead: read(expect.staleRead) }),
      },
    })),
  }));
  // ASCII JSON, one space per level: every non-ASCII code unit escaped, so the bytes name each code point.
  const json = JSON.stringify({ about: ABOUT, cases }, null, 1).replace(/[\u0080-￿]/g, c => `\\u${c.charCodeAt(0).toString(16).padStart(4, '0')}`);
  return [{ file: join(example, 'macos', 'tests', 'terminal', 'vectors.json'), text: `${json}\n` }];
}
