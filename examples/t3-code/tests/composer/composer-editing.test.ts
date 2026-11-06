// Composer editing rules followed from T3 Code 74541bab17: each is source and caret in, source and caret out.
import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { applyComposerEdit, composerMarkdownKey, type ComposerEdit, type ComposerKeyResult } from '../../src/composer/composer-markdown-edit';
import { listContinuationForEnter, listIndentForTab } from '../../src/composer/composer-list-continuation';
import { ComposerSourceSession } from '../../src/composer/composer-source-session';
import { composerChips, composerTrigger } from '../../src/composer/composer-source-chips';
import { composerSurround } from '../../src/composer/composer-surround';
import { composerMarkdown } from '../../src/composer/composer-markdown';
import { composerHistoryRecall } from '../../src/composer/composer-prompt-history';
import { queuedPreviewText } from '../../src/composer/composer-controls-queue';
import { DEFAULT_SEND_RULES, composerRequiresModifier, sendChords, sendIntent } from '../../src/composer/composer-editor-intent';
import { NO_DESCRIPTION, chipPopover } from '../../src/composer/composer-chip-popover';
import type { Obj } from '../../src/client/domain';

const after = (source: string, edit: ComposerEdit | null) => edit && { source: applyComposerEdit(source, edit), caret: edit.selection.start };
const enter = (source: string, caret: number): ComposerKeyResult => composerMarkdownKey(source, { start: caret, end: caret }, { name: 'Enter', shift: true, command: false });
const rich = (source: string, caret: number) => { const result = enter(source, caret); return result.kind === 'edit' ? after(source, result.edit) : result.kind; };

describe('lists typed as Markdown', () => {
  test('Enter on a marker typed without its space adds a line instead of deleting it (#18786)', () => {
    for (const source of ['asd\n2.', '1)', '-', '- [ ]']) {
      expect(listContinuationForEnter(source, source.length, [])).toBeNull();
      expect(rich(source, source.length)).toEqual({ source: `${source}\n`, caret: source.length + 1 });
    }
    // An empty item with its space still ends the list.
    for (const source of ['1. foo\n2. ', '- ', '1)\t', '- [ ] ']) {
      const plain = after(source, listContinuationForEnter(source, source.length, []));
      expect(plain).toEqual(rich(source, source.length) as { source: string; caret: number });
      expect(plain?.source).toBe(source.startsWith('1. foo') ? '1. foo\n' : '');
    }
  });

  test('continuing a numbered list renumbers the items after it, keeping their padding (#14332)', () => {
    const value = '1. @README.md go\n2. two\n03. three\n10. four', caret = value.indexOf('go');
    const expected = { source: '1. @README.md \n2. go\n3. two\n04. three\n5. four', caret: '1. @README.md \n2. '.length };
    expect(after(value, listContinuationForEnter(value, caret, composerChips(value)))).toEqual(expected);
    expect(rich(value, caret)).toEqual(expected);
  });

  test('renumbering skips deeper items, stops at a parent, a blank line, text or another delimiter', () => {
    const nested = '  1) a\n\t1) child\n  2) b\n1) parent\n  3) untouched';
    expect(after(nested, listContinuationForEnter(nested, nested.indexOf('\n'), []))).toEqual({
      source: '  1) a\n  2) \n\t1) child\n  3) b\n1) parent\n  3) untouched', caret: '  1) a\n  2) '.length });
    for (const boundary of ['', 'text', '1) other list']) {
      const value = `1. a\n2. b\n${boundary}\n3. untouched`;
      expect(after(value, listContinuationForEnter(value, value.indexOf('\n'), []))?.source).toBe(`1. a\n2. \n3. b\n${boundary}\n3. untouched`);
    }
  });

  test('a line break inside a reference does not start an item', () => {
    const value = '1. a\n2. @"file\n2. name" \n3. b';
    expect(after(value, listContinuationForEnter(value, value.indexOf('\n'), composerChips(value)))?.source).toBe('1. a\n2. \n3. @"file\n2. name" \n4. b');
  });

  test('the plain composer continues and indents lists; Tab keeps the caret on the item', () => {
    const session = new ComposerSourceSession(), value = '1. a\n2. b';
    session.observe('draft', value, '', 'plain');
    const continued = session.key('draft', value, { start: 4, end: 4 }, { name: 'Enter', shift: true, command: false });
    expect(continued.kind === 'edit' && after(value, continued.edit)).toEqual({ source: '1. a\n2. \n3. b', caret: 8 });
    expect(after('- item', listIndentForTab('- item', 6, 6, []))).toEqual({ source: '  - item', caret: 8 });
    expect(session.key('draft', value, { start: 4, end: 4 }, { name: 'Enter', shift: false, command: false })).toEqual({ kind: 'send' });
    // A literal answer keeps its text as typed.
    const literal = new ComposerSourceSession();
    literal.observe('draft', value, '', 'literal');
    const typed = literal.key('draft', value, { start: 4, end: 4 }, { name: 'Enter', shift: true, command: false });
    expect(typed.kind === 'edit' && typed.edit.text).toBe('\n');
  });
});

describe('quotes, backticks and Command-arrows', () => {
  test('leaving a quote keeps a blank line, so the comment under it is its own paragraph (#19450)', () => {
    expect(rich('> quoted\n> ', 11)).toEqual({ source: '> quoted\n\n', caret: 10 });
    expect(rich('> a\n> b\n> ', 10)).toEqual({ source: '> a\n> b\n\n', caret: 9 });
    // A quote's own first empty line has nothing above it to join.
    expect(rich('> ', 2)).toEqual({ source: '', caret: 0 });
  });

  test('text typed between backticks is source, which the field paints as inline code (#17435)', () => {
    // No app edit is involved: the field types into `` and the Markdown source reads as a code span.
    const session = new ComposerSourceSession();
    session.observe('draft', 'use `` here', '', 'rich');
    const typed = session.input('draft', 'use `` here', { start: 5, end: 5 }, 'npm');
    expect(typed.kind === 'edit' && after('use `` here', typed.edit)).toEqual({ source: 'use `npm` here', caret: 8 });
  });

  test('a list marker is source text: no gutter can clip it, and Command-arrows move over visible markers (#18972, #22011)', () => {
    expect(composerMarkdown('123456789. wide\n**bold**').blocks.map(block => block.kind)).toEqual(['list', 'paragraph']);
  });
});

describe('references', () => {
  test('a path with a second slash does not open the slash menu (#19671)', () => {
    expect(composerTrigger('/tmp/build.sh', 13)).toBeNull();
    expect(composerTrigger('/etc/hosts', 10)).toBeNull();
    expect(composerTrigger('/etc/hosts now', 3)).toBeNull();
    expect(composerTrigger('/plan', 5)).toMatchObject({ kind: 'slash-command', query: 'plan' });
    expect(composerTrigger('/skill:review later', 13)).toMatchObject({ kind: 'slash-command', query: 'skill:review' });
  });

  test('a pasted decorator call stays text; a quoted or linked one is a mention (#8415)', () => {
    for (const reference of ['@namespace.foo(bar)', '@écrire(option)', '@écrire(option)']) expect(composerChips(`${reference}\nclass Example {}`)).toEqual([]);
    for (const reference of ['@"foo(bar)"', '[foo(bar)](foo%28bar%29)']) expect(composerChips(`${reference} `)).toMatchObject([{ kind: 'mention', path: 'foo(bar)', source: reference }]);
    expect(composerChips('Inspect @README.md (notes)')).toMatchObject([{ kind: 'mention', path: 'README.md' }]);
  });

  test("only the provider's skills become chips, and they follow a provider change without editing the source (#14834)", () => {
    const value = 'echo $HOME $PATH $babysit $20 ';
    expect(composerChips(value, name => name === 'babysit').map(chip => chip.source)).toEqual(['$babysit']);
    const session = new ComposerSourceSession();
    session.observe('draft', value, '', 'rich', ['babysit']);
    expect(session.chips.map(chip => chip.source)).toEqual(['$babysit']);
    session.observe('draft', value, '', 'rich', []);
    expect(session.chips).toEqual([]);
    session.observe('draft', value, '', 'rich', ['babysit', 'HOME']);
    expect(session.chips.map(chip => chip.source)).toEqual(['$HOME', '$babysit']);
  });

  test("a skill's details are its description, else its short description; a disabled one has none", () => {
    const press = { open: true, kind: 'skill', name: 'babysit', label: 'Babysit', surface: 'composer', frame: [0, 0, 10, 10] };
    expect(chipPopover(press, [{ name: 'babysit', description: ' ', shortDescription: '  Short  ', path: '/s.md' }])).toMatchObject({ description: 'Short', path: '/s.md' });
    expect(chipPopover(press, [{ name: 'babysit', enabled: false, description: 'Old' }]).description).toBe(NO_DESCRIPTION);
  });
});

describe('selection surround', () => {
  test('a bracket or quote typed over plain text wraps it and keeps it selected', () => {
    expect(composerSurround('a word', { start: 2, end: 6 }, '"', [], [])).toEqual({ start: 2, end: 6, text: '"word"', selection: { start: 3, end: 7 } });
    expect(composerSurround('a word', { start: 2, end: 6 }, '(', [], [])?.text).toBe('(word)');
  });

  test('a collapsed caret, a reference, styled text or the space beside a mention is not wrapped (#20350)', () => {
    expect(composerSurround('word', { start: 4, end: 4 }, '"', [], [])).toBeNull();
    expect(composerSurround('a word', { start: 2, end: 6 }, 'x', [], [])).toBeNull();
    const mention = 'see @a.ts now', chips = composerChips(mention);
    expect(composerSurround(mention, { start: 4, end: 9 }, '"', chips, [])).toBeNull();
    expect(composerSurround(mention, { start: 0, end: 4 }, '"', chips, [])).toBeNull();
    const styled = 'a **bold** b `code` [link](https://x.dev) ~~gone~~', blocks = composerMarkdown(styled).blocks;
    // Any selection that holds styled text, inside its run too, is replaced, as the reference never wraps marked text.
    for (const word of ['**bold**', 'bold', 'ol', 'code', 'link', 'gone', 'a **b']) {
      const at = styled.indexOf(word);
      expect(composerSurround(styled, { start: at, end: at + word.length }, '"', [], blocks)).toBeNull();
    }
    // Plain text beside a run, an underscore inside a word and a lone star are not styling.
    for (const plain of ['clean node_modules now', 'a * b', 'snake_case_name']) {
      expect(composerSurround(plain, { start: 0, end: plain.length }, '"', [], composerMarkdown(plain).blocks)?.text).toBe(`"${plain}"`);
    }
    expect(composerSurround(styled, { start: 0, end: 1 }, '(', [], blocks)?.text).toBe('(a)');
    // Code holds no styling, so its text is wrapped.
    const code = '```\na*b\n```';
    expect(composerSurround(code, { start: 4, end: 7 }, '"', [], composerMarkdown(code).blocks)?.text).toBe('"a*b"');
  });

  test('a session wraps typed input but never a paste, and replaces the selection when it does not wrap', () => {
    const session = new ComposerSourceSession();
    session.observe('draft', 'a word', '', 'rich');
    expect(session.input('draft', 'a word', { start: 2, end: 6 }, '"')).toMatchObject({ kind: 'edit', edit: { text: '"word"' } });
    expect(session.input('draft', 'a word', { start: 2, end: 6 }, '"', true)).toMatchObject({ kind: 'edit', edit: { start: 2, end: 6, text: '"' } });
  });
});

describe('recall, queue and send', () => {
  test('↑ recall skips messages sent by automations, agents and T3 Code (#21510)', () => {
    const client = new T3Client();
    const user = (id: string, item: Obj) => ({ sourceThreadId: 'thread', sourceItemId: id, item: { type: 'user_message', ...item } });
    client.thread = { sequence: 1, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null, projection: { visibleTurnItems: [
      user('m1', { text: 'typed' }),
      user('m2', { text: 'scheduled', scheduledTaskId: 'task' }),
      user('m3', { text: '[Triggered by schedule task: Daily]\n\nlegacy', messageId: 'scheduled-task-message:task:1:scheduled' }),
      user('m4', { text: 'from agent', createdBy: 'agent', creationSource: 'mcp' }),
      user('m5', { text: 'from server', createdBy: 'agent', creationSource: 'server' }),
    ] } };
    expect(composerHistoryRecall(client, 'draft', '', true)?.prompt).toBe('typed');
  });

  test('a queued follow-up shows a quote as its text and a chip as its label (#18999)', () => {
    const quote = '[Assistant quote](t3-citation://v1/e/t/m?text=the+quoted+text&start=0&end=15&prefix=&suffix=)';
    expect(queuedPreviewText(`${quote} and more`, false)).toBe('the quoted text and more');
    expect(queuedPreviewText('see [T](t3-context://v1/thread/thread_x) ![shot.png](t3-context://v1/image/image_1)', true)).toBe('see T');
  });

  test('with ⌘↩ required to send, ⌘↩ follows the follow-up default and ⌘⇧↩ takes the other (#20929)', () => {
    const config: Obj = { keybindings: DEFAULT_SEND_RULES };
    expect(composerRequiresModifier('mod-enter', 'one line')).toBe(true);
    expect(composerRequiresModifier('mod-enter-multiline', 'one line')).toBe(false);
    expect(composerRequiresModifier('mod-enter-multiline', 'two\nlines')).toBe(true);
    const key = (modifiers: string): Obj => ({ modifiers, source: 'key', ageMs: 5 });
    expect(sendIntent(config, key('meta'), true, false, false, false)).toBe('alternate');
    expect(sendIntent(config, key('meta'), true, false, false, true)).toBe('foreground');
    expect(sendIntent(config, key('meta+shift'), true, false, false, true)).toBe('alternate');
    expect(sendChords(config, true, false, false, true).split(' ')).toEqual(['Meta+Enter', 'Meta+Shift+Enter', 'Meta+Alt+Enter']);
    expect(sendChords(config, true, false, false, false).split(' ')).toEqual(['Meta+Enter', 'Meta+Alt+Enter']);
  });
});

describe('chips copied between drafts (#17821)', () => {
  const { adoptPastedChips, heldImageIds, rememberCopiedChips, stagedInUse } = require('../../src/composer/composer-copied-chips') as typeof import('../../src/composer/composer-copied-chips');
  const { draftFiles, setDraftFiles, fileChipLink } = require('../../src/composer/composer-editor-files') as typeof import('../../src/composer/composer-editor-files');
  const { imageChipLink } = require('../../src/composer/composer-editor-attach') as typeof import('../../src/composer/composer-editor-attach');
  const imageId = '11111111-2222-4333-8444-555555555555', fileId = '66666666-7777-4888-8999-aaaaaaaaaaaa';
  function setup() {
    const client = new T3Client();
    client.environmentId = 'env'; client.threadId = 'a';
    const source = client.draftKey, image = { id: imageId, name: 'shot.png', mimeType: 'image/png', sizeBytes: 9, uploadId: 'up-1' };
    client.local.snapshotDrafts[source] = [image];
    const file = { id: fileId, contextId: `file_${fileId}`, draftKey: source, environmentId: 'env', name: 'notes.txt', mimeType: 'text/plain', sizeBytes: 3, source: 'attached', attachmentId: '', status: 'staged' as const };
    setDraftFiles(client.local, [file]);
    const text = `see ${imageChipLink(image)} and ${fileChipLink(file)} `;
    return { client, source, image, file, text };
  }
  test('a paste into another draft adopts the image and file the source draft still holds', () => {
    const { client, source, text } = setup();
    client.threadId = 'b';
    expect(adoptPastedChips(client, text, 100)).toBe(2);
    // The image uploads again from its bytes; the file is the same staged copy.
    expect(client.local.snapshotDrafts[client.draftKey]).toEqual([{ id: imageId, name: 'shot.png', mimeType: 'image/png', sizeBytes: 9 }]);
    expect(draftFiles(client.local).map(file => file.draftKey)).toEqual([source, client.draftKey]);
    // A second paste of the same chips adds nothing; the limit is kept.
    expect(adoptPastedChips(client, text, 100)).toBe(0);
    client.threadId = 'c';
    expect(adoptPastedChips(client, text, 1)).toBe(1);
  });
  test('the latest copy keeps the chips after their draft was sent, until another copy replaces it', () => {
    const { client, source, text } = setup();
    rememberCopiedChips(client, text);
    delete client.local.snapshotDrafts[source];
    setDraftFiles(client.local, []);
    expect(heldImageIds(client.local)).toEqual(new Set([imageId]));
    expect(stagedInUse(client.local, fileId)).toBe(true);
    client.threadId = 'b';
    expect(adoptPastedChips(client, text, 100)).toBe(2);
    rememberCopiedChips(client, 'plain text');
    expect(heldImageIds(client.local)).toEqual(new Set());
  });
  test('the paste intent adopts them as its edit goes in', () => {
    const { client, text } = setup();
    client.threadId = 'b';
    const { composerIntentSource } = require('../../src/composer/composer-intent-source') as typeof import('../../src/composer/composer-intent-source');
    const { composerSourceSession } = require('../../src/composer/composer-rich-state') as typeof import('../../src/composer/composer-rich-state');
    composerSourceSession(client).observe('draft', '', '', 'rich');
    const reply = composerIntentSource(client, { snapshotOwner: client.snapshotOwner, owner: 'draft', value: '', serial: 1, start: 0, end: 0, composing: false, mode: 'rich', kind: 'paste', text });
    expect(reply).toMatchObject({ kind: 'edit', text });
    expect(client.snapshotDrafts.map(image => image.id)).toEqual([imageId]);
  });
  test("a file uploaded on another environment's server is not adopted", () => {
    const { client, file, text } = setup();
    setDraftFiles(client.local, [{ ...file, status: 'ready', attachmentId: 'server-1' }]);
    client.environmentId = 'other';
    expect(adoptPastedChips(client, text, 100)).toBe(1);
    expect(draftFiles(client.local)).toHaveLength(1);
  });
});
