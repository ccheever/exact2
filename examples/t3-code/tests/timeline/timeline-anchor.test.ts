// The transcript's remembered position per thread, Contract's since T3ComposerInput.swift's
// native restoration retired (its cases are ported here). Reference: T3 Code 1e2ecbd975 (MIT, see LICENSE-T3)
// timelineScrollAnchoring.ts rememberTimelinePosition (scoped thread keys, 100 entries) and MessagesTimeline.tsx's
// handleScroll / restoration (scrollToIndex with -offsetWithinRow, else scrollToOffset, else the end). ChatColumn's
// actions (app-main.contract) run with Contract's action semantics (contract-action-fixture.ts); the functions of
// timeline-anchor.contract run as JavaScript (contract-fn-fixture.ts).
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { contractComponent, value, type Env } from '../support/contract-action-fixture';
import { contractFns } from '../support/contract-fn-fixture';
import { appPath } from '../support/app-files';

const source = (file: string) => readFileSync(appPath(file), 'utf8');
const column = contractComponent('app-main.contract', 'ChatColumn');
const anchorFns = contractFns('timeline-anchor.contract', ['TimelineAnchor']);
const minimapFns = contractFns('timeline-minimap.contract');
const TimelineAnchor = (fields: Env) => fields;
const A = 'http://127.0.0.1:3774:p1:t-a', B = 'http://127.0.0.1:3774:p1:t-b';
const rowsOf = (prefix: string, count = 6) => Array.from({ length: count }, (_, index) => ({ id: `${prefix}${index}` }));

/** The chat column over one window: the open thread's rows, a pending command, the end jumps, host commands. */
function chat() {
  const calls: unknown[][] = [];
  const state: Env = { firstKey: '', anchors: [], anchor: TimelineAnchor({ owner: '', key: '', offset: 0, top: 0, end: true }), fallbackTop: 0, fallbackJump: -1, turnsSeen: [], turnsOwner: '', tipEpoch: 0, tipArmed: false, wheelTop: 0, wheelJump: -1, wheelPending: false, scrolledTop: 0, scrolledMax: 0, scrolledPort: 0 };
  const scope: Env = { ...anchorFns, ...minimapFns, TimelineAnchor, min: Math.min, max: Math.max, composerOwner: A, commandPending: false, jumpRequest: 1,
    canvas: { ready: false }, conversationWidth: 1024, rem: 16,
    data: { messages: rowsOf('a'), minimap: [], citing: false, threadId: 't-a', look: { chatMax: 768 } },
    citeScrolled: () => {}, restScrolled: () => {}, restGesture: () => {}, scrollIntoView: (...args: unknown[]) => calls.push(args), frame: () => ({ unavailable: false }) };
  const now = () => ({ ...scope, ...column.derives(state, scope) });
  const c = {
    calls, state, scope,
    /** The thread `owner` with `rows` is drawn (the commit after its data); its marker's mount report follows. */
    open(owner: string, rows = rowsOf(owner === A ? 'a' : 'b')) {
      scope.composerOwner = owner;
      (scope.data as Env).messages = rows;
      column.call('transcriptOpened', state, now(), [owner, { isIntersecting: true, intersectionRatio: 1 }]);
    },
    /** One ScrollEvent from the transcript (3000 tall, a 600 port). */
    scroll(top: number, key?: string, offset = 0) {
      column.call('transcriptScrolled', state, now(), [0, top, { firstVisibleKey: key, firstVisibleOffset: offset, scrollTop: top, scrollHeight: 3000, clientHeight: 600 }]);
    },
    /** A thread switch: the command runs (selectThread's jump to the end lands on the old rows), then the next thread draws. */
    switchTo(owner: string, rows?: { id: string }[]) {
      scope.commandPending = true;
      scope.jumpRequest = (scope.jumpRequest as number) + 1;
      c.scroll(2400, (scope.data as Env).messages ? ((scope.data as Env).messages as { id: string }[]).at(-1)!.id : undefined);
      c.open(owner, rows);
      c.scroll(2400, (rows ?? rowsOf(owner === A ? 'a' : 'b')).at(-1)!.id);
      scope.commandPending = false;
    },
    saved(owner: string) { return (anchorFns.anchorOf as (anchors: unknown, owner: string) => Env)([...(state.anchors as Env[]), state.anchor as Env], owner); },
  };
  return c;
}

describe('each thread keeps its own place', () => {
  test('a thread left scrolled up returns to its row and the offset into it; another one opens at its end', () => {
    const c = chat();
    c.open(A);
    expect(c.calls).toEqual([]);
    c.scroll(2400, 'a4', 400); c.scroll(1234, 'a2', 234);
    expect(c.state.anchor).toEqual({ owner: A, key: 'a2', offset: 234, top: 1234, end: false });
    c.switchTo(B);
    expect(c.calls).toEqual([]); // B was never left scrolled up: it stays at the end its switch jumped to
    c.scroll(2100, 'b3', 300);
    c.switchTo(A);
    expect(c.calls).toEqual([['transcript', 'a2', { offset: 234, behavior: 'instant' }]]);
    c.switchTo(B);
    expect(c.calls.at(-1)).toEqual(['transcript', 'b3', { offset: 300, behavior: 'instant' }]);
  });
  test('rows prepended above (history) keep the saved row and offset, not the old scroll offset', () => {
    const c = chat();
    c.open(A); c.scroll(1234, 'a2', 234);
    c.switchTo(B);
    c.switchTo(A, [...rowsOf('h', 4), ...rowsOf('a')]);
    expect(c.calls).toEqual([['transcript', 'a2', { offset: 234, behavior: 'instant' }]]);
  });
  test('a switch never records under the old owner: the jump to the end on the old rows, the new rows before their marker', () => {
    const c = chat();
    c.open(A); c.scroll(1234, 'a2', 234);
    c.scope.commandPending = true; // selectThread: its end jump lands on A's rows
    c.scroll(2400, 'a5', 0);
    expect(c.state.anchor).toMatchObject({ owner: A, key: 'a2', end: false });
    c.scope.commandPending = false;
    c.scope.composerOwner = B; (c.scope.data as Env).messages = rowsOf('b'); // B's rows draw before its marker reports
    c.scroll(2400, 'b5', 0);
    expect(c.state.anchor).toMatchObject({ owner: A, key: 'a2' });
    c.open(B);
    expect(c.saved(A)).toEqual({ owner: A, key: 'a2', offset: 234, top: 1234, end: false });
  });
  test('the reader\'s scrolling before a switch is kept to its last report (a momentum tail), nothing after the switch starts', () => {
    const c = chat();
    c.open(A);
    for (const [top, key, offset] of [[1800, 'a3', 300], [1500, 'a3', 0], [1298, 'a2', 298], [1234, 'a2', 234]] as const) c.scroll(top, key, offset);
    c.scope.commandPending = true;
    c.scroll(1200, 'a2', 200); // the tail still settling once the switch was asked
    c.open(B, rowsOf('b'));
    expect(c.saved(A)).toEqual({ owner: A, key: 'a2', offset: 234, top: 1234, end: false });
  });
  test('a thread left at its end opens at its end; a report with no row (padding only) or a stale row keeps the anchor', () => {
    const c = chat();
    c.open(A); c.scroll(2400, 'a5', 100);
    expect(c.state.anchor).toMatchObject({ end: true });
    c.switchTo(B); c.switchTo(A);
    expect(c.calls).toEqual([]);
    c.scroll(600, 'a1', 100);
    c.scroll(500, undefined, 0);
    c.scroll(450, 'b1', 50);
    expect(c.state.anchor).toMatchObject({ key: 'a1', offset: 100, end: false });
  });
  test('a saved row that is gone lands on its saved offset through scrollTop, until the next jump to the end', () => {
    const c = chat();
    c.open(A); c.scroll(1234, 'a2', 234);
    c.switchTo(B);
    c.switchTo(A, rowsOf('a').filter(row => row.id !== 'a2'));
    expect(c.calls).toEqual([]);
    expect([c.state.fallbackTop, c.state.fallbackJump]).toEqual([1234, c.scope.jumpRequest]);
    expect(source('app-main.contract')).toContain('scrollTop=listTop id="transcript"');
    const list = /derive listTop = (.+)/.exec(source('app-main.contract'))![1]!;
    expect(value(list, { ...c.state, jumpRequest: c.scope.jumpRequest })).toBe(1234);
    expect(value(list, { ...c.state, jumpRequest: (c.scope.jumpRequest as number) + 1 })).toBe(1000000 + (c.scope.jumpRequest as number) + 1);
    // Another thread opening without a jump to the end (a citation, a chip) never inherits that offset.
    c.open(B);
    expect(c.state.fallbackJump).toBe(-1);
  });
  test('the same thread in two environments keeps two places', () => {
    const c = chat();
    const elsewhere = 'https://remote.example:p1:t-a';
    c.open(A); c.scroll(1234, 'a2', 234);
    c.switchTo(elsewhere, rowsOf('a')); c.scroll(600, 'a1', 10);
    c.switchTo(A);
    expect(c.calls.at(-1)).toEqual(['transcript', 'a2', { offset: 234, behavior: 'instant' }]);
    c.switchTo(elsewhere, rowsOf('a'));
    expect(c.calls.at(-1)).toEqual(['transcript', 'a1', { offset: 10, behavior: 'instant' }]);
  });
  test('the list remounting for the same thread (from an empty one) returns to where it was', () => {
    const c = chat();
    c.open(A); c.scroll(1234, 'a2', 234);
    c.scope.composerOwner = B; (c.scope.data as Env).messages = []; // an empty thread: no list, no marker
    c.open(A);
    expect(c.calls).toEqual([['transcript', 'a2', { offset: 234, behavior: 'instant' }]]);
  });
  test('a citation placing its answer is not overridden by the thread\'s remembered place', () => {
    const c = chat();
    c.open(A); c.scroll(1234, 'a2', 234);
    c.switchTo(B);
    (c.scope.data as Env).citing = true;
    c.open(A);
    expect([c.calls, c.state.anchor]).toEqual([[], { owner: A, key: 'a2', offset: 234, top: 1234, end: false }]);
  });
  test('at most 100 threads are kept, the oldest dropped first and a re-saved one the newest', () => {
    const kept = anchorFns.anchorKept as (anchors: Env[], entry: Env) => Env[];
    let anchors: Env[] = [];
    for (let index = 0; index < 101; index++) anchors = kept(anchors, { owner: `o${index}`, key: 'k', offset: 0, top: 0, end: false });
    expect([anchors.length, anchors[0]!.owner, anchors.at(-1)!.owner]).toEqual([100, 'o1', 'o100']);
    anchors = kept(anchors, { owner: 'o1', key: 'k2', offset: 5, top: 0, end: false });
    expect([anchors.length, anchors[0]!.owner, anchors.at(-1)]).toEqual([100, 'o2', { owner: 'o1', key: 'k2', offset: 5, top: 0, end: false }]);
    expect(kept(anchors, { owner: '', key: '', offset: 0, top: 0, end: true })).toBe(anchors);
  });
});

describe('one writer per navigation', () => {
  test('the marker beside the list reports each thread\'s transcript, and T3ComposerInput keeps only the composer\'s input', () => {
    const main = source('app-main.contract');
    expect(main).toContain('          for owner in [composerOwner] key=owner\n            box intersect=transcriptOpened(owner) position="absolute"');
    const c = chat();
    expect(column.drawn('intersect=transcriptOpened(owner)', c.state, c.scope)).toBe(true);
    (c.scope.data as Env).messages = [];
    expect(column.drawn('intersect=transcriptOpened(owner)', c.state, c.scope)).toBe(false);
    const input = source('modules/apple/T3ComposerInput.swift');
    expect(input).not.toMatch(/func remember|func step|restoring|turns\[|boundsDidChange|timelineOwner/);
    for (const kept of ['func responderChanged', 'static func chordKey', 'static func commitMarkedText', 'commit-on-press']) expect(input).toContain(kept);
  });
});
