// Source Control → Bitbucket credentials. Reference
// BitbucketCredentialsSettings.tsx: an access token, or an Atlassian account
// email + API token, never both. Tokens are write-only: the server keeps them
// in its secret store and reports a redacted value; resending that value keeps
// the saved token. A save (or Remove) writes the three fields and rescans: to
// `sourceControlHosts.bitbucket` on a server that has it (reference 9c161462c9,
// 4f63275ac7: each host's settings live under its kind, and a secret left out is
// removed), else to the older `bitbucket` key.
import type { T3Client } from '../client/client';
import { obj, str, type Obj } from '../client/domain';
import { ClientError, type Native } from '../client/protocol';

export type BitbucketView = { key: string; saved: string; savedLabel: string; email: string; apiTokenSaved: boolean; environmentId: string };
type Saved = { accessToken: string; email: string; apiToken: string };
/** The server keeps source control hosts' settings under `sourceControlHosts` (always present on one that does). */
const hostsKeyed = (settings: Obj) => settings.sourceControlHosts !== null && typeof settings.sourceControlHosts === 'object';
const savedOf = (settings: Obj): Saved => { const b = hostsKeyed(settings) ? obj(obj(settings.sourceControlHosts).bitbucket) : obj(settings.bitbucket); return { accessToken: str(b.accessToken).trim(), email: str(b.email).trim(), apiToken: str(b.apiToken).trim() }; };
/** savedMethod: the access token wins; an email + API token pair is the other method. */
export function savedMethod(saved: Saved): '' | 'access-token' | 'api-token' {
  if (saved.accessToken.length > 0) return 'access-token';
  if (saved.email.length > 0 && saved.apiToken.length > 0) return 'api-token';
  return '';
}
const LABELS = { 'access-token': 'access token', 'api-token': 'api token' } as const;

/** The form's view; `key` changes after every save so the drafts start empty again. */
export function bitbucketView(settings: Obj, environmentId: string, revision: number): BitbucketView {
  const saved = savedOf(settings), method = savedMethod(saved);
  return { key: `${environmentId}:${revision}`, saved: method, savedLabel: method ? LABELS[method] : '', email: saved.email, apiTokenSaved: saved.apiToken.length > 0, environmentId };
}

/** The patch a Save sends (or null when it is not savable), exactly as the reference composes it. */
export function bitbucketPatch(saved: Saved, method: string, accessDraft: string, emailDraft: string | null, apiDraft: string): Saved | null {
  const email = (emailDraft ?? saved.email).trim(), access = accessDraft.trim(), api = apiDraft.trim();
  const current = savedMethod(saved), methodIsSaved = current === method;
  const patch = method === 'access-token' ? (access ? { accessToken: access, email: '', apiToken: '' } : null)
    : email && (api || saved.apiToken) ? { accessToken: '', email, apiToken: api || saved.apiToken } : null;
  const canSave = patch !== null && (method === 'access-token' || !methodIsSaved || api !== '' || email !== saved.email);
  return canSave ? patch : null;
}

/** rest:bitbucket — `action=save&method=…&accessToken=…&email=…&emailEdited=…&apiToken=…` or `action=remove`. */
export async function bitbucketCommand(client: T3Client, native: Native, environmentId: string, input: Record<string, string>, rescan: () => void): Promise<string> {
  if (environmentId && environmentId !== client.environmentId) throw new ClientError('That environment is no longer selected.');
  const access = client.restAccess(native);
  const settings = await access.request('server.getSettings');
  const saved = savedOf(settings);
  let next: Saved | null;
  if (input.action === 'remove') next = { accessToken: '', email: '', apiToken: '' };
  else if (input.action === 'save') {
    if (input.method !== 'access-token' && input.method !== 'api-token') throw new ClientError('Choose a Bitbucket sign-in method.');
    next = bitbucketPatch(saved, input.method, str(input.accessToken), input.emailEdited === 'true' ? str(input.email) : null, str(input.apiToken));
    if (!next) throw new ClientError(input.method === 'access-token' ? 'Enter an access token.' : 'Enter your Atlassian account email and an API token.');
  } else throw new ClientError('Unsupported Bitbucket action.');
  const updated = await access.request('server.updateSettings', { patch: hostsKeyed(settings) ? { sourceControlHosts: { ...obj(settings.sourceControlHosts), bitbucket: next } } : { bitbucket: next } }, true);
  client.config = { ...client.config, settings: updated };
  rescan();
  return '';
}
