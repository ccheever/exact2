// The history swipe's edge arrow (history-swipe.contract; app-window.contract historySwiped), over the window root's
// `historyswipe`: v0.1.6's HistorySwipeNavigation.tsx SwipeIndicator state, driven by AppKit's tracked swipe.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { contractFns } from '../support/contract-fn-fixture';
import { contractComponent } from '../support/contract-action-fixture';
import { APP } from '../support/app-files';

type Swipe = { shown: boolean; tracking: boolean; back: boolean; progress: number; y: number; left: number; width: number; height: number };
type SwipeEvent = { phase: string; direction: string; amount: number; clientX: number; clientY: number };
const fns = contractFns('src/shell/history-swipe.contract', ['HistorySwipe']) as unknown as {
  historySwipeIdle: () => Swipe;
  historySwipeStep: (s: Swipe, e: SwipeEvent, left: number, width: number, height: number) => Swipe;
};
const at = (phase: string, amount: number, direction = 'back'): SwipeEvent => ({ phase, direction, amount, clientX: 600, clientY: 300 });

describe('the history swipe arrow', () => {
  test('a swipe shows it from its start, armed where a lift at rest steps, and fades it where it stands', () => {
    let s = fns.historySwipeIdle();
    expect(s.shown).toBe(false);
    s = fns.historySwipeStep(s, at('began', 0.05), 300, 980, 840);
    expect(s).toEqual({ shown: true, tracking: true, back: true, progress: 0.2, y: 300, left: 300, width: 980, height: 840 });
    s = fns.historySwipeStep(s, at('changed', 0.5), 0, 1280, 840);
    expect(s.progress).toBe(1);
    expect(s.left).toBe(300); // the bounds are read as the swipe began
    s = fns.historySwipeStep(s, at('ended', 0.5), 0, 1280, 840);
    expect(s).toMatchObject({ shown: true, tracking: false, progress: 1 });
    s = fns.historySwipeStep(s, at('began', 0.1, 'forward'), 300, 980, 840);
    expect(s).toMatchObject({ tracking: true, back: false, progress: 0.4 });
    expect(fns.historySwipeStep(s, at('cancelled', 0.1, 'forward'), 300, 980, 840)).toMatchObject({ tracking: false, back: false, progress: 0.4 });
  });

  test('the window root hears the swipe and reads the main content beside the sidebar', () => {
    const root = readFileSync(join(APP, 'src/shell/app-window.contract'), 'utf8').split('\n').find(line => line.trimStart().startsWith('main navigationKey="t3-code"'))!;
    expect(root).toContain('historyswipe=historySwiped');
    const window = contractComponent('app-window.contract', 'T3Window');
    const state = { historySwipe: fns.historySwipeIdle() };
    window.call('historySwiped', state, { data: { sidebarOpen: true }, sidebarWidth: 256, viewport: { width: 1280, height: 840 }, historySwipeStep: fns.historySwipeStep }, [at('began', 0.125)]);
    expect(state.historySwipe).toMatchObject({ shown: true, left: 256, width: 1024, height: 840, progress: 0.5 });
  });
});
