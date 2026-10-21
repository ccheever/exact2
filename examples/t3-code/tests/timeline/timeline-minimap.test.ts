// The timeline minimap (timelineMinimapItems.ts, MessagesTimeline.tsx; T3 Code 74541bab17, MIT, see LICENSE-T3).
// Which turns are on screen and the jump are ChatColumn's (app-main.contract):
// its actions and derives run with Contract's action semantics (contract-action-fixture.ts), its functions from
// timeline-minimap.contract (contract-fn-fixture.ts).
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { minimapItems } from '../../src/timeline/timeline-minimap';
import { contractComponent, type Env } from '../support/contract-action-fixture';
import { contractFns } from '../support/contract-fn-fixture';
import { appPath } from '../support/app-files';

const rows = [
  { id: 'u1', kind: 'user', body: 'First  question\nwith two lines' }, { id: 'w', kind: 'work', body: '' },
  { id: 'a1', kind: 'assistant', body: 'Draft' }, { id: 'a2', kind: 'assistant', body: '**Final** answer\n\nhere' },
  { id: 'u2', kind: 'user', body: '' }, { id: 'u3', kind: 'user', body: 'Third' }, { id: 'a3', kind: 'assistant', body: 'Done.' },
];
const source = (file: string) => readFileSync(appPath(file), 'utf8');

describe('timeline minimap', () => {
  test('one strip per user message, previewing the turn\'s final answer, with its row in the drawn rows', () => {
    expect(minimapItems(rows).map(item => [item.id, item.index, item.label, item.excerpt, item.row])).toEqual([
      ['u1', 0, 'First question with two lines', 'Final answer here', 0], ['u2', 1, 'User message', '', 4], ['u3', 2, 'Third', 'Done.', 5]]);
  });
  // timelineMinimapItems.test.ts at 74541bab17 (#15715).
  test('the answer\'s Markdown previews as plain text; the user\'s text stays literal', () => {
    const userText = 'Explain **operators** and [x] in [the docs](https://example.com/docs)';
    const assistantText = [
      '## Operators', 'Use **either** option from [the docs](https://example.com/docs).', '',
      '| Operator | Meaning |', '| --- | :---: |', '| `a \\| b` | either |', '',
      '- [x] Match `**/*.ts` files', '', '[x] deployment complete', '', '> Note: ~~old~~ new  ', '> Next line', '',
      '```sh', '# install deps', '- **not bold**', 'const snake_case = a * b;', '```', '', 'foo<br>bar', '', '<div>Critical <em>answer</em></div>',
    ].join('\n');
    const [item] = minimapItems([{ id: 'u', kind: 'user', body: userText }, { id: 'a', kind: 'assistant', body: assistantText }]);
    expect([item!.label, item!.excerpt]).toEqual([userText,
      'Operators Use either option from the docs. Operator Meaning a | b either Match **/*.ts files [x] deployment complete Note: old new Next line # install deps - **not bold** const snake_case = a * b; foo bar Critical answer']);
  });
  test('a thread past 256 answered turns renders each preview once, not on every snapshot', () => {
    const answer = (i: number) => `## Heading ${i}\n\nSome **bold** text with \`code\` and [a link](https://x.y/${i}).\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n`.repeat(10);
    const many = Array.from({ length: 300 }, (_, i) => [{ id: `u${i}`, kind: 'user', body: `q${i}` }, { id: `a${i}`, kind: 'assistant', body: answer(i) }]).flat();
    const first = minimapItems(many);
    const started = performance.now();
    for (let pass = 0; pass < 5; pass++) expect(minimapItems(many)).toEqual(first);
    // Rendered on every snapshot, 300 previews took about half a second; read from the cache, well under a millisecond.
    expect((performance.now() - started) / 5).toBeLessThan(20);
    expect(first[299]!.excerpt.startsWith('Heading 299 Some bold text with code and a link.')).toBe(true);
  });
  test('only the first 1,500 units of a long answer are read', () => {
    const [item] = minimapItems([{ id: 'u', kind: 'user', body: 'q' }, { id: 'a', kind: 'assistant', body: `${'word '.repeat(400)}**tail**` }]);
    expect(item!.excerpt.length).toBeLessThan(1_500);
    expect(item!.excerpt).not.toContain('tail');
  });
});

const column = contractComponent('app-main.contract', 'ChatColumn');
const fns = contractFns('timeline-minimap.contract');
type Entry = { isIntersecting: boolean; intersectionRatio: number };
const hit = (isIntersecting: boolean): Entry => ({ isIntersecting, intersectionRatio: isIntersecting ? 0.5 : 0 });

/** ChatColumn's minimap half over `drawn` rows: report intersections, scrolls (the first visible key) and read the strips. */
function transcript(drawn = rows, owner = 'http://a:p:t1') {
  const calls: unknown[][] = [];
  const mounted = new Set(drawn.map(row => row.id));
  const state: Env = { turnsSeen: [], turnsOwner: '', firstKey: '', anchor: { owner: '' }, tipEpoch: 0, tipArmed: false, fallbackTop: 0, fallbackJump: -1, wheelTop: 0, wheelJump: -1, wheelPending: false, scrolledTop: 0, scrolledMax: 0, scrolledPort: 0 };
  // The column's other derives read the canvas and the lane (laneInset, stackWidth): a plain wide window.
  const scope: Env = { ...fns, min: Math.min, max: Math.max, composerOwner: owner, jumpRequest: 0, viewport: { prefersReducedMotion: false }, canvas: { ready: false }, conversationWidth: 1024, rem: 16,
    data: { messages: drawn.map(row => ({ id: row.id })), minimap: minimapItems(drawn), threadId: 't1', look: { chatMax: 768 } },
    scrollIntoView: (...args: unknown[]) => calls.push(['scrollIntoView', ...args]), chatLocal: (...args: unknown[]) => calls.push(['chatLocal', ...args]),
    citeEnd: () => calls.push(['citeEnd']),
    citeScrolled: () => {}, restScrolled: () => {}, restGesture: () => {}, anchorKey: (key?: string) => key ?? '', commandPending: false,
    // frame(): a user turn's box exists while the virtual window holds its row.
    frame: (id: string) => ({ unavailable: !mounted.has(id.replace(/^turn-/, '')) }) };
  const now = () => ({ ...scope, ...column.derives(state, scope) });
  return {
    calls, state, scope,
    seen(id: string, entry: Entry) { column.call('turnSeen', state, now(), [id, entry]); },
    first(key: string) { state.firstKey = key; },
    /** A ScrollEvent whose first visible row is `key`, after the window let go of every row but `held`. */
    scroll(key: string, held: string[]) {
      mounted.clear(); held.forEach(id => mounted.add(id));
      column.call('transcriptScrolled', state, now(), [0, 0, { firstVisibleKey: key, firstVisibleOffset: 0, scrollTop: 0, scrollHeight: 3000, clientHeight: 600 }]);
    },
    jump(id: string, index: number) { column.call('turnJump', state, now(), [id, index]); },
    get shown() { return now().turnsShown as string[]; },
    get current() { return now().turnCurrent as number; },
  };
}

describe('The turns on screen come from the user turns\' intersect reports', () => {
  test('mount reports, enter and leave, a duplicate report, several turns on screen', () => {
    const t = transcript();
    expect([t.shown, t.current]).toEqual([[], -1]); // no turn reported, no scroll yet: no current strip
    t.seen('u2', hit(true)); t.seen('u3', hit(true));
    expect([t.shown, t.current]).toEqual([['u2', 'u3'], 1]);
    t.seen('u2', hit(true));
    expect(t.state.turnsSeen).toEqual(['u3', 'u2']);
    expect([t.shown, t.current]).toEqual([['u2', 'u3'], 1]);
    // The reader scrolls down: u2 leaves at the top, the first visible row is now u3's.
    t.first('u3'); t.seen('u2', hit(false));
    expect([t.shown, t.current]).toEqual([['u3'], 2]);
  });
  test('a long answer filling the port: the last turn before its first visible row; none before the first turn', () => {
    const t = transcript();
    t.first('a2');
    expect([t.shown, t.current]).toEqual([[], 0]);
    t.first('a3'); t.seen('u3', hit(false));
    expect([t.shown, t.current]).toEqual([[], 2]);
    const plain = transcript([{ id: 'a0', kind: 'assistant', body: 'x' }, ...rows]);
    plain.first('a0');
    expect(plain.current).toBe(-1);
    expect(transcript(rows.filter(row => row.kind !== 'user')).current).toBe(-1); // no user turn, no current index
  });
  test('edge contact: the port is taken 1px in, so a turn only touching it is reported off screen', () => {
    const t = transcript();
    t.first('u2'); t.seen('u1', hit(false)); t.seen('u2', hit(true));
    expect([t.shown, t.current]).toEqual([['u2'], 1]);
  });
  test('a fast distant jump recycles rows without their leave: stale reports never survive', () => {
    const t = transcript();
    t.first('u3'); t.seen('u3', hit(true));
    // Jump to the top: u3's row is let go with no leave; u1 mounts on screen.
    t.first('u1'); t.seen('u1', hit(true));
    expect(t.state.turnsSeen).toEqual(['u3', 'u1']);
    expect([t.shown, t.current]).toEqual([['u1'], 0]);
    // And to the end, past every turn: u1 is stale too.
    t.first('a3');
    expect([t.shown, t.current]).toEqual([[], 2]);
    // A row removed from the thread (Edit from here) drops out at the next report.
    t.seen('u2', hit(true));
    const removed = transcript(rows.filter(row => row.id !== 'u1'));
    Object.assign(removed.state, t.state);
    removed.seen('u3', hit(true));
    expect(removed.state.turnsSeen).toEqual(['u2', 'u3']);
  });
  test('a jump that lets go of a row next to the new run drops it at the scroll report, so it never stays lit', () => {
    const t = transcript();
    t.first('u2'); t.seen('u2', hit(true)); t.seen('u3', hit(true));
    // An instant jump to the top: the long answer pushes u2 and u3 out of the window, u1 mounts on screen; no leaves.
    t.seen('u1', hit(true));
    t.scroll('u1', ['u1', 'w', 'a1', 'a2']);
    expect([t.state.turnsSeen, t.shown, t.current]).toEqual([['u1'], ['u1'], 0]);
    // Rows still held keep their reports.
    t.seen('u2', hit(true)); t.scroll('u1', ['u1', 'w', 'a1', 'a2', 'u2']);
    expect(t.shown).toEqual(['u1', 'u2']);
    expect(source('timeline.contract')).toContain('column id=`turn-${message.id}` intersect=seen(message.id)');
  });
  test('reports arriving out of order and prepended history keep the rows\' order', () => {
    const t = transcript();
    t.seen('u3', hit(true)); t.seen('u2', hit(true)); t.first('u2');
    expect([t.shown, t.current]).toEqual([['u2', 'u3'], 1]);
    const longer = transcript([{ id: 'u0', kind: 'user', body: 'earlier' }, { id: 'a0', kind: 'assistant', body: 'old' }, ...rows]);
    Object.assign(longer.state, t.state);
    expect([longer.shown, longer.current]).toEqual([['u2', 'u3'], 2]);
  });
  test('another thread or environment starts from nothing', () => {
    const t = transcript();
    t.seen('u2', hit(true));
    const other = transcript(rows, 'http://b:p:t1');
    Object.assign(other.state, t.state);
    expect(other.shown).toEqual([]);
    other.seen('u3', hit(true));
    expect([other.state.turnsSeen, other.state.turnsOwner]).toEqual([['u3'], 'http://b:p:t1']);
  });
});

// MessagesTimeline.logic.test.ts at 74541bab17 (#16191): only rows on screen are lit, and the current strip is the first
// visible user message, else the turn whose response is being read.
describe('the lit strips and the current one follow the rows on screen', () => {
  const drawn = Array.from({ length: 50 }, (_, row) => row % 10 === 0 ? { id: `u${row}`, kind: 'user', body: `turn ${row}` } : { id: `a${row}`, kind: 'assistant', body: 'reply' });
  function reading(start: number, end: number) {
    const t = transcript(drawn);
    t.first(drawn[start]!.id);
    for (let row = start; row <= end; row++) if (drawn[row]!.kind === 'user') t.seen(drawn[row]!.id, hit(true));
    return [t.shown, t.current];
  }
  test('a user message on screen is current; a response alone is its turn\'s', () => {
    expect(reading(19, 31)).toEqual([['u20', 'u30'], 2]);
    expect(reading(21, 29)).toEqual([[], 2]);
    expect(reading(41, 45)).toEqual([[], 4]);
    expect(reading(0, 0)).toEqual([['u0'], 0]);
    expect(transcript(drawn).current).toBe(-1); // no report yet: no current strip
  });
});

// ThreadTerminalDrawer over TimelineMinimap at 74541bab17 (#14383): the rail centres on the timeline, which ends at an
// open drawer's top, and what of it runs past that edge is clipped, as the drawer covers it there.
test('the minimap spans the timeline and clips its vertical overflow', () => {
  expect(source('timeline-minimap.contract')).toContain('column wheel=wheel position="absolute" top=0 bottom=0 left=0 width="4.5rem" overflow-x="visible" overflow-y="clip" pointer-events="none"');
  expect(source('timeline-minimap.contract')).not.toContain('height=(windowHeight - 52)');
});

describe('A strip or chevron jumps through the public keyed command', () => {
  test('the turn\'s row 24pt under the top edge, smooth, or at once under reduced motion', () => {
    const t = transcript();
    t.jump('u2', -1);
    t.jump('', 2);
    t.jump('', 9);
    // Each jump first ends a citation's placement (one writer), then asks the keyed command.
    expect(t.calls).toEqual([['citeEnd'], ['scrollIntoView', 'transcript', 'u2', { offset: -24, behavior: 'smooth' }], ['citeEnd'], ['scrollIntoView', 'transcript', 'u3', { offset: -24, behavior: 'smooth' }]]);
    (t.scope.viewport as Env).prefersReducedMotion = true;
    t.jump('u1', -1);
    expect(t.calls.at(-1)).toEqual(['scrollIntoView', 'transcript', 'u1', { offset: -24, behavior: 'instant' }]);
  });
});
