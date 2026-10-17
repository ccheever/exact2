// The ChatGPT sign-in request and redirect rules, T3 Code 1e2ecbd975 (MIT, see LICENSE-T3):
// packages/shared/src/codexAuthHandoff.ts:27-80 (codexAuthorizationRequest, codexCallbackUrl).
// The loopback listener (modules/apple/T3CodexAuth.swift) carries the same rules in Swift; this
// copy decides which flows the app hands to it and which pasted redirect URLs it sends.
// Changes: none in the rules. The hosted-web delivery half of the file (codexAuthHandoffUrl,
// readCodexAuthHandoff, codexAuthDeliveryUrl, readCodexAuthDelivery) is out of scope: the
// clone registers no URL scheme and serves no hosted web client.

export type CodexAuthorizationRequest = { authorizationUrl: string; redirectUri: string; state: string };

const INVALID_REQUEST = 'Invalid ChatGPT sign-in request.';

/** The helper only opens OpenAI's authorize endpoint and receives a loopback callback. */
export function codexAuthorizationRequest(value: string): CodexAuthorizationRequest {
  const url = new URL(value);
  if (
    value.length > 16_384 ||
    url.origin !== 'https://auth.openai.com' ||
    url.pathname !== '/api/accounts/authorize' ||
    url.username ||
    url.password ||
    url.hash
  )
    throw new Error(INVALID_REQUEST);
  const single = (key: string) => {
    const values = url.searchParams.getAll(key);
    if (values.length !== 1 || !values[0]) throw new Error(INVALID_REQUEST);
    return values[0];
  };
  const redirectUri = single('redirect_uri');
  const redirect = new URL(redirectUri);
  const state = single('state');
  if (
    !/^http:\/\/127\.0\.0\.1:[1-9]\d{0,4}\/auth\/callback$/u.test(redirectUri) ||
    Number(redirect.port) > 65_535 ||
    !/^[\w-]{16,128}$/u.test(state) ||
    single('response_type') !== 'code' ||
    single('code_challenge_method') !== 'S256' ||
    !/^[\w-]{43}$/u.test(single('code_challenge')) ||
    !/^(dynamic_agent_client|oaiapp_[\w-]+)$/u.test(single('client_id'))
  )
    throw new Error(INVALID_REQUEST);
  return { authorizationUrl: url.toString(), redirectUri, state };
}

/** Validation is shared by the desktop listener and the environment receiving the code. */
export function codexCallbackUrl(value: string, redirectUri: string, state: string): URL {
  const callback = new URL(value);
  const expected = new URL(redirectUri);
  const states = callback.searchParams.getAll('state');
  const codes = callback.searchParams.getAll('code');
  const errors = callback.searchParams.getAll('error');
  const clients = callback.searchParams.getAll('client_id');
  if (
    value.length > 16_384 ||
    callback.origin !== expected.origin ||
    callback.pathname !== expected.pathname ||
    callback.username ||
    callback.password ||
    callback.hash ||
    states.length !== 1 ||
    states[0] !== state ||
    clients.length > 1 ||
    (clients.length === 1 && !/^oaiapp_[\w-]+$/u.test(clients[0]!)) ||
    !(
      (codes.length === 1 && Boolean(codes[0]) && errors.length === 0) ||
      (errors.length === 1 && Boolean(errors[0]) && codes.length === 0)
    )
  )
    throw new Error('This redirect URL does not belong to the current sign-in.');
  return callback;
}

/** codexAuthorizationRequest without the throw: the request, or null for anything the helper refuses. */
export function readCodexAuthorizationRequest(value: string): CodexAuthorizationRequest | null {
  try { return codexAuthorizationRequest(value); } catch { return null; }
}

/** isLoopbackHost (packages/shared/src/preview.ts:20-37): the hosts the reference treats as this computer. */
export function isLoopbackHost(host: string): boolean {
  return ['localhost', '127.0.0.1', '0.0.0.0', '::1', '[::1]'].includes(host);
}
