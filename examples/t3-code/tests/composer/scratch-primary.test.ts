// A thread without a project starts on the primary environment (T3 Code v0.1.5 useScratchProject.test.ts, #26301).
import { describe, expect, test } from 'bun:test';
import { scratchEnvironmentId, type ScratchEnvironment } from '../../src/composer/scratch-project';

const machine = (environmentId: string, scratchWorkspaceRoot: string | null): ScratchEnvironment => ({ environmentId, connection: 'connected', config: { scratchWorkspaceRoot } });
const machines = (localRoot: string | null, remoteRoot: string | null = '/remote/scratch') =>
  [machine('local', localRoot), machine('remote', remoteRoot), machine('other', '/other/scratch')];

describe('scratchEnvironmentId', () => {
  test('prefers the primary environment over the one on screen', () => {
    expect(scratchEnvironmentId(machines('/local/scratch'), 'local', 'remote')).toBe('local');
    expect(scratchEnvironmentId(machines('/local/scratch'), 'local', null)).toBe('local');
  });
  test('falls back when the primary environment offers no scratch root', () => {
    expect(scratchEnvironmentId(machines(null), 'local', 'remote')).toBe('remote');
    expect(scratchEnvironmentId(machines(null), null, 'remote')).toBe('remote');
    expect(scratchEnvironmentId(machines(null, null), null, 'remote')).toBe('other');
    expect(scratchEnvironmentId(machines(null), 'local', null)).toBeNull();
  });
  test('a machine that is not connected offers no folder', () => {
    expect(scratchEnvironmentId([{ ...machine('local', '/local/scratch'), connection: 'connecting' }, machine('remote', '/r')], 'local', 'remote')).toBe('remote');
  });
});
