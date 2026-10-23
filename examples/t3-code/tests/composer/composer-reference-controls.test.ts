import {describe, expect, test} from 'bun:test';
import {contractComponent, type Env} from '../support/contract-action-fixture';
import {serializeAssistantCitation} from '../../src/timeline/diff-citations';

const root = contractComponent('app.contract', 'T3Code');
const references = contractComponent('composer-chip-controls.contract', 'ComposerChipControls');
const code = contractComponent('composer-code-headers.contract', 'ComposerCodeHeaders');

function fixture(surface = 'composer', token = '$frontend-design') {
  const value = `${token} later`;
  const chip = {start: 0, end: token.length, source: token};
  const owner = `${surface}-owner`, serial = 7;
  const current = {generation: 3, owner, value, chips: [chip], mode: 'rich'};
  const requests: unknown[] = [], frames: string[] = [];
  const bounds = {unavailable: false, x: 24, y: 480, width: 160, height: 28};
  const scope: Env = {
    richComposer: current, richPromptPreview: current, data: {snapshotOwner: 'snapshot'},
    composerFieldLive: true, composerFieldInputSerial: serial, richComposerOwner: owner,
    composerText: value, composerComposing: false,
    promptPreviewVisible: true, promptPreviewSerial: serial, promptPreviewOwner: owner, promptPreviewText: value,
    frame: (id: string) => {frames.push(id); return bounds;},
    ComposerChipRequest: (request: unknown) => request,
    composerChipPressed: (request: unknown) => requests.push(request),
  };
  function press(comment = false, props: Env = {}) {
    const anchor = `${surface}-chip-${comment ? 'comment-' : ''}${chip.start}`;
    references.call('press', {}, {
      surface, owner, value, serial, ...props,
      composerChipPress: (...args: unknown[]) => root.call('composerChipPress', {}, scope, args),
    }, [chip.start, chip.end, chip.source, comment, anchor]);
    return anchor;
  }
  return {scope, chip, current, owner, serial, value, requests, frames, bounds, press};
}

describe('ordinary reference controls dispatch through the real Contract actions', () => {
  test('a current reference uses its own button frame and exact source identity', () => {
    const f = fixture(), anchor = f.press();
    expect(f.frames).toEqual([anchor]);
    expect(f.requests).toEqual([{
      generation: 3, snapshotOwner: 'snapshot', owner: f.owner, value: f.value, inputSerial: 7,
      ...f.chip, surface: 'composer', comment: false, frame: f.bounds,
    }]);
  });

  test('stale owner, source value or input serial refuses before reading an anchor', () => {
    for (const props of [{owner: 'previous-owner'}, {value: 'previous source'}, {serial: 6}]) {
      const f = fixture(); f.press(false, props);
      expect(f.requests).toEqual([]); expect(f.frames).toEqual([]);
    }
  });

  test('ordinary blur alone does not erase a current reference, while later input still invalidates it', () => {
    const f = fixture(); f.scope.composerFocused = false; f.press();
    expect(f.requests).toHaveLength(1);
    f.requests.length = 0; f.frames.length = 0;
    f.scope.composerFieldInputSerial = f.serial + 1; f.press();
    expect(f.requests).toEqual([]); expect(f.frames).toEqual([]);
  });

  test('missing or zero-sized button geometry cannot open a reference', () => {
    for (const patch of [{unavailable: true}, {width: 0}, {height: 0}]) {
      const f = fixture(); Object.assign(f.bounds, patch); f.press();
      expect(f.frames).toHaveLength(1); expect(f.requests).toEqual([]);
    }
  });

  test('citation comment activation uses the comment button anchor', () => {
    const citation = serializeAssistantCitation({environmentId: 'env', threadId: 'thread', messageId: 'message', start: 0, end: 5, text: 'hello'});
    const f = fixture('composer', citation), anchor = f.press(true);
    expect(anchor).toBe('composer-chip-comment-0');
    expect(f.frames).toEqual([anchor]);
    expect(f.requests).toEqual([{
      generation: 3, snapshotOwner: 'snapshot', owner: f.owner, value: f.value, inputSerial: 7,
      ...f.chip, surface: 'composer', comment: true, frame: f.bounds,
    }]);
  });

  test('preview keeps its own identity and refuses when its surface is no longer visible', () => {
    const f = fixture('preview'); f.press();
    expect(f.requests).toHaveLength(1);
    expect(f.requests[0]).toMatchObject({surface: 'preview', owner: 'preview-owner'});
    f.requests.length = 0; f.frames.length = 0; f.scope.promptPreviewVisible = false; f.press();
    expect(f.requests).toEqual([]); expect(f.frames).toEqual([]);
  });

  test('a removed or duplicate source reference and a composing or covered field refuse dispatch', () => {
    const removed = fixture(); removed.current.chips = []; removed.press(); expect(removed.requests).toEqual([]);
    const duplicate = fixture(); duplicate.current.chips.push({...duplicate.chip}); duplicate.press(); expect(duplicate.requests).toEqual([]);
    for (const patch of [{composerComposing: true}, {composerFieldLive: false}]) {
      const f = fixture(); Object.assign(f.scope, patch); f.press();
      expect(f.requests).toEqual([]); expect(f.frames).toEqual([]);
    }
  });
});

describe('ordinary code-language control retains the current source owner', () => {
  function fixture() {
    const calls: unknown[][] = [], focus: string[] = [];
    const scope: Env = {
      composerRichCurrent: true, richComposer: {mode: 'rich'}, richComposerOwner: 'current-owner',
      composerText: '```js\nvalue\n```', composerComposing: false,
      focus: (id: string) => focus.push(id), composerIntentRequest: (...args: unknown[]) => calls.push(args),
    };
    return {scope, calls, focus, pick() {
      code.call('pick', {}, {
        composerLanguage: (...args: unknown[]) => root.call('composerLanguage', {}, scope, args),
      }, [0, 5, 'typescript']);
    }};
  }

  test('language selection preserves header range and reaches the current composer', () => {
    const f = fixture(); f.pick();
    expect(f.focus).toEqual(['composer']);
    expect(f.calls).toEqual([['language', 'typescript', '', false, false, 0, 5, false]]);
  });

  test('plain mode, a stale projection or composition cannot enqueue an edit', () => {
    // The projection's owner and source must be the field's (composerRichCurrent); the intent queue rechecks both.
    for (const patch of [{richComposer: {mode: 'plain'}}, {composerRichCurrent: false}, {composerComposing: true}]) {
      const f = fixture(); Object.assign(f.scope, patch); f.pick(); expect(f.focus).toEqual([]); expect(f.calls).toEqual([]);
    }
  });
});
