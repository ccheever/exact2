// Cite's comment popover and the rest of Cite, against T3 Code 1e2ecbd975 (MIT, LICENSE-T3):
// AssistantCitationChip.tsx, AssistantCitationCommentEditor.tsx, assistantCitationCommentDismissal.ts,
// ComposerPromptEditorTiptap.tsx ComposerCitationNodeView, ChatComposer.tsx citeAssistantText and submitCitationAndSend,
// lib/assistantTextSelection.ts readAssistantText. The reference's numbers were read over CDP on the reference with
// its Cite fixture answer (a paragraph with inline code, a ```ts block, a closing paragraph), 1280×840.
import { describe, expect, test } from 'bun:test';
import type { Obj } from '../../src/client/domain';
import { chipPopover, NO_CHIP } from '../../src/composer/composer-chip-popover';
import { citationCommentLocal, citationOfLink, citationPopover, citedStart, openCitationComment, rememberCitedStart } from '../../src/timeline/citation-comment';
import { createAssistantTextSelector, formatAssistantCitationHref, resolveAssistantCitationCommentDismissal, type AssistantCitation } from '../../src/timeline/diff-citations';
import { parseCiteValue } from '../../src/diff/diff-review';
import { composerChipClose, composerChipCurrent, composerChipFinishView, takeComposerChipFinish } from '../../src/composer/composer-chip-bridge';
import { citationField } from './citation-field.fixture';
import { contractFn } from '../support/contract-fn-fixture';
import { appPath } from '../support/app-files';

const source = (file: string) => Bun.file(appPath(file)).text();
const citation: AssistantCitation = { version: 1, environmentId: 'env', threadId: 'verify-timeline', messageId: 'message:fixture-cite',
  text: 'parser keeps parsePatch pure', start: 16, end: 44, prefix: 'Cite check: the ', suffix: ', so a quote across inline cod' };
const href = formatAssistantCitationHref(citation);
const link = (h: string) => `[Assistant quote](${h})`;
const press = (extra: Obj = {}): Obj => ({ seq: 7, open: true, surface: 'composer', owner: 'draft:t1', kind: 'citation', name: 'parser keeps parsePatch pure',
  label: 'parser keeps parsePatch pure', frame: [293, 694, 245, 17], link: link(href), anchor: 'chip', anchorFrame: [516.5, 695.5, 14, 14], removeOnCancel: false, ...extra });

describe('the citation chip\'s comment popover', () => {
  test('a citation press is the comment\'s popover: its saved comment, its anchor and the cited answer', () => {
    const view = chipPopover(press(), []);
    expect(view).toMatchObject({ seq: 7, open: true, kind: 'citation', title: 'Edit citation comment', comment: '', anchor: 'chip', ax: 516.5, ay: 695.5, aw: 14, ah: 14,
      x: 293, y: 694, width: 245, height: 17, message: 'fixture-cite', quote: 'parser keeps parsePatch pure', at: -1 });
    const commented = link(formatAssistantCitationHref({ ...citation, comment: 'Check the pure claim' }));
    expect(chipPopover(press({ link: commented, anchor: 'source', anchorFrame: [568.8, 175.5, 320, 22] }), [], () => 23)).toMatchObject({
      comment: 'Check the pure claim', anchor: 'source', ax: 568.8, ay: 175.5, aw: 320, ah: 22, at: 23 });
    // A skill chip's details are unchanged, and nothing open shows nothing.
    expect(chipPopover({ ...press(), kind: 'skill', name: 'x', label: 'X' }, []).kind).toBe('skill');
    expect(chipPopover({ seq: 9, open: false }, [])).toEqual({ ...NO_CHIP, seq: 9 });
    expect(citationOfLink('[Assistant quote](t3-citation://v1/bad)')).toBeNull();
    expect(citationPopover(press({ link: 'nothing' }), NO_CHIP)).toMatchObject({ comment: '', message: '', quote: '' });
  });

  test('Cite remembers where its quote starts in the answer, for the highlight', () => {
    const owner = {};
    expect(citedStart(owner, citation)).toBe(-1);
    rememberCitedStart(owner, citation, 16);
    expect(citedStart(owner, { ...citation, comment: 'later' } as AssistantCitation)).toBe(16);
    expect(citedStart({}, citation)).toBe(-1);
    for (let i = 0; i < 70; i++) rememberCitedStart(owner, { ...citation, start: 100 + i, end: 101 + i }, i);
    expect(citedStart(owner, citation)).toBe(-1); // only the newest 64
  });

  test('Cite inserts once at selection start and opens only from its independent input ACK', async () => {
    const state = citationField('left right', 4, 10), waiting = () => state.next();
    let complete = false;
    const pending = openCitationComment(state.client, state.native, href, [412, 180.5, 540, 22]).then(value => { complete = true; return value; });
    await pending;
    const command = waiting();
    expect(command).toMatchObject({ start: 4, end: 5, focus: true });
    expect(state.current().value).toBe('left right');
    expect(composerChipCurrent(state.client)).toBeNull();
    state.input(command);
    await Promise.resolve();
    expect(complete).toBe(true);
    expect(state.current().value).toBe(`left ${link(href)} right`);
    expect(composerChipCurrent(state.client)).toBeNull();
    expect((await state.ack(command))).toMatchObject({ applied: true, send: false });
    expect(composerChipCurrent(state.client)).toMatchObject({ kind: 'citation', open: true, removeOnCancel: true, anchor: 'source', anchorFrame: [412, 180.5, 540, 22] });
    expect(await pending).toBe(true);
    expect(state.calls.filter(call => (call as {op:string}).op !== 'wake')).toEqual([]);
  });

  test('Cite preserves a selected atom instead of replacing its source', async () => {
    const state = citationField('$skill prose', 0, 6), waiting = () => state.next();
    const pending = openCitationComment(state.client, state.native, href, [10, 20, 80, 18]);
    await pending;
    const command = waiting();
    expect(command).toMatchObject({ start: 0, end: 0 });
    state.input(command); (await state.ack(command)); await pending;
    expect(state.current().value).toBe(`${link(href)} $skill prose`);
    expect(state.session.chips.map(chip => chip.kind)).toEqual(['citation', 'skill']);
  });

  test('fresh and existing saves replace only the UTF-16 chip range; send is admitted once by ACK', async () => {
    for (const fresh of [false, true]) for (const op of ['chip-cite-save', 'chip-cite-send', 'chip-cite-commit']) {
      const prefix = 'before 😀 ', state = citationField(fresh ? `${prefix}after` : `${prefix}${link(href)} after`, prefix.length);
      if (fresh) {
        const waiting = () => state.next(), opened = openCitationComment(state.client, state.native, href, [10, 20, 80, 18]);
        await opened;
    const insertion = waiting();
        state.input(insertion); (await state.ack(insertion)); await opened;
      } else state.press();
      const current = composerChipCurrent(state.client);
      if (!current) throw new Error('Expected the current citation comment');
      expect(current.removeOnCancel).toBe(fresh);
      const waiting = () => state.next();
      const pending = citationCommentLocal(state.client, state.native, op, String(current.seq), '  Check the pure claim ');
      await pending;
    const command = waiting();
      expect(command).toMatchObject({ start: prefix.length, end: prefix.length + link(href).length, focus: op !== 'chip-cite-commit' });
      expect(citationOfLink(command.text)).toEqual({ ...citation, comment: 'Check the pure claim' });
      state.input(command);
      expect(state.current().value).toBe(`${prefix}${command.text} after`);
      expect((await state.ack(command))).toMatchObject({ applied: true, send: op === 'chip-cite-send' });
      expect(composerChipCurrent(state.client)).toBeNull();
      expect((await state.ack(command)).send).toBe(false);
      await pending;
      expect(state.calls.filter(call => (call as {op:string}).op !== 'wake')).toEqual([]);
    }
  });

  test('clearing a saved comment retains its original quote selector', async () => {
    const commented = formatAssistantCitationHref({ ...citation, comment: 'saved' });
    const state = citationField(link(commented)), press = state.press(), waiting = () => state.next();
    const pending = citationCommentLocal(state.client, state.native, 'chip-cite-save', String(press.seq), '   ');
    await pending;
    const command = waiting();
    expect(citationOfLink(command.text)).toEqual(citation);
    state.input(command); (await state.ack(command)); await pending;
    expect(state.current().value).toBe(link(href));
  });

  test('fresh Cancel removes only its adjacent owned spaces after surrounding inputs', async () => {
    const state = citationField('left right', 4, 10), waiting = () => state.next();
    const opened = openCitationComment(state.client, state.native, href, [10, 20, 80, 18]);
    await opened;
    const insertion = waiting();
    state.input(insertion); (await state.ack(insertion)); await opened;
    state.type(`prefix ${state.current().value}`);
    state.type(`${state.current().value} suffix`);
    const current = composerChipCurrent(state.client);
    expect(current).toMatchObject({ kind: 'citation', removeOnCancel: true });
    const cancellation = () => state.next();
    const pending = citationCommentLocal(state.client, state.native, 'chip-cite-cancel', String(current?.seq), 'ignored');
    await pending;
    const command = cancellation();
    expect(command.text).toBe('');
    state.input(command);
    expect(state.current().value).toBe('prefix left right suffix');
    expect((await state.ack(command))).toMatchObject({ applied: true, send: false });
    await pending;
  });

  test('unchanged save/send, existing Cancel and unchanged outside commit use one no-edit completion', async () => {
    for (const op of ['chip-cite-save', 'chip-cite-send', 'chip-cite-cancel', 'chip-cite-commit']) {
      const state = citationField(link(href)), current = state.press(), before = state.current();
      await citationCommentLocal(state.client, state.native, op, String(current.seq), '   ');
      const finish = composerChipFinishView(state.client);
      expect(finish).toMatchObject({ seq: current.seq, value: link(href), send: op === 'chip-cite-send', focus: op === 'chip-cite-commit' ? 'outside' : 'after' });
      expect(state.current()).toEqual(before);
      expect(composerChipCurrent(state.client)).toBeNull();
      expect(takeComposerChipFinish(state.client, current.seq)).toEqual(finish);
      expect(takeComposerChipFinish(state.client, current.seq)).toBeNull();
      expect(state.calls.filter(call => (call as {op:string}).op !== 'wake')).toEqual([]);
    }
  });

  test('a queued no-edit send cannot survive replacement, typing, IME, unmount or a newer press', async () => {
    for (const situation of ['owner', 'input', 'composition', 'unmounted', 'newer']) {
      const state = citationField(`${link(href)} $skill `), press = state.press();
      await citationCommentLocal(state.client, state.native, 'chip-cite-send', String(press.seq), '');
      expect(composerChipFinishView(state.client)?.send).toBe(true);
      if (situation === 'owner') { state.client.threadId = 'replacement'; state.observe({ owner: state.owner() }); }
      if (situation === 'input') state.type(`${state.current().value}later`);
      if (situation === 'composition') state.observe({ composing: true });
      if (situation === 'unmounted') state.observe({ live: false });
      if (situation === 'newer') state.press('skill');
      expect(composerChipFinishView(state.client)).toBeNull();
      expect(takeComposerChipFinish(state.client, press.seq)).toBeNull();
      expect(state.calls.filter(call => (call as {op:string}).op !== 'wake')).toEqual([]);
    }
  });

  test('an accepted insertion cannot displace a newer press or explicit dismissal at ACK', async () => {
    for (const situation of ['newer', 'dismissed']) {
      const state = citationField('$skill '), previous = state.press('skill'), waiting = () => state.next();
      const pending = openCitationComment(state.client, state.native, href, [10, 20, 80, 18]);
      await pending;
    const command = waiting();
      state.input(command);
      const newer = situation === 'newer' ? state.press('skill') : null;
      if (situation === 'dismissed') composerChipClose(state.client, previous.seq);
      expect((await state.ack(command)).applied).toBe(true);
      expect(await pending).toBe(true);
      expect(composerChipCurrent(state.client)?.seq).toBe(newer?.seq);
    }
  });

  test('owner, newer input, composition, unmount and shared input refusal cannot open a fresh comment', async () => {
    for (const situation of ['owner', 'input', 'composition', 'unmounted', 'refused']) {
      const state = citationField('kept'), waiting = () => state.next();
      const pending = openCitationComment(state.client, state.native, href, [10, 20, 80, 18]);
      const refused = pending.then(() => null, error => error);
      await pending;
    const command = waiting();
      if (situation === 'owner') { state.client.threadId = 'replacement'; state.observe({ owner: state.owner() }); }
      if (situation === 'input') state.type('newer input');
      if (situation === 'composition') state.observe({ composing: true });
      if (situation === 'unmounted') state.observe({ live: false });
      expect((await state.ack(command, situation !== 'refused')).applied).toBe(false);
      expect(await refused).toBeNull();
      expect(composerChipCurrent(state.client)).toBeNull();
      expect(composerChipFinishView(state.client)).toBeNull();
      expect(state.calls.filter(call => (call as {op:string}).op !== 'wake')).toEqual([]);
    }
  });

  test('late typing or dismissal suppresses a changed save-and-send completion', async () => {
    for (const situation of ['typing', 'dismissed', 'newer']) {
      const state = citationField(`${link(href)} $skill `), current = state.press(), waiting = () => state.next();
      const pending = citationCommentLocal(state.client, state.native, 'chip-cite-send', String(current.seq), 'saved');
      await pending;
    const command = waiting();
      state.input(command);
      if (situation === 'typing') state.type(`${state.current().value}later`);
      if (situation === 'dismissed') composerChipClose(state.client, current.seq);
      const newer = situation === 'newer' ? state.press('skill') : null;
      expect((await state.ack(command))).toMatchObject({ applied: true, send: false });
      await pending;
      expect(composerChipCurrent(state.client)?.seq).toBe(newer?.seq);
    }
  });

  test('stale press and overlong comment cannot queue an edit; invalid geometry never inserts', async () => {
    const state = citationField(link(href)), current = state.press();
    await citationCommentLocal(state.client, state.native, 'chip-cite-save', String(current.seq + 1), 'x');
    await expect(citationCommentLocal(state.client, state.native, 'chip-cite-save', String(current.seq), 'x'.repeat(8001))).rejects.toThrow('Comments can contain up to 8,000 characters.');
    for (const frame of [[], [1, Number.NaN, 3, 4], [1, 2, 0, 4]]) expect(await openCitationComment(state.client, state.native, href, frame)).toBe(false);
    expect(state.current().value).toBe(link(href));
    expect(composerChipCurrent(state.client)?.seq).toBe(current.seq);
    expect(parseCiteValue(`16-44|568.5,175,320,22|stream|with bars`)).toEqual({ ranges: [[16, 44]], source: [568.5, 175, 320, 22], stream: 'stream|with bars' });
    expect(parseCiteValue('16-44||s')).toEqual({ ranges: [[16, 44]], source: [], stream: 's' });
    expect(parseCiteValue('16-44|s')).toBeNull();
    expect(state.calls.filter(call => (call as {op:string}).op !== 'wake')).toEqual([]);
  });

  test('the window settles the draft as Base UI\'s onOpenChange with resolveAssistantCitationCommentDismissal does', async () => {
    expect(resolveAssistantCitationCommentDismissal({ reason: 'outside-press', draft: null, savedComment: 'a' })).toEqual({ kind: 'close' });
    expect(resolveAssistantCitationCommentDismissal({ reason: 'outside-press', draft: ' a ', savedComment: 'a' })).toEqual({ kind: 'close' });
    expect(resolveAssistantCitationCommentDismissal({ reason: 'outside-press', draft: 'b', savedComment: 'a' })).toEqual({ kind: 'commit', comment: 'b' });
    expect(resolveAssistantCitationCommentDismissal({ reason: 'outside-press', draft: 'x'.repeat(8001), savedComment: undefined })).toEqual({ kind: 'keep-open' });
    expect(resolveAssistantCitationCommentDismissal({ reason: 'escape-key', draft: 'x'.repeat(8001), savedComment: undefined })).toEqual({ kind: 'close' });
  });

  test('its place: align end on the anchor, 4 pt off, under the cited text or above the chip\'s button, flipped and kept 5 pt inside', async () => {
    const left = await contractFn('citation-comment.contract', 'citationPopoverLeft', ['chip', 'width', 'viewportWidth'], { max: Math.max, min: Math.min });
    const below = await contractFn('citation-comment.contract', 'citationPopoverBelow', ['chip', 'height', 'viewportHeight']);
    const height = await contractFn('citation-comment.contract', 'citationPopoverHeight', ['tooLong']);
    // The reference after Cite: the selection's last line ended at x 887.8, its bottom 193.5: the popup 568.8..888.8, top 197.5.
    const source = { anchor: 'source', ax: 400, ay: 175.5, aw: 487.8, ah: 18 };
    expect(left(source, 320, 1280)).toBeCloseTo(567.8, 1);
    expect(below(source, height(false), 840)).toBe(true);
    expect(height(false)).toBe(114);
    expect(height(true)).toBe(134);
    // Near the window's bottom the side flips above the cited text.
    expect(below({ ...source, ay: 760 }, 114, 840)).toBe(false);
    // From the pencil (the reference: its button 516.7..530.8 at 695.4, the popup's right edge on 530.8 and bottom 691.4): above.
    const chip = { anchor: 'chip', ax: 516.7, ay: 695.4, aw: 14.1, ah: 14.1 };
    expect(left(chip, 320, 1280)).toBeCloseTo(210.8, 1);
    expect(below(chip, 114, 840)).toBe(false);
    expect(below({ ...chip, ay: 60 }, 114, 840)).toBe(true);
    // Kept 5 pt inside the window.
    expect(left({ ...chip, ax: 100 }, 320, 1280)).toBe(5);
    expect(left({ ...chip, ax: 1290 }, 320, 1280)).toBe(955);
  });

  test('the cited text is marked in its answer while the comment is open, as the reference\'s highlight', async () => {
    const md = await source('markdown.contract');
    expect(md).toContain('derive citeMarked = source != "" and citeQuote != "" and citeMessage != "" and (source.indexOf(`${citeMessage}"]`) >= 0 or source == citeMessage)');
    expect(md).toContain('text value.slice(a, b) background-color="light-dark(#1b4ed873, #346bf173)"');
    const window = await source('app-window.contract');
    expect(window).toContain('citeMessage = (citeShown ? chip.message : "")\n    citeQuote = (citeShown ? chip.quote : "")\n    citeAt = (citeShown ? chip.at : -1)');
    const runAt = await contractFn('markdown.contract', 'runAt', ['runs', 'id'], { max: Math.max });
    const marks = await contractFn('markdown.contract', 'citeMarks', ['ms', 'me', 'at', 'size']);
    const runs = [{ id: 'a', text: 'Cite ' }, { id: 'b', text: 'check: ' }, { id: 'c', text: 'parsePatch' }];
    expect([runAt(runs, 'a'), runAt(runs, 'b'), runAt(runs, 'c')]).toEqual([0, 5, 12]);
    expect([marks(6, 14, 0, 5), marks(6, 14, 5, 7), marks(6, 14, 12, 10), marks(-1, -1, 0, 5), marks(6, 14, 14, 3)]).toEqual([false, true, true, false, false]);
  });
});

describe('the rest of Cite', () => {
  // The Cite fixture answer as the native parser lays it out: the paragraph flows word by word around its code span
  // (markdown.rs flow_tokens: a word keeps its trailing spaces, a code span is whole, a space after it stands alone).
  const words = (text: string) => text.match(/\S+\s*|\s+/g)!;
  const flow = [...words('Cite check: the parser keeps '), 'parsePatch', ...words(' pure, so a quote across inline code reads as written. Select across this paragraph and press Cite to quote it into the composer.')]
    .map((text, i) => ({ id: `${i}`, text, mono: text === 'parsePatch' }));
  const code = 'export function parsePatch(text: string) {\n  return text.split(\'\\n\');\n}';
  const blocks = [{ id: '0', kind: 'paragraph', text: '', href: '', runs: flow }, { id: '1', kind: 'code', text: code, href: 'ts', runs: [] },
    { id: '2', kind: 'paragraph', text: '', href: '', runs: [{ id: '0', text: 'A closing paragraph after the code block, long enough to wrap onto a second line in the timeline lane so a selection can end on its last line.' }] }];
  // readAssistantText over the reference's DOM (CDP, the same answer): react-markdown's line breaks between blocks; TS has a file icon, so no language line.
  const reference = 'Cite check: the parser keeps parsePatch pure, so a quote across inline code reads as written. Select across this paragraph and press Cite to quote it into the composer.\n\n\nexport function parsePatch(text: string) {\n  return text.split(\'\\n\');\n}\n\n\n\nA closing paragraph after the code block, long enough to wrap onto a second line in the timeline lane so a selection can end on its last line.';
  const normal = (text: string) => text.replace(/\s+/g, ' ');
  const std = { max: Math.max };
  const fns = async (icon: string) => {
    const label = await contractFn('markdown.contract', 'citeCodeLabel', ['b', 'code'], std);
    const blockText = await contractFn('markdown.contract', 'citeBlockText', ['b', 'code'], { ...std, citeCodeLabel: label });
    const stream = await contractFn('markdown.contract', 'citeStream', ['blocks', 'code'], { ...std, citeBlockText: blockText });
    const offset = await contractFn('markdown.contract', 'citeOffset', ['blocks', 'code', 'blockId'], { ...std, citeBlockText: blockText });
    const runOffset = await contractFn('markdown.contract', 'citeRunOffset', ['blocks', 'code', 'blockId', 'runId'], { ...std, citeCodeLabel: label });
    const highlights = [{ id: 'h', code, icon, tokens: [] }];
    return { stream: () => stream(blocks, highlights) as string, at: (blockId: string, runId: string) => (offset(blocks, highlights, blockId) as number) + (runOffset(blocks, highlights, blockId, runId) as number) };
  };

  test('the answer\'s text stream reads as the reference\'s, whitespace collapsed as its selectors are', async () => {
    const { stream } = await fns('typescript');
    expect(normal(stream())).toBe(normal(reference));
    // A language the header names in words (no file icon) reads its name on a line before the code.
    const { stream: named } = await fns('');
    expect(named()).toContain('composer.\nts\nexport function');
  });

  test('a selection across inline code (word by word) cites what the reference cites', async () => {
    const { stream, at } = await fns('typescript');
    // The reader drags from "parser" to "pure": "parser ", "keeps ", the code span, " " and "pure" report their parts.
    const parts = [['3', 0, 7], ['4', 0, 6], ['5', 0, 10], ['6', 0, 1], ['7', 0, 4]].map(([run, s, e]) => [at('0', String(run)) + Number(s), at('0', String(run)) + Number(e)]);
    const selector = createAssistantTextSelector(stream(), Math.min(...parts.map(p => p[0]!)), Math.max(...parts.map(p => p[1]!)))!;
    expect(selector.text).toBe('parser keeps parsePatch pure');
    const expected = createAssistantTextSelector(reference, reference.indexOf('parser keeps'), reference.indexOf('parser keeps') + 'parser keeps parsePatch pure'.length)!;
    expect(selector).toEqual(expected);
  });

  test('a selection in a code block cites its code, offsets after the blocks before it', async () => {
    const { stream, at } = await fns('typescript');
    const start = at('1', 'code') + code.indexOf('parsePatch'), end = start + 'parsePatch(text: string)'.length;
    const selector = createAssistantTextSelector(stream(), start, end)!;
    expect(selector.text).toBe('parsePatch(text: string)');
    const from = reference.indexOf('parsePatch(text');
    expect(selector).toEqual(createAssistantTextSelector(reference, from, from + selector.text.length)!);
    // With a language line, the code starts after it.
    const named = await fns('');
    expect(named.at('1', 'code') - named.at('1', '')).toBe('ts\n'.length);
  });
});
