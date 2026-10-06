import { expect, test } from 'bun:test';
import { terminalTranscriptUpdate, providerAuthOp, providerAuthView, watchProviderAuth, providerAuthEvent } from './provider-auth-terminal';
import type { ProviderHost } from './providers';
import type { Native } from './protocol';
import type { Obj } from './domain';
test('transcript appends the suffix when offset advances within retained output', () => { expect(terminalTranscriptUpdate(3, 'abcdef', 6)).toEqual({ written: 6, reset: false, data: 'def' }); });
test('transcript writes nothing when offset is unchanged', () => { expect(terminalTranscriptUpdate(6, 'abcdef', 6)).toEqual({ written: 6, reset: false, data: '' }); });
test('transcript resets after gaps or offset rewind', () => {
  expect(terminalTranscriptUpdate(3, 'def', 9)).toEqual({ written: 9, reset: true, data: 'def' });
  expect(terminalTranscriptUpdate(9, 'a', 1)).toEqual({ written: 1, reset: true, data: 'a' });
});
test('account subscribes and starts the selected terminal method; native view owns input', async () => {
  const calls: { method: string; payload: Obj }[] = [];
  const host: ProviderHost & { generation: number } = { generation: 1, config: { providers: [{ instanceId: 'fixture', setup: { canAuthenticate: true } }] }, ready: true, writable: true, local: { favoriteModels: [] }, rpc: async (_, method, payload) => { calls.push({ method, payload }); return { flowId: 'flow', phase: 'waiting', interaction: { type: 'terminal', id: 'input', output: 'prompt', outputOffset: 6 } }; } };
  const native: Native = { available: true, watch: () => {}, later: async () => ({ ok: true, generation: 1, value: { id: 'sub-1' } }) };
  await watchProviderAuth(host, native, 'fixture');
  providerAuthEvent(host, { generation: 1, key: 'provider-auth:fixture', subscriptionId: 'sub-1', value: { phase: 'idle', methods: [{ id: 'login', name: 'CLI login', type: 'terminal' }] } });
  expect(providerAuthView(host, 'fixture', {}).methodId).toBe('login');
  await providerAuthOp(host, native, 'provider-auth-start', 'fixture', JSON.stringify({ value: 'login' }));
  expect(calls).toEqual([{ method: 'provider.auth.start', payload: { instanceId: 'fixture', methodId: 'login' } }]);
});

test('native auth failures are flow-scoped and cancel removes the active terminal', async () => {
  const calls: string[] = [];
  const host: ProviderHost = { config: {}, ready: true, writable: true, local: { favoriteModels: [] }, rpc: async (_, method) => {
    calls.push(method); return method === 'provider.auth.start' ? { flowId: 'flow', phase: 'waiting', interaction: { type: 'terminal', id: 'terminal' } } : { phase: 'cancelled' };
  } };
  const native: Native = { available: true, watch() {}, async later() { throw Error('events must not send native requests'); } };
  await providerAuthOp(host, native, 'provider-auth-start', 'fixture', '{}');
  const event = (authFlow: string, type: string) => providerAuthOp(host, native, 'provider-auth-event', 'fixture', JSON.stringify({ value: JSON.stringify({ authFlow, type }) }));
  await event('old:terminal', 'auth-error'); expect(providerAuthView(host, 'fixture', {}).error).toBe('');
  await event('flow:terminal', 'auth-error'); expect(providerAuthView(host, 'fixture', {}).error).toBe('The provider sign-in terminal is no longer available.');
  await event('flow:terminal', 'auth-recovered'); expect(providerAuthView(host, 'fixture', {}).error).toBe('');
  await providerAuthOp(host, native, 'provider-auth-cancel', 'fixture', '{}');
  expect(providerAuthView(host, 'fixture', {}).terminal).toBe(false);
  expect(calls).toEqual(['provider.auth.start', 'provider.auth.cancel']);
});
