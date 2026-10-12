import { test, expect } from 'bun:test';
import { isSshAuthFailure, buildSshChildEnvironment, ASKPASS_POSIX_SCRIPT, promptView, sshPromptSource, sshPromptAnswer, promptErrorMessage, formatRemainingSeconds, EXPIRED_MESSAGE, KEYS_HINT } from './ssh-auth';
import { obj, type Obj } from './domain';
import type { Native } from './protocol';

// packages/ssh/src/auth.test.ts
test('detects ssh auth failures from common permission denied messages', () => {
  expect(isSshAuthFailure(new Error('julius@100.65.180.100: Permission denied (publickey,password,keyboard-interactive).'))).toBe(true);
  expect(isSshAuthFailure(new Error('Permission denied (publickey).'))).toBe(true);
  expect(isSshAuthFailure(new Error('Connection timed out'))).toBe(false);
  expect(isSshAuthFailure(new Error('mkdir: Permission denied'))).toBe(false);
});

test('creates askpass env for cached password prompts', () => {
  const env = buildSshChildEnvironment({ authSecret: 'super-secret', interactiveAuth: true, askpassDirectory: '/tmp/t3-ssh-askpass-test', baseEnv: {} });
  expect(env.SSH_ASKPASS).toBe('/tmp/t3-ssh-askpass-test/ssh-askpass.sh');
  expect(env.SSH_ASKPASS_REQUIRE).toBe('force');
  expect(env.T3_SSH_AUTH_SECRET).toBe('super-secret');
  expect(env.DISPLAY).toBe('t3code');
  expect(ASKPASS_POSIX_SCRIPT).toContain('printf "%s\\n" "$T3_SSH_AUTH_SECRET"');
  expect(buildSshChildEnvironment({ interactiveAuth: false, askpassDirectory: '/x', baseEnv: { A: '1' } })).toEqual({ A: '1' });
  expect(buildSshChildEnvironment({ interactiveAuth: true, askpassDirectory: '/x', baseEnv: { DISPLAY: ':0' } }).DISPLAY).toBe(':0');
});

test('the dialog shows the target, the prompt and a m:ss countdown that turns into Expired', () => {
  const request = { requestId: 'r1', destination: 'devbox', username: 'me', prompt: 'Enter the SSH password for me@devbox.', expiresAt: 1_000_000 + 180_000, expired: false, error: '' };
  const open = promptView(request, 1_000_000);
  expect(open).toMatchObject({ open: true, target: 'me@devbox', prompt: 'Enter the SSH password for me@devbox.', remaining: '3:00', expired: false, hint: KEYS_HINT, error: '' });
  expect(promptView(request, 1_000_000 + 179_001).remaining).toBe('0:01');
  expect(promptView(request, 1_000_000 + 180_000)).toMatchObject({ remaining: 'Expired', expired: true, error: EXPIRED_MESSAGE, hint: EXPIRED_MESSAGE });
  // The module's own expiry wins over the window's clock.
  expect(promptView({ ...request, expired: true }, 1_000_000).remaining).toBe('Expired');
  expect(promptView({ ...request, username: null }, 0).target).toBe('devbox');
  expect(promptView({ ...request, timeoutMs: 180_000 }, 0).remaining).toBe('3:00');
  expect(promptView({ ...request, error: 'Invalid SSH password prompt id.' }, 1_000_000).hint).toBe('Invalid SSH password prompt id.');
  expect(promptView(null, 0).open).toBe(false);
  expect(formatRemainingSeconds(65)).toBe('1:05');
  expect(promptErrorMessage(new Error('SSH password prompt expired. Try connecting again.'))).toBe(EXPIRED_MESSAGE);
  expect(promptErrorMessage(new Error('is no longer pending'))).toBe(EXPIRED_MESSAGE);
});

test('the source watches the queue and the answer never carries a password', async () => {
  const calls: Obj[] = [], watched: string[] = [];
  const native: Native = { available: true, watch: topic => { watched.push(topic); }, later: async input => {
    const request = obj(input); calls.push(request);
    if (request.op === 'sshPromptState') return { ok: true, generation: 0, value: { request: { requestId: 'r1', destination: 'devbox', username: null, prompt: 'Enter the SSH password for devbox.', expiresAt: 61_000, expired: false, error: '' }, queued: 1 } };
    if (request.requestId === 'gone') return { ok: false, generation: 0, error: { kind: 'Prompt', message: 'SSH password prompt expired. Try connecting again.', uncertain: false } };
    return { ok: true, generation: 0, value: { resolved: true } };
  } };
  expect(await sshPromptSource(native, 1_000)).toMatchObject({ open: true, requestId: 'r1', remaining: '1:00', queued: 1 });
  expect(watched).toEqual(['t3.ssh-prompt']);
  expect(await sshPromptAnswer(native, 'r1', 'submit')).toEqual({ ok: true, error: '' });
  expect(await sshPromptAnswer(native, 'gone', 'submit')).toEqual({ ok: false, error: EXPIRED_MESSAGE });
  expect(await sshPromptAnswer(native, 'r1', 'anything')).toEqual({ ok: true, error: '' });
  expect(calls.filter(call => call.op === 'sshPromptResolve').map(call => call.answer)).toEqual(['submit', 'submit', 'cancel']);
  expect(calls.every(call => Object.keys(call).every(key => ['op', 'requestId', 'answer'].includes(key)))).toBe(true);
});
