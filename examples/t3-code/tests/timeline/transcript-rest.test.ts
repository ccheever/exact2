// The transcript's viewport for the composer's reservation (composer-overlay.ts' pulls), as the root reads it from the
// transcript's own scroll, wheel and keys (app.contract restScrolled, restGesture), never from message lengths. Ported
// from the AppKit cases T3Timeline.swift had.
import { describe, expect, test } from 'bun:test';
import { contractComponent, type Env } from '../support/contract-action-fixture';
import { appText } from '../support/app-files';

const root = contractComponent('app.contract', 'T3Code');
const column = contractComponent('app-main.contract', 'ChatColumn');

/** The root's viewport slots over a 100pt port of a 500pt transcript, on a clock `now` in ms. */
function transcript() {
  const state: Env = { restOwner: '', restAtEnd: true, restPulls: 0, restLastPull: -1000000, findViewport: '' };
  const clock = { now: 0 };
  const scope = () => ({ max: Math.max, performanceNow: () => clock.now });
  const at = (top: number, owner = 'A', size: { content?: number; key?: string } = {}) =>
    root.call('restScrolled', state, scope(), [owner, { scrollTop: top, scrollHeight: size.content ?? 500, clientHeight: 100, clientWidth: 300, firstVisibleKey: size.key }]);
  const gesture = (towardEnd: boolean) => root.call('restGesture', state, scope(), [towardEnd]);
  return { state, clock, at, gesture };
}

describe('the transcript\'s viewport', () => {
  test('the same scroll report records the find viewport key and clears a missing key', () => {
    const t = transcript();
    t.at(400, 'A', { key: '["thread","message"]' });
    expect(t.state.findViewport).toBe('["thread","message"]');
    t.at(350);
    expect(t.state.findViewport).toBe('');
  });

  test('a report names its transcript and whether it is at its end (2pt short counts)', () => {
    const t = transcript();
    t.at(350);
    expect([t.state.restOwner, t.state.restAtEnd]).toEqual(['A', false]);
    t.at(399, 'B');
    expect([t.state.restOwner, t.state.restAtEnd]).toEqual(['B', true]);
    t.at(0, 'B', { content: 80 });
    expect(t.state.restAtEnd).toBe(true); // nothing to scroll
  });

  test('a pull toward the end while already there counts once each half second', () => {
    const t = transcript();
    t.at(400);
    t.gesture(true); t.gesture(true);
    expect(t.state.restPulls).toBe(1);
    t.clock.now = 600; t.gesture(true);
    t.clock.now = 700; t.gesture(false);
    expect(t.state.restPulls).toBe(2);
    t.at(200); t.clock.now = 1300; t.gesture(true);
    expect(t.state.restPulls).toBe(2); // away from the end, no pull
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
  test('no resting state is left in the root, the composer, the window or the snapshot', () => {
    const files = ['app.contract', 'src/timeline/cite-members.contract', 'src/composer/composer-members.contract', 'src/shell/app-window.contract',
      'src/shell/app-main.contract', 'src/shared/shapes.contract', 'src/client/presentation.ts', 'src/timeline/transcript-facts.ts'];
    const left = files.flatMap(file => ['restResting', 'restText', 'restMotion', 'canRest', 'composerResting', 'composerPressed']
      .filter(name => new RegExp(`\\b${name}\\b`).test(appText(file))).map(name => `${file}: ${name}`));
    expect(left).toEqual([]);
  });
});
