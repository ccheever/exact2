// 20261005-embedded-server-runtime: the TypeScript side of the embedded server's status.
import { describe, expect, test } from 'bun:test';
import { parseLocalBackendStatus, readLocalBackend, unknownLocalBackend } from './local-backend';

describe('local backend status', () => {
  test('reads a ready server', () => {
    const status = parseLocalBackendStatus({ state: 'ready', port: 16437, httpBaseUrl: 'http://127.0.0.1:16437', wsBaseUrl: 'ws://127.0.0.1:16437',
      bearerReady: true, restartAttempt: 0, nextRestartMs: null, lastExit: null, version: '0.0.46-nightly.20261005.2667', pid: 4242 });
    expect(status).toMatchObject({ state: 'ready', port: 16437, bearerReady: true, restartAttempt: 0, nextRestartMs: null, lastExit: '', pid: 4242, install: null });
  });

  test('keeps the restart ladder and the install progress', () => {
    expect(parseLocalBackendStatus({ state: 'restarting', restartAttempt: 3, nextRestartMs: 4000, lastExit: 'code=1' }))
      .toMatchObject({ state: 'restarting', restartAttempt: 3, nextRestartMs: 4000, lastExit: 'code=1' });
    expect(parseLocalBackendStatus({ state: 'installing', install: { phase: 'extract', fraction: 0.42 } }).install)
      .toEqual({ phase: 'extract', fraction: 0.42, reason: '' });
  });

  test('a development build without its variables is refused, with the reason', () => {
    const status = parseLocalBackendStatus({ state: 'refused', refused: 'Development build: set T3_LOCAL_HOME and T3_LOCAL_PORT to start the local server.' });
    expect(status.state).toBe('refused');
    expect(status.refused).toContain('T3_LOCAL_HOME');
  });

  test('an unknown state reads as stopped', () => {
    expect(parseLocalBackendStatus({ state: 'exploded' }).state).toBe('stopped');
    expect(parseLocalBackendStatus(null)).toEqual(unknownLocalBackend());
  });

  test('reading watches t3.local and asks for localBackendStatus', async () => {
    const watched: string[] = [], asked: unknown[] = [];
    const native = { available: true, watch: (topic: string) => { watched.push(topic); },
      later: async (request: unknown) => { asked.push(request); return { ok: true, generation: 0, value: { state: 'starting', port: 16437 } }; } };
    const status = await readLocalBackend(native);
    expect(watched).toEqual(['t3.local']);
    expect(asked).toEqual([{ op: 'localBackendStatus' }]);
    expect(status).toMatchObject({ state: 'starting', port: 16437 });
  });

  test('a failed read is the empty status', async () => {
    const native = { available: true, watch: () => {}, later: async () => ({ ok: false, generation: 0, error: { kind: 'x', message: 'no', uncertain: false } }) };
    expect(await readLocalBackend(native)).toEqual(unknownLocalBackend());
  });
});
