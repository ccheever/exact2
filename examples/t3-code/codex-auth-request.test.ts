// Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3) packages/shared/src/codexAuthHandoff.test.ts
// (original names): "rejects duplicated authorization parameters and non-loopback callback
// addresses" and the OpenAI-host case of "rejects other handlers, schemes, arbitrary return
// sites, and non-OpenAI authorization". The hosted-web delivery cases are out of scope (no URL
// scheme, no hosted web); the delivery assertion becomes the callback rule it relies on.
import { describe, expect, it } from 'bun:test';
import { codexAuthorizationRequest, codexCallbackUrl, isLoopbackHost } from './codex-auth-request';

const authorizationUrl = () => {
  const url = new URL('https://auth.openai.com/api/accounts/authorize');
  url.search = new URLSearchParams({
    client_id: 'dynamic_agent_client', response_type: 'code', redirect_uri: 'http://127.0.0.1:54213/auth/callback',
    state: 'a'.repeat(43), code_challenge_method: 'S256', code_challenge: 'b'.repeat(43),
  }).toString();
  return url.toString();
};
const input = { authorizationUrl: authorizationUrl() };
const callbackUrl = `http://127.0.0.1:54213/auth/callback?state=${'a'.repeat(43)}&code=one-time-code&client_id=oaiapp_test`;

describe('Codex desktop handoff', () => {
  it('rejects other handlers, schemes, arbitrary return sites, and non-OpenAI authorization', () => {
    expect(() => codexAuthorizationRequest(input.authorizationUrl.replace('auth.openai.com', 'attacker.example'))).toThrow('Invalid ChatGPT sign-in request.');
  });
  it('rejects duplicated authorization parameters and non-loopback callback addresses', () => {
    expect(() => codexAuthorizationRequest(input.authorizationUrl + '&redirect_uri=http://localhost:1/auth/callback')).toThrow();
    const url = new URL(input.authorizationUrl);
    url.searchParams.set('redirect_uri', 'http://localhost:54213/auth/callback');
    expect(() => codexAuthorizationRequest(url.toString())).toThrow();
    url.searchParams.set('redirect_uri', 'https://attacker.example/auth/callback');
    expect(() => codexAuthorizationRequest(url.toString())).toThrow();
    expect(() => codexCallbackUrl(callbackUrl + '&state=another', 'http://127.0.0.1:54213/auth/callback', 'a'.repeat(43))).toThrow();
  });
});

describe('the request the loopback listener accepts (clone)', () => {
  it('keeps the redirect and state of a valid request', () => {
    expect(codexAuthorizationRequest(input.authorizationUrl)).toEqual({ authorizationUrl: input.authorizationUrl, redirectUri: 'http://127.0.0.1:54213/auth/callback', state: 'a'.repeat(43) });
  });
  it('refuses a bad state, challenge, client id, method, response type or a fragment', () => {
    const bad = (key: string, value: string) => { const url = new URL(input.authorizationUrl); url.searchParams.set(key, value); return url.toString(); };
    for (const value of [bad('state', 'short'), bad('code_challenge', 'b'.repeat(42)), bad('client_id', 'other_client'), bad('code_challenge_method', 'plain'), bad('response_type', 'token'), input.authorizationUrl + '#x'])
      expect(() => codexAuthorizationRequest(value)).toThrow('Invalid ChatGPT sign-in request.');
  });
  it('accepts exactly one code or one error on the callback path with the right state', () => {
    const redirect = 'http://127.0.0.1:54213/auth/callback', state = 'a'.repeat(43);
    expect(codexCallbackUrl(callbackUrl, redirect, state).searchParams.get('code')).toBe('one-time-code');
    expect(codexCallbackUrl(`${redirect}?state=${state}&error=access_denied`, redirect, state).searchParams.get('error')).toBe('access_denied');
    for (const value of [callbackUrl + '&code=second', callbackUrl + '&error=both', `${redirect}?state=${state}`, callbackUrl.replace('oaiapp_test', 'other'),
      callbackUrl.replace('54213', '54214'), callbackUrl.replace('/auth/callback', '/auth/other'), callbackUrl + '#fragment'])
      expect(() => codexCallbackUrl(value, redirect, state)).toThrow('This redirect URL does not belong to the current sign-in.');
  });
  it('treats the reference loopback hosts as this computer', () => {
    expect(['localhost', '127.0.0.1', '0.0.0.0', '::1', '[::1]'].every(isLoopbackHost)).toBe(true);
    expect(isLoopbackHost('devbox.example.ts.net')).toBe(false);
  });
});
