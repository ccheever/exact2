// Ported from T3 Code 6d450a4284 apps/web/src/lib/toolCallDiff.test.ts (MIT; see LICENSE-T3), reading the patch with the
// clone's parse (renderablePatch) where the reference reads it with Pierre's, then the diff panel's tool-call source
// (DiffPanel.tsx and diffPanelStore.ts selectToolCall): its request, its label, and its recorded content.
import { describe, expect, it } from 'bun:test';
import { renderablePatch } from '../../src/pull-requests/pages-pr-code-logic';
import { toolCallDiff } from '../../src/diff/tool-call-diff';
import { T3Client } from '../../src/client/client';
import { adoptDiff, diffRequest, diffSnapshot, selectToolCall } from '../../src/diff/diff';

const render = (patch: string | undefined) => renderablePatch(patch ?? '', false);

describe('toolCallDiff', () => {
  it('renders recorded hunks without reading a repository, including renamed paths', () => {
    for (const changes of [undefined, [{ operation: 'update', oldPath: 'old.ts', path: 'new.ts' }]]) {
      const { patch } = toolCallDiff({ fileName: 'old.ts', diffStr: '@@ -1 +1 @@\n-before\n+after\n', ...(changes ? { changes } : {}) });
      const rendered = render(patch);
      if (rendered?.kind !== 'files') throw new Error('Expected a file diff');
      expect(rendered.files).toHaveLength(1);
      expect(rendered.files[0]!.path).toBe(changes ? 'new.ts' : 'old.ts');
      expect(rendered.files[0]!.additionLines.join('')).toContain('after');
      expect(rendered.files[0]!.deletionLines.join('')).toContain('before');
    }
  });

  it('renders added and deleted file contents as a file diff', () => {
    for (const operation of ['add', 'delete']) {
      for (const [content, ending] of [
        ['first\nsecond', ''], ['@@ -0,0 +1 @@\n+content', '\n'], ['@@ -1 +0,0 @@\n-content', '\n'], ['first\nsecond', '\n'], ['--- notes\nsecond', '\n'],
        ['diff --git notes\nsecond', '\n'], ['--- a/embedded.ts\n+++ b/embedded.ts\n@@ -1 +1 @@\n-before\n+after', '\n'],
        ['diff --git a/embedded.ts b/embedded.ts\n--- a/embedded.ts\n+++ b/embedded.ts\n@@ -1 +1 @@\n-before\n+after', '\n'],
      ] as const) {
        const recordedContent = `${content}${ending}`;
        const rendered = render(toolCallDiff({ fileName: 'file with spaces.ts', diffStr: recordedContent, changes: [{ operation, path: 'file with spaces.ts' }] }).patch);
        if (rendered?.kind !== 'files') throw new Error('Expected a file diff');
        const file = rendered.files[0]!, lines = content.split('\n').length;
        expect(file.path).toBe('file with spaces.ts');
        expect(file.additionLines).toHaveLength(operation === 'add' ? lines : 0);
        expect(file.deletionLines).toHaveLength(operation === 'delete' ? lines : 0);
        expect((operation === 'add' ? file.additionLines : file.deletionLines).join('')).toBe(recordedContent.replace(/\n$/, '').replaceAll('\n', ''));
      }
    }
  });

  it('keeps content-only writes out of the patch and preserves empty recorded content', () => {
    for (const operation of ['add', 'delete']) {
      expect(toolCallDiff({ fileName: 'empty.ts', diffStr: '', changes: [{ operation, path: 'empty.ts' }] }).content)
        .toEqual([{ label: operation === 'add' ? 'Recorded content after' : 'Recorded content before', text: '' }]);
    }
    expect(toolCallDiff({ fileName: 'a.ts', oldStr: 'before', newStr: '' })).toEqual({
      patch: undefined, content: [{ label: 'Recorded content before', text: 'before' }, { label: 'Recorded content after', text: '' }], truncated: false, recordedFiles: 0 });
    expect(toolCallDiff({ fileName: 'a.ts', newStr: 'after' }).content).toEqual([{ label: 'Recorded content after', text: 'after' }]);
  });

  it('removes transport truncation markers before converting file contents', () => {
    const suffix = '\n… output truncated for transport';
    const diff = toolCallDiff({ fileName: 'a.ts', diffStr: `first\nsecond${suffix}`, changes: [{ operation: 'add', path: 'a.ts' }] });
    expect(diff.truncated).toBe(true);
    expect(diff.patch).not.toContain('output truncated');
    expect(render(diff.patch)?.kind).toBe('files');
    expect(toolCallDiff({ fileName: 'a.ts', newStr: `content${suffix}` })).toMatchObject({ content: [{ label: 'Recorded content after', text: 'content' }], truncated: true });
  });

  it('preserves complete patches and unsupported provider output', () => {
    for (const operation of ['add', 'delete']) {
      const diffStr = ['diff --git a/a.ts b/a.ts', `--- ${operation === 'add' ? '/dev/null' : 'a/a.ts'}`, `+++ ${operation === 'add' ? 'b/a.ts' : '/dev/null'}`,
        operation === 'add' ? '@@ -0,0 +1 @@' : '@@ -1 +0,0 @@', operation === 'add' ? '+content' : '-content', ''].join('\n');
      expect(toolCallDiff({ fileName: 'a.ts', diffStr, changes: [{ operation, path: 'a.ts' }] }).patch).toBe(diffStr);
    }
    for (const diffStr of [undefined, '', 'provider output', '--- a/a.ts\n+++ b/a.ts\n@@ -1 +1 @@\n-a\n+b\n']) {
      expect(toolCallDiff({ fileName: 'a.ts', ...(diffStr === undefined ? {} : { diffStr }) }).patch).toBe(diffStr);
    }
  });
});

describe('the diff panel\'s tool-call source (DiffPanel)', () => {
  const fixture = () => {
    const client = new T3Client();
    Object.assign(client, { environmentId: 'env', projectId: 'p1', threadId: 't1', diffOpen: true });
    const change = { id: 'item-1', type: 'file_change', status: 'completed', updatedAt: '2026-10-31T10:00:00Z', fileName: 'src/a.ts' };
    client.thread = { projection: { thread: { id: 't1' }, visibleTurnItems: [{ item: change }], turnItems: [] }, sequence: 0, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null };
    return client;
  };
  it('asks for the item at its revision, labels the scope "Tool call", and shows its patch', () => {
    const client = fixture();
    selectToolCall(client, 'item-1');
    const request = diffRequest(client);
    expect([request.method, request.payload]).toEqual(['orchestration.getTurnItem', { threadId: 't1', itemId: 'item-1', revision: '2026-10-31T10:00:00Z' }]);
    client.diffText = adoptDiff(client, request, { item: { id: 'item-1', type: 'file_change', fileName: 'src/a.ts', diffStr: '@@ -1 +1 @@\n-a\n+b\n',
      changes: [{ operation: 'update', path: 'src/a.ts' }, { operation: 'update', path: 'src/b.ts' }] } });
    const view = diffSnapshot(client, 0);
    expect([view.diffScopeLabel, view.diffScope, view.diffCanRefresh, view.diffFiles.map(file => file.path), view.diffTreeLabel]).toEqual(['Tool call', '', false, ['src/a.ts'], 'Tool call files']);
    expect(view.diffToolCallNote).toBe('This tool call changed 2 files. The recorded patch omits 1 of them.');
  });
  it('a change recorded as content shows that content', () => {
    const client = fixture();
    selectToolCall(client, 'item-1');
    client.diffText = adoptDiff(client, diffRequest(client), { item: { id: 'item-1', type: 'file_change', fileName: 'src/a.ts', oldStr: 'old', newStr: '' } });
    expect(diffSnapshot(client, 0).diffToolCallContent).toEqual([{ label: 'Recorded content before', text: 'old' }, { label: 'Recorded content after', text: 'Empty content.' }]);
  });
  it('a row that is not a file change opens nothing', () => {
    expect(() => selectToolCall(fixture(), 'missing')).toThrow('That file change is no longer available.');
  });
});
