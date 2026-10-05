// Lane r4-timeline: sent-message chips resolved against HEAD's message shapes
// (OrchestrationMessageContext records, ChatAttachment), file-link paths, and
// the Markdown settings the chat reads.
import { describe, expect, test } from 'bun:test';
import { attachmentSize, chippedAttachmentIds, diffSchemeOf, fileLinkTarget, markdownEnv, messageChips, middleTruncate } from './r4-timeline-chips';
import type { T3Client } from './client';

const item = (text: string, records: object[] = [], attachments: object[] = []) => ({ type: 'user_message', text, context: { version: 1, records }, attachments });

describe('sent-message context chips', () => {
  test('a file record bound to its attachment shows name, size and the two-line tooltip; its file row is left out', () => {
    const chips = messageChips(item('see [notes.md](t3-context://v1/file/file_abc) ok',
      [{ version: 1, kind: 'file', contextId: 'file_abc', label: 'notes.md', attachmentId: 'att-1', name: 'notes.md', mimeType: 'text/markdown', sizeBytes: 812 }],
      [{ type: 'file', id: 'att-1', name: 'notes.md', mimeType: 'text/markdown', sizeBytes: 812 }]), '/w', [], 'm1');
    expect(chips).toEqual([{ id: 'chip-0', href: 't3-context://v1/file/file_abc', kind: 'file', label: 'notes.md', size: '1 KB', tip: 'notes.md\n1 KB', detail: '', icon: 'markdown', target: 'att-1', owner: 'm1' }]);
    expect([...chippedAttachmentIds(chips)]).toEqual(['att-1']);
  });
  test('a reference without its record, or whose attachment is gone, reads "This context is no longer available."', () => {
    const chips = messageChips(item('[deploy](t3-context://v1/skill/skill_deploy) and ![a.png](t3-context://v1/image/image_x)',
      [{ version: 1, kind: 'image', contextId: 'image_x', label: 'a.png', attachmentId: 'gone', name: 'a.png', mimeType: 'image/png', sizeBytes: 10 }]), '', []);
    expect(chips.map(chip => [chip.kind, chip.label, chip.tip])).toEqual([
      ['unresolved', 'deploy', 'This context is no longer available.'], ['unresolved', 'a.png', 'This context is no longer available.']]);
  });
  test('skill, mention, thread and image records resolve to their own presentation', () => {
    const chips = messageChips(item('[Deploy](t3-context://v1/skill/skill_d) [a.ts](t3-context://v1/mention/mention_a) [Old](t3-context://v1/thread/thread_t) ![shot.png](t3-context://v1/image/image_s) [x](t3-context://v1/thread/thread_u)', [
      { version: 1, kind: 'skill', contextId: 'skill_d', label: 'Deploy', name: 'deploy' },
      { version: 1, kind: 'mention', contextId: 'mention_a', label: 'a.ts', path: 'src/a.ts' },
      { version: 1, kind: 'thread', contextId: 'thread_t', label: 'Old', environmentId: 'e', threadId: 't1', title: 'Old' },
      { version: 1, kind: 'thread', contextId: 'thread_u', label: 'x', environmentId: 'e', threadId: 'missing', title: 'x' },
      { version: 1, kind: 'image', contextId: 'image_s', label: 'shot.png', attachmentId: 'img', name: 'shot.png', mimeType: 'image/png', sizeBytes: 2048 },
    ], [{ type: 'image', id: 'img', name: 'shot.png', mimeType: 'image/png', sizeBytes: 2048 }]), '', [{ id: 't1', title: 'Renamed thread' }], 'm');
    expect(chips.map(chip => [chip.kind, chip.label, chip.tip, chip.target])).toEqual([
      ['skill', 'Deploy', '$deploy', ''], ['mention', 'a.ts', 'src/a.ts', ''], ['thread', 'Renamed thread', 'Open thread', 't1'],
      ['image', 'shot.png', 'Image attachment, shot.png, 2 KB', 'img'], ['thread', 'x', 'Thread no longer available', '']]);
  });
  test('a video file chip leads with the film glyph; popover kinds carry their details', () => {
    const chips = messageChips(item('[clip.mov](t3-context://v1/file/file_v) [out](t3-context://v1/terminal/terminal_1)', [
      { version: 1, kind: 'file', contextId: 'file_v', label: 'clip.mov', attachmentId: 'v', name: 'clip.mov', mimeType: 'video/quicktime', sizeBytes: 3 * 1024 * 1024 },
      { version: 1, kind: 'terminal', contextId: 'terminal_1', label: 'out', terminalId: 't', terminalLabel: 'Terminal 1', lineStart: 2, lineEnd: 4, text: 'ls\nok' },
    ], [{ type: 'file', id: 'v', name: 'clip.mov', mimeType: 'video/quicktime', sizeBytes: 3 * 1024 * 1024 }]), '', []);
    expect(chips.map(chip => [chip.kind, chip.size, chip.tip, chip.detail])).toEqual([['video', '3.0 MB', 'clip.mov\n3.0 MB', ''], ['terminal', '', 'Terminal 1 lines 2-4', 'ls\nok']]);
  });
  test('workspace file links carry their full path; web links and images are not chips', () => {
    const chips = messageChips({ text: 'Open [a](src/a.ts#L3), [b](./docs/../b.md:4), [abs](/etc/hosts), [web](https://x.dev) and ![i](pic.png)' }, '/repo/root', []);
    expect(chips.map(chip => [chip.href, chip.tip])).toEqual([
      ['t3-file:src/a.ts#L3', '/repo/root/src/a.ts:3'], ['t3-file:./docs/../b.md:4', '/repo/root/b.md:4'], ['t3-file:/etc/hosts', '/etc/hosts']]);
  });
  test('an assistant quote reads its quote cut at 64 characters and "View source" opens the cited thread', () => {
    const quote = 'q'.repeat(70);
    const chips = messageChips({ text: `see [Assistant quote](t3-citation://v1/env/thread-9/msg-2?text=${quote}&start=0&end=70)` }, '', []);
    expect(chips.map(chip => [chip.kind, chip.label, chip.tip, chip.target, chip.detail])).toEqual([['citation', `${'q'.repeat(64)}…`, 'View source', 'thread-9', 'msg-2']]);
  });
  test('formatAttachmentSize, middle truncation and path resolution follow the reference helpers', () => {
    expect([attachmentSize(0), attachmentSize(1025), attachmentSize(1024 * 1024)]).toEqual(['1 KB', '2 KB', '1.0 MB']);
    expect(middleTruncate('a-very-long-attachment-name-that-goes-on-and-on.png')).toBe('a-very-long-attachmen…-on-and-on.png');
    expect(fileLinkTarget('../x.md', '/a/b')).toBe('/a/x.md');
  });
});

describe('Markdown settings', () => {
  const client = (settings: object) => ({ local: { clientSettings: settings } }) as unknown as T3Client;
  test('code size clamps to 10–18, word wrap and the monospace family come from Settings → Appearance', () => {
    expect(markdownEnv(client({ fontSizeCode: 22, wordWrap: false, fontFamilyCode: 'Menlo' }))).toEqual({ codeFont: 'monospace', codeSize: 18, wrap: false, chips: [] });
    expect(markdownEnv(client({ fontSizeCode: 13, wordWrap: true, fontFamilyCode: '' }))).toEqual({ codeFont: 'ui-monospace', codeSize: 13, wrap: true, chips: [] });
    expect(markdownEnv({} as T3Client).codeSize).toBe(13);
  });
  test('diff colors default to red-green', () => {
    expect([diffSchemeOf(client({ diffColorScheme: 'blue-orange' })), diffSchemeOf(client({})), diffSchemeOf({} as T3Client)]).toEqual(['blue-orange', 'red-green', 'red-green']);
  });
});

describe('subagent divider (TimelineSystemDivider)', () => {
  test('"Subagent of" and the parent title are separate parts, the bot glyph leads and the pill opens the parent', async () => {
    const { subagentLead } = await import('./sidebar-lineage');
    const client = { threadId: 'kid', shell: { threads: [{ id: 'mom', title: 'Parent work', lineage: {} },
      { id: 'kid', title: 'Explore', createdAt: '2026-10-04T10:00:00.000Z', lineage: { relationshipToParent: 'subagent', parentThreadId: 'mom' } }] } } as unknown as T3Client;
    expect(subagentLead(client)).toEqual([expect.objectContaining({ kind: 'fork', icon: 'bot', title: 'Subagent of', detail: 'Parent work', actionLabel: 'Open parent thread', targetId: 'mom' })]);
  });
});

describe('image chip accent (averageImageColor)', () => {
  test('the fill, border and ink follow color-mix in oklab as Chrome computes them for rgb(129 140 168)', async () => {
    const { imageChipInks } = await import('./r4-timeline-chips');
    const inks = imageChipInks('129 140 168')!;
    expect(inks.fill).toBe('#818ca81c');
    expect(inks.hover).toBe('#818ca82b');
    // Chrome: border oklab(0.825 0.0003 -0.0175) = #c1c5d2, text oklab(0.355 0.001 -0.0142) = #393b43 (light).
    const channel = (hex: string, at: number) => parseInt(hex.slice(at, at + 2), 16);
    const near = (hex: string, want: string) => [1, 3, 5].every(at => Math.abs(channel(hex, at) - channel(want, at)) <= 2);
    const light = (value: string) => /light-dark\((#[0-9a-f]{6})/.exec(value)![1]!;
    expect(near(light(inks.border), '#c1c5d2')).toBe(true);
    expect(near(light(inks.ink), '#393b43')).toBe(true);
    expect(imageChipInks('')).toBeNull();
  });
});

describe('Pull Requests diff colors and Markdown settings', () => {
  test('the list and the detail carry Appearance → Diff colors and the code settings', async () => {
    const { pullRequestsPage } = await import('./pages-prs');
    const { pullRequestDetail } = await import('./pages-pr-detail');
    const client = { local: { clientSettings: { diffColorScheme: 'blue-orange', fontSizeCode: 15, wordWrap: false, fontFamilyCode: '' } },
      config: { environment: { capabilities: { pullRequests: true } } }, shell: { projects: [], threads: [] }, ready: false, environmentId: 'e' } as unknown as T3Client;
    const list = await pullRequestsPage(client, null, { open: false, refresh: 0, now: 0, selected: '', query: '', typed: false });
    expect(list.diffScheme).toBe('blue-orange');
    const detail = await pullRequestDetail(client, null, { selected: '', refresh: 0, now: 0 });
    expect([detail.diffScheme, detail.md]).toEqual(['blue-orange', { codeFont: 'ui-monospace', codeSize: 15, wrap: false, chips: [] }]);
  });
});

// Ported providerSkills.test.ts names; the pipeline assertion is clone-authored.
describe('formatProviderSkillDisplayName', () => {
  test('prefers the provider display name', async () => {
    const { formatProviderSkillDisplayName } = await import('./r4-timeline-chips');
    expect(formatProviderSkillDisplayName({ name: 'review-follow-up', displayName: 'Review Follow-up' })).toBe('Review Follow-up');
  });
  test('falls back to a title-cased skill name', async () => {
    const { formatProviderSkillDisplayName } = await import('./r4-timeline-chips');
    expect(formatProviderSkillDisplayName({ name: 'review-follow-up' })).toBe('Review Follow Up');
  });
  test('Markdown skill candidates exclude prices, invalid names and repeated names', async () => {
    const { markdownSkills } = await import('./r4-timeline-chips');
    expect(markdownSkills(['5', '1k', '1e9', '5foo', 'review-follow-up', 'review-follow-up', '_invalid', 'with space', 'unknown!'].map(name => ({ name })))).toEqual([
      { name: '5foo', displayName: '5foo' }, { name: 'review-follow-up', displayName: 'Review Follow Up' },
    ]);
  });
});
