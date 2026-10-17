// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/provider-auth-terminal.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// ProviderAuthenticationSection and ProviderAuthTerminal, T3 Code 1e2ecbd975 (MIT).
import { arr, obj, str, num, type Obj } from './domain';
import { bridgeReply, type Native } from './protocol';
import type { ProviderHost } from './providers';
import { letGo } from './let-go';
export function terminalTranscriptUpdate(written: number, output: string, outputOffset = output.length) {
  const delta = outputOffset - written;
  return { written: outputOffset, reset: delta !== 0 && !(delta > 0 && delta <= output.length), data: delta === 0 ? '' : delta > 0 && delta <= output.length ? output.slice(-delta) : output };
}
type Account = { state: Obj; error: string; subscription: string; generation: number; identity: string };
const accounts = new WeakMap<object, Map<string, Account>>();
function account(host: object, id: string): Account {
  let all = accounts.get(host); if (!all) accounts.set(host, all = new Map());
  let value = all.get(id); if (!value) all.set(id, value = { state: {}, error: '', subscription: '', generation: -1, identity: '' });
  return value;
}
export function providerAuthEvent(host: object, entry: Obj): boolean {
  const key = str(entry.key); if (!key.startsWith('provider-auth:')) return false;
  const value = account(host, key.slice(14));
  if (Number(entry.generation) !== value.generation || (value.subscription && str(entry.subscriptionId) !== value.subscription)) return true;
  const state = obj(entry.value);
  if (state._streamEnded || state._retryDue) { value.subscription = ''; return true; }
  if (state._transportError) { value.error = str(obj(state._transportError).message, 'The provider sign-in terminal is no longer available.'); return true; }
  adopt(value, state); return true;
}
function adopt(value: Account, state: Obj) {
  value.identity = `${str(state.flowId)}:${str(obj(state.interaction).id)}`;
  value.state = state;
}
export async function watchProviderAuth(host: ProviderHost & { generation: number }, native: Native, id: string): Promise<void> {
  if (!id || !host.ready) return;
  const provider = arr(host.config.providers).find(item => item.instanceId === id);
  if (obj(provider?.setup).canAuthenticate !== true) return;
  const value = account(host, id);
  if (value.generation !== host.generation) { value.subscription = ''; value.state = {}; value.generation = host.generation; }
  if (value.subscription) return;
  const generation = host.generation;
  const reply = await bridgeReply(native, { op: 'subscribe', key: `provider-auth:${id}`, method: 'provider.auth.subscribe', payload: { instanceId: id }, generation: host.generation });
  if (host.generation !== generation || reply.generation !== generation) return;
  if (reply.ok) { value.subscription = str(obj(reply.value).id); value.error = ''; }
  else value.error = reply.error?.message || 'Could not load the sign-in terminal. Cancel and retry sign-in.';
}
export function providerAuthView(host: ProviderHost, id: string, provider: Obj | undefined) {
  const value = account(host, id), state = value.state, interaction = obj(state.interaction);
  const active = ['starting', 'waiting', 'verifying'].includes(str(state.phase));
  const methods = arr(state.methods).filter(method => method.type === 'terminal').map(method => ({ id: str(method.id), name: str(method.name) }));
  const signedIn = obj(provider?.auth).status === 'authenticated' || (obj(provider?.auth).status === 'unknown' && state.phase === 'succeeded');
  const discovering = provider?.driver === 'acpRegistry' && !active && !signedIn && !value.error && !Array.isArray(state.methods);
  const description = state.phase === 'starting' ? 'Starting sign-in…' : state.phase === 'verifying' ? 'Checking your account…' : interaction.type === 'terminal' && active ? 'Complete sign-in in the terminal below.' : signedIn ? 'Signed in.' : discovering ? 'Discovering sign-in methods…' : `Sign in on ${str(obj(host.config.environment).label, 'this environment')}.`;
  return { disabled: !str(state.phase) || discovering || !!value.error || provider?.enabled === false || provider?.installed === false, multiple: methods.length > 1, methodId: methods.length === 1 ? methods[0]!.id : '', available: obj(provider?.setup).canAuthenticate === true, active, terminal: interaction.type === 'terminal' && active,
    flow: value.identity, output: str(interaction.output), offset: num(interaction.outputOffset, str(interaction.output).length),
    message: description,
    error: value.error || (state.phase === 'failed' ? str(state.message) : ''), methods,
    label: signedIn ? 'Change account' : ['failed', 'cancelled'].includes(str(state.phase)) ? 'Retry sign-in' : 'Sign in' };
}
export async function providerAuthOp(host: ProviderHost, native: Native, op: string, id: string, raw: string): Promise<void> {
  const value = account(host, id), input = obj(JSON.parse(raw || '{}'));
  if (!host.writable) return;
  if (op !== 'provider-auth-event') value.error = '';
  try {
    if (op === 'provider-auth-start') {
      const methodId = str(input.value);
      adopt(value, await host.rpc(native, 'provider.auth.start', { instanceId: id, ...(methodId ? { methodId } : {}) }, true));
    } else if (op === 'provider-auth-cancel') {
      const flowId = str(value.state.flowId);
      if (flowId) adopt(value, await host.rpc(native, 'provider.auth.cancel', { instanceId: id, flowId }, true));
    } else {
      const event = obj(typeof input.value === 'string' ? JSON.parse(input.value) : input.value);
      if (str(event.authFlow) && str(event.authFlow) !== value.identity) return;
      const flowId = str(value.state.flowId);
      if (!flowId || obj(value.state.interaction).type !== 'terminal') return;
      value.error = '';
      if (event.type === 'auth-error') value.error = 'The provider sign-in terminal is no longer available.';
      if (event.type === 'link-error') value.error = 'Could not open the provider link.';
      if (event.type === 'error') value.error = 'Could not load the sign-in terminal. Cancel and retry sign-in.';
    }
  } catch (error) { if (letGo(error)) throw error; value.error = error instanceof Error ? error.message : 'Could not send input to the provider sign-in terminal.'; }
}
