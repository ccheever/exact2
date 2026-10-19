import { describe, expect, test } from 'bun:test';
import { ComposerSourceSession, type ComposerSessionResult } from '../../src/composer/composer-source-session';
import { applyComposerEdit } from '../../src/composer/composer-markdown-edit';
import { composerMarkdown } from '../../src/composer/composer-markdown';
import { T3Client } from '../../src/client/client';
import { composerRichSource } from '../../src/composer/composer-rich-source';
import { composerIntentSource } from '../../src/composer/composer-intent-source';

function apply(session: ComposerSourceSession, owner: string, source: string, result: ComposerSessionResult): { source: string; start: number; end: number } {
  if (result.kind !== 'edit') throw new Error(`expected edit, got ${result.kind}`);
  const value = applyComposerEdit(source, result.edit);
  session.observe(owner, value);
  return { source: value, ...result.edit.selection };
}
const enter = { name: 'Enter', shift: false, command: false };
describe('composer live source identity', () => {
  const block = (session: ComposerSourceSession, source: string) => composerMarkdown(source, { blockOrigins: session.blockOrigins }).blocks[0];
  function typed(source: string): ComposerSourceSession {
    const session = new ComposerSourceSession();
    session.observe('one', '');
    for (let end = 1; end <= source.length; end++) session.observe('one', source.slice(0, end), 'insertText');
    return session;
  }
  test('typing admits block gestures at the upstream boundary, without changing source', () => {
    for (const source of ['-', '- ', '- [', '- [ ]', '>', '***', '___', '1.', '  - x']) {
      const session = typed(source);
      expect(block(session, source)?.kind).toBe('paragraph');
      expect(session.matches('one', source)).toBe(true);
    }
    for (const [source, kind] of [['- a', 'list'], ['* ', 'list'], ['+ ', 'list'], ['003) ', 'list'], ['> ', 'quote'], ['## ', 'heading'], ['---', 'rule'], ['*** ', 'rule'], ['___ ', 'rule']] as const) {
      expect(block(typed(source), source)?.kind).toBe(kind);
    }
    for (const source of ['- [ ] ', '- [X] ', '* [x] ', '+ [ ] ']) {
      expect(block(typed(source), source)).toMatchObject({ kind: 'list', list: 'task', checked: /[xX]/.test(source) });
    }
    expect(block(typed('* [x]'), '* [x]')).toMatchObject({ kind: 'list', list: 'bullet' });
    expect(block(typed('1. [x] '), '1. [x] ')).toMatchObject({ kind: 'list', list: 'ordered' });
    expect(block(typed('* * *'), '* * *')).toMatchObject({ kind: 'list', list: 'bullet' });
    // One insertText containing an entire non-gesture prefix is not a restore.
    const chunk = typed('');
    chunk.observe('one', '# body', 'insertText');
    expect(block(chunk, '# body')?.kind).toBe('paragraph');
  });
  test('the inserted prefix, rather than the line suffix, admits a gesture in existing prose', () => {
    const session = new ComposerSourceSession();
    session.observe('one', 'body');
    session.observe('one', '>body', 'insertText');
    expect(block(session, '>body')?.kind).toBe('paragraph');
    session.observe('one', '> body', 'insertText');
    expect(block(session, '> body')?.kind).toBe('quote');
    session.observe('one', '> # body', 'insertText');
    expect(block(session, '> # body')?.kind).toBe('quote');
  });
  test('keys use the same effective blocks as projection before and after admission', () => {
    const session = typed('- '), shiftEnter = { ...enter, shift: true };
    const literal = apply(session, 'one', '- ', session.key('one', '- ', { start: 2, end: 2 }, shiftEnter));
    expect(literal.source).toBe('- \n');
    expect(block(session, literal.source)?.kind).toBe('paragraph');
    const admitted = typed('- a');
    expect(apply(admitted, 'one', '- a', admitted.key('one', '- a', { start: 3, end: 3 }, shiftEnter)).source).toBe('- a\n- ');
    const task = typed('* [x] ');
    expect(apply(task, 'one', '* [x] ', task.key('one', '* [x] ', { start: 6, end: 6 }, shiftEnter)).source).toBe('');
  });
  test('deleting an unadmitted list prefix permits the remaining fence to open on Enter', () => {
    const session = typed('- [```');
    expect(block(session, '- [```')?.kind).toBe('paragraph');
    session.observe('one', '```', 'deleteContentBackward');
    expect(session.blockOrigins).toEqual([]);
    const opened = apply(session, 'one', '```', session.key('one', '```', { start: 3, end: 3 }, enter));
    expect(opened.source).toBe('```\n\n```');
    expect(block(session, opened.source)).toMatchObject({ kind: 'code', fence: '```' });
    session.observe('one', '- [```', 'historyUndo');
    expect(block(session, '- [```')?.kind).toBe('paragraph');
    session.observe('one', opened.source, 'historyRedo');
    expect(block(session, opened.source)?.kind).toBe('code');
  });
  test('history restores live origins; paste, owner replacement and rich-mode entry parse restored Markdown', () => {
    const session = typed('***');
    session.observe('one', '*** ', 'insertText');
    expect(block(session, '*** ')?.kind).toBe('rule');
    session.observe('one', '***', 'historyUndo');
    expect(block(session, '***')?.kind).toBe('paragraph');
    session.observe('one', '*** ', 'historyRedo');
    expect(block(session, '*** ')?.kind).toBe('rule');
    session.observe('one', '***', 'historyUndo');
    session.observe('one', '- [ ]', 'insertFromPaste');
    expect(block(session, '- [ ]')).toMatchObject({ kind: 'list', list: 'task' });
    session.observe('two', '***', 'insertText');
    expect(block(session, '***')?.kind).toBe('rule');
    const literal = typed('- ');
    literal.observe('one', '- ', '', 'plain');
    literal.observe('one', '- ', '', 'rich');
    expect(block(literal, '- ')?.kind).toBe('list');
  });
  test('origins follow unaffected UTF16 line offsets, while paste replaces only touched policy', () => {
    const session = typed('***');
    session.observe('one', '😀\n***', 'insertFromPaste');
    expect(composerMarkdown('😀\n***', { blockOrigins: session.blockOrigins }).blocks.map(block => block.kind)).toEqual(['paragraph', 'paragraph']);
    session.observe('one', '😀\n***\n> quote', 'insertFromPaste');
    expect(composerMarkdown('😀\n***\n> quote', { blockOrigins: session.blockOrigins }).blocks.map(block => block.kind)).toEqual(['paragraph', 'paragraph', 'quote']);
    session.observe('one', '😀\n*** \n> quote', 'insertFromPaste');
    expect(composerMarkdown('😀\n*** \n> quote', { blockOrigins: session.blockOrigins }).blocks.map(block => block.kind)).toEqual(['paragraph', 'rule', 'quote']);
  });
  test('source-signature history evicts block metadata at the existing source and character budgets', () => {
    const session = typed('***');
    // Distinct sources retain the same unadmitted first line. The oldest source
    // eventually loses only its metadata, never its editable source or undo.
    for (let n = 1; n <= 33; n++) session.observe('one', '***\n' + 'x'.repeat(n), 'insertText');
    session.observe('one', '***', 'historyUndo');
    expect(block(session, '***')?.kind).toBe('rule');
    const large = typed('***');
    for (let n = 1; n <= 4; n++) large.observe('one', '***\n' + String(n).repeat(70_000), 'insertText');
    large.observe('one', '***', 'historyUndo');
    expect(block(large, '***')?.kind).toBe('rule');
  });
  test('block origins share the 1024-origin history budget and live tracking stays bounded', () => {
    const session = new ComposerSourceSession();
    let source = Array(1025).fill('x').join('\n');
    session.observe('one', source);
    for (let n = 0; n < 1025; n++) {
      source = source.slice(0, n * 2) + '-' + source.slice(n * 2 + 1);
      session.observe('one', source, 'insertText');
    }
    expect(session.blockOrigins).toHaveLength(1024);
    expect(composerMarkdown(source, { blockOrigins: session.blockOrigins }).blocks[0]?.kind).toBe('list');
    expect(composerMarkdown(source, { blockOrigins: session.blockOrigins }).blocks[1024]?.kind).toBe('paragraph');
    const history = new ComposerSourceSession();
    let retained = Array(600).fill('x').join('\n');
    history.observe('one', retained);
    for (let n = 0; n < 600; n++) {
      retained = retained.slice(0, n * 2) + '-' + retained.slice(n * 2 + 1);
      history.observe('one', retained, 'insertText');
    }
    history.observe('one', retained + '\nbody', 'insertFromPaste');
    expect(history.blockOrigins).toHaveLength(600);
    // Two 600-origin signatures already exceed the origin budget despite
    // fitting comfortably within the source-count and character budgets.
    history.observe('one', retained, 'historyUndo');
    expect(block(history, retained)?.kind).toBe('list');
    expect(history.blockOrigins).toEqual([]);
  });
  test('lengthened serialization retains the original closing gesture for that occurrence', () => {
    const session = new ComposerSourceSession(), source = '```ts\nx\n```';
    session.observe('one', source);
    const protectedSource = apply(session, 'one', source, session.input('one', source, { start: 7, end: 7 }, '\n```'));
    expect(protectedSource.source).toBe('````ts\nx\n```\n````');
    const exited = apply(session, 'one', protectedSource.source, session.key('one', protectedSource.source, protectedSource, enter));
    expect(exited).toEqual({ source: '```ts\nx\n```\n', start: 12, end: 12 });
  });
  test('language undo and redo restore logical fence gestures through the actual source producer', () => {
    for (const history of ['historyUndo', 'historyRedo']) {
      const client = new T3Client(), owner = 'thread', source = '```ts\nx\n```';
      const project = (value: string, inputType = 'insertReplacementText') => composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner, value, inputType });
      const common = { snapshotOwner: client.snapshotOwner, owner, composing: false, mode: 'rich' as const };
      const replace = (value: string, edit: { start: number; end: number; text: string }) => value.slice(0, edit.start) + edit.text + value.slice(edit.end);
      project(source);
      const insertion = composerIntentSource(client, { ...common, value: source, serial: 1, start: 7, end: 7, kind: 'input', text: '\n```' });
      expect(insertion.kind).toBe('edit');
      const protectedSource = replace(source, insertion), header = project(protectedSource).headers[0];
      if (!header) throw new Error('expected protected fence header');
      const language = composerIntentSource(client, { ...common, value: protectedSource, serial: 2, start: header.start, end: header.end, kind: 'language', language: 'go' });
      expect(language.kind).toBe('edit');
      const updated = replace(protectedSource, language);
      project(updated);
      project(protectedSource, 'historyUndo');
      if (history === 'historyRedo') project(updated, history);
      const value = history === 'historyUndo' ? protectedSource : updated;
      const close = composerIntentSource(client, { ...common, value, serial: 3, start: insertion.selectionStart, end: insertion.selectionEnd,
        kind: 'key', key: 'Enter', shift: false, command: false });
      expect(close.kind).toBe('edit');
      expect(replace(value, close)).toBe(`\`\`\`${history === 'historyUndo' ? 'ts' : 'go'}\nx\n\`\`\`\n`);
    }
  });
  test('logical fences share the bounded history origin budget with block metadata', () => {
    for (const suppressed of [false, true]) {
      const session = new ComposerSourceSession(), source = '```ts\nx\n```' + '\n\n```txt\nx\n```'.repeat(511);
      session.observe('one', source);
      const inserted = apply(session, 'one', source, session.input('one', source, { start: 7, end: 7 }, '\n```'));
      const tail = inserted.source + '\nx';
      session.observe('one', tail, 'insertText');
      const retained = suppressed ? inserted.source + '\n-' : tail;
      if (suppressed) session.observe('one', retained, 'insertText');
      session.observe('one', retained + (suppressed ? ' ' : 'y'), 'insertText');
      // Two sources with 512 fences fit exactly. Adding one paragraph origin
      // to each exceeds the same combined 1024-origin budget and evicts the old one.
      session.observe('one', retained, 'historyUndo');
      const close = session.key('one', retained, inserted, enter);
      expect(close.kind).toBe('edit');
      if (close.kind !== 'edit') throw new Error('expected fence edit');
      expect(close.edit.start).toBe(suppressed ? inserted.start : 0);
      expect(close.edit.text).toBe(suppressed ? '\n' : '```ts\nx\n```\n');
    }
  });
  test('closing a middle body line retains following source as paragraphs', () => {
    const session = new ComposerSourceSession(), source = '```\nx\n```\nafter';
    session.observe('one', source);
    const inserted = apply(session, 'one', source, session.input('one', source, { start: 5, end: 5 }, '\n```\ny'));
    const closingCaret = inserted.source.indexOf('\n```\n') + 4;
    const result = apply(session, 'one', inserted.source, session.key('one', inserted.source, { start: closingCaret, end: closingCaret }, enter));
    expect(result.source).toBe('```\nx\n```\n\ny\nafter');
    expect(result.start).toBe(10);
  });
  test('native undo/redo keeps grammar metadata and stale owners or source cannot edit another draft', () => {
    const session = new ComposerSourceSession(), source = '```\nx\n```';
    session.observe('one', source);
    const protectedSource = apply(session, 'one', source, session.input('one', source, { start: 5, end: 5 }, '\n```'));
    session.observe('one', source, 'historyUndo');
    session.observe('one', protectedSource.source, 'historyRedo');
    expect(apply(session, 'one', protectedSource.source, session.key('one', protectedSource.source, protectedSource, enter)).source).toBe('```\nx\n```\n');
    session.observe('two', protectedSource.source);
    expect(session.key('one', protectedSource.source, protectedSource, enter)).toEqual({ kind: 'stale' });
    expect(session.input('two', source, { start: 5, end: 5 }, 'x')).toEqual({ kind: 'stale' });
    // A restored source has its actual longer fence, without the previous owner's editing metadata.
    const ordinary = session.key('two', protectedSource.source, protectedSource, enter);
    expect(ordinary.kind).toBe('edit');
    if (ordinary.kind === 'edit') expect(applyComposerEdit(protectedSource.source, ordinary.edit)).toBe('````\nx\n```\n\n````');
  });
});
