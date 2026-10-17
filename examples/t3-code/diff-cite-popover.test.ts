// Cite's comment popover and the rest of Cite, against T3 Code 1e2ecbd975 (MIT, LICENSE-T3):
// AssistantCitationChip.tsx, AssistantCitationCommentEditor.tsx, assistantCitationCommentDismissal.ts,
// ComposerPromptEditorTiptap.tsx ComposerCitationNodeView, ChatComposer.tsx citeAssistantText and submitCitationAndSend,
// lib/assistantTextSelection.ts readAssistantText. The reference's numbers were read over CDP on the reference with
// its Cite fixture answer (a paragraph with inline code, a ```ts block, a closing paragraph), 1280×840.
import { describe, expect, test } from 'bun:test';
import type { Obj } from './domain';
import type { Native } from './protocol';
import { chipPopover, chipPopoverLocal, NO_CHIP } from './composer-chip-popover';
import { citationCommentLocal, citationOfLink, citationPopover, citedStart, openCitationComment, rememberCitedStart } from './citation-comment';
import { createAssistantTextSelector, formatAssistantCitationHref, parseAssistantCitationHref, resolveAssistantCitationCommentDismissal, type AssistantCitation } from './diff-citations';
import { parseCiteValue } from './diff-review';
import { contractFn } from './contract-fn-fixture';

const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
const citation: AssistantCitation = { version: 1, environmentId: 'env', threadId: 'verify-timeline', messageId: 'message:fixture-cite',
  text: 'parser keeps parsePatch pure', start: 16, end: 44, prefix: 'Cite check: the ', suffix: ', so a quote across inline cod' };
const href = formatAssistantCitationHref(citation);
const link = (h: string) => `[Assistant quote](${h})`;
const press = (extra: Obj = {}): Obj => ({ seq: 7, open: true, surface: 'composer', owner: 'draft:t1', kind: 'citation', name: 'parser keeps parsePatch pure',
  label: 'parser keeps parsePatch pure', frame: [293, 694, 245, 17], link: link(href), anchor: 'chip', anchorFrame: [516.5, 695.5, 14, 14], removeOnCancel: false, ...extra });

/** A native editor stand-in: `editorChip` answers `state`, `editorChipFinish` is recorded. */
function editor(state: Obj, applied = true) {
  const calls: Obj[] = [];
  const native = { available: true, watch() {}, async later(request: Obj) {
    calls.push(request);
    if (request.op === 'editorChip') return { ok: true, generation: 0, value: state };
    if (request.op === 'editorChipFinish') return { ok: true, generation: 0, value: { applied, sent: request.send === true } };
    if (request.op === 'editorCiteOpen') return { ok: true, generation: 0, value: { opened: true } };
    return { ok: true, generation: 0, value: {} };
  } } as unknown as Native;
  return { native, calls, finishes: () => calls.filter(call => call.op === 'editorChipFinish') };
}

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

  test('Save rewrites the chip with the comment (withAssistantCitationComment), the focus after it', async () => {
    const { native, finishes } = editor(press());
    await chipPopoverLocal({} as never, native, 'chip-cite-save', '7', '  Check the pure claim ');
    const [finish] = finishes();
    expect(finish).toMatchObject({ op: 'editorChipFinish', seq: 7, focus: true, send: false });
    const saved = parseAssistantCitationHref(/\((t3-citation:[^)]+)\)$/.exec(String(finish!.text))![1]!);
    expect(saved).toEqual({ ...citation, comment: 'Check the pure claim' });
    // An empty comment removes it (the chip reads the quote again).
    const second = editor(press({ link: String(finish!.text) }));
    await chipPopoverLocal({} as never, second.native, 'chip-cite-save', '7', '   ');
    expect(parseAssistantCitationHref(/\((t3-citation:[^)]+)\)$/.exec(String(second.finishes()[0]!.text))![1]!)).toEqual(citation);
  });

  test('Command/Ctrl+Enter saves and sends; an outside press commits without asking for the focus (the native editor gives it only when the press left it nowhere)', async () => {
    const send = editor(press());
    await citationCommentLocal(send.native, 'chip-cite-send', '7', 'why?');
    expect(send.finishes()[0]).toMatchObject({ focus: true, send: true });
    const commit = editor(press());
    await citationCommentLocal(commit.native, 'chip-cite-commit', '7', 'why?');
    expect(commit.finishes()[0]).toMatchObject({ focus: false, send: false });
  });

  test('Cancel and Escape: after Cite the chip goes again (removeOnCancel), from the pencil it stays', async () => {
    const cited = editor(press({ anchor: 'source', removeOnCancel: true }));
    await citationCommentLocal(cited.native, 'chip-cite-cancel', '7', '');
    expect(cited.finishes()).toEqual([{ op: 'editorChipFinish', seq: 7, text: '', focus: true, send: false }]);
    const pencil = editor(press());
    await citationCommentLocal(pencil.native, 'chip-cite-cancel', '7', 'ignored draft');
    expect(pencil.finishes()).toEqual([{ op: 'editorChipFinish', seq: 7, focus: true, send: false }]);
  });

  test('the window\'s Escape asks the native editor, which knows where the focus is: in the textarea the editor\'s Cancel, elsewhere Base UI\'s escape-key close', async () => {
    // The native editor drops `text` (the chip stays) and asks no focus when the textarea does not hold it
    // (T3ComposerChipPress.finishCitation `escape`; the AppKit test covers that half).
    const cited = editor(press({ anchor: 'source', removeOnCancel: true }));
    await chipPopoverLocal({} as never, cited.native, 'chip-cite-escape', '7', '');
    expect(cited.finishes()).toEqual([{ op: 'editorChipFinish', seq: 7, text: '', focus: true, send: false, escape: true }]);
    const pencil = editor(press());
    await citationCommentLocal(pencil.native, 'chip-cite-escape', '7', 'a draft past the limit'.repeat(400));
    expect(pencil.finishes()).toEqual([{ op: 'editorChipFinish', seq: 7, focus: true, send: false, escape: true }]);
    const window = await source('app-window.contract');
    // The popover is not hidden first: its textarea must still hold the focus when the native editor looks (run 3 found it).
    expect(window).toContain('  action chipEscape\n    if chip.kind == "citation"\n      editorOp("chip-cite-escape", `${chip.seq}`, "")\n    else');
  });

  test('only the comment the window showed; past 8,000 characters nothing is saved', async () => {
    const stale = editor(press({ seq: 8 }));
    await citationCommentLocal(stale.native, 'chip-cite-save', '7', 'x');
    expect(stale.finishes()).toEqual([]);
    const closed = editor({ seq: 7, open: false });
    await citationCommentLocal(closed.native, 'chip-cite-cancel', '7', '');
    expect(closed.finishes()).toEqual([]);
    const long = editor(press());
    await expect(citationCommentLocal(long.native, 'chip-cite-save', '7', 'x'.repeat(8001))).rejects.toThrow('Comments can contain up to 8,000 characters.');
    expect(long.finishes()).toEqual([]);
    const failed = editor(press(), false);
    await expect(citationCommentLocal(failed.native, 'chip-cite-save', '7', 'x')).rejects.toThrow('The prompt could not be updated.');
  });

  test('Cite opens the chip just inserted under the cited text', async () => {
    const { native, calls } = editor({});
    expect(await openCitationComment(native, href, [412, 180.5, 540, 22])).toBe(true);
    expect(calls.at(-1)).toEqual({ op: 'editorCiteOpen', href, source: [412, 180.5, 540, 22] });
    await openCitationComment(native, href, [1, Number.NaN, 3, 4]);
    expect(calls.at(-1)).toEqual({ op: 'editorCiteOpen', href, source: [] });
    expect(parseCiteValue(`16-44|568.5,175,320,22|stream|with bars`)).toEqual({ ranges: [[16, 44]], source: [568.5, 175, 320, 22], stream: 'stream|with bars' });
    expect(parseCiteValue('16-44||s')).toEqual({ ranges: [[16, 44]], source: [], stream: 's' });
    expect(parseCiteValue('16-44|s')).toBeNull();
  });

  test('the window settles the draft as Base UI\'s onOpenChange with resolveAssistantCitationCommentDismissal does', async () => {
    const window = await source('app-window.contract');
    // Untouched (the reference's null draft) or unchanged: close; changed: commit; too long: keep open.
    expect(window).toContain('  action citeSettle\n    if citeShown and (citeDraftSeq != chip.seq or citeDraft.trim() == chip.comment.trim())\n      chipClose(false)\n    else if citeShown and citeDraft.length <= 8000\n      chipClosed = chip.seq\n      editorOp("chip-cite-commit", `${chip.seq}`, citeDraft)');
    expect(resolveAssistantCitationCommentDismissal({ reason: 'outside-press', draft: null, savedComment: 'a' })).toEqual({ kind: 'close' });
    expect(resolveAssistantCitationCommentDismissal({ reason: 'outside-press', draft: ' a ', savedComment: 'a' })).toEqual({ kind: 'close' });
    expect(resolveAssistantCitationCommentDismissal({ reason: 'outside-press', draft: 'b', savedComment: 'a' })).toEqual({ kind: 'commit', comment: 'b' });
    expect(resolveAssistantCitationCommentDismissal({ reason: 'outside-press', draft: 'x'.repeat(8001), savedComment: undefined })).toEqual({ kind: 'keep-open' });
    // Escape closes whatever the draft (in the textarea the editor's onCancel, elsewhere escape-key); an outside press here or
    // the textarea's blur settles.
    expect(resolveAssistantCitationCommentDismissal({ reason: 'escape-key', draft: 'x'.repeat(8001), savedComment: undefined })).toEqual({ kind: 'close' });
    expect(window).toMatch(/if chip\.kind == "citation"\n {8}citeSettle\(\)\n {6}else\n {8}\/\/[^\n]*\n {8}chipClosed = chip\.seq\n {8}editorOp\("chip-close", `\$\{chip\.seq\}`, ""\)/);
    expect(window).toContain('CitationCommentLayer(chip=chip, draft=citeText, viewportWidth=viewport.width, viewportHeight=viewport.height, edit=citeEdit, save=citeSave, cancel=citeCancel, settle=citeSettle)');
    expect(window).toContain('derive citeText = citeDraftSeq == chip.seq ? citeDraft : chip.comment');
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
