import { sourceControlHostFields as fields } from '../shared/source-control-hosts';
// T3 Code 9c161462c9: SourceControlHostSettings and GitHub{Account,Token}Settings.
import type { T3Client } from '../client/client';
import { arr, obj, str, type Obj } from '../client/domain';
import { ClientError, type Native } from '../client/protocol';
import { redactedValue } from '../shared/redacted-text';
import { requirePermission } from '../connections/command-permissions';


export function groupGitHubAccounts(accounts: Obj[]) {
  return [...new Set(accounts.map(account => str(account.host)))].map(host => {
    const entries = accounts.filter(account => account.host === host), stored = entries.filter(account => account.environmentVariable === undefined);
    const selectable = stored.filter(account => account.authenticated === true);
    return { host, active: str((selectable.find(account => account.active === true) ?? selectable[0])?.account), selectable,
      broken: stored.filter(account => account.authenticated !== true), environmentVariable: str(entries.find(account => account.environmentVariable)?.environmentVariable) };
  });
}
export function nextGitHubHosts(current: Obj, host: string, change: { enabled?: boolean; account?: string | null }): Obj {
  const previous = obj(current[host]), enabled = change.enabled ?? previous.enabled !== false;
  const account = change.account === undefined ? str(previous.account) : change.account;
  const { [host]: _previous, ...rest } = current;
  return enabled && !account ? rest : { ...rest, [host]: { enabled, ...(account ? { account } : {}) } };
}
export function hostCredentials(settings: Obj, discovered: Obj, environmentId: string, revision: number, writable: boolean) {
  const hosts = obj(settings.sourceControlHosts), github = obj(hosts.github);
  const auth = obj(arr(discovered.sourceControlProviders).find(provider => provider.kind === 'github')?.auth);
  const groups = groupGitHubAccounts(arr(auth.accounts)).map(group => {
    const choice = obj(obj(github.hosts)[group.host]), pin = str(choice.account), stale = !!pin && !group.selectable.some(account => account.account === pin);
    const account = pin && !stale ? pin : group.active;
    const redacted = redactedValue(account);
    return { host: group.host, enabled: choice.enabled !== false, selected: pin && !stale ? pin : '',
      accountValue: redacted.value, accountPlaceholder: redacted.placeholder,
      options: [{ value: '', label: 'Active gh account', selected: !pin || stale }, ...group.selectable.map((account, index) => ({ value: str(account.account), label: `Account ${index + 1}`, selected: account.account === pin && !stale }))],
      warning: stale ? 'The chosen login is no longer signed in, so the active gh login is used.' : '',
      environmentWarning: group.environmentVariable ? `${group.environmentVariable} is set on the server, so it overrides the account chosen here until it is unset.` : '',
      broken: group.broken.map((account, index) => ({ key: `${index}`, ...redactedValue(str(account.account)), error: str(account.error, 'gh reports this login as invalid.') })) };
  });
  return ['github', 'gitcafe', 'bitbucket'].map(kind => ({ kind, key: `${environmentId}:${kind}:${revision}`, environmentId, disabled: !writable,
    groups: kind === 'github' ? groups : [], savedHosts: Object.entries(obj(github.tokens)).filter(([, token]) => typeof token === 'string' && token !== '').map(([host]) => ({ host })),
    fields: fields.filter(field => field.kind === kind).map(field => ({ key: field.key, label: field.label, description: field.description, secret: field.secret,
      value: field.secret ? '' : str(obj(hosts[kind])[field.key]), saved: str(obj(hosts[kind])[field.key]) !== '' })),
  }));
}
export type HostCredentials = ReturnType<typeof hostCredentials>[number];

/** Read current settings before replacing either host map; redaction markers retain other secrets. */
export async function hostCredentialsCommand(client: T3Client, native: Native, input: Record<string, string>, rescan: () => void): Promise<string> {
  if (input.environment && input.environment !== client.environmentId) throw new ClientError('That environment is no longer selected.');
  if (!client.ready) throw new ClientError('Reconnect before changing source control host settings.');
  requirePermission(client.scopes, 'settings:write');
  const access = client.restAccess(native), settings = obj((await access.request('server.getConfig')).settings);
  const hosts = obj(settings.sourceControlHosts), kind = input.kind;
  if (!['github', 'gitcafe', 'bitbucket'].includes(kind)) throw new ClientError('Unsupported source control host.');
  const saved = obj(hosts[kind]);
  let next: Obj;
  if (kind === 'github') {
    const host = str(input.host).trim().toLowerCase();
    if (!host || !/^[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?(?::\d+)?$/.test(host)) throw new ClientError('Enter a host name.');
    if (input.action === 'token') {
      const token = str(input.value).trim();
      if (!token && input.remove !== 'true') throw new ClientError('Enter a token.');
      next = { ...saved, tokens: { ...obj(saved.tokens), [host]: token } };
    } else if (input.action === 'enabled') next = { ...saved, hosts: nextGitHubHosts(obj(saved.hosts), host, { enabled: input.value === 'true' }) };
    else if (input.action === 'account') next = { ...saved, hosts: nextGitHubHosts(obj(saved.hosts), host, { account: input.value || null }) };
    else throw new ClientError('Unsupported GitHub action.');
  } else {
    const definition = fields.find(field => field.kind === kind && field.key === input.key);
    if (input.action === 'remove') next = Object.fromEntries(fields.filter(field => field.kind === kind).map(field => [field.key, '']));
    else if (input.action === 'field' && definition) next = { ...saved, [definition.key]: str(input.value).trim() };
    else throw new ClientError('Unsupported host setting.');
  }
  const updated = await access.request('server.updateSettings', { patch: { sourceControlHosts: { ...hosts, [kind]: next } } }, true);
  client.config = { ...client.config, settings: updated };
  rescan();
  return '';
}
