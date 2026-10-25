import {describe, expect, test} from 'bun:test';
import {contractComponent, type Env} from '../support/contract-action-fixture';
import {serializeAssistantCitation} from '../../src/timeline/diff-citations';

const root = contractComponent('app.contract', 'T3Code');
const card = contractComponent('composer.contract', 'ComposerCard');
const preview = contractComponent('settings-prompt-preview.contract', 'PromptFontPreview');
const code = contractComponent('composer-code-headers.contract', 'ComposerCodeHeaders');

function fixture(surface = 'composer', token = '$frontend-design') {
  const value = `${token} later`;
  const chip = {start: 0, end: token.length, source: token};
  const owner = `${surface}-owner`, serial = 7;
  const current = {generation: 3, owner, value, chips: [chip], mode: 'rich'};
  const requests: unknown[] = [], frames: unknown[][] = [];
  const bounds = {unavailable: false, x: 24, y: 480, width: 160, height: 28};
  const scope: Env = {
    richComposer: current, richPromptPreview: current, data: {snapshotOwner: 'snapshot'},
    composerFieldLive: true, composerFieldInputSerial: serial, richComposerOwner: owner,
    composerText: value, composerComposing: false,
    promptPreviewVisible: true, promptPreviewSerial: serial, promptPreviewOwner: owner, promptPreviewText: value,
    rangeFrame: (...args: unknown[]) => {frames.push(args); return bounds;},
    ComposerChipRequest: (request: unknown) => request,
    composerChipPressed: (request: unknown) => requests.push(request),
  };
  // The field's atompress, through the surface's own action, with the AtomEvent the editor hands it.
  function press(event: Env = {}, props: Env = {}) {
    const e = {kind: token.startsWith('[') ? 'citation' : 'skill', source: chip.source, href: '', start: chip.start, end: chip.end,
      shiftKey: false, ctrlKey: false, altKey: false, metaKey: false, ...event};
    const composerChipPress = (...args: unknown[]) => root.call('composerChipPress', {}, scope, args);
    if (surface === 'preview') {
      preview.call('pressed', {}, {promptPreviewOwner: owner, promptPreviewText: value, promptPreviewSerial: serial, composerChipPress, ...props}, [e]);
    } else {
      card.call('richAtom', {}, {richComposerOwner: owner, text: value, composerFieldInputSerial: serial, composerChipPress, ...props}, [e]);
    }
  }
  return {scope, chip, current, owner, serial, value, requests, frames, bounds, press};
}

describe('an atom press dispatches through the real Contract actions', () => {
  test('a current reference anchors at its pill and keeps its exact source identity', () => {
    const f = fixture(); f.press();
    expect(f.frames).toEqual([['composer', 0, f.chip.end]]);
    expect(f.requests).toEqual([{
      generation: 3, snapshotOwner: 'snapshot', owner: f.owner, value: f.value, inputSerial: 7,
      ...f.chip, surface: 'composer', comment: false, frame: f.bounds,
    }]);
  });

  test('stale owner, source value or input serial refuses before reading an anchor', () => {
    for (const props of [{richComposerOwner: 'previous-owner'}, {text: 'previous source'}, {composerFieldInputSerial: 6}]) {
      const f = fixture(); f.press({}, props);
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

  test('missing or zero-sized pill geometry cannot open a reference', () => {
    for (const patch of [{unavailable: true}, {width: 0}, {height: 0}]) {
      const f = fixture(); Object.assign(f.bounds, patch); f.press();
      expect(f.frames).toHaveLength(1); expect(f.requests).toEqual([]);
    }
  });

  test('an Option press on a citation opens its comment, the reference chip\'s pencil', () => {
    const citation = serializeAssistantCitation({environmentId: 'env', threadId: 'thread', messageId: 'message', start: 0, end: 5, text: 'hello'});
    const f = fixture('composer', citation);
    f.press(); f.press({altKey: true});
    expect(f.requests.map(request => (request as {comment: boolean}).comment)).toEqual([false, true]);
    const skill = fixture(); skill.press({altKey: true});
    expect(skill.requests).toMatchObject([{comment: false}]);
  });

  test('preview keeps its own identity and refuses when its surface is no longer visible', () => {
    const f = fixture('preview'); f.press();
    expect(f.frames).toEqual([['prompt-font-preview', 0, f.chip.end]]);
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
