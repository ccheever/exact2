// composer-fidelity A15 (unit only: the fixture provider cannot produce subagents).
// Ports of T3 Code 1e2ecbd975 (MIT; see LICENSE-T3) packages/client-runtime/src/state/
// subagentDisplay.test.ts (resolveSubagentMetadata, subagentDetailPreview) plus the
// SubagentTooltipContent account rule (daa1d0ed94).
import { describe, expect, test } from 'bun:test';
import { subagentCard, subagentMetadata, subagentPreview } from './subagent-card';

const claude = (instanceId: string, extra = {}) => ({ instanceId, driver: 'claudeAgent', displayName: instanceId === 'claudeAgent' ? 'Claude' : 'Work account', ...extra,
  models: [{ slug: 'claude-haiku-4-5', name: 'Claude Haiku 4.5', shortName: 'Haiku 4.5', aliases: ['claude-haiku-4-5-20251001'] }] });

describe('subagentMetadata (resolveSubagentMetadata)', () => {
  test('resolves provider aliases to catalog names, including custom models', () => {
    expect(subagentMetadata({ model: 'claude-haiku-4-5-20251001', provider: claude('claudeAgent') }).modelLabel).toBe('Haiku 4.5');
    expect(subagentMetadata({ model: 'my-model', provider: { driver: 'acpRegistry', models: [{ slug: 'my-model', name: 'Cloud+ / My custom model', subProvider: 'Cloud+', isCustom: true }] } }).modelLabel).toBe('My custom model');
  });
  test('keeps unknown model identities and does not invent an unreported model', () => {
    expect(subagentMetadata({ model: ' custom/model ' }).modelLabel).toBe('custom/model');
    expect(subagentMetadata({ model: null, provider: { driver: 'codex', models: [] } }).modelLabel).toBe('Not reported');
    expect(subagentMetadata({ model: ' ' }).modelLabel).toBe('Not reported');
  });
  const parentThread = { projectId: 'parent', worktreePath: null }, parentProject = { workspaceRoot: '/repo' };
  test('shows another project and its branch when the child has a different workspace', () => {
    expect(subagentMetadata({ model: null, parentThread, parentProject, childThread: { branch: 'fix/agents', worktreePath: '/worktrees/agents' },
      childProject: { id: 'child', title: 'Other project', workspaceRoot: '/other' } }).workspace).toEqual([{ label: 'Project', value: 'Other project' }, { label: 'Branch', value: 'fix/agents' }]);
  });
  test('labels a detached worktree or another project workspace without a branch', () => {
    expect(subagentMetadata({ model: null, parentThread, parentProject, childThread: { branch: null, worktreePath: '/worktrees/agents' } }).workspace).toEqual([{ label: 'Worktree', value: 'agents' }]);
    expect(subagentMetadata({ model: null, parentThread, parentProject, childProject: { id: 'parent', title: 'Same project', workspaceRoot: '/other' } }).workspace).toEqual([{ label: 'Workspace', value: 'other' }]);
  });
  test('hides redundant workspace metadata and tolerates unavailable child shells', () => {
    expect(subagentMetadata({ model: null, parentThread, parentProject, childThread: { branch: 'main', worktreePath: '/repo' }, childProject: { id: 'parent', title: 'Same project', workspaceRoot: '/repo' } }).workspace).toEqual([]);
    expect(subagentMetadata({ model: null, parentThread, parentProject }).workspace).toEqual([]);
  });
});

describe('subagentPreview (subagentDetailPreview)', () => {
  test('prefers progress for live work and results for settled work', () => {
    const details = { progress: 'Reading files', result: 'Found two\n  problems' };
    expect(subagentPreview({ ...details, status: 'running' })).toBe('Reading files');
    expect(subagentPreview({ ...details, status: 'completed' })).toBe('Found two problems');
    expect(subagentPreview({ status: 'failed', progress: 'Last progress', result: ' ' })).toBe('Last progress');
    expect(subagentPreview({ status: 'pending' })).toBe('');
  });
});

describe('subagentCard (SubagentTooltipContent)', () => {
  test('names the account only when several accounts share the provider', () => {
    const one = claude('claudeAgent');
    expect(subagentCard({ title: 'Explore', model: 'claude-haiku-4-5', provider: one, providers: [one], status: 'running' }).modelLine).toBe('Haiku 4.5');
    const work = claude('claude_work');
    expect(subagentCard({ title: 'Explore', model: 'claude-haiku-4-5', provider: work, providers: [one, work], status: 'running' }).modelLine).toBe('Haiku 4.5 · Work account');
    const tinted = claude('claude_work', { accentColor: '#3b82f6' });
    expect(subagentCard({ title: 'Explore', model: 'claude-haiku-4-5', provider: tinted, providers: [tinted], status: 'running' })).toMatchObject({ modelLine: 'Haiku 4.5 · Work account', accent: '#3b82f6' });
  });
  test('status, tone and rows', () => {
    expect(subagentCard({ title: 'T', model: null, status: 'in_progress', elapsed: '12s', progress: 'Reading' })).toMatchObject({ status: 'in progress', tone: 'info', statusIcon: 'circle-dashed', elapsed: '12s', preview: 'Reading', modelLine: 'Not reported' });
    expect(subagentCard({ title: 'T', model: null, status: 'failed' })).toMatchObject({ tone: 'error', statusIcon: 'circle-x' });
    expect(subagentCard({ title: 'T', model: null, status: 'completed', parentThread: { projectId: 'p', worktreePath: null }, parentProject: { workspaceRoot: '/repo' },
      childThread: { branch: 'fix/x', worktreePath: '/wt/x' } })).toMatchObject({ tone: 'success', statusIcon: 'check', rows: [{ label: 'Branch', value: 'fix/x', icon: 'git-branch' }] });
  });
});
