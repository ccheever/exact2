// The tracked clone's labels, banner, send block and toast coordinator, derived from
// T3 Code 1e2ecbd975 (MIT, see LICENSE-T3): packages/contracts/src/projectClone.ts,
// components/ProjectCloneToastCoordinator.tsx and the clone banner of ChatView.tsx (the
// reference ships no test for the two components).
import { describe, expect, test } from 'bun:test';
import { cloneToast, projectCloneBanner, projectCloneDisplayName, projectCloneProgressSummary, projectCloneSendBlockReason, syncCloneToasts, type CloneToast, type ToastPort, type TrackedToast } from './project-clones';
import type { Obj } from './domain';

const clone = (over: Obj = {}): Obj => ({ projectId: 'p1', remoteUrl: 'file:///repos/pr-demo-origin.git', destinationPath: '/work/pr-demo', repository: null,
  phase: 'running', stage: 'receiving', percent: 45, detail: '12.3 MiB | 5.0 MiB/s', error: null, startedAt: '2026-10-06T00:00:00.000Z', endedAt: null, sequence: 1, ...over });

function fakePort() {
  const live = new Map<number, CloneToast>(); const log: string[] = []; const closers = new Map<number, () => void>(); let next = 1;
  const port: ToastPort = {
    add: (key, toast, onClose) => { const id = next++; live.set(id, toast); closers.set(id, onClose); log.push(`add ${key} ${toast.kind} ${toast.title}`); return id; },
    update: (id, toast) => { live.set(id, toast); log.push(`update ${id} ${toast.kind} ${toast.title}`); },
    close: id => { live.delete(id); log.push(`close ${id}`); },
    live: id => live.has(id),
  };
  return { port, live, log, closers };
}

describe('projectClone.ts labels', () => {
  test('display name prefers owner/repo, else the destination folder', () => {
    expect(projectCloneDisplayName(clone())).toBe('pr-demo');
    expect(projectCloneDisplayName(clone({ repository: { nameWithOwner: 'acme/widgets' } }))).toBe('acme/widgets');
    expect(projectCloneDisplayName(clone({ destinationPath: 'C:\\work\\repo\\' }))).toBe('repo');
  });
  test('progress summary joins the stage, percent and detail', () => {
    expect(projectCloneProgressSummary(clone())).toBe('Receiving objects · 45% · 12.3 MiB | 5.0 MiB/s');
    expect(projectCloneProgressSummary(clone({ stage: 'connecting', percent: null, detail: null }))).toBe('Connecting');
    expect(['counting', 'resolving', 'checkout'].map(stage => projectCloneProgressSummary(clone({ stage, percent: null, detail: null })))).toEqual(['Counting objects', 'Resolving deltas', 'Checking out files']);
  });
});

describe('ChatView clone banner and send block', () => {
  test('send waits while cloning and after a failure, not once done', () => {
    expect(projectCloneSendBlockReason(null)).toBe('');
    expect(projectCloneSendBlockReason(clone())).toBe('Cloning repository');
    expect(projectCloneSendBlockReason(clone({ phase: 'failed' }))).toBe('Repository not cloned');
    expect(projectCloneSendBlockReason(clone({ phase: 'cancelled' }))).toBe('Repository not cloned');
    expect(projectCloneSendBlockReason(clone({ phase: 'done' }))).toBe('');
  });
  test('banner: running offers Cancel; cancelled and failed offer Remove project and Retry', () => {
    expect(projectCloneBanner(clone())).toMatchObject({ id: 'project-clone:p1', variant: 'info', title: 'Cloning pr-demo', description: 'Receiving objects · 45% · 12.3 MiB | 5.0 MiB/s', action: 'shell:clone-cancel', actionLabel: 'Cancel', action2: '' });
    expect(projectCloneBanner(clone({ phase: 'cancelled' }))).toMatchObject({ variant: 'warning', title: 'Cancelled cloning pr-demo', description: 'Retry to bring in the repository.', actionLabel: 'Remove project', action2Label: 'Retry' });
    expect(projectCloneBanner(clone({ phase: 'failed', error: "fatal: repository 'x' does not exist" }))).toMatchObject({ variant: 'error', title: 'Failed to clone pr-demo', description: "fatal: repository 'x' does not exist" });
    expect(projectCloneBanner(clone({ phase: 'done' }))).toBeNull();
  });
});

describe('ProjectCloneToastCoordinator', () => {
  test('each phase is its toast: loading with Cancel, 8 s success with Open project, error and info with Retry and Remove project', () => {
    expect(cloneToast(clone(), 'env', true)).toMatchObject({ kind: 'loading', title: 'Cloning pr-demo', timeoutMs: 0, hideCopy: true, action: { label: 'Cancel', op: 'shell:clone-cancel', id: 'p1', value: 'env' }, secondary: null });
    expect(cloneToast(clone({ phase: 'done' }), 'env', true)).toMatchObject({ kind: 'success', title: 'Cloned pr-demo', description: '/work/pr-demo', timeoutMs: 8000, action: { label: 'Open project', op: 'new-thread', id: 'p1' } });
    expect(cloneToast(clone({ phase: 'done' }), 'env', false).action).toBeNull();
    expect(cloneToast(clone({ phase: 'failed', error: 'boom' }), 'env', true)).toMatchObject({ kind: 'error', title: 'Failed to clone pr-demo', description: 'boom', timeoutMs: 0, hideCopy: false, action: { label: 'Retry' }, secondary: { label: 'Remove project', op: 'shell:clone-remove' } });
    expect(cloneToast(clone({ phase: 'cancelled' }), 'env', true)).toMatchObject({ kind: 'info', title: 'Cancelled cloning pr-demo', description: '/work/pr-demo', hideCopy: true });
  });
  test('updates in place, skips identical redraws, and a finished clone that leaves keeps its timed toast', () => {
    const { port, log, live } = fakePort(), tracked = new Map<string, TrackedToast>();
    syncCloneToasts(tracked, [clone()], 'env', '', port);
    syncCloneToasts(tracked, [clone()], 'env', '', port);
    syncCloneToasts(tracked, [clone({ percent: 90 })], 'env', '', port);
    syncCloneToasts(tracked, [clone({ phase: 'done' })], 'env', '', port);
    syncCloneToasts(tracked, [], 'env', '', port);
    expect(log).toEqual(['add clone:env:p1 loading Cloning pr-demo', 'update 1 loading Cloning pr-demo', 'update 1 success Cloned pr-demo']);
    expect(live.has(1)).toBe(true);
    expect(tracked.size).toBe(0);
  });
  test('a running or failed clone that leaves the list closes its toast', () => {
    const { port, log } = fakePort(), tracked = new Map<string, TrackedToast>();
    syncCloneToasts(tracked, [clone({ phase: 'failed', error: 'boom' })], 'env', '', port);
    syncCloneToasts(tracked, [], 'env', '', port);
    expect(log).toEqual(['add clone:env:p1 error Failed to clone pr-demo', 'close 1']);
  });
  test("the toast steps aside while the project's draft is open and returns when it is not", () => {
    const { port, log } = fakePort(), tracked = new Map<string, TrackedToast>();
    syncCloneToasts(tracked, [clone()], 'env', '', port);
    syncCloneToasts(tracked, [clone()], 'env', 'p1', port);
    syncCloneToasts(tracked, [clone({ percent: 60 })], 'env', 'p1', port);
    syncCloneToasts(tracked, [clone({ percent: 60 })], 'env', 'other', port);
    expect(log).toEqual(['add clone:env:p1 loading Cloning pr-demo', 'close 1', 'add clone:env:p1 loading Cloning pr-demo']);
  });
  test('× dismisses for good; a toast its own button closed comes back on the next change', () => {
    const { port, log, live, closers } = fakePort(), tracked = new Map<string, TrackedToast>();
    syncCloneToasts(tracked, [clone({ phase: 'failed', error: 'boom' })], 'env', '', port);
    live.delete(1); // Retry pressed: the action button closed the toast (toast.ts)
    syncCloneToasts(tracked, [clone({ percent: 1 })], 'env', '', port);
    expect(log.at(-1)).toBe('add clone:env:p1 loading Cloning pr-demo');
    closers.get(2)!(); live.delete(2); // the × orb
    syncCloneToasts(tracked, [clone({ percent: 50 })], 'env', '', port);
    expect(log.length).toBe(2);
  });
});
