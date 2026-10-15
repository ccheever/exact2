// The composer chips' presses against T3 Code 1e2ecbd975 (MIT, LICENSE-T3): composerContextPresentation.tsx (the
// review comment's and preview annotation's ContextChipPopover, ComposerContextActions), contextChipParts.tsx
// PullRequestChip, and MessagesTimeline.tsx's sent pull request chip.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Obj } from './domain';
import type { Native } from './protocol';
import { chipPopover } from './composer-chip-popover';
import { chipAct, composerImagePreview, composerImageStep, previewAnnotationSummary, reviewCommentDetails } from './composer-chip-details';
import { adoptReviewComments, pullRequestRecords } from './composer-editor';
import { messageChips } from './r4-timeline-chips';
import type { PreviewAnnotationPayload } from './browser-annotation';

const comment = { version: 1, kind: 'review-comment', contextId: 'review-comment_x', label: 'app.ts L3', sectionId: 's1', sectionTitle: 'Working tree', filePath: 'src/app.ts',
  startIndex: 0, endIndex: 1, rangeLabel: '+3', text: ' Rename this. ', diff: '@@ -1 +1 @@\n-a\n+b' };
const annotation: PreviewAnnotationPayload = { id: 'a1', pageUrl: 'http://localhost:5173/', pageTitle: 'Home', comment: 'Make it blue', elements: [], regions: [{ id: 'r', rect: { x: 0, y: 0, width: 1, height: 1 } }],
  strokes: [], styleChanges: [{ targetId: 't', selector: null, property: 'color', previousValue: '', value: 'blue' }, { targetId: 't', selector: null, property: 'margin', previousValue: '', value: '0' }], screenshot: null, createdAt: '' };

describe('a review comment\'s and a preview annotation\'s details', () => {
  test('ComposerReviewCommentDetails: the path, "<section> · <range>", the comment and the diff; titled by its kind', () => {
    expect(reviewCommentDetails(comment, 'app.ts L3')).toEqual({ kind: 'review-comment', label: 'src/app.ts', title: 'Review comment, app.ts L3', description: 'Rename this.',
      lines: 'Working tree · +3', rows: 3, diff: '@@ -1 +1 @@\n-a\n+b', image: '', aspect: 0 });
    // A legacy pull request summary (no metadata) keeps the popover with "Pull request" as its kind; one with metadata is a button.
    expect(reviewCommentDetails({ ...comment, sectionId: 'pull-request:7', filePath: 'PR #7', diff: '' }, '#7')?.title).toBe('Pull request, #7');
    expect(reviewCommentDetails({ ...comment, pullRequest: { number: 7 } }, '#7')).toBeNull();
  });
  test('previewAnnotationTooltip: the page, the comment and what it marks', () => {
    expect(previewAnnotationSummary(annotation)).toBe('Home\n\nMake it blue\n\n1 region, 2 style changes');
    expect(previewAnnotationSummary({ ...annotation, pageTitle: ' ', comment: '', regions: [], styleChanges: [] })).toBe('http://localhost:5173/');
  });
  test('the editor\'s press opens them from the draft\'s records, at the chip', () => {
    const press = { seq: 5, open: true, surface: 'composer', kind: 'review-comment', context: 'review-comment_x', name: 'app.ts L3', label: 'app.ts L3', frame: [10, 400, 80, 19] };
    expect(chipPopover(press, [], undefined, { 'review-comment_x': comment })).toMatchObject({ seq: 5, open: true, kind: 'review-comment', title: 'Review comment, app.ts L3', x: 10, y: 400, width: 80, height: 19 });
    const shot = { annotation, imageId: '11111111-2222-3333-4444-555555555555', width: 400, height: 200 };
    expect(chipPopover({ ...press, kind: 'preview-annotation', context: 'preview-annotation_a1' }, [], undefined, { 'preview-annotation_a1': shot }))
      .toMatchObject({ open: true, kind: 'preview-annotation', title: 'Preview annotation, Make it blue', image: shot.imageId, aspect: 0.5 });
    // No record: nothing opens.
    expect(chipPopover({ ...press, context: 'gone' }, [], undefined, {}).open).toBe(false);
    // An action press is closed and names its action.
    expect(chipPopover({ seq: 6, open: false, surface: 'composer', act: 'thread', context: 'thread_t2' }, [])).toMatchObject({ seq: 6, open: false, act: 'thread' });
  });
});

describe('the review comments behind the draft\'s chips are saved with it', () => {
  test('adoptReviewComments keeps the records a saved draft names', () => {
    const next: Obj = {};
    adoptReviewComments(next, { drafts: { d1: 'see [app.ts L3](t3-context://v1/review-comment/review-comment_x)' }, reviewComments: { 'review-comment_x': comment, 'review-comment_y': { ...comment, contextId: 'review-comment_y' }, bad: { kind: 'x' } } });
    expect(Object.keys(next.reviewComments as Obj)).toEqual(['review-comment_x']);
    const client = { local: next } as unknown as T3Client;
    expect(pullRequestRecords(client, '[app.ts L3](t3-context://v1/review-comment/review-comment_x)')).toEqual([comment]);
  });
});

function harness(press: Obj, extra: Obj = {}) {
  const calls: Obj[] = [];
  const native = { available: true, watch() {}, later: async (request: Obj) => { calls.push(request); return { ok: true, generation: 0, value: request.op === 'editorChip' ? press : {} }; } } as unknown as Native;
  const client = { snapshotOwner: 'draft:t1', draftKey: 'draft:t1', local: { drafts: {} }, snapshotDrafts: [{ id: 'img-1', name: 'one.png' }, { id: 'img-2', name: 'two.png' }], ...extra } as unknown as T3Client;
  return { calls, native, client };
}

describe('an action press is taken once and performed', () => {
  test('an image opens the expanded preview over the draft\'s images, stepping round', async () => {
    const { calls, native, client } = harness({ seq: 9, open: false, surface: 'composer', owner: 'draft:t1', act: 'image', context: 'image_img-2' });
    expect(await chipAct(client, native, 9)).toBe('');
    expect(calls.map(call => call.op)).toEqual(['editorChip', 'editorChipClose']);
    expect(calls[1]).toMatchObject({ seq: 9 });
    // A client that cannot read the images' sizes leaves them unknown: the preview keeps the whole media box.
    expect(composerImagePreview(client)).toEqual([{ open: true, id: 'img-2', name: 'two.png', position: '(2/2)', many: true, width: 0, height: 0 }]);
    composerImageStep(client, 'chip-image-next');
    expect(composerImagePreview(client)[0]?.id).toBe('img-1');
    composerImageStep(client, 'chip-image-previous');
    expect(composerImagePreview(client)[0]?.id).toBe('img-2');
    composerImageStep(client, 'chip-image-close');
    expect(composerImagePreview(client)).toEqual([]);
  });
  test('opening reads each draft image\'s natural size once (ZoomableImage\'s onLoad), so the preview fits the image', async () => {
    const sizes: Obj = { 'img-3': { width: 640, height: 480 } };
    const asked: string[] = [];
    const { native, client } = harness({ seq: 4, open: false, surface: 'composer', owner: 'draft:t1', act: 'image', context: 'image_img-3' }, {
      snapshotDrafts: [{ id: 'img-3', name: 'three.png' }, { id: 'img-4', name: 'four.png' }],
      call: async (_native: Native, request: Obj) => { asked.push(`${request.op} ${request.id}`); if (!sizes[String(request.id)]) throw new Error('unreadable'); return sizes[String(request.id)]; } });
    expect(await chipAct(client, native, 4)).toBe('');
    expect(asked).toEqual(['snapshotDraftSize img-3', 'snapshotDraftSize img-4']);
    expect(composerImagePreview(client)[0]).toMatchObject({ id: 'img-3', width: 640, height: 480 });
    composerImageStep(client, 'chip-image-next');
    expect(composerImagePreview(client)[0]).toMatchObject({ id: 'img-4', width: 0, height: 0 });
    composerImageStep(client, 'chip-image-close');
  });
  test('a stale press, another surface\'s or another draft\'s is dropped', async () => {
    for (const [press, seq] of [[{ seq: 9, open: false, surface: 'composer', act: 'image', context: 'image_img-1' }, 8], [{ seq: 9, open: false, surface: 'preview', act: 'image', context: 'image_img-1' }, 9],
      [{ seq: 9, open: false, surface: 'composer', owner: 'draft:other', act: 'image', context: 'image_img-1' }, 9]] as const) {
      const { native, client } = harness(press);
      await chipAct(client, native, seq);
      expect(composerImagePreview(client)).toEqual([]);
    }
  });
  test('a thread opens it, a mention its file, a missing pull request says so', async () => {
    const opened: string[] = [];
    const { native, client } = harness({ seq: 3, open: false, surface: 'composer', act: 'thread', context: 'thread_t2' },
      { shell: { threads: [{ id: 't2' }], projects: [] }, openSelected: async (_: Native, id: string) => { opened.push(id); } });
    await chipAct(client, native, 3);
    expect(opened).toEqual(['t2']);
    const missing = harness({ seq: 4, open: false, surface: 'composer', act: 'pr-open', context: 'review-comment_gone' });
    await expect(chipAct(missing.client, missing.native, 4)).rejects.toThrow('That pull request is no longer available.');
  });
});

describe('a sent pull request chip opens the pull request (PullRequestChip)', () => {
  test('"#N" with its state, named "Open <state> pull request #N: <title>"', () => {
    const item = { type: 'user_message', text: '[#12](t3-context://v1/review-comment/pr12) and [app.ts L3](t3-context://v1/review-comment/review-comment_x)', attachments: [],
      context: { version: 1, records: [{ ...comment, contextId: 'pr12', label: '#12', pullRequest: { number: 12, title: 'Fix tabs', url: 'https://github.com/o/r/pull/12', state: 'open', isDraft: true } }, comment] } };
    expect(messageChips(item, '', []).map(chip => [chip.kind, chip.label, chip.tip, chip.detail, chip.target])).toEqual([
      ['pr', '#12', 'Open Draft pull request #12: Fix tabs', 'draft', 'https://github.com/o/r/pull/12'],
      ['review', 'app.ts L3', 'src/app.ts', 'Working tree · +3\nRename this.', '']]);
  });
});
