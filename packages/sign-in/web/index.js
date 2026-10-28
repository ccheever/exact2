// @exact/sign-in on the web (LLP 1133.014 D3, D5, D6): each provider's
// authorization page in a popup through the host's auth session, answered to
// the app server's return route, which hands the page a single-use handle.
// No provider's script runs in the page (§13 question 3).
//
// - `identity` is the member of `@exact/passkeys`' `get` (D10 item 1). The
//   page runtime hands it here whole: the web never shares a sheet with a
//   passkey (D1). It makes `state` from the system's random source and the
//   provider's authorization URL (D3 step 2), opens it through the host's
//   auth session with the page's own callback (D10 item 3), and answers
//   `{relay}` from the callback, or the provider's `error`.
// - `exact-sign-in-button` draws the provider's button. At a press, in the
//   button's own click handler, before the click reaches the element where the
//   page delivers `press`, it opens the popup's window on `about:blank`, inside
//   the press's activation, and leaves it waiting for the request the
//   press's action makes (D5); one no request claims soon after is closed.
//   Under the agent it opens nothing: the substitute answers.
// - `getIdentityCapabilities` is the cells and the grants: each provider
//   active where the app has its line, never passive (D3).

const APPLE = 'https://appleid.apple.com';
const GOOGLE = 'https://accounts.google.com/gsi/fedcm.json';
const CALLBACK = '/.exact/auth/callback';
// How long a window the button opened waits for its request (D5 item 3).
const UNCLAIMED_MS = 2000;

const b64 = bytes => btoa(String.fromCharCode(...bytes)).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
const random = n => b64(crypto.getRandomValues(new Uint8Array(n)));

/** The provider's authorization URL for one provider of a request (D3 step 2). */
export function authorizationURL(provider, state) {
  const fields = provider.fields ?? ['name', 'email', 'picture'];
  const back = provider.web?.redirectURI;
  const nonce = provider.params?.nonce;
  const q = new URLSearchParams();
  if (provider.configURL === APPLE) {
    q.set('client_id', provider.clientId);
    q.set('redirect_uri', back);
    q.set('response_type', 'code id_token');
    q.set('response_mode', 'form_post');
    const scope = ['name', 'email'].filter(f => fields.includes(f)).join(' ');
    if (scope) q.set('scope', scope);
    q.set('state', state);
    if (nonce) q.set('nonce', nonce);
    return `${APPLE}/auth/authorize?${q}`;
  }
  if (provider.configURL === GOOGLE) {
    q.set('client_id', provider.clientId);
    q.set('redirect_uri', back);
    q.set('response_type', 'code');
    q.set('scope', 'openid email profile');
    q.set('state', state);
    if (nonce) q.set('nonce', nonce);
    if (provider.loginHint) q.set('login_hint', provider.loginHint);
    if (provider.domainHint) q.set('hd', provider.domainHint);
    // A server that wants PKCE minted its verifier with the nonce (D3).
    if (provider.params?.code_challenge) {
      q.set('code_challenge', provider.params.code_challenge);
      q.set('code_challenge_method', 'S256');
    }
    return `https://accounts.google.com/o/oauth2/v2/auth?${q}`;
  }
  return null;
}

/** The answer a callback URL carries: the credential, or the provider's error (D3 step 4). */
export function answered(provider, url) {
  const q = new URL(url).searchParams;
  const error = q.get('error');
  if (error) {
    const cancelled = error === 'user_cancelled_authorize' || error === 'access_denied';
    return cancelled ? { error: 'NetworkError', message: error } : { error: 'IdentityCredentialError', message: error };
  }
  const relay = q.get('relay');
  if (!relay) return { error: 'IdentityCredentialError', message: 'the return route answered no handle' };
  return { type: 'identity', id: '', configURL: provider.configURL, isAutoSelected: false, token: { relay }, handle: random(16) };
}

/** The host auth session's status as the error D1 names. */
const failure = e => {
  const status = e?.status;
  if (status === 499) return { error: 'NetworkError' };
  if (status === 409) return { error: 'NotAllowedError', message: 'busy' };
  if (status === 428) return { error: 'NotAllowedError', message: 'activation' };
  if (status === 403) return { error: 'SecurityError', message: e?.message ?? '' };
  return { error: 'NetworkError', message: e?.message ?? String(e) };
};

export function implementation(context = {}) {
  const lines = String(context.grants ?? '').split('\n').map(l => l.trim().split(/\s+/)[0]);
  const log = line => context.log?.(line);
  const open = new Map(); // request → the session's cancel
  return {
    agent: !!context.agent,
    getIdentityCapabilities() {
      const can = line => ({ active: lines.includes(line), passive: false, withPublicKey: false });
      return { available: true, providers: { apple: can('auth.apple'), google: can('auth.google') } };
    },
    preventSilentAccess() { return null; },
    async identity(args, told = {}) {
      const provider = args.identity?.providers?.[0];
      if (args.identity?.mode !== 'active' || !provider) return { error: 'NotSupportedError', message: 'the web offers a provider from a press alone' };
      const state = random(32);
      const url = authorizationURL(provider, state);
      if (!url) return { error: 'NotSupportedError' };
      const session = context.authSession;
      if (!session) return { error: 'NotSupportedError', message: "the host's auth session is not loaded" };
      log(`sign-in: ${provider.configURL === APPLE ? 'apple' : 'google'} opened`);
      try {
        const back = await session.open({ url, callback: location.origin + CALLBACK, state, request: told.request, cancelled: c => open.set(told.request, c) });
        return answered(provider, back);
      } catch (e) {
        return failure(e);
      } finally {
        open.delete(told.request);
      }
    },
    cancel(request) { open.get(request)?.(); },
    /** A credential the runtime dropped: its handle is never redeemed and expires at the server (D12). */
    dispose(ids) { for (const id of ids) log(`dispose sign-in ${id}: its handle expires unredeemed`); },
  };
}

// ---------------------------------------------------------------- the button (D5)
const WORDS = {
  apple: { signin: 'Sign in with Apple', signup: 'Sign up with Apple', continue: 'Continue with Apple', use: 'Sign in with Apple' },
  google: { signin: 'Sign in with Google', signup: 'Sign up with Google', continue: 'Continue with Google', use: 'Sign in with Google' },
};
// The marks, drawn in the package: the Apple logo in the button's text colour, Google's four-colour "G".
const APPLE_MARK = '<svg viewBox="0 0 170 170" width="16" height="16" aria-hidden="true"><path fill="currentColor" d="M150.37 130.25c-2.45 5.66-5.35 10.87-8.71 15.66-4.58 6.53-8.33 11.05-11.22 13.56-4.48 4.12-9.28 6.23-14.42 6.35-3.69 0-8.14-1.05-13.32-3.18-5.2-2.12-9.97-3.17-14.34-3.17-4.58 0-9.49 1.05-14.75 3.17-5.27 2.13-9.51 3.24-12.75 3.35-4.93.21-9.84-1.96-14.75-6.52-3.13-2.73-7.04-7.41-11.73-14.04-5.03-7.08-9.17-15.29-12.41-24.65-3.47-10.11-5.21-19.9-5.21-29.38 0-10.86 2.35-20.22 7.04-28.07 3.69-6.3 8.6-11.27 14.75-14.92s12.79-5.51 19.95-5.63c3.91 0 9.05 1.21 15.43 3.59 6.36 2.39 10.45 3.6 12.24 3.6 1.34 0 5.88-1.42 13.57-4.24 7.28-2.62 13.42-3.7 18.45-3.28 13.63 1.1 23.87 6.47 30.68 16.15-12.19 7.39-18.22 17.73-18.1 31 .11 10.34 3.86 18.94 11.23 25.77 3.34 3.17 7.07 5.62 11.22 7.36-.9 2.61-1.85 5.11-2.86 7.51zM119.11 7.24c0 8.1-2.96 15.67-8.86 22.67-7.12 8.32-15.73 13.13-25.07 12.37a25.2 25.2 0 0 1-.19-3.07c0-7.78 3.39-16.1 9.4-22.91 3-3.45 6.82-6.31 11.45-8.6 4.62-2.25 8.99-3.5 13.1-3.71.12 1.08.17 2.17.17 3.24z"/></svg>';
const GOOGLE_MARK = '<svg viewBox="0 0 48 48" width="18" height="18" aria-hidden="true"><path fill="#EA4335" d="M24 9.5c3.54 0 6.71 1.22 9.21 3.6l6.85-6.85C35.9 2.38 30.47 0 24 0 14.62 0 6.51 5.38 2.56 13.22l7.98 6.19C12.43 13.72 17.74 9.5 24 9.5z"/><path fill="#4285F4" d="M46.98 24.55c0-1.57-.15-3.09-.38-4.55H24v9.02h12.94c-.58 2.96-2.26 5.48-4.78 7.18l7.73 6c4.51-4.18 7.09-10.36 7.09-17.65z"/><path fill="#FBBC05" d="M10.53 28.59c-.48-1.45-.76-2.99-.76-4.59s.27-3.14.76-4.59l-7.98-6.19C.92 16.46 0 20.12 0 24c0 3.88.92 7.54 2.56 10.78l7.97-6.19z"/><path fill="#34A853" d="M24 48c6.48 0 11.93-2.13 15.89-5.81l-7.73-6c-2.15 1.45-4.92 2.3-8.16 2.3-6.26 0-11.57-4.22-13.47-9.91l-7.98 6.19C6.51 42.62 14.62 48 24 48z"/></svg>';
const STYLE = `
.exact-sign-in { box-sizing: border-box; display: flex; align-items: center; justify-content: center; gap: 8px; width: 100%; height: 100%; min-width: 140px; min-height: 40px; padding: 0 16px; border: 1px solid transparent; font: 500 16px -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Arial, sans-serif; cursor: pointer; }
.exact-sign-in.apple { background: #000; color: #fff; border-radius: 6px; }
.exact-sign-in.google { font-family: Roboto, Arial, sans-serif; font-size: 14px; border-radius: 4px; }
.exact-sign-in.google.outline { background: #fff; color: #1f1f1f; border-color: #747775; }
.exact-sign-in.google.filled_blue { background: #1a73e8; color: #fff; }
.exact-sign-in.google.filled_black { background: #131314; color: #e3e3e3; border-color: #8e918f; }
.exact-sign-in.google.filled_blue svg { background: #fff; border-radius: 50%; padding: 2px; }
.exact-sign-in.google.pill { border-radius: 20px; }
@media (prefers-color-scheme: dark) { .exact-sign-in.apple { background: #fff; color: #000; } }
`;
let styled = null;

class SignInButton {
  constructor({ implementation, element, props, events }) {
    const doc = element.ownerDocument;
    if (styled !== doc) { const s = doc.createElement('style'); s.textContent = STYLE; doc.head.append(s); styled = doc; }
    this.events = events;
    this.agent = !!implementation?.agent;
    this.button = doc.createElement('button');
    this.button.type = 'button';
    this.button.addEventListener('click', () => this.press());
    element.append(this.button);
    this.setProps(props);
    events.load();
  }

  setProps(props) {
    const provider = props.provider === 'google' ? 'google' : 'apple';
    const words = WORDS[provider][props.context ?? 'signin'] ?? WORDS[provider].signin;
    const b = this.button;
    b.className = `exact-sign-in ${provider} ${props.theme ?? 'outline'} ${props.shape ?? ''}`;
    if (provider === 'apple' && props.cornerRadius != null) b.style.borderRadius = `${Number(props.cornerRadius)}px`;
    b.setAttribute('aria-label', words);
    b.innerHTML = `${provider === 'apple' ? APPLE_MARK : GOOGLE_MARK}<span>${words}</span>`;
  }

  // At the press: the popup's window, inside the press's activation (D5). The click then reaches the element, where
  // the page delivers `press` to the app's action, after this handler.
  press() {
    if (!this.agent && typeof window === 'object') {
      const name = `exact-auth-${random(12)}`;
      const win = window.open('about:blank', name, 'popup,width=520,height=680');
      if (win) {
        const waiting = { window: win, claimed: false };
        (globalThis.exact ??= {}).authWaiting = waiting;
        setTimeout(() => {
          if (waiting.claimed) return;
          if (globalThis.exact?.authWaiting === waiting) globalThis.exact.authWaiting = null;
          try { win.close(); } catch {}
        }, UNCLAIMED_MS);
      }
    }
  }

  destroy() { this.button.remove(); }
}

export const roster = { 'exact-sign-in-button': { view: SignInButton } };
