// ThreadStatusIndicators.tsx PullRequestBadge (T3 Code 74541bab17, MIT; see LICENSE-T3): the badge's tooltip is a popup the
// pointer can enter, its rows links to their pull requests, the list held to the window's height. Here the badge hands
// the window's hover layer a "pr-badge" card (strip-pr-badge.contract StripPrBadge, StripPrTipCard).
import { describe, expect, test } from 'bun:test';
import { contractComponent, contractFns } from '../support/contract-action-fixture';

const badge = contractComponent('strip-pr-badge.contract', 'StripPrBadge');
const row = contractComponent('strip-pr-badge.contract', 'StripPrTipRow');
const layer = contractFns('hover-layer.contract', { HoverTip: (record: unknown) => record });
const tips = [{ key: 'a', number: '#7', title: 'Keep the notes', icon: 'git-pull-request-arrow', state: 'open', url: 'https://github.com/o/r/pull/7', target: '{"number":7}', depth: 0, stack: '' }];

describe('the composer badge\'s pull request list', () => {
  test('the pointer on the badge opens the "pr-badge" card over the badge; leaving starts its close', () => {
    const asked: unknown[][] = [];
    const scope = { tips, rem: 16, frame: () => ({ x: 600, y: 700, width: 40, height: 24 }), hoverTipAtFrame: layer.hoverTipAtFrame, hoverTipAt: (...args: unknown[]) => asked.push(args) };
    const state = badge.state();
    badge.call('hover', state, scope, [true]);
    badge.call('hover', state, scope, [false]);
    expect(asked.map(([tip, part, inside]) => [(tip as { kind: string }).kind, (tip as { y: number }).y, part, inside])).toEqual([['pr-badge', 700, 'trigger', true], ['pr-badge', 700, 'trigger', false]]);
    // 600 ms to open (Base UI's tooltip delay), 100 ms of grace to cross into the card.
    expect([layer.hoverOpenDelay('pr-badge'), layer.hoverDelay('pr-badge')]).toEqual([600, 100]);
  });
  test('a row opens its pull request (the panel where a project reads it), ⌘ or Ctrl the browser', () => {
    const sent: unknown[][] = [];
    const scope = { tip: tips[0], command: (...args: unknown[]) => sent.push(args) };
    row.call('open', {}, scope, [{ metaKey: false, ctrlKey: false }]);
    row.call('open', {}, scope, [{ metaKey: true, ctrlKey: false }]);
    expect(sent).toEqual([['chatlocal:surface-linked-pr-open', '{"number":7}', 'https://github.com/o/r/pull/7', 0], ['chatlocal:surface-linked-pr-open', 'held', 'https://github.com/o/r/pull/7', 0]]);
  });
});
