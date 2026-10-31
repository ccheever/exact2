import { expect, test } from 'bun:test';
import { shellCommand } from '../../src/highlight/shell-command';
import { inlineCodeFilePathCandidate } from '../../src/timeline/inline-code-files';
import { fileLinkTarget, messageChips } from '../../src/timeline/message-chips';
import { eventRow, handoffEndpoints } from '../../src/timeline/timeline-events';
import { settleHighlights } from '../support/highlights-fixture';

test('embedded script colors map through shell quotes without changing the command', () => {
  const command = 'node -e "const answer = 42; console.log(answer)"';
  shellCommand(command);
  settleHighlights(); // the command and its script wait for the lexers (highlights.ts)
  const tokens = shellCommand(command);
  expect(tokens.map(token => token.text).join('')).toBe(command);
  expect(tokens.some(token => token.text.includes('42') && token.cls === 'num')).toBe(true);
  expect(tokens.some(token => token.text.includes('const') && token.cls !== 'str')).toBe(true);
  expect(shellCommand('echo \u001b[31m').map(token => token.text).join('')).toBe('echo ␛[31m');
});

test('inline file candidates exclude hostnames and commands, and repo paths in file previews use the workspace root', () => {
  expect(inlineCodeFilePathCandidate('src/parser.ts:12')).toBe('src/parser.ts:12');
  expect(inlineCodeFilePathCandidate('example.com/docs.ts')).toBeNull();
  expect(inlineCodeFilePathCandidate('npm run test')).toBeNull();
  expect(fileLinkTarget(String.raw`C:\repo\src\parser.ts:12`, '/repo')).toBe(String.raw`C:\repo\src\parser.ts:12`);
  const chips = messageChips({ text: '`src/parser.ts:12` and `./src/parser.ts`' }, '/repo/docs', [], '', { cwd: '/repo' });
  expect(chips.map(chip => [chip.href, chip.tip, chip.label])).toEqual([
    ['t3-inline-code:src/parser.ts:12', '/repo/src/parser.ts:12', 'parser.ts · L12'],
    ['t3-inline-code:./src/parser.ts', '/repo/docs/src/parser.ts', 'parser.ts'],
  ]);
});

test('handoff endpoints show provider icons and model names, with preceding-run fallback', () => {
  const runs = [
    { id: 'old', ordinal: 1, providerInstanceId: 'claude', modelSelection: { instanceId: 'claude', model: 'sonnet' } },
    { id: 'new', ordinal: 2, providerInstanceId: 'codex', modelSelection: { instanceId: 'codex', model: 'gpt' } },
  ];
  const item = { type: 'handoff', runId: 'new', fromProviderInstanceIds: ['claude'], toProviderInstanceId: 'codex' };
  expect(handoffEndpoints(item, runs)).toEqual({ from: [{ instanceId: 'claude', model: 'sonnet' }], to: { instanceId: 'codex', model: 'gpt' } });
  const row = eventRow(item, [], {}, { agents: [], runs, providers: [
    { instanceId: 'claude', driver: 'claudeAgent', displayName: 'Claude', models: [{ slug: 'sonnet', name: 'Sonnet' }] },
    { instanceId: 'codex', driver: 'codex', displayName: 'Codex', models: [{ slug: 'gpt', name: 'GPT' }] },
  ] });
  expect(row).toMatchObject({ kind: 'handoff', title: 'Context handoff', detail: 'Sonnet → GPT' });
  expect(row.activities?.map(endpoint => [endpoint.label, endpoint.detail, endpoint.status])).toEqual([
    ['Sonnet', 'claudeAgent', 'from'], ['GPT', 'codex', 'to'],
  ]);
  // A catalog model reads its spoken short name under its driver (#24205); a model the catalog lacks stays as reported.
  const spoken = eventRow(item, [], {}, { agents: [], runs: [runs[0]!, { ...runs[1]!, modelSelection: { instanceId: 'codex', model: 'gpt-9-preview' } }], providers: [
    { instanceId: 'claude', driver: 'claudeAgent', displayName: 'Claude', models: [{ slug: 'sonnet', name: 'Claude Sonnet 4.5' }] },
    { instanceId: 'codex', driver: 'codex', displayName: 'Codex', models: [] },
  ] });
  expect(spoken.activities?.map(endpoint => endpoint.label)).toEqual(['Sonnet 4.5', 'gpt-9-preview']);
});
