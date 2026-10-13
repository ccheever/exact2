// macos/tests/terminal/vectors.json: the shared terminal output buffer vectors. CASES are the inputs; each
// step's `expect` is what terminal-output.ts (T3 Code's terminalOutput.ts, copied) does with them, so the
// AppKit tests hold T3TerminalOutput.swift to the reference's buffer (terminal-output.test.ts replays them
// too). generate.mjs writes the file.

import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const example = join(here, '..', '..');

const ABOUT = 'Shared terminal output buffer vectors (terminal-output.ts, T3TerminalOutput.swift). Generated from the reference terminalOutput.ts port; strings longer than 256 UTF-16 units are {length (UTF-16 units), head and tail (64 code points each)}. An op\'s `parts` ([text, count] pairs, joined) replace its empty `data`. Each step applies `op` (`max` bytes, default 524288) `repeat` times; `readEach` reads after every repeat and joins the appended data, false skips the read, else one read follows. `staleRead` reads from the initial cursor.';

/** The cases: each a name and its steps (op, data, then max, stale, repeat, readEach, parts as needed). */
const CASES = [
  ['caps retained output by UTF-8 byte length', [{ op: 'append', data: '🙂🙂', max: 4 }]],
  ['reduces a snapshot then output under an 8 byte budget', [{ op: 'reset', data: 'hello' }, { op: 'append', data: ' world', max: 8, stale: true }]],
  ['preserves a BOM code point at a new output chunk boundary', [{ op: 'reset', data: '' }, { op: 'append', data: `${'x'.repeat(16384)}﻿${'y'.repeat(20)}` }]],
  ['preserves a leading BOM in the retained snapshot tail', [{ op: 'reset', data: 'discard﻿tail', max: 7 }]],
  ['trims whole Unicode code points from a partially retained chunk', [
    { op: 'reset', data: 'é界🙂end', max: 12 }, { op: 'append', data: 'x', max: 12 }, { op: 'append', data: 'yz', max: 12 }, { op: 'append', data: 'abc', max: 12 },
  ]],
  ['delivers all output when several events arrive before a renderer reads', [
    { op: 'reset', data: 'hello' }, { op: 'append', data: ' one', readEach: false }, { op: 'append', data: ' two', readEach: false }, { op: 'append', data: ' three' },
  ]],
  ['discards output when the byte budget is 0', [{ op: 'reset', data: 'hello' }, { op: 'append', data: '', max: 0, parts: [['x', 65536]] }]],
  ['keeps appending while compacting metadata for many small writes', [{ op: 'reset', data: '' }, { op: 'append', data: 'x', repeat: 2500, readEach: true }]],
  ['appends every unread character across a compaction boundary', [{ op: 'reset', data: '' }, { op: 'append', data: 'xé界🙂', repeat: 300, readEach: true }]],
  ['trims a 600 KiB flood to the 512 KiB tail and keeps the last line', [
    { op: 'reset', data: '' }, { op: 'append', data: '', repeat: 38, readEach: false, parts: [['0123456789abcdef', 1024]] }, { op: 'append', data: '\nlast line\n' },
  ]],
  ['resets for a repeated snapshot even when the text repeats', [{ op: 'reset', data: 'hello' }, { op: 'reset', data: 'hello' }, { op: 'reset', data: '' }]],
];

/** A long string as its length and its first and last 64 code points. */
const text = (value) => {
  if (value.length <= 256) return value;
  const points = Array.from(value);
  return { length: value.length, head: points.slice(0, 64).join(''), tail: points.slice(-64).join('') };
};

export async function generate() {
  const output = await import(join(example, 'terminal-output.ts'));
  const cases = CASES.map(([name, steps]) => {
    let state = output.EMPTY_TERMINAL_OUTPUT_STATE, cursor = output.INITIAL_TERMINAL_OUTPUT_CURSOR;
    return {
      name,
      steps: steps.map((step) => {
        const data = step.parts ? step.parts.map(([part, count]) => part.repeat(count)).join('') : step.data;
        const max = step.max ?? output.DEFAULT_MAX_TERMINAL_BUFFER_BYTES;
        const types = [];
        let received = '', read;
        for (let index = 0; index < (step.repeat ?? 1); index++) {
          state = step.op === 'reset' ? output.resetOutput(state, data, max) : output.appendOutput(state, data, max);
          if (!step.readEach) continue;
          const update = output.readTerminalOutputUpdate(state, cursor);
          if (!types.includes(update.type)) types.push(update.type);
          if (update.type !== 'none') received += update.data;
          cursor = update.cursor;
        }
        if (step.readEach) read = { type: types.join(','), data: text(received) };
        else if (step.readEach === false) read = { type: 'skipped', data: '' };
        else {
          const update = output.readTerminalOutputUpdate(state, cursor);
          read = { type: update.type, data: text(update.type === 'none' ? '' : update.data) };
          cursor = update.cursor;
        }
        const expect = { text: text(output.terminalOutputText(state)), retainedBytes: state.retainedBytes, chunks: state.chunks.length, nextOffset: state.nextOffset, resetVersion: state.resetVersion, read };
        if (step.stale) {
          const update = output.readTerminalOutputUpdate(state, output.INITIAL_TERMINAL_OUTPUT_CURSOR);
          expect.staleRead = { type: update.type, data: text(update.type === 'none' ? '' : update.data) };
        }
        return { ...step, expect };
      }),
    };
  });
  // ASCII JSON, one space per level: every non-ASCII code unit escaped, so the bytes name each code point.
  const json = JSON.stringify({ about: ABOUT, cases }, null, 1).replace(/[\u0080-￿]/g, c => `\\u${c.charCodeAt(0).toString(16).padStart(4, '0')}`);
  return [{ file: join(example, 'macos', 'tests', 'terminal', 'vectors.json'), text: `${json}\n` }];
}
