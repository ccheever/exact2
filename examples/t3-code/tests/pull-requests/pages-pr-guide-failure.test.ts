// A failed review guide (T3 Code 2359643cff, PullRequestGuideTab.tsx #26073): the empty state says it could not write
// one, points at the review guide model and keeps Try again; the run's error sits behind "Technical details". A failed
// rewrite says so in the banner over the old guide, with the same disclosure.
import { describe, expect, test } from 'bun:test';
import { contractComponent, value } from '../support/contract-action-fixture';
import { appText } from '../support/app-files';

const tour = contractComponent('pages-pr-guide.contract', 'GuideTour');
const details = contractComponent('pages-pr-guide.contract', 'GuideFailureDetails');
const hint = 'Check the review guide model in Settings → Source control.';
const guide = (over: Record<string, unknown>) => ({ hasGuide: false, phase: 'empty', running: false, failure: '', error: '', canWrite: true, stale: false, stopCount: 0, ...over });
const scope = (over: Record<string, unknown>) => ({ guide: guide(over), width: 1200, rem: 16, pgtFailureHint: () => hint });

describe('a failed review guide', () => {
  test('the empty state reads as a failure, with the hint, Try again and the details', () => {
    const source = appText('pages-pr-guide.contract');
    const title = /text (\(guide\.running \? "Writing a review guide".*?\)) font-size/.exec(source)![1]!;
    const body = /text (\(guide\.running \? "An agent is reading.*?\)) max-width/.exec(source)![1]!;
    const failed = scope({ failure: 'model unavailable' });
    expect(value(title, failed)).toBe('Could not write a review guide');
    expect(value(body, failed)).toBe(hint);
    expect(tour.drawn('GuideFailureDetails(detail=guide.failure, testId="pull-request-guide-failure-details")', {}, failed)).toBe(true);
    const fresh = scope({});
    expect(value(title, fresh)).toBe('No review guide yet');
    expect(tour.drawn('GuideFailureDetails(detail=guide.failure, testId="pull-request-guide-failure-details")', {}, fresh)).toBe(false);
    expect(tour.drawn('GuideFailureDetails(detail=guide.failure, testId="pull-request-guide-failure-details")', {}, scope({ failure: 'x', running: true }))).toBe(false);
    expect(source).toContain('label=(guide.failure != "" ? "Try again" : "Write guide")');
  });
  test('a failed rewrite keeps the guide and says so in its banner', () => {
    const source = appText('pages-pr-guide.contract');
    const banner = /text (\(guide\.running \? "Rewriting this guide.*?\)) font-size/.exec(source)![1]!;
    expect(value(banner, scope({ hasGuide: true, failure: 'boom' }))).toBe(`Could not rewrite this guide. ${hint}`);
    expect(value(banner, scope({ hasGuide: true, stale: true }))).toBe('Written before the latest push.');
  });
  test('Technical details opens to the error', () => {
    const state = details.state({});
    expect(details.drawn('testId=`${testId}-text`', state, { detail: 'boom', testId: 't' })).toBe(false);
    details.call('toggle', state, { detail: 'boom', testId: 't' }, []);
    expect(details.drawn('testId=`${testId}-text`', state, { detail: 'boom', testId: 't' })).toBe(true);
  });
});
