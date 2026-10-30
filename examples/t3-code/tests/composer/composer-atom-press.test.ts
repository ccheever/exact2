import {describe, expect, test} from 'bun:test';
import {contractComponent, contractFns, type Env} from '../support/contract-action-fixture';
import {readFileSync} from 'node:fs';
import {appPath} from '../support/app-files';
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

describe('a chip\'s face is its pill', () => {
  const fns = contractFns('src/composer/composer-rich.contract', {AtomFace: (face: Env) => face, ...contractFns('src/timeline/markdown-chip-inks.contract')}) as {
    composerAtomFace: (chip: Env, upload: string) => Env; composerAtomSymbol: (icon: string) => string;
  };
  const atomFace = (chip: Env, upload = '') => fns.composerAtomFace(chip, upload);
  const face = (patch: Env) => ({kind: 'mention', label: 'README.md', suffix: '', icon: 'at-sign', fileToken: '', imageId: '', available: true,
    interactive: true, comment: false, tip: 'README.md', glyph: {kind: ''}, ariaLabel: 'Preview README.md', ...patch});
  test('label and suffix, the kind\'s SF Symbol, and the tooltip', () => {
    expect(atomFace({kind: 'mention', source: '@README.md', path: 'README.md', face: face({})})).toEqual({source: '@README.md', label: 'README.md', icon: 'symbol:sf/doc',
      title: 'README.md', color: '', backgroundColor: '', direction: 'ltr', ariaLabel: 'Preview README.md'});
    expect(atomFace({source: '[x](t3-context://v1/terminal/a)', face: face({kind: 'terminal', label: 'Terminal', suffix: 'lines 1-4', icon: 'square-terminal', tip: ''})}))
      .toMatchObject({label: 'Terminal lines 1-4', icon: 'symbol:sf/apple.terminal', title: ''});
    // A citation's tooltip says how its comment opens; the preview's citation has no comment.
    expect(atomFace({source: '[q](t3-citation://v1/e/t/m)', face: face({kind: 'citation', label: 'the quote', icon: 'quote', tip: 'the quote', comment: true})}))
      .toMatchObject({icon: 'symbol:sf/quote.opening', title: 'the quote\n⌥-click to comment'});
  });
  test('an image\'s chip follows its size with its upload\'s state (ImageContextChip)', () => {
    const image = {kind: 'context', contextKind: 'image', source: '[sunset.png](t3-context://v1/image/i1)', label: 'sunset.png',
      face: face({kind: 'image', label: 'sunset.png', suffix: '8.0 MB', icon: 'image', imageId: 'img-0', tip: ''})};
    expect([atomFace(image).label, atomFace(image, '25%').label, atomFace(image, 'upload failed').label])
      .toEqual(['sunset.png 8.0 MB', 'sunset.png 8.0 MB 25%', 'sunset.png 8.0 MB upload failed']);
    // Its accessible name stays ImageChipButton's aria-label while the drawn label follows the upload.
    image.face.ariaLabel = 'Image attachment, sunset.png, 8.0 MB';
    expect(atomFace(image, '25%').ariaLabel).toBe('Image attachment, sunset.png, 8.0 MB');
  });
  test('a file mention keeps its icon before its path in a right-to-left line; the other chips follow their line', () => {
    expect(atomFace({kind: 'mention', source: '[plan.md](docs/plan.md)', path: 'docs/plan.md', face: face({label: 'plan.md'})}).direction).toBe('ltr');
    for (const kind of ['skill', 'context', 'citation']) expect(atomFace({kind, source: 'x', face: face({kind})}).direction).toBe('');
  });
  test('a document chip takes its type\'s accent over its kind\'s paint (documentIconColorForFileName)', () => {
    const paint = (chip: Env) => { const {color, backgroundColor} = atomFace(chip); return [color, backgroundColor]; };
    // A workspace mention by its path, in either spelling of the source.
    expect(paint({kind: 'mention', source: '@docs/Report.PDF', path: 'docs/Report.PDF', face: face({label: 'Report.PDF'})}))
      .toEqual(['light-dark(#4c2e2f, #fcd8d4)', 'light-dark(#d52c361c, #ff67621c)']);
    expect(paint({kind: 'mention', source: '[plan.xlsx](plan.xlsx)', path: 'plan.xlsx', face: face({label: 'plan.xlsx'})}))
      .toEqual(['light-dark(#2d4032, #d7edd8)', 'light-dark(#199f431c, #5ecc711c)']);
    // An attached file by its name. One whose bytes were lost has no record (fileChipContexts), so its face is
    // unavailable: it draws unresolved, keeping its accent's fill with the field's text (ContextChip's `unresolved`).
    const attached = (patch: Env) => ({kind: 'context', source: '[brief.docx](t3-context://v1/file/f1)', label: 'brief.docx', contextKind: 'file',
      face: face({kind: 'file', label: 'brief.docx', icon: 'file', ...patch})});
    expect(paint(attached({}))).toEqual(['light-dark(#2b3b4c, #d7e7f9)', 'light-dark(#1a85d41c, #69b1ff1c)']);
    expect(paint(attached({kind: 'neutral', available: false, icon: 'circle-dashed'}))).toEqual(['', 'light-dark(#1a85d41c, #69b1ff1c)']);
    // Another context reference by a document's name is not a file: a thread titled so keeps its kind's paint.
    expect(paint({...attached({kind: 'thread', icon: 'messages-square'}), contextKind: 'thread'})).toEqual(['', '']);
    // Anything else keeps its kind's paint: a source file, a folder, a video, a skill.
    expect(paint({kind: 'mention', source: '@src/app.ts', path: 'src/app.ts', face: face({})})).toEqual(['', '']);
    expect(paint({kind: 'mention', source: '@docs', path: 'docs', face: face({icon: 'folder'})})).toEqual(['', '']);
    expect(paint({...attached({kind: 'video', icon: 'film'}), label: 'clip.pdf'})).toEqual(['', '']);
    expect(paint({kind: 'skill', source: '$deck.pptx', name: 'deck.pptx', face: face({kind: 'skill'})})).toEqual(['', '']);
  });
  test('each glyph the faces use has an SF Symbol, and an unknown one the dashed circle', () => {
    const symbols = Object.fromEntries(['at-sign', 'file', 'folder', 'sparkles', 'film', 'quote', 'image', 'messages-square', 'square-terminal',
      'message-circle', 'mouse-pointer-2', 'git-merge', 'git-pull-request', 'git-pull-request-draft', 'git-pull-request-closed', 'circle-dashed', 'nope']
      .map(icon => [icon, fns.composerAtomSymbol(icon)]));
    expect(symbols).toEqual({'at-sign': 'doc', file: 'doc', folder: 'folder', sparkles: 'sparkles', film: 'film', quote: 'quote.opening', image: 'photo',
      'messages-square': 'bubble.left.and.bubble.right', 'square-terminal': 'apple.terminal', 'message-circle': 'bubble.left', 'mouse-pointer-2': 'cursorarrow',
      'git-merge': 'arrow.triangle.merge', 'git-pull-request': 'arrow.triangle.pull', 'git-pull-request-draft': 'arrow.triangle.pull',
      'git-pull-request-closed': 'arrow.triangle.pull', 'circle-dashed': 'circle.dashed', nope: 'circle.dashed'});
  });
  test('the skill atom begins with every currency symbol composerChips\' skill token does', () => {
    const declared = /^atom skill prefix="([^"]+)"/m.exec(readFileSync(appPath('src/composer/composer-rich.contract'), 'utf8'))?.[1] ?? '';
    let currency = '';
    for (let c = 0; c <= 0x10ffff; c++) if ((c < 0xd800 || c > 0xdfff) && /\p{Sc}/u.test(String.fromCodePoint(c))) currency += String.fromCodePoint(c);
    expect(declared).toBe(currency);
  });
});

describe('focus coming into the composer expands a resting one (ChatComposer onFocusCapture)', () => {
  test('a focus ends the rest', () => {
    const state: Env = {composerFocused: false, composerComposing: false, composerHeldMode: '', restResting: true};
    root.call('composerFocus', state, {richComposer: {mode: 'rich'}, modifierEditableFocus: () => {}}, []);
    expect([state.composerFocused, state.restResting]).toEqual([true, false]);
  });
});

describe('a change of mode remounts the composer and keeps its focus', () => {
  const run = (focused: boolean) => {
    const calls: unknown[][] = [];
    const state: Env = {composerFocused: false, composerComposing: false, composerHeldMode: '', composerTyping: false, composerShift: false, composerCommand: false, restResting: false};
    const scope = (mode: string): Env => ({richComposer: {mode}, composerText: 'Look at @README.md later', composerSelectionStart: 12, composerSelectionEnd: 12, min: Math.min,
      modifierEditableFocus: () => {}, focus: (id: string) => calls.push(['focus', id]), setSelectionRange: (...args: unknown[]) => calls.push(['select', ...args])});
    if (focused) root.call('composerFocus', state, scope('rich'), []);
    // The setting turns Markdown off: the field is replaced, and its removal blurs it after the mode changed.
    if (focused) root.call('composerBlur', state, scope('plain'), []);
    root.call('composerRestore', state, scope('plain'), []);
    return calls;
  };
  test('a field focused across the change takes back the focus at its selection; an unfocused one does not', () => {
    expect(run(true)).toEqual([['focus', 'composer'], ['select', 'composer', 12, 12]]);
    expect(run(false)).toEqual([]);
  });
  test('a blur before the change ends the hold', () => {
    const calls: unknown[][] = [];
    const state: Env = {composerFocused: false, composerComposing: false, composerHeldMode: '', composerTyping: false, composerShift: false, composerCommand: false, restResting: false};
    const scope = (mode: string): Env => ({richComposer: {mode}, composerText: 'x', composerSelectionStart: 1, composerSelectionEnd: 1, min: Math.min, modifierEditableFocus: () => {},
      focus: (id: string) => calls.push(['focus', id]), setSelectionRange: (...args: unknown[]) => calls.push(['select', ...args])});
    root.call('composerFocus', state, scope('rich'), []);
    root.call('composerBlur', state, scope('rich'), []);
    root.call('composerRestore', state, scope('plain'), []);
    expect(calls).toEqual([]);
  });
});
