import { afterEach, expect, test } from 'bun:test';
import { mobileClient } from './client';
import { obj, type Obj } from './shared/domain';
import type { Native } from './shared/protocol';
import { mobileProviderAccounts, mobileProviderAccountsSnapshot, mobileProviderCommand, mobileProviderField, mobileProviderProjection, settingsProviderEvents, settingsProviderNative } from './settings-provider';
const provider: Obj = { instanceId: 'test-provider', driver: 'acpRegistry', displayName: 'Test provider', enabled: true, installed: true, setup: { canAuthenticate: true }, auth: { status: 'unknown' } };
const source = { key: 'test', label: 'Workstation' };
test('discovery and external setup follow actual method availability', () => {
  const loading = mobileProviderProjection(source, provider, null, { key: 'test' });
  expect(loading.message).toContain('Discovering'); expect(loading.actions[0]!.disabled).toBe(true);
  const external = mobileProviderProjection(source, provider, { phase: 'idle', methods: [] }, { key: 'test' });
  expect(external.message).toContain('No in-app sign-in'); expect(external.actions).toHaveLength(0);
});
test('browser consent, device code, credential and terminal prompts retain mobile differences', () => {
  const browser = mobileProviderProjection(source, provider, { phase: 'waiting', flowId: 'flow', interaction: { id: 'step', type: 'browser', url: 'https://example.test', acceptsCallback: true } }, { key: 'test', externalLinksAvailable: false });
  expect(browser.fields[0]!.name).toBe('callback'); expect(browser.actions.find(item => item.op === 'open')!.disabled).toBe(true);
  expect(browser.actions.find(item => item.op === 'complete')!.disabled).toBe(true);
  const terminal = mobileProviderProjection(source, provider, { phase: 'waiting', flowId: 'flow', interaction: { id: 'step', type: 'terminal', output: '\x1b[31mPrompt\x1b[0m' } }, { key: 'test' });
  expect(terminal.output).toBe('Prompt'); expect(terminal.fields[0]!.secret).toBe(true); expect(terminal.fields[0]!.limit).toBe(4095);
  const credentials = mobileProviderProjection(source, provider, { phase: 'waiting', flowId: 'flow', interaction: { id: 'step', type: 'credentials', fields: [{ name: 'token', label: 'Token', secret: true }] } }, { key: 'test' });
  expect(credentials.fields[0]!.secret).toBe(true); expect(credentials.actions[0]!.op).toBe('respond-credentials');
  const code = mobileProviderProjection(source, provider, { phase: 'waiting', interaction: { type: 'deviceCode', userCode: 'SAMPLE', url: 'https://example.test' } }, { key: 'test' });
  expect(code.deviceCode).toBe('Enter code SAMPLE on the sign-in page.');
});
test('email is masked until revealed and ordinary write errors allow retry', () => {
  const signed = { ...provider, auth: { status: 'authenticated', email: 'example@example.test', canLogout: true } };
  expect(mobileProviderProjection(source, signed, { phase: 'idle' }, { key: 'test' }).email).toBe('••••••@••••••');
  expect(mobileProviderProjection(source, signed, { phase: 'idle' }, { key: 'test', revealed: true }).email).toBe('example@example.test');
  expect(mobileProviderProjection(source, signed, { phase: 'idle' }, { key: 'test', error: 'Write failed' }).actions[0]!.disabled).toBe(false);
  expect(mobileProviderProjection(source, signed, { phase: 'idle' }, { key: 'test', queryError: 'Read failed' }).actions[0]!.disabled).toBe(true);
});
const original = { origin: mobileClient.origin, environmentId: mobileClient.environmentId, connection: mobileClient.connection, config: mobileClient.config, generation: mobileClient.generation };
afterEach(() => Object.assign(mobileClient, original));
test('observer forwards one shared read, ignores stale stream events, resets prompt drafts and releases by key', async () => {
  const origin = 'http://provider.test', requests: Obj[] = [];
  Object.assign(mobileClient, { origin, environmentId: 'provider-test', connection: 'connected', generation: 81,
    config: { providers: [provider], settings: {}, environment: { label: 'Workstation' } } });
  let events: Obj[] = [];
  const raw: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); requests.push(request);
    const value = request.op === 'environments' ? { saved: [{ origin, environmentId: 'provider-test' }] }
      : request.op === 'http' ? { authenticated: true, permissions: ['providers:manage'] }
      : request.op === 'subscribe' ? { id: 'subscription-test' }
      : request.op === 'events' ? { events } : {};
    return { ok: true, generation: 81, value };
  } };
  const native = settingsProviderNative(raw);
  const initial = await mobileProviderAccounts('["provider-test"]', true, native, true);
  const key = initial.sections[0]!.accounts[0]!.key;
  const event = (seq: number, prompt: string): Obj => ({ key, seq, generation: 81, subscriptionId: 'subscription-test', value: { phase: 'waiting', flowId: 'flow', interaction: { type: 'terminal', id: prompt, output: 'Prompt' } } });
  events = [event(1, 'one')]; await native.later({ op: 'events', after: 0 });
  expect(requests.filter(request => request.op === 'events')).toHaveLength(1);
  const beforeEdit = requests.length;
  mobileProviderField(key, 'input', 'sample response');
  let data = mobileProviderAccountsSnapshot('["provider-test"]', true);
  expect(requests.length).toBe(beforeEdit);
  await mobileProviderCommand('reveal', key, '', native);
  mobileProviderAccountsSnapshot('["provider-test"]', true);
  expect(requests.length).toBe(beforeEdit);
  expect(mobileProviderAccountsSnapshot('["another-environment"]', true).ready).toBe(false);
  expect(data.sections[0]!.accounts[0]!.fields[0]!.value).toBe('sample response');
  settingsProviderEvents([{ ...event(2, 'stale'), generation: 80 }]);
  expect((await mobileProviderAccounts('["provider-test"]', true, native, true)).sections[0]!.accounts[0]!.fields[0]!.value).toBe('sample response');
  settingsProviderEvents([event(3, 'two')]);
  data = await mobileProviderAccounts('["provider-test"]', true, native, true);
  expect(data.sections[0]!.accounts[0]!.fields[0]!.value).toBe('');
  await mobileProviderCommand('respond-terminal', key, '', native);
  const sent = requests.find(request => request.method === 'provider.auth.respond')!;
  expect(obj(sent.payload)).toMatchObject({ instanceId: 'test-provider', flowId: 'flow', interactionId: 'two', response: { type: 'terminal', data: '\r' } });
  await mobileProviderAccounts('[]', false, native);
  expect(requests.some(request => request.op === 'unsubscribe' && request.key === key)).toBe(true);
  expect(requests.some(request => request.op === 'ack')).toBe(false);
  let touched = false;
  const quiet: Native = { available: true, watch() { touched = true; }, async later() { touched = true; throw new Error('Inactive provider route performed native I/O'); } };
  await mobileProviderAccounts('[]', false, quiet);
  expect(touched).toBe(false);
});
test('closing before catalog preparation finishes prevents a late subscription', async () => {
  const origin = 'http://late-provider.test', requests: Obj[] = [];
  Object.assign(mobileClient, { origin, environmentId: 'late-provider', connection: 'connected', generation: 91,
    config: { providers: [provider], settings: {}, environment: { label: 'Late provider' } } });
  let complete!: (value: unknown) => void;
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); requests.push(request);
    if (request.op === 'environments') return new Promise(resolve => { complete = resolve; });
    return { ok: true, generation: 91, value: {} };
  } };
  const opening = mobileProviderAccounts('["late-provider"]', true, native);
  await mobileProviderAccounts('[]', false, native);
  complete({ ok: true, generation: 91, value: { saved: [{ origin, environmentId: 'late-provider' }] } });
  expect((await opening).ready).toBe(false);
  expect(requests.some(request => request.op === 'subscribe')).toBe(false);
});
