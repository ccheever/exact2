import { expect, test } from 'bun:test';
import { contractComponent } from '../support/contract-action-fixture';
import { prTargetOf } from '../../src/panel/saved-surfaces';
import { prSurfaceId, selectionOf } from '../../src/panel/pr-surface';

const detail = 'src/pull-requests/pages-pr-detail.contract';

test('tour navigation clamps, restores focus and scrolls to the stop heading', () => {
  const component = contractComponent('src/pull-requests/pages-pr-guide.contract', 'GuideTour');
  const positions: number[] = [], focused: string[] = [], scrolled: unknown[][] = [];
  const state = { railOpen: true };
  const scope = { guide: { stopCount: 3 }, wide: false, navigate: (p: number) => positions.push(p), focus: (id: string) => focused.push(id), scrollIntoView: (...args: unknown[]) => scrolled.push(args), min: Math.min, max: Math.max };
  component.call('go', state, scope, [99]);
  expect(state.railOpen).toBe(false); expect(positions).toEqual([2]); expect(focused).toEqual(['guide-tour-picker']); expect(scrolled[0]).toEqual(['guide-tour-scroll', 'guide-tour-top', { block: 'start' }]);
  component.call('go', state, { ...scope, wide: true }, [-2]);
  expect(positions).toEqual([2, 0]); expect(focused[1]).toBe('guide-stop-0');
});

test('J/K stay within the visible guide and ignore editable focus, modal/palette state and modifiers', () => {
  const component = contractComponent(detail, 'PrdBody');
  const positions: number[] = [], prevented: boolean[] = [];
  const scope = { current: 'guide', detail: { guide: { hasGuide: true, position: 1, stopCount: 3 } }, guideKeysBlocked: false, dialogFocusId: '', querySelectorAll: () => ['guide-lines-question'], guideNavigate: (p: number) => positions.push(p), preventDefault: () => prevented.push(true), scrollIntoView: () => {}, min: Math.min, max: Math.max };
  const event = { metaKey: false, ctrlKey: false, altKey: false, shiftKey: false };
  component.call('guideKeys', {}, scope, ['j', event]);
  component.call('guideKeys', {}, scope, ['k', event]);
  for (const override of [{ current: 'code' }, { guideKeysBlocked: true }, { dialogFocusId: 'guide-lines-question' }]) component.call('guideKeys', {}, { ...scope, ...override }, ['j', event]);
  for (const modifier of Object.keys(event)) component.call('guideKeys', {}, scope, ['j', { ...event, [modifier]: true }]);
  expect(positions).toEqual([2, 0]); expect(prevented).toHaveLength(2);
});

test('ready guide maximizes once per open panel and preserves a manual restore across tab switches', () => {
  const component = contractComponent('src/shell/app-main.contract', 'PrGuideMaximize'), state = { reported: false }, sizes: boolean[] = [];
  const scope = { change: (size: boolean) => sizes.push(size) };
  component.call('observe', state, scope, [3, 1]);
  component.call('observe', state, scope, [2, 1]); component.call('observe', state, scope, [3, 1]);
  expect(sizes).toEqual([true]);
  component.call('observe', state, scope, [1, 1]); component.call('observe', state, scope, [3, 1]);
  expect(sizes).toEqual([true, false, true]);
});

test('each Finish request selects review mode without clearing drafts; saved PR surface retains its owner', () => {
  const component = contractComponent('src/pull-requests/pages-pr-compose.contract', 'PrdComposer');
  const state = { requestedReview: 0, mode: 'comment', summaryText: 'Keep this review', commentText: 'Keep this comment' };
  component.call('opening', state, { reviewRequest: 1, reviewStarted: false }, []); expect(state.mode).toBe('review');
  state.mode = 'comment'; component.call('opening', state, { reviewRequest: 2, reviewStarted: false }, []); expect(state.mode).toBe('review');
  expect(state.summaryText).toBe('Keep this review'); expect(state.commentText).toBe('Keep this comment');
  const target = prTargetOf({ projectId: 'p', host: 'github.com', repository: 'o/r', number: 3, url: 'https://github.com/o/r/pull/3', environmentId: 'original-owner' })!;
  expect(JSON.parse(selectionOf(target)).environmentId).toBe('original-owner'); expect(prSurfaceId(target)).toContain('original-owner');
});
