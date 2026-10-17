// The reply-link rows match the reference (the user's decision of 2026-10-11). A Markdown `#fragment`
// link's click leaves the thread for the new-thread draft, as Electron's
// hash router does with the hash the click sets (T3 Code 1e2ecbd975, MIT, see LICENSE-T3: ChatMarkdown.tsx
// `handleMarkdownFragmentClick`, main.tsx `createHashHistory`, routes/_chat.index.tsx `IndexDraftLanding`). The
// navigation itself is browser-links.test.ts; the link branches are external-link-menu.test.ts; the shell's menu for the
// ContextMenu key is the AppKit `contextmenu` binary (reply-links.swift).
import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import type { Files, Native } from '../../src/client/protocol';
import type { Obj } from '../../src/client/domain';

const files: Files = { fs: { mkdir: async () => undefined, readFile: async () => { throw new Error('none'); }, atomicWriteFile: async () => undefined } } as unknown as Files;

describe('a fragment link leaves the thread', () => {
  test('chatlocal:link-open with a fragment answers sidebar:new-thread with the most recently active project\'s draft open', async () => {
    const owner = new T3Client();
    owner.origin = 'http://127.0.0.1:41857'; owner.generation = 0; owner.environmentId = 'env'; owner.projectId = 'fixture'; owner.threadId = 'verify-timeline';
    owner.shell = { sequence: 1, projects: [{ id: 'fixture', title: 'Verification fixture', updatedAt: '2026-10-01T00:00:00Z' }, { id: 'work', title: 'work', updatedAt: '2026-10-09T00:00:00Z' }],
      threads: [{ id: 'verify-timeline', projectId: 'fixture', updatedAt: '2026-10-05T00:00:00Z' }] };
    const calls: Obj[] = [];
    const native: Native = { available: true, watch() {}, later: async (input: unknown) => { calls.push(input as Obj); return { ok: true, generation: 0, value: {} }; } };
    const result = await owner.command('chatlocal:link-open', 'link|', '#notes', 0, native, files);
    expect(result.message).toBe('sidebar:new-thread');
    expect([owner.threadId, owner.projectId]).toEqual(['', 'work']);
    // The click's modifiers ride with its press; nothing goes to the system browser, the
    // module's opener or its gesture recorder.
    expect(calls.map(call => call.op).filter(op => op !== 'devicePresentation')).toEqual([]);
    // A mailto link (in prose or a table cell) answers nothing and keeps the thread.
    owner.threadId = 'verify-timeline'; owner.projectId = 'fixture';
    expect((await owner.command('chatlocal:link-open', 'link', 'mailto:list@example.test', 0, native, files)).message).toBe('');
    expect([owner.threadId, owner.projectId]).toEqual(['verify-timeline', 'fixture']);
  });
});
