import { afterEach, expect, test } from 'bun:test';
import { mobileClient } from './client';
import { obj, str, type Obj } from './shared/domain';
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
const original = { origin: mobileClient.origin, environmentId: mobileClient.environmentId, connection: mobileClient.connection, config: mobileClient.config, generation: mobileClient.generation, revision: mobileClient.revision };
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

const emailMask = '••••••@••••••';
const signedProvider = (email = 'a@example.test', instanceId = 'test-provider'): Obj => ({ ...provider, instanceId,
  auth: { status: 'authenticated', email, canLogout: true } });
type EmailRow = ReturnType<typeof mobileProviderProjection>;
type EmailPress = { key: string; value: string };
interface EmailFixture {
  requests: Obj[];
  native: Native;
  providers(providers: Obj[]): void;
  prepare(): ReturnType<typeof mobileProviderAccounts>;
  row(instanceId?: string): EmailRow;
  emit(instanceId: string, ...states: Obj[]): Promise<void>;
  press(captured: EmailPress): ReturnType<typeof mobileProviderCommand>;
  holdSession(): { entered: Promise<void>; release(permissions: string[]): void };
}
const emailPress = (row: EmailRow): EmailPress => ({ key: row.key, value: str(obj(row).emailToken) });
async function withEmailAccounts(run: (fixture: EmailFixture) => Promise<void>) {
  const saved = { origin: mobileClient.origin, environmentId: mobileClient.environmentId, connection: mobileClient.connection,
    config: mobileClient.config, generation: mobileClient.generation, revision: mobileClient.revision };
  const origin = 'http://email-provider.test', environmentId = 'email-provider-test', requests: Obj[] = [], unexpected: Obj[] = [];
  const keys = new Map<string, string>();
  let sequence = 0, events: Obj[] = [];
  let heldSession: { entered(): void; reply: Promise<Obj> } | null = null;
  Object.assign(mobileClient, { origin, environmentId, connection: 'connected', generation: 381,
    config: { providers: [signedProvider()], settings: {}, environment: { label: 'Email workstation' } } });
  const raw: Native = { available: true, watch(topic) {
    if (!['t3.status', 't3.fleet'].includes(topic)) { unexpected.push({ watch: topic }); throw new Error(`Unexpected provider watch ${topic}`); }
  }, async later(input) {
    const request = obj(input); requests.push(request);
    let value: unknown;
    if (request.op === 'environments') value = { saved: [{ origin, environmentId }] };
    else if (request.op === 'http' && request.path === '/api/auth/session' && (request.method === undefined || request.method === 'GET')) {
      const held = heldSession; heldSession = null;
      if (held) { held.entered(); value = await held.reply; }
      else value = { authenticated: true, permissions: ['providers:manage'] };
    }
    else if (request.op === 'subscribe' && request.method === 'provider.auth.subscribe') {
      keys.set(str(obj(request.payload).instanceId), str(request.key));
      value = { id: `email-subscription:${mobileClient.generation}:${str(request.key)}` };
    } else if (request.op === 'events') { value = { events }; events = []; }
    else if (request.op === 'unsubscribe' && [...keys.values()].includes(str(request.key))) value = {};
    else { unexpected.push(request); throw new Error(`Unexpected provider operation ${str(request.op)}`); }
    return { ok: true, generation: mobileClient.generation, value };
  } };
  const native = settingsProviderNative(raw), scope = JSON.stringify([environmentId]);
  const fixture: EmailFixture = { requests, native,
    providers(providers) { mobileClient.config = { ...mobileClient.config, providers }; },
    async prepare() {
      const data = await mobileProviderAccounts(scope, true, native, true);
      expect(data.error).toBe(''); expect(data.ready).toBe(true);
      return data;
    },
    row(instanceId = 'test-provider') {
      return mobileProviderAccountsSnapshot(scope, true).sections.flatMap(section => section.accounts).find(row => row.key === keys.get(instanceId))!;
    },
    async emit(instanceId, ...states) {
      const key = keys.get(instanceId)!;
      events = states.map(value => ({ key, seq: ++sequence, generation: mobileClient.generation,
        subscriptionId: `email-subscription:${mobileClient.generation}:${key}`, value }));
      await native.later({ op: 'events', after: 0 });
    },
    press: captured => mobileProviderCommand('reveal', captured.key, captured.value, native),
    holdSession() {
      if (heldSession) throw new Error('A provider session read is already held');
      let entered!: () => void, release!: (permissions: string[]) => void;
      const ready = new Promise<void>(resolve => { entered = resolve; });
      const reply = new Promise<Obj>(resolve => { release = permissions => resolve({ authenticated: true, permissions }); });
      heldSession = { entered, reply };
      return { entered: ready, release };
    },
  };
  try { await run(fixture); }
  finally {
    try { await mobileProviderAccounts('[]', false, native); }
    finally { Object.assign(mobileClient, saved); }
    expect(unexpected).toEqual([]);
  }
}

test('email reveal stays local and survives a continuously displayed raw email', async () => withEmailAccounts(async f => {
  await f.prepare();
  const initial = f.row(), captured = emailPress(initial);
  expect(initial.email).toBe(emailMask); expect(initial.emailAction).toBe('Reveal account email');
  const before = f.requests.length;
  await f.press(captured);
  expect(f.row().email).toBe('a@example.test'); expect(f.row().emailAction).toBe('Hide account email');
  expect(f.requests.length).toBe(before);
  await f.prepare();
  expect(f.row().email).toBe('a@example.test'); expect(emailPress(f.row())).toEqual(captured);
  const refreshed = f.requests.length;
  await f.press(captured);
  expect(f.row().email).toBe(emailMask); expect(f.requests.length).toBe(refreshed);
  expect(captured.value.length).toBeGreaterThan(0);
}));

for (const phase of ['starting', 'waiting', 'verifying']) test(`email remasks after its ${phase} row is hidden and restored`, async () => withEmailAccounts(async f => {
  await f.prepare(); const captured = emailPress(f.row()); await f.press(captured);
  await f.emit('test-provider', { phase, flowId: 'flow' });
  expect(f.row().email).toBe(''); expect(str(obj(f.row()).emailToken)).toBe('');
  await f.emit('test-provider', { phase: 'cancelled', message: 'Cancelled' });
  const restored = f.row();
  expect(restored.email).toBe(emailMask); expect(emailPress(restored).value).not.toBe(captured.value);
}));

test('email changes and A to B to A each create a masked row', async () => withEmailAccounts(async f => {
  await f.prepare(); const a = emailPress(f.row()); await f.press(a);
  f.providers([signedProvider('b@example.test')]); await f.prepare();
  const b = emailPress(f.row()); expect(f.row().email).toBe(emailMask);
  await f.press(b); expect(f.row().email).toBe('b@example.test');
  f.providers([signedProvider()]); await f.prepare();
  expect(f.row().email).toBe(emailMask);
  expect(emailPress(f.row()).value).not.toBe(a.value); expect(b.value).not.toBe(a.value);
}));

for (const replace of ['email', 'hidden'] as const) test(`a captured ${replace} row cannot reveal its successor`, async () => withEmailAccounts(async f => {
  await f.prepare(); const captured = emailPress(f.row());
  if (replace === 'email') { f.providers([signedProvider('b@example.test')]); await f.prepare(); }
  else {
    await f.emit('test-provider', { phase: 'waiting', flowId: 'flow' }); expect(f.row().email).toBe('');
    await f.emit('test-provider', { phase: 'idle' });
  }
  const successor = emailPress(f.row()), before = f.requests.length, revision = mobileClient.revision;
  const result = await f.press(captured);
  expect(f.row().email).toBe(emailMask); expect(result.revision).toBe(revision);
  expect(mobileClient.revision).toBe(revision); expect(f.requests.length).toBe(before);
  expect(successor.value).not.toBe(captured.value);
}));

test('batched auth states preserve reveal when no hidden row is projected', async () => withEmailAccounts(async f => {
  await f.prepare(); const captured = emailPress(f.row()); await f.press(captured);
  await f.emit('test-provider', { phase: 'waiting', flowId: 'flow' }, { phase: 'idle' });
  expect(f.row().email).toBe('a@example.test'); expect(emailPress(f.row())).toEqual(captured);
}));

test('raw email whitespace controls identity and display while blank email has no row', async () => withEmailAccounts(async f => {
  f.providers([signedProvider(' a@example.test ')]); await f.prepare();
  const raw = emailPress(f.row()); await f.press(raw);
  expect(f.row().email).toBe(' a@example.test ');
  f.providers([signedProvider()]); await f.prepare();
  expect(f.row().email).toBe(emailMask); expect(emailPress(f.row()).value).not.toBe(raw.value);
  const trimmed = emailPress(f.row());
  f.providers([signedProvider(' \t\n ')]); await f.prepare();
  expect(f.row().email).toBe(''); expect(str(obj(f.row()).emailToken)).toBe('');
  f.providers([signedProvider()]); await f.prepare();
  expect(f.row().email).toBe(emailMask); expect(emailPress(f.row()).value).not.toBe(trimmed.value);
}));

for (const [name, scope, active] of [['inactive', '["email-provider-test"]', false], ['invalid', '{}', true], ['mismatched', '["other-environment"]', true]] as const) {
  test(`${name} projection ends the displayed email lifetime`, async () => withEmailAccounts(async f => {
    await f.prepare(); const captured = emailPress(f.row()); await f.press(captured);
    const before = f.requests.length;
    expect(mobileProviderAccountsSnapshot(scope, active).ready).toBe(false);
    expect(f.requests.length).toBe(before);
    expect(f.row().email).toBe(emailMask); expect(emailPress(f.row()).value).not.toBe(captured.value);
    const revision = mobileClient.revision; await f.press(captured);
    expect(f.row().email).toBe(emailMask); expect(mobileClient.revision).toBe(revision);
  }));
}

test('a generation replacement rejects an earlier email press', async () => withEmailAccounts(async f => {
  await f.prepare(); const captured = emailPress(f.row());
  mobileClient.generation++; await f.prepare();
  const replacement = emailPress(f.row()), before = f.requests.length, revision = mobileClient.revision;
  await f.press(captured);
  expect(f.row().email).toBe(emailMask); expect(mobileClient.revision).toBe(revision);
  expect(f.requests.length).toBe(before); expect(replacement.value).not.toBe(captured.value);
}));

test('provider email tokens isolate rows and replacement or removal resets only its owner', async () => withEmailAccounts(async f => {
  f.providers([signedProvider(), signedProvider('other@example.test', 'other-provider')]); await f.prepare();
  const a = emailPress(f.row()), other = emailPress(f.row('other-provider'));
  const before = f.requests.length, revision = mobileClient.revision;
  await f.press({ key: other.key, value: a.value });
  expect(f.row('other-provider').email).toBe(emailMask); expect(mobileClient.revision).toBe(revision);
  expect(f.requests.length).toBe(before); expect(a.value).not.toBe(other.value);
  await f.press(a); await f.press(other);
  f.providers([signedProvider('b@example.test'), signedProvider('other@example.test', 'other-provider')]); await f.prepare();
  expect(f.row().email).toBe(emailMask); expect(f.row('other-provider').email).toBe('other@example.test');
  expect(emailPress(f.row('other-provider'))).toEqual(other);
  const b = emailPress(f.row()); await f.press(b);
  f.providers([signedProvider('other@example.test', 'other-provider')]); await f.prepare();
  expect(f.row()).toBeUndefined(); expect(f.row('other-provider').email).toBe('other@example.test');
  f.providers([signedProvider('b@example.test'), signedProvider('other@example.test', 'other-provider')]); await f.prepare();
  expect(f.row().email).toBe(emailMask); expect(emailPress(f.row()).value).not.toBe(b.value);
  expect(f.row('other-provider').email).toBe('other@example.test'); expect(emailPress(f.row('other-provider'))).toEqual(other);
  const returnedRevision = mobileClient.revision; await f.press(b);
  expect(f.row().email).toBe(emailMask); expect(mobileClient.revision).toBe(returnedRevision);
}));

for (const initiallyRevealed of [false, true]) test(`email ${initiallyRevealed ? 'hide' : 'reveal'} remains local while an account command is pending`, async () => withEmailAccounts(async f => {
  await f.prepare(); const stale = emailPress(f.row());
  f.providers([signedProvider('b@example.test')]); await f.prepare();
  await f.emit('test-provider', { phase: 'idle', methods: [{ id: 'one', name: 'One' }, { id: 'two', name: 'Two' }], authorizationUrl: 'https://example.test' });
  const current = emailPress(f.row());
  await mobileProviderCommand('choose', current.key, '', f.native);
  expect(f.row().actions.some(action => action.op === 'choose-cancel')).toBe(true);
  if (initiallyRevealed) await f.press(current);
  const held = f.holdSession(), pending = mobileProviderCommand('logout-confirmed', current.key, '', f.native);
  try {
    await held.entered;
    expect(f.row().pending).toBe(true);
    const before = f.requests.length, revision = mobileClient.revision;
    for (const captured of [stale, { key: current.key, value: '' }]) {
      const refused = await f.press(captured);
      expect(refused.revision).toBe(revision); expect(mobileClient.revision).toBe(revision);
    }
    for (const op of ['choose-cancel', 'choose', 'logout-confirmed', 'docs', 'unknown']) {
      const refused = await mobileProviderCommand(op, current.key, '', f.native);
      expect(refused.revision).toBe(revision); expect(mobileClient.revision).toBe(revision);
    }
    expect(mobileProviderField(current.key, 'callback', 'blocked while pending').revision).toBe(revision);
    expect(f.row().fields.find(field => field.name === 'callback')!.value).toBe('');
    expect(f.row().actions.some(action => action.op === 'choose-cancel')).toBe(true);
    expect(f.requests.length).toBe(before);
    await f.press(current);
    expect(f.row().email).toBe(initiallyRevealed ? emailMask : 'b@example.test');
    expect(mobileClient.revision).toBe(revision + 1);
    await f.press(current);
    expect(f.row().email).toBe(initiallyRevealed ? 'b@example.test' : emailMask);
    expect(mobileClient.revision).toBe(revision + 2);
    expect(f.row().pending).toBe(true); expect(f.requests.length).toBe(before);
  } finally { held.release([]); await pending; }
  expect(f.row().pending).toBe(false);
  expect(f.row().error).toBe('This connection does not allow provider account changes.');
  expect(f.requests.some(request => request.op === 'request')).toBe(false);
}));
