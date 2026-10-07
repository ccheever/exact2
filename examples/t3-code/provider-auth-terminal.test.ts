import { expect, test } from 'bun:test';
import { terminalTranscriptUpdate, terminalEventError } from './provider-auth-terminal';
import { providerSetupEvent, providerSetupOp, setupOf, watchProviderSetup } from './provider-setup';
import { providerAccount } from './provider-auth';
import type { ProviderHost } from './providers';
import type { Native } from './protocol';
import type { Obj } from './domain';
test('transcript appends the suffix when offset advances within retained output', () => { expect(terminalTranscriptUpdate(3, 'abcdef', 6)).toEqual({ written: 6, reset: false, data: 'def' }); });
test('transcript writes nothing when offset is unchanged', () => { expect(terminalTranscriptUpdate(6, 'abcdef', 6)).toEqual({ written: 6, reset: false, data: '' }); });
test('transcript resets after gaps or offset rewind', () => {
  expect(terminalTranscriptUpdate(3, 'def', 9)).toEqual({ written: 9, reset: true, data: 'def' });
  expect(terminalTranscriptUpdate(9, 'a', 1)).toEqual({ written: 1, reset: true, data: 'a' });
});
test('terminal view events name the row error; a recovered terminal clears it', () => {
  expect(terminalEventError('auth-error')).toBe('The provider sign-in terminal is no longer available.');
  expect(terminalEventError('link-error')).toBe('Could not open the provider link.');
  expect(terminalEventError('error')).toBe('Could not load the sign-in terminal. Cancel and retry sign-in.');
  expect(terminalEventError('auth-recovered')).toBe('');
});
test('account subscribes and starts the selected terminal method; native view owns input', async () => {
  const calls: { method: string; payload: Obj }[] = [];
  const live = { instanceId: 'fixture', enabled: true, installed: true, setup: { canAuthenticate: true } };
  const host: ProviderHost & { generation: number } = { generation: 1, config: { providers: [live] }, ready: true, writable: true, local: { favoriteModels: [] }, rpc: async (_, method, payload) => { calls.push({ method, payload }); return {}; } };
  const native: Native = { available: true, watch: () => {}, later: async () => ({ ok: true, generation: 1, value: { id: 'sub-1' } }) };
  await watchProviderSetup(host, native, 'page', { auth: ['fixture'], install: [] });
  providerSetupEvent(host, { generation: 1, key: 'provider-auth:fixture', subscriptionId: 'sub-1', value: { phase: 'idle', flowId: null, methods: [{ id: 'login', name: 'CLI login', type: 'terminal' }] } });
  expect(providerAccount('fixture', live, setupOf(host, 'fixture'), 'Fixture Mac')).toMatchObject({ pickMethod: false, showStart: true, startLabel: 'Sign in' });
  await providerSetupOp(host, native, 'setup:auth-start', 'fixture', 'idle:', 'login');
  expect(calls).toEqual([{ method: 'provider.auth.start', payload: { instanceId: 'fixture', methodId: 'login' } }]);
});
