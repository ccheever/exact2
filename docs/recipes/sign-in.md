# Recipe: Sign in with Apple and Google

`@exact/sign-in` is FedCM's `identity` request on `@exact/passkeys`' `get`
(LLP 1133.014). The provider's ID token goes to your server; the plan never
sees it. `apps/auth-fixture` (`app.contract`, `app.ts`, `sign-in.mjs`) is
the whole app and server to copy from.

1. **Grant** each client: `auth.apple <Services ID>`, `auth.google <web
   client>`, `auth.google <iOS client>`, and `net.fetch` for your server.

2. **Show the provider's own button** where the target offers it; never
   draw "Sign in with Apple" yourself.

   ```
   resource ids = getIdentityCapabilities() as shape IdentityCapabilities
   if ids.providers.apple.active
     exact-sign-in-button provider="apple" press=appleSignIn testId="apple-sign-in"
   ```

3. **Fetch a nonce when the screen shows; call `get` at the press** with
   nothing awaited before it (on the web the button holds the popup for
   it). Fetch a fresh nonce after every request.

   ```ts
   const credential = await credentials.get({ identity: { mode: 'active', providers: [{
     configURL: 'https://appleid.apple.com', clientId: 'com.example.web', params: { nonce },
     web: { redirectURI: `${api}/sign-in/return` },   // the web only
   }] } });
   const { handle, ...posted } = credential;          // post `posted`, never the handle
   ```

   Google's `configURL` is `https://accounts.google.com/gsi/fedcm.json`;
   on Apple it also takes `apple: { clientId: <iOS client> }`. Show
   nothing for `NetworkError`, `NotAllowedError` or `AbortError`.

4. **Verify on the server:** the signature against the provider's keys,
   `iss`, `aud` among the audiences the build prints (natively Apple's is
   the bundle ID, Google's the iOS client), `exp`, and `nonce` equal to the
   one you issued to this client, once. Keep Apple's `user.name` the first
   time: it never comes again.

5. **The web's return route** on your server takes Apple's `form_post` or
   Google's redirect (exchanging the code with your secret), keeps the ID
   token under a single-use, short-lived handle, and answers `303` to the
   page's `/.exact/auth/callback` with `state` and `relay=<handle>`. Redeem
   the handle once, then check as in step 4.

6. **Sign out** with `preventSilentAccess()`.

Under the agent a request holds a `credential` ticket naming the provider,
audience and nonce; `packages/sign-in/test/idp.mjs` signs what a provider
would, and `tap @t cancel` dismisses it.
