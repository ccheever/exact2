import { expect, test } from 'bun:test';
import { dynamicToolTitle, entryDisplayLabel, projectedWorkEntry, summarizeToolGroup } from '../../src/timeline/timeline-worklog';
import { extractToolActivityPresentation } from '../../src/timeline/timeline-work-rows';
import { presentProviderGoal } from '../../src/composer/provider-goal';
import { t3ToolDefinition, t3ToolPresentation, summarizeT3ToolCalls } from '../../src/timeline/timeline-t3tools';
import { applyCommandOutputFrame, commandOutputFrame, EMPTY_COMMAND_OUTPUT } from '../../src/timeline/command-output';
import { terminalOutputPlainText } from '../../src/timeline/terminal-output';

test('computer-use metadata survives absent or generic retained titles', () => {
  for (const name of ['js', 'node_repl.js', 'cua_repl.js']) {
    expect(dynamicToolTitle(name, { title: 'Open the editor' }, 'js')).toBe('Open the editor');
    expect(dynamicToolTitle(name, {}, 'Review parser')).toBe('Review parser');
    const entry = projectedWorkEntry({ item: { id: 'tool', type: 'dynamic_tool', toolName: name, title: 'js', input: { code: 'await sky.click()' }, status: 'running' } });
    expect(entryDisplayLabel(entry, '')).toBe('Computer use');
    expect(extractToolActivityPresentation(entry.item)).toMatchObject({ toolSurface: 'computer', toolSource: { key: 'computer-use' } });
  }
  expect(dynamicToolTitle('node_repl.js', {})).toBe('Run JavaScript');
  expect(extractToolActivityPresentation({ type: 'dynamic_tool', toolName: 'custom', input: { code: 'sky.click()' } })).toEqual({});
  expect(extractToolActivityPresentation({ toolSource: { kind: 'browser', key: 'browser', name: 'Browser' } })).toMatchObject({ toolSurface: 'browser' });
});
test('review guide tools count per-guide refusals without hiding successful starts', () => {
  expect(t3ToolPresentation(t3ToolDefinition('mcp__t3_code__write_pull_request_guides'), 'inProgress')).toMatchObject({ displayName: 'Writing review guides', icon: 'pull-request' });
  const calls = [{ input: {}, output: { guides: [{ outcome: 'started' }, { outcome: 'failed' }] }, outcome: 'completed' }] satisfies Parameters<typeof summarizeT3ToolCalls>[1];
  expect(summarizeT3ToolCalls('write-pr-guides', calls)).toEqual({ label: 'Started 1 review guide, 1 failed', failedCount: 1 });
  expect(summarizeT3ToolCalls('write-pr-guides', [{ ...calls[0]!, output: { guides: [{ outcome: 'failed' }] } }])).toEqual({ label: 'Could not start 1 review guide', failedCount: 1 });
  expect(summarizeT3ToolCalls('read-pr-guide', [{ input: {}, output: {}, outcome: 'completed' }]).label).toBe('Read 1 review guide');
  const entry = projectedWorkEntry({ item: { id: 'guide', type: 'dynamic_tool', toolName: 'write_pull_request_guides', status: 'completed', input: {}, output: calls[0]!.output } });
  expect(summarizeToolGroup([entry])).toEqual({ summary: 'Started 1 review guide, 1 failed', hasFailure: true });
});
test('goal presentation distinguishes idle active goals and resumable outcomes', () => {
  expect(presentProviderGoal({ objective: 'Fix parser', status: 'active', tokensUsed: 12500, tokenBudget: 100000, timeUsedSeconds: 65, checks: 1 }, false))
    .toEqual({ title: 'Goal set', objective: 'Fix parser', usage: '13k / 100k tokens · 1m 5s · 1 check', canResume: false });
  expect(presentProviderGoal({ objective: 'Fix parser', status: 'paused' }, false)).toMatchObject({ title: 'Goal paused', canResume: true });
  expect(presentProviderGoal({ objective: 'Fix parser', status: 'complete' }, false)).toMatchObject({ title: 'Goal complete', canResume: false });
});
test('command stream replacements reset terminal state and appends preserve split escapes', () => {
  expect(commandOutputFrame({ kind: 'append', text: 'a', truncated: false })).toBeNull();
  let view = applyCommandOutputFrame(EMPTY_COMMAND_OUTPUT, { kind: 'replace', text: 'Progress 10%\r', truncated: false, running: true });
  view = applyCommandOutputFrame(view, { kind: 'append', text: 'Progress 50%\u001b[3', truncated: false, running: true });
  view = applyCommandOutputFrame(view, { kind: 'append', text: '2m ready', truncated: false, running: true });
  expect(terminalOutputPlainText(view.output.text)).toBe('Progress 50% ready');
  view = applyCommandOutputFrame(view, { kind: 'replace', text: 'Done\n', truncated: true, running: false });
  expect(view).toMatchObject({ output: { text: 'Done\n', truncated: true, pending: '' }, running: false });
});

test('tool images replace raster payload text while preserving unsupported blocks and asset order', async () => {
  const { turnItemOutputText } = await import('../../src/timeline/timeline-item-detail');
  const { toolOutputImages, turnItemOutputImages } = await import('../../src/timeline/tool-output-images');
  const { toolImages } = await import('../../src/timeline/tool-images');
  const output = { content: [{ type: 'text', text: 'Screenshot captured.' }, { type: 'image', mimeType: 'IMAGE/PNG' },
    { type: 'image', source: { type: 'base64', media_type: 'image/jpeg', data: 'aGVsbG8=' } }] };
  const item = { id: 'capture', threadId: 'source', type: 'dynamic_tool', output };
  expect(turnItemOutputText(item)).toBe('Screenshot captured.');
  expect(toolOutputImages(output).map(image => image.mimeType)).toEqual(['image/png', 'image/jpeg']);
  expect(turnItemOutputImages(item)).toEqual([{ _tag: 'tool-output-image', threadId: 'source', itemId: 'capture', index: 0 }, { _tag: 'tool-output-image', threadId: 'source', itemId: 'capture', index: 1 }]);
  expect(toolImages(item)[0]?.source).toContain('tool-output-image');
  expect(turnItemOutputText({ type: 'dynamic_tool', output: { type: 'image', mimeType: 'image/svg+xml', data: '<svg />' } })).toBe('[image]');
  expect(toolOutputImages(Array.from({ length: 30 }, () => ({ type: 'image', mimeType: 'image/png' })))).toHaveLength(8);
  expect(turnItemOutputImages({ ...item, outputOmitted: true })).toEqual([]);
});


test('REPL arguments preserve labels and values while highlighting only code', async () => {
  const { replArguments } = await import('../../src/timeline/tool-activity');
  const args = replArguments('node_repl.js', { title: 'Inspect', code: 'const answer = 42;', timeout_ms: 1000 });
  expect(args.map(arg => [arg.key, arg.text])).toEqual([['title', 'Inspect'], ['code', 'const answer = 42;'], ['timeout_ms', '1000']]);
  expect(args[1]?.tokens.map(token => token.text).join('')).toBe('const answer = 42;');
  expect(args[1]?.tokens.some(token => token.cls !== '')).toBe(true);
  expect(args[0]?.tokens).toEqual([]);
  expect(replArguments('other', { code: 'const answer = 42;' })).toEqual([]);
});
