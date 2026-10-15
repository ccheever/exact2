// The timeline minimap (timelineMinimapItems.ts, MessagesTimeline.tsx; T3 Code 1e2ecbd975, MIT, see LICENSE-T3).
// Which turns are on screen and the jump are ChatColumn's (app-main.contract):
// its actions and derives run with Contract's action semantics (contract-action-fixture.ts), its functions from
// timeline-minimap.contract (contract-fn-fixture.ts).
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { minimapItems } from './timeline-minimap';
import { contractComponent, type Env } from './contract-action-fixture';
import { contractFns } from './contract-fn-fixture';

const rows = [
  { id: 'u1', kind: 'user', body: 'First  question\nwith two lines' }, { id: 'w', kind: 'work', body: '' },
  { id: 'a1', kind: 'assistant', body: 'Draft' }, { id: 'a2', kind: 'assistant', body: '**Final** answer\n\nhere' },
  { id: 'u2', kind: 'user', body: '' }, { id: 'u3', kind: 'user', body: 'Third' }, { id: 'a3', kind: 'assistant', body: 'Done.' },
];
const source = (file: string) => readFileSync(`${import.meta.dir}/${file}`, 'utf8');

describe('timeline minimap', () => {
  test('one strip per user message, previewing the turn\'s final answer, with its row in the drawn rows', () => {
    expect(minimapItems(rows).map(item => [item.id, item.index, item.label, item.excerpt, item.row])).toEqual([
      ['u1', 0, 'First question with two lines', '**Final** answer here', 0], ['u2', 1, 'User message', '', 4], ['u3', 2, 'Third', 'Done.', 5]]);
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
  const state: Env = { turnsSeen: [], turnsOwner: '', firstKey: '', anchor: { owner: '' } };
  // The column's other derives read the canvas and the lane (laneInset, stackWidth): a plain wide window.
  const scope: Env = { ...fns, min: Math.min, max: Math.max, composerOwner: owner, viewport: { prefersReducedMotion: false }, canvas: { ready: false }, conversationWidth: 1024, rem: 16,
    data: { messages: drawn.map(row => ({ id: row.id })), minimap: minimapItems(drawn), threadId: 't1', look: { chatMax: 768 } },
    scrollIntoView: (...args: unknown[]) => calls.push(['scrollIntoView', ...args]), chatLocal: (...args: unknown[]) => calls.push(['chatLocal', ...args]),
    citeScrolled: () => {}, anchorKey: (key?: string) => key ?? '', commandPending: false,
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
  test('each user turn is observed (not its row or the answers); the user-turn native hook is gone', () => {
    const user = source('timeline.contract').split('\ncomponent UserRow\n')[1]!;
    expect(user).toContain('intersect=seen(message.id) intersect-margin="-1px 0px" width="100%" align-items="flex-end"');
    expect(user).not.toContain('hatch="t3-turn" data-turn=message.id');
    // Assistant answers keep the shared hook as citation targets only.
    expect(source('timeline.contract')).toContain('column hatch="t3-turn" data-turn=`cite:${message.id}`');
    expect(source('app-main.contract')).toContain('TimelineMinimap(items=data.minimap, current=turnCurrent, visible=turnsShown,');
    expect(source('timeline-minimap.ts')).not.toMatch(/turnsInView|turnAbove|timelineJump/);
  });
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

describe('A strip or chevron jumps through the public keyed command', () => {
  test('the turn\'s row 24pt under the top edge, smooth, or at once under reduced motion', () => {
    const t = transcript();
    t.jump('u2', -1);
    t.jump('', 2);
    t.jump('', 9);
    // Each jump first ends a citation's native placement and hold (one writer), then asks the keyed command.
    const release = ['chatLocal', 'timeline-release', '', ''];
    expect(t.calls).toEqual([release, ['scrollIntoView', 'transcript', 'u2', { offset: -24, behavior: 'smooth' }], release, ['scrollIntoView', 'transcript', 'u3', { offset: -24, behavior: 'smooth' }]]);
    (t.scope.viewport as Env).prefersReducedMotion = true;
    t.jump('u1', -1);
    expect(t.calls.at(-1)).toEqual(['scrollIntoView', 'transcript', 'u1', { offset: -24, behavior: 'instant' }]);
    expect(source('timeline-presentation.ts')).toContain("if (op === 'timeline-release') { await client.restAccess(native).call({ op: 'timelineRelease' })");
  });
  test('the strips and chevrons call it; no native jump or chatLocal route remains', () => {
    const minimap = source('timeline-minimap.contract');
    expect(minimap).toContain('button press=jump(item.id, -1) hover=setActive(item.index)');
    expect(minimap).toContain('button press=jump("", target) hover=hover disabled=(target < 0)');
    expect(minimap).not.toContain('local("jump"');
    expect(source('timeline-presentation.ts')).not.toContain("op === 'jump'");
    expect(source('modules/apple/T3TimelineTurns.swift')).not.toMatch(/turnsInView|boundsDidChangeNotification|func measure/);
  });
});
