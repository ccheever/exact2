// The transcript's viewport for the composer (ChatComposer's collapse on scroll, composer-overlay.ts' pulls), as the root
// reads it from the transcript's own scroll, wheel and keys (app.contract restScrolled, restGesture), never from
// message lengths. Ported from the AppKit cases T3Timeline.swift had.
import { describe, expect, test } from 'bun:test';
import { contractComponent, type Env } from '../support/contract-action-fixture';
import { appText } from '../support/app-files';

const root = contractComponent('app.contract', 'T3Code');
const column = contractComponent('app-main.contract', 'ChatColumn');

/** The root's rest slots over a 100pt port of a 500pt transcript, on a clock `now` in ms. */
function transcript(canRest = true) {
  const state: Env = { restOwner: '', restResting: false, restAtEnd: true, restPulls: 0, restGestureUntil: -1, restLastPull: -1000000,
    restTop: -1, restHeight: -1, restContent: -1, restWidth: -1, restMovement: 0, findViewport: '', restText: '' };
  const clock = { now: 0 };
  const draft = { text: '' };
  const scope = () => ({ canRest, composerText: draft.text, max: Math.max, performanceNow: () => clock.now });
  const at = (top: number, owner = 'A', size: { content?: number; width?: number; key?: string } = {}) =>
    root.call('restScrolled', state, scope(), [owner, { scrollTop: top, scrollHeight: size.content ?? 500, clientHeight: 100, clientWidth: size.width ?? 300, firstVisibleKey: size.key }]);
  const gesture = (towardEnd: boolean) => root.call('restGesture', state, scope(), [towardEnd]);
  return { state, clock, draft, at, gesture };
}

describe('the composer rests on a reader\'s movement away from the end', () => {
  test('the same scroll report records the find viewport key and clears a missing key', () => {
    const t = transcript();
    t.at(400, 'A', { key: '["thread","message"]' });
    expect(t.state.findViewport).toBe('["thread","message"]');
    t.at(350);
    expect(t.state.findViewport).toBe('');
  });

  test('programmatic movement never rests it; a gesture authorizes the movement after it; the end and a new owner end it', () => {
    const t = transcript();
    t.at(400); t.at(350);
    expect([t.state.restResting, t.state.restAtEnd]).toEqual([false, false]);
    t.gesture(false); t.at(320);
    expect(t.state.restResting).toBe(true);
    t.at(400);
    expect([t.state.restAtEnd, t.state.restResting]).toEqual([true, false]);
    t.gesture(false); t.at(370);
    expect(t.state.restResting).toBe(true); // near the end too
    t.at(370, 'B');
    expect([t.state.restResting, t.state.restOwner]).toEqual([false, 'B']);
  });

  test('a resize or the transcript growing is no reader\'s movement; the gesture lapses after 0.8 s', () => {
    const t = transcript();
    t.at(100);
    t.gesture(false);
    t.at(60, 'A', { width: 250 });
    expect(t.state.restResting).toBe(false);
    t.at(30, 'A', { width: 250, content: 650 });
    expect(t.state.restResting).toBe(false);
    t.gesture(false); t.clock.now = 900; t.at(0, 'A', { width: 250, content: 650 });
    expect(t.state.restResting).toBe(false);
  });

  test('the rest keeps the prompt it came to rest on, for an edit to end it (restTyped)', () => {
    const t = transcript();
    t.draft.text = 'half a thought';
    t.at(400); t.gesture(false); t.at(350);
    expect([t.state.restResting, t.state.restText]).toEqual([true, 'half a thought']);
    t.draft.text = 'half a thought, more';
    t.gesture(false); t.at(300);
    expect(t.state.restText).toBe('half a thought'); // further movement keeps the first
  });

  test('without eligibility (a multi-line composer, a request) nothing rests', () => {
    const t = transcript(false);
    t.at(300); t.gesture(false); t.at(260);
    expect(t.state.restResting).toBe(false);
  });

  test('a pull toward the end while already there counts once each half second', () => {
    const t = transcript();
    t.at(400);
    t.gesture(true); t.gesture(true);
    expect(t.state.restPulls).toBe(1);
    t.clock.now = 600; t.gesture(true);
    t.clock.now = 700; t.gesture(false);
    expect(t.state.restPulls).toBe(2);
  });

  test('the transcript\'s wheel and scroll keys are the gestures; other keys are not', () => {
    const calls: boolean[] = [];
    const scope = { citeEnd: () => {}, restGesture: (towardEnd: boolean) => calls.push(towardEnd) };
    column.call('transcriptWheel', {}, scope, [{ deltaY: 12 }]);
    column.call('transcriptWheel', {}, scope, [{ deltaY: -12 }]);
    for (const key of ['End', 'PageUp', ' ', 'a']) column.call('transcriptKeys', {}, scope, [key]);
    expect(calls).toEqual([true, false, true, false, true]);
  });
});

describe('the composer never rests (T3 Code v0.1.5 #25869)', () => {
  test('the root never lets it rest, and the prompt region needs no press to expand it', () => {
    expect(appText('src/timeline/cite-members.contract')).toContain('derive canRest = false');
    expect(appText('src/composer/composer.contract')).not.toContain('pointerdown=composerPressed');
  });
});

