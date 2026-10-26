import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { composerRichSource } from '../../src/composer/composer-rich-source';
import { composerIntentSource } from '../../src/composer/composer-intent-source';
import { composerSourceSession } from '../../src/composer/composer-rich-state';
import { refreshFileMetadata } from '../../src/shared/file-metadata';

/** The selected provider offers these skills to users, so their `$name` becomes a chip. */
function offerSkills(client: T3Client, names: string[]): void {
  client.providerId = 'codex';
  client.config.providers = [{ instanceId: 'codex', skills: names.map(name => ({ name })) }];
}

describe('root composer projection source', () => {
  test('live star/plus task prefixes are task blocks for the typing field only', () => {
    for (const source of ['* [x] ', '+ [ ] ']) {
      const client = new T3Client(), owner = 'thread';
      const project = (value: string, inputType = '') => composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner, value, inputType });
      project('');
      for (let end = 1; end <= source.length; end++) project(source.slice(0, end), 'insertText');
      expect(project(source).blocks[0]).toMatchObject({ kind: 'list', start: 0, end: source.length, contentStart: source.length });
      // A new mounted field restores */+ Markdown as bullets, without inheriting a task origin.
      const replacement = composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner: 'replacement', value: source });
      expect(replacement.blocks[0]).toMatchObject({ kind: 'list', contentStart: 2 });
    }
  });
  test('a language answer preserves logical fence identity through its accepted field observation', () => {
    const client = new T3Client(), owner = 'thread', source = '```ts\nx\n```';
    const project = (value: string) => composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner, value, inputType: 'insertReplacementText' });
    project(source);
    const common = { snapshotOwner: client.snapshotOwner, owner, composing: false, mode: 'rich' as const };
    const insertion = composerIntentSource(client, { ...common, value: source, serial: 1, start: 7, end: 7, kind: 'input', text: '\n```' });
    expect(insertion.kind).toBe('edit');
    const protectedSource = source.slice(0, insertion.start) + insertion.text + source.slice(insertion.end);
    const protectedView = project(protectedSource), header = protectedView.headers[0];
    if (!header) throw new Error('expected language header');
    const language = composerIntentSource(client, { ...common, value: protectedSource, serial: 2, start: header.start, end: header.end, kind: 'language', language: 'go' });
    expect(language).toMatchObject({ kind: 'edit', text: 'go', preserveSelection: true });
    const updated = protectedSource.slice(0, language.start) + language.text + protectedSource.slice(language.end);
    expect(project(updated).headers[0]?.language).toBe('go');
    const close = composerIntentSource(client, { ...common, value: updated, serial: 3, start: insertion.selectionStart, end: insertion.selectionEnd,
      kind: 'key', key: 'Enter', shift: false, command: false });
    expect(close.kind).toBe('edit');
    expect(updated.slice(0, close.start) + close.text + updated.slice(close.end)).toBe('```go\nx\n```\n');
  });
  test('typing a fence before an existing newline stays literal until Enter admits its header', () => {
    const client = new T3Client(), owner = 'thread';
    const project = (value: string, inputType = '') => composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner, value, inputType });
    project('\nbody');
    const source = '```\nbody';
    const unadmitted = project(source, 'insertText');
    expect(unadmitted.blocks.map(block => block.kind)).toEqual(['paragraph', 'paragraph']);
    expect(unadmitted.headers).toEqual([]);
    const common = { snapshotOwner: client.snapshotOwner, owner, value: source, serial: 1, start: 0, end: 3, composing: false, mode: 'rich' as const };
    expect(composerIntentSource(client, { ...common, kind: 'language', language: 'go' }).kind).toBe('stale');
    const enter = composerIntentSource(client, { ...common, start: 3, kind: 'key', key: 'Enter', shift: false, command: false });
    expect(enter.kind).toBe('edit');
    const next = source.slice(0, enter.start) + enter.text + source.slice(enter.end);
    expect(project(next, 'insertReplacementText').headers).toHaveLength(1);
    expect(composerIntentSource(client, { ...common, value: next, serial: 2, kind: 'language', language: 'go' }).kind).toBe('edit');
    expect(project(source, 'historyUndo').headers).toEqual([]);
    expect(project(next, 'historyRedo').headers).toHaveLength(1);
  });
  test('path paste in an unadmitted fence paragraph keeps its source and opens no code header', () => {
    const client = new T3Client(), owner = 'thread', source = '```\n@', text = '__name__.ts **body**';
    const project = (value: string, inputType = '') => composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner, value, inputType });
    project('\n@');
    expect(project(source, 'insertText').headers).toEqual([]);
    const reply = composerIntentSource(client, { snapshotOwner: client.snapshotOwner, owner, value: source, serial: 1,
      start: source.length, end: source.length, composing: false, mode: 'rich', kind: 'paste', text });
    expect(reply).toMatchObject({ kind: 'edit', start: source.length, end: source.length, text });
    expect(project(source + text, 'insertReplacementText').headers).toEqual([]);
  });
  test('actual insertText observations share live block admission with guarded key intents', () => {
    const client = new T3Client(), owner = 'thread';
    const project = (value: string, inputType = '') => composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner, value, inputType });
    project('');
    for (const value of ['-', '- ', '- [', '- [ ', '- [ ]']) {
      const view = project(value, 'insertText');
      expect(view.blocks.map(block => block.kind)).toEqual(['paragraph']);
    }
    const literal = '- [ ]';
    expect(composerIntentSource(client, { snapshotOwner: client.snapshotOwner, owner, value: literal, serial: 1,
      start: literal.length, end: literal.length, composing: false, mode: 'rich', kind: 'key', key: 'Tab', shift: false, command: false }).kind).toBe('default');
    const admitted = project('- [ ] ', 'insertText');
    expect(admitted.blocks[0]?.kind).toBe('list');
    expect(admitted.blocks[0]?.contentStart).toBe(6);
    expect(project(literal, 'historyUndo').blocks[0]?.kind).toBe('paragraph');
    expect(project('- [ ] ', 'historyRedo').blocks[0]?.kind).toBe('list');
    expect(project('***', 'insertFromPaste').blocks[0]?.kind).toBe('rule');
    // A different mounted field restores Markdown rather than inheriting typing origins.
    expect(composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner: 'another', value: literal }).blocks[0]?.kind).toBe('list');
  });
  test('preview file glyphs ignore conflicting active-workspace metadata', async () => {
    const client = new T3Client(), value = '@sample.ts ';
    Object.assign(client, { environmentId: 'env', threadId: 't1', projectId: 'p1', scopes: ['filesystem:read'], connection: 'connected', configLive: true, shellLive: true, threadLive: true });
    client.shell = { projects: [{ id: 'p1', workspaceRoot: '/repo' }], threads: [] };
    client.local.drafts[client.draftKey] = value;
    client.restAccess = () => ({ request: async () => ({ entries: [{ kind: 'directory' }] }) }) as ReturnType<T3Client['restAccess']>;
    const native = { available: true, watch() {}, later: async () => ({}) };
    await refreshFileMetadata(client, native, 0);
    const active = composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner: 'thread', value });
    const preview = composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner: 'preview', value, surface: 'preview' });
    expect(active.chips[0]?.face.icon).toBe('folder');
    expect(preview.chips[0]?.face.icon).toBe('at-sign');
    expect(preview.chips[0]?.face.fileToken).toBe('');
  });
  test('a path pasted after @ keeps its underscores in the source', () => {
    const client = new T3Client(), owner = 'thread', text = 'src/__name__/file.ts **body**';
    composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner, value: '@' });
    const reply = composerIntentSource(client, { snapshotOwner: client.snapshotOwner, owner, value: '@', serial: 1,
      start: 1, end: 1, composing: false, mode: 'rich', kind: 'paste', text });
    expect(reply).toMatchObject({ kind: 'edit', start: 1, end: 1, text, selectionStart: 1 + text.length });
    expect(composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner, value: '@' + reply.text }).value).toBe('@' + text);
  });
  test('rich-off and literal answers retain ordinary Enter, newline and paste semantics without block grammar', () => {
    for (const literal of [false, true]) {
      const client = new T3Client(), owner = 'thread';
      client.local.clientSettings.composerRichTextEnabled = literal;
      if (literal) client.thread = { projection: {
        runtimeRequests: [{ id: 'q', kind: 'user_input', status: 'pending' }],
        turnItems: [{ type: 'user_input_request', requestId: 'q', questions: [{ id: 'edit', allowCustomAnswer: true, initialAnswer: '```' }] }],
      }, sequence: 0, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null };
      const mode: 'literal' | 'plain' = literal ? 'literal' : 'plain';
      for (const value of ['```', '- item', '> quote', '```ts\n```']) {
        const view = composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner, value });
        // The plain composer continues and indents a list typed as text (composer-list-continuation.ts); a literal answer does not.
        const list = !literal && value === '- item';
        expect(view.mode).toBe(mode);
        expect(view.blocks.map(block => block.kind)).toEqual(view.blocks.map(() => list ? 'list' : 'paragraph'));
        const common = { snapshotOwner: client.snapshotOwner, owner, value, serial: 1, start: value.length, end: value.length, composing: false, mode };
        expect(composerIntentSource(client, { ...common, kind: 'key', key: 'Enter', shift: false, command: false })).toMatchObject({ kind: 'send', mode });
        expect(composerIntentSource(client, { ...common, kind: 'key', key: 'Enter', shift: true, command: false }))
          .toMatchObject({ kind: 'edit', start: value.length, end: value.length, text: list ? '\n- ' : '\n' });
        expect(composerIntentSource(client, { ...common, kind: 'key', key: 'Tab', shift: false, command: false }))
          .toMatchObject(list ? { kind: 'edit', start: 0, end: 0, text: '  ', selectionStart: value.length + 2 } : { kind: 'default' });
        expect(composerIntentSource(client, { ...common, mode: 'rich', kind: 'key', key: 'Enter', shift: false, command: false }).kind).toBe('stale');
      }
      const value = '```ts\n```';
      composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner, value });
      for (const kind of ['input', 'paste']) {
        const common = { snapshotOwner: client.snapshotOwner, owner, value, serial: 1, start: 6, end: 6, composing: false, mode };
        const result = kind === 'input' ? composerIntentSource(client, { ...common, kind: 'input', text: '\n```\ny' })
          : composerIntentSource(client, { ...common, kind: 'paste', text: '\n```\ny' });
        expect(result).toMatchObject({ kind: 'edit', start: 6, end: 6, text: '\n```\ny' });
      }
    }
  });
  test('chips are their source in independently observed fields: a separator deleted or undone follows the editor', () => {
    const client = new T3Client(), owner = 'thread';
    offerSkills(client, ['deploy']);
    const project = (value: string, inputType: string) => composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner, value, inputType });
    project('', '');
    expect(project('$dep', 'insertText').chips).toEqual([]);
    expect(project('$deploy ', 'insertText').chips).toHaveLength(1);
    // A token at the end of the source is no atom (LLP 1045.000 D1), so it is no chip either.
    expect(project('$deploy', 'deleteContentBackward').chips).toEqual([]);
    const preview = (value: string, inputType: string) => composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner: 'preview', value, inputType, surface: 'preview' });
    expect(preview('$frontend-design ', '').chips).toHaveLength(1);
    expect(preview('$frontend-design', 'deleteContentBackward').chips).toEqual([]);
    expect(composerSourceSession(client).matches(owner, '$deploy')).toBe(true);
    expect(project('$deploy ', 'historyUndo').chips.map(chip => chip.source)).toEqual(['$deploy']);
    expect(project('', 'historyRedo').chips).toEqual([]);
    const changed = composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner: 'another', value: '$deploy ' });
    expect(changed.chips).toHaveLength(1);
  });
  test('Settings preview shares chip projection without rich syntax or changing the active composer session', () => {
    const client = new T3Client(), owner = 'thread', value = '```ts';
    composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner, value });
    const sample = '**sample** $frontend-design ';
    const preview = composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner: 'preview', value: sample, surface: 'preview' });
    expect(preview.mode).toBe('plain');
    expect(preview.blocks.every(block => block.kind === 'paragraph')).toBe(true);
    expect(preview.chips).toHaveLength(1);
    expect(composerIntentSource(client, { snapshotOwner: client.snapshotOwner, owner, value, serial: 1,
      start: value.length, end: value.length, composing: false, mode: 'rich', kind: 'key', key: 'Enter', shift: false, command: false }))
      .toMatchObject({ kind: 'edit', text: '```ts\n\n```' });
  });
  test('product faces own the same label and state on composer and preview', () => {
    const client = new T3Client(), value = '$frontend-design ';
    client.providerId = 'codex';
    client.config.providers = [{ instanceId: 'codex', skills: [{ name: 'frontend-design', displayName: 'Custom Workspace Skill' }] }];
    const active = composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner: 'thread', value });
    const preview = composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner: 'preview', value, surface: 'preview' });
    expect(active.chips[0]?.face.label).toBe('Custom Workspace Skill');
    expect(preview.chips[0]?.face.label).toBe('Frontend Design');
    const refs = '[PR #7](t3-context://v1/review-comment/review-comment_p) [gone](t3-context://v1/terminal/terminal_missing) [old](t3-context://v1/terminal/terminal_old)';
    client.local.reviewComments = { 'review-comment_p': { kind: 'review-comment', contextId: 'review-comment_p', pullRequest: { state: 'merged', number: 7 } } };
    client.local.terminalContexts = { terminal_old: { kind: 'terminal', contextId: 'terminal_old', text: '' } };
    const contexts = composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner: 'thread', value: refs });
    expect(contexts.chips.map(chip => chip.face.kind)).toEqual(['pr-merged', 'neutral', 'invalid']);
  });
  test('projects the live controlled value independently of persisted draft timing', () => {
    const client = new T3Client(), value = '**live** $frontend-design ';
    offerSkills(client, ['frontend-design']);
    const view = composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner: 'thread:question', value });
    expect(view.owner).toBe('thread:question');
    expect(view.value).toBe(value);
    expect(view.chips[0]).toMatchObject({
      kind: 'skill', start: 9, end: 25, source: '$frontend-design', label: 'frontend-design', name: 'frontend-design',
      path: '', contextKind: '', contextId: '', image: false, href: '', comment: '', quote: '',
    });
  });
  test('a stale snapshot owner gets no active metadata or chip actions', () => {
    const client = new T3Client(), value = '# Heading\n$skill ';
    const view = composerRichSource(client, { snapshotOwner: client.snapshotOwner + '/old', owner: 'old-thread', value });
    expect(view.owner).toBe('');
    expect(view.mode).toBe('literal');
    expect(view.headers).toEqual([]);
    expect(view.chips).toEqual([]);
  });
});
