// A flow that holds state across a wait on its own timer has a send no other flow replaces: an answer let go meanwhile
// has its timers cleared and never resumes (LLP 1027.000 D1), so its `finally` would never run. A queued send is never
// let go; a send its flow shares only with its own next step is let go only by a step that takes its state over.
import { describe, expect, test } from 'bun:test';
import { contractComponent } from '../support/contract-action-fixture';
import { appText } from '../support/app-files';

const queued = (mutation: string) => new RegExp(`^  mutation ${mutation} as shape \\w+ queue\\b`, 'm').test(appText('app.contract'));
/** The mutation each call of the root's `action` sends on, with its command's op. */
function sends(action: string, calls: unknown[][], scope: object = {}): string[] {
  const root = contractComponent('app.contract', 'T3Code'), out: string[] = [];
  const env = { command: (op: string) => op, sent: (mutation: string, op: string) => out.push(`${mutation} ${op}`),
    wallTime: { epochAtZero: 0 }, performanceNow: () => 0, ...scope };
  for (const args of calls) root.call(action, { foldHold: false, rightPanelAt: '' }, env, args);
  return out;
}

describe('the flows that wait on their own timers have sends of their own', () => {
  test('the Browser\'s Capture queues, so a frame report during a Record start\'s paint wait lets nothing go', () => {
    expect(sends('chatLocal', [['surface-browser-capture', 'tab-1', 'record'], ['surface-browser-frame-done', 'tab-1', '']]))
      .toEqual(['capturePressed chatlocal:surface-browser-capture', 'localChanged chatlocal:surface-browser-frame-done']);
    expect(queued('capturePressed')).toBe(true);
  });

  test('the checkout dialog\'s edits share a send only with each other, so only the next edit, which takes the debounce over, lets one go', () => {
    expect(sends('chatLocal', [['git-pr-text', '', '#12'], ['git-pr-close', '', '']])).toEqual(['prCheckoutTyped chatlocal:git-pr-text', 'localChanged chatlocal:git-pr-close']);
    expect(queued('prCheckoutTyped')).toBe(false);
  });

  test('View source goes on citationShown from a transcript chip and from the composer; the composer\'s other chip acts do not', () => {
    expect(sends('chatLocal', [['chip-citation', 't2', 'b1']])).toEqual(['citationShown chatlocal:chip-citation']);
    expect(sends('editorOp', [['chip-act', '7', '']], { chip: { act: 'citation' } })).toEqual(['citationShown editorlocal:chip-act']);
    expect(sends('editorOp', [['chip-act', '8', '']], { chip: { act: 'mention' } })).toEqual(['localChanged editorlocal:chip-act']);
    expect(sends('editorOp', [['stash', '', '']], { chip: { act: 'citation' } })).toEqual(['localChanged editorlocal:stash']);
  });

  test('the prompt links queue beside a fragment link\'s press, so neither lets a switch to another machine\'s Scratch project go', () => {
    expect(sends('promptLinkAct', [['open']])).toEqual(['fragmentChanged shelllocal:prompt-link-action']);
    expect(sends('chatLocal', [['link-open', 'm1', '#notes']])).toEqual(['fragmentChanged chatlocal:link-open']);
    expect(queued('fragmentChanged')).toBe(true);
  });

  test('a sound\'s start and stop queue beside the pages\' reports', () => {
    expect(sends('chatLocal', [['audio-playing', 'row', '1memo.wav'], ['math-frame', 'row', '']])).toEqual(['frameReported chatlocal:audio-playing', 'frameReported chatlocal:math-frame']);
    expect(queued('frameReported')).toBe(true);
  });
});
