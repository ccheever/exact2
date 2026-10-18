import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { composerRichSource } from '../../src/composer/composer-rich-source';
import { composerIntentSource } from '../../src/composer/composer-intent-source';
import { composerSourceSession } from '../../src/composer/composer-rich-state';
import { refreshFileMetadata } from '../../src/shared/file-metadata';

describe('root composer projection source', () => {
  test('actual insertText observations share live block admission with guarded key intents', () => {
    const client = new T3Client(), owner = 'thread';
    const project = (value: string, inputType = '') => composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner, value, inputType });
    project('');
    for (const value of ['-', '- ', '- [', '- [ ', '- [ ]']) {
      const view = project(value, 'insertText');
      expect(view.blocks[0]?.kind).toBe('paragraph');
      expect(view.decorations.blocks).toEqual([]);
      expect(view.decorations.hidden).toEqual([]);
    }
    const literal = '- [ ]';
    expect(composerIntentSource(client, { snapshotOwner: client.snapshotOwner, owner, value: literal, serial: 1,
      start: literal.length, end: literal.length, composing: false, mode: 'rich', kind: 'key', key: 'Tab', shift: false, command: false }).kind).toBe('default');
    const admitted = project('- [ ] ', 'insertText');
    expect(admitted.blocks[0]?.kind).toBe('list');
    expect(admitted.decorations.blocks[0]?.marker).toBe('checkbox:off');
    expect(project(literal, 'historyUndo').blocks[0]?.kind).toBe('paragraph');
    expect(project('- [ ] ', 'historyRedo').decorations.blocks[0]?.marker).toBe('checkbox:off');
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
  test('actual path-query paste protects only its literal prefix and rebases with surrounding edits', () => {
    const client = new T3Client(), owner = 'thread', text = 'src/__name__/file.ts **body**';
    composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner, value: '@' });
    const reply = composerIntentSource(client, { snapshotOwner: client.snapshotOwner, owner, value: '@', serial: 1,
      start: 1, end: 1, composing: false, mode: 'rich', kind: 'paste', text });
    expect(reply.kind).toBe('edit');
    const value = '@' + reply.text;
    const view = composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner, value });
    expect(view.value).toBe('@' + text);
    expect(view.decorations.spans.map(span => value.slice(span.start, span.end))).toEqual(['body']);
    expect(composerSourceSession(client).literalRanges).toEqual([{ start: 1, end: 1 + 'src/__name__/file.ts'.length }]);
    const shifted = '😀\n' + value;
    composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner, value: shifted, inputType: 'insertText' });
    expect(composerSourceSession(client).literalRanges).toEqual([{ start: 4, end: 4 + 'src/__name__/file.ts'.length }]);
    composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner: 'another-thread', value: shifted });
    expect(composerSourceSession(client).literalRanges).toEqual([]);
  });
  test('native undo and redo restore pasted literal origins through the actual producer without crossing owners', () => {
    const client = new T3Client(), owner = 'thread', text = 'src/__name__/file.ts';
    composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner, value: '@' });
    const reply = composerIntentSource(client, { snapshotOwner: client.snapshotOwner, owner, value: '@', serial: 1,
      start: 1, end: 1, composing: false, mode: 'rich', kind: 'paste', text });
    expect(reply.kind).toBe('edit');
    const value = '@' + reply.text;
    const project = (value: string, inputType: string) => composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner, value, inputType });
    expect(project(value, 'insertReplacementText').decorations.spans).toEqual([]);
    expect(project('@', 'historyUndo').decorations.spans).toEqual([]);
    expect(composerSourceSession(client).literalRanges).toEqual([]);
    expect(project(value, 'historyRedo').decorations.spans).toEqual([]);
    expect(composerSourceSession(client).literalRanges).toEqual([{ start: 1, end: value.length }]);
    project('😀\n' + value, 'insertText');
    expect(composerSourceSession(client).literalRanges).toEqual([{ start: 4, end: value.length + 3 }]);
    expect(project(value, 'historyUndo').decorations.spans).toEqual([]);
    const changedOwner = composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner: 'other', value, inputType: 'historyRedo' });
    expect(composerSourceSession(client).literalRanges).toEqual([]);
    expect(changedOwner.decorations.spans.map(span => value.slice(span.start, span.end))).toEqual(['name']);
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
        expect(view.mode).toBe(mode);
        expect(view.blocks.every(block => block.kind === 'paragraph')).toBe(true);
        const common = { snapshotOwner: client.snapshotOwner, owner, value, serial: 1, start: value.length, end: value.length, composing: false, mode };
        expect(composerIntentSource(client, { ...common, kind: 'key', key: 'Enter', shift: false, command: false })).toMatchObject({ kind: 'send', mode });
        expect(composerIntentSource(client, { ...common, kind: 'key', key: 'Enter', shift: true, command: false }))
          .toMatchObject({ kind: 'edit', start: value.length, end: value.length, text: '\n' });
        expect(composerIntentSource(client, { ...common, kind: 'key', key: 'Tab', shift: false, command: false }).kind).toBe('default');
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
  test('completed chips survive separator deletion and native history in independently observed fields', () => {
    const client = new T3Client(), owner = 'thread';
    const project = (value: string, inputType: string) => composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner, value, inputType });
    project('', '');
    expect(project('$dep', 'insertText').decorations.atoms).toEqual([]);
    expect(project('$deploy ', 'insertText').decorations.atoms).toHaveLength(1);
    const completed = project('$deploy', 'deleteContentBackward');
    expect(completed.chips.map(chip => chip.source)).toEqual(['$deploy']);
    const preview = (value: string, inputType: string) => composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner: 'preview', value, inputType, surface: 'preview' });
    expect(preview('$frontend ', '').decorations.atoms).toHaveLength(1);
    expect(preview('$frontend', 'deleteContentBackward').chips.map(chip => chip.source)).toEqual(['$frontend']);
    expect(composerSourceSession(client).matches(owner, '$deploy')).toBe(true);
    expect(project('', 'deleteContentBackward').decorations.atoms).toEqual([]);
    expect(project('$deploy', 'historyUndo').chips.map(chip => chip.source)).toEqual(['$deploy']);
    expect(project('', 'historyRedo').decorations.atoms).toEqual([]);
    expect(project('$deploy', 'historyUndo').decorations.atoms).toHaveLength(1);
    const changed = composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner: 'another', value: '$deploy' });
    expect(changed.decorations.atoms).toEqual([]);
  });
  test('Settings preview shares chip projection without rich syntax or changing the active composer session', () => {
    const client = new T3Client(), owner = 'thread', value = '```ts';
    composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner, value });
    const sample = '**sample** $frontend-design ';
    const preview = composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner: 'preview', value: sample, surface: 'preview' });
    expect(preview.mode).toBe('plain');
    expect(preview.decorations.spans).toEqual([]);
    expect(preview.decorations.atoms).toHaveLength(1);
    expect(composerIntentSource(client, { snapshotOwner: client.snapshotOwner, owner, value, serial: 1,
      start: value.length, end: value.length, composing: false, mode: 'rich', kind: 'key', key: 'Enter', shift: false, command: false }))
      .toMatchObject({ kind: 'edit', text: '```ts\n```' });
  });
  test('product faces own the same atom label and state palette on composer and preview', () => {
    const client = new T3Client(), value = '$frontend-design ';
    client.providerId = 'codex';
    client.config.providers = [{ instanceId: 'codex', skills: [{ name: 'frontend-design', displayName: 'Custom Workspace Skill' }] }];
    const active = composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner: 'thread', value });
    const preview = composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner: 'preview', value, surface: 'preview' });
    expect(active.decorations.atoms[0]?.label).toBe('\u2003Custom Workspace Skill');
    expect(active.chips[0]?.face.label).toBe('Custom Workspace Skill');
    expect(preview.decorations.atoms[0]?.label).toBe('\u2003Frontend Design');
    expect(preview.chips[0]?.face.label).toBe('Frontend Design');
    const refs = '[PR #7](t3-context://v1/review-comment/review-comment_p) [gone](t3-context://v1/terminal/terminal_missing) [old](t3-context://v1/terminal/terminal_old)';
    client.local.reviewComments = { 'review-comment_p': { kind: 'review-comment', contextId: 'review-comment_p', pullRequest: { state: 'merged', number: 7 } } };
    client.local.terminalContexts = { terminal_old: { kind: 'terminal', contextId: 'terminal_old', text: '' } };
    const contexts = composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner: 'thread', value: refs });
    expect(contexts.chips.map(chip => chip.face.kind)).toEqual(['pr-merged', 'neutral', 'invalid']);
    expect(contexts.decorations.atoms.map(atom => atom.name)).toEqual(['cmp-chip-review-comment', 'cmp-chip-neutral', 'cmp-chip-invalid']);
    expect(contexts.decorations.atoms.map(atom => atom.label)).toEqual(contexts.chips.map(chip => chip.face.atomLabel));
  });
  test('projects the live controlled value independently of persisted draft timing', () => {
    const client = new T3Client(), value = '**live** $frontend-design ';
    const view = composerRichSource(client, { snapshotOwner: client.snapshotOwner, owner: 'thread:question', value });
    expect(view.owner).toBe('thread:question');
    expect(view.value).toBe(value);
    expect(view.decorations.value).toBe(value);
    expect(view.decorations.spans.map(span => value.slice(span.start, span.end))).toEqual(['live']);
    expect(view.chips[0]).toMatchObject({
      kind: 'skill', start: 9, end: 25, source: '$frontend-design', label: 'frontend-design', name: 'frontend-design',
      path: '', contextKind: '', contextId: '', image: false, href: '', comment: '', quote: '',
    });
  });
  test('a stale snapshot owner gets no active metadata or chip actions', () => {
    const client = new T3Client(), value = '# Heading\n$skill ';
    const view = composerRichSource(client, { snapshotOwner: client.snapshotOwner + '/old', owner: 'old-thread', value });
    expect(view.owner).toBe('');
    expect(view.decorations).toEqual({ value, spans: [], blocks: [], hidden: [], atoms: [] });
    expect(view.headers).toEqual([]);
    expect(view.chips).toEqual([]);
  });
});
