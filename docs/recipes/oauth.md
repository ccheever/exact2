# Recipe: sign in with OAuth (Google, Microsoft, Auth0, Okta…)

`@exact/oauth` signs a person in and keeps them signed in: PKCE in the system
browser, refresh, requests with the token, sign-out. Write no token request,
refresh or expiry check of your own. Install it in the app's directory with
`EXACT_APP_DIR=$PWD bun <exact2>/scripts/exact.mjs add oauth`. Each provider's
settings, and tests against a fake provider: `bun exact.mjs docs sign-in providers`.

```ts
import { oauth } from '@exact/oauth';
// Literal lines. `net.fetch` and `auth.session` take an origin, no path:
// the issuer's, its token endpoint's when it differs, each API's; the
// sign-in page's. Then your registered callback and the session's key.
export const grants = ['net.fetch https://login.example.com', 'net.fetch https://api.example.com',
  'auth.session https://login.example.com', 'auth.callback com.example.app:/oauth',
  'secret.keep example.session'].join('\n');
// The issuer as the provider names it, path and trailing slash included.
// Discovery finds the endpoints; leave `callback` unset.
const provider = oauth({ issuer: 'https://login.example.com/oauth2/default', clientId: 'abc123',
  scopes: ['openid', 'profile', 'offline_access'], key: 'example.session',
  apis: ['https://api.example.com'] }); // only these origins get the token
const sources: Sources = {
  session: (_a, store) => provider.session(store),
  signIn: (_a, store) => provider.signIn(store),
  signOut: (_a, store) => provider.signOut(store),
  me: async ([signedIn], store) => signedIn
    ? { name: String(((await provider.json(store, 'https://api.example.com/me')) as { name?: string }).name ?? '') }
    : { name: '' },
};
```

```text
use OAuthSession, OAuthResult from "@exact/oauth"   // at the top of the file

shape Me
  name: string

component Account
  resource session = session() as shape OAuthSession
  resource me = me(session.signedIn) as shape Me else empty(name="")
  mutation signedIn as shape OAuthResult refreshes session, me
  mutation signedOut as shape OAuthResult refreshes session, me
  derive problem = match signedIn { case some(r) => (r.error != "" ? r.error : session.error), case none => session.error }
  action signIn
    send signedIn = signIn()
  action signOut
    send signedOut = signOut()
  action reload
    refresh me
```

- Show the person's data only while `session.signedIn`. `problem` says why the
  last sign-in failed or the provider ended the session: show it when not `""`.
- `fetch` and `json` throw `OAuthError`. Let it throw: the resource fails and
  keeps what it showed. A failed read is not a sign-in problem; do not show it
  as one.
- `refresh me` reads again; sending a mutation instead asks the API twice.
- Keep `session` a resource the sign-in screen reads: it readies the next
  sign-in, so the press opens the browser at once.
- Pass no time and read no token yourself. When the provider ends the session,
  `session.error` says why and the screen leaves the signed-in state; a
  network failure, 429 or 5xx keeps it.
