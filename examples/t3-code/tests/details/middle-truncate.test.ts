// Where a label keeps its tail when it is cut (T3 Code 7c3b3b3605ef, MIT, see LICENSE-T3: the original tests of
// apps/web/src/components/ui/middle-truncate.test.ts, with their names), and the details card's branch row drawing
// the head and the tail apart.
import { describe, expect, test } from 'bun:test';
import { splitForMiddleTruncate } from '../../src/details/middle-truncate';
import { contractComponent } from '../support/contract-action-fixture';

describe('splitForMiddleTruncate', () => {
  test('keeps a short last path segment whole', () => {
    expect(splitForMiddleTruncate('feature/ui-polish')).toEqual({ head: 'feature/', tail: 'ui-polish' });
  });
  test('starts a long segment\'s tail on a word, not mid-word', () => {
    expect(splitForMiddleTruncate('t3/clone-repo-to-other-machine')?.tail).toBe('machine');
    expect(splitForMiddleTruncate('t3/investigate-release-build-times')?.tail).toBe('times');
  });
  test('falls back to an earlier word when the last one is too short', () => {
    expect(splitForMiddleTruncate('t3/compare-long-featurenames-ab')?.tail).toBe('featurenames-ab');
  });
  test('keeps the fixed tail when no word break is near', () => {
    expect(splitForMiddleTruncate('t3/abcdefghijklmnopqrstuvwxyz')?.tail).toBe('qrstuvwxyz');
  });
  test('respects an explicit tail length', () => {
    expect(splitForMiddleTruncate('t3/clone-repo-to-other-machine', 6)?.tail).toBe('achine');
  });
});

test('the branch row draws a split label as its head and tail, and a short one whole', () => {
  const rows = contractComponent('git.contract', 'GitRows');
  const git = (branch: { label: string; labelHead: string; labelTail: string }) => ({ KmItem: (item: unknown) => item, details: { items: { branch: true }, git: { menu: [], branch: { show: true, ...branch } } } });
  const split = git({ label: 'feature/ui-polish', labelHead: 'feature/', labelTail: 'ui-polish' });
  expect(rows.drawn('text details.git.branch.labelTail', rows.state(), split)).toBe(true);
  expect(rows.drawn('text details.git.branch.label title=', rows.state(), split)).toBe(false);
  expect(splitForMiddleTruncate('main')).toBeNull();
  expect(rows.drawn('text details.git.branch.label title=', rows.state(), git({ label: 'main', labelHead: '', labelTail: '' }))).toBe(true);
});
