// Ported from T3 Code 7c3b3b3605ef apps/web/src/components/ui/middle-truncate.test.ts splitForMiddleTruncate (MIT,
// see LICENSE-T3), and the sidebar row's branch.
import { describe, expect, it } from 'bun:test';
import { splitForMiddleTruncate } from '../../src/sidebar/middle-truncate';

describe('splitForMiddleTruncate', () => {
  it('keeps a short last segment whole', () => {
    expect(splitForMiddleTruncate('feature/sidebar-fix')).toEqual({ head: 'feature/', tail: 'sidebar-fix' });
  });
  it('keeps the last ten characters of a long segment, snapped forward to a word', () => {
    expect(splitForMiddleTruncate('feature/sidebar-route-reveal-history')).toEqual({ head: 'feature/sidebar-route-reveal-', tail: 'history' });
  });
  it('leaves a value that fits its tail and four more alone', () => {
    expect(splitForMiddleTruncate('main')).toBeNull();
    expect(splitForMiddleTruncate('release-1')).toBeNull();
  });
  it('honours an explicit tail without snapping', () => {
    expect(splitForMiddleTruncate('abcdefghijklmnop', 3)).toEqual({ head: 'abcdefghijklm', tail: 'nop' });
  });
  it('matches the reference\'s own cases', () => {
    expect(splitForMiddleTruncate('feature/ui-polish')).toEqual({ head: 'feature/', tail: 'ui-polish' });
    expect(splitForMiddleTruncate('t3/clone-repo-to-other-machine')?.tail).toBe('machine');
    expect(splitForMiddleTruncate('t3/investigate-release-build-times')?.tail).toBe('times');
    expect(splitForMiddleTruncate('t3/compare-long-featurenames-ab')?.tail).toBe('featurenames-ab');
    expect(splitForMiddleTruncate('t3/abcdefghijklmnopqrstuvwxyz')?.tail).toBe('qrstuvwxyz');
    expect(splitForMiddleTruncate('t3/clone-repo-to-other-machine', 6)?.tail).toBe('achine');
  });
  it('never cuts a surrogate pair', () => {
    const split = splitForMiddleTruncate('🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂', 5)!;
    expect(Array.from(split.tail)).toHaveLength(5);
    expect(split.head + split.tail).toBe('🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂🙂');
  });
});
