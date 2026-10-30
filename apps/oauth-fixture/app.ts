// The OAuth fixture's data module (LLP 1146): one `@exact/oauth` provider
// per fixture profile, each as its provider's documentation says to
// configure it (`packages/oauth/test/PROVIDERS.md`).
import type { Answer, Sources } from './app.contract.d.ts';
import { oauth, type OAuthConfig, type OAuthProvider } from '@exact/oauth';

export const appId = 'com.exact.oauthfixture';
const FIXTURE = 'http://127.0.0.1:4400';
export const grants = [
  `net.fetch ${FIXTURE}`,
  // Google's token endpoint is on a second origin (PROVIDERS.md GOOG-1).
  'net.fetch http://127.0.0.1:4401',
  `auth.session ${FIXTURE}`,
  'auth.callback com.exact.oauthfixture:/oauth',
  'secret.keep oauth.spec', 'secret.keep oauth.google', 'secret.keep oauth.entra', 'secret.keep oauth.entra-v1',
  'secret.keep oauth.auth0', 'secret.keep oauth.okta', 'secret.keep oauth.onelogin', 'secret.keep oauth.github',
  'secret.keep oauth.salesforce',
].join('\n');

const at = (profile: string): string => `${FIXTURE}/p/${profile}`;
const common = (profile: string): Pick<OAuthConfig, 'clientId' | 'key' | 'apis'> => ({
  clientId: `client-${profile}`, key: `oauth.${profile}`, apis: [FIXTURE],
});
const CONFIGS: Record<string, OAuthConfig> = {
  spec: { ...common('spec'), issuer: at('spec'), scopes: ['openid', 'profile', 'offline_access'] },
  google: { ...common('google'), issuer: at('google'), scopes: ['openid', 'profile', 'email'], authorizationParams: { access_type: 'offline', prompt: 'consent' }, clientSecret: 'fixture-secret', issuerAliases: ['accounts.google.com'] },
  entra: { ...common('entra'), issuer: `${at('entra')}/common/v2.0`, issuerTemplate: true, scopes: ['openid', 'profile', 'offline_access'] },
  'entra-v1': { ...common('entra-v1'), issuer: `${at('entra-v1')}/common/v2.0`, issuerTemplate: true, scopes: ['openid', 'profile', 'offline_access'] },
  auth0: { ...common('auth0'), issuer: `${at('auth0')}/`, scopes: ['openid', 'profile', 'offline_access'] },
  okta: { ...common('okta'), issuer: at('okta'), scopes: ['openid', 'profile', 'offline_access'], retryLostRefresh: true },
  onelogin: { ...common('onelogin'), issuer: `${at('onelogin')}/oidc/2`, scopes: ['openid', 'profile'], endedErrors: [{ error: 'invalid_request', status: 401 }] },
  github: { ...common('github'), clientSecret: 'fixture-secret', endpoints: { authorization: `${at('github')}/authorize`, token: `${at('github')}/token` }, scopes: ['read:user'], endedErrors: ['bad_refresh_token'], commaScopes: true },
  salesforce: { ...common('salesforce'), issuer: at('salesforce'), scopes: ['openid', 'api', 'refresh_token'] },
};
const providers = new Map<string, OAuthProvider>();
const provider = (profile: string): OAuthProvider => {
  let p = providers.get(profile);
  if (!p) {
    const config = CONFIGS[profile];
    if (!config) throw new Error(`no profile ${profile}`);
    p = oauth(config);
    providers.set(profile, p);
  }
  return p;
};

const sources: Sources = {
  session: ([profile], store) => provider(profile).session(store),
  signIn: ([profile], store) => provider(profile).signIn(store),
  signOut: ([profile], store) => provider(profile).signOut(store),
  me: async ([profile, signedIn], store) => {
    if (!signedIn) return { name: '' };
    const person = await provider(profile).json(store, `${at(profile)}/api/me`) as { name?: unknown };
    return { name: String(person.name ?? '') };
  },
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args as never, store, storage, native);
