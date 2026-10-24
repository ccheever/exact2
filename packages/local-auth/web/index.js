// @exact/local-auth on the web (LLP 1133.010 D15): the cell is `later`, to be
// filled over the passkeys module (WebAuthn's user verification, and its
// `prf` extension for a locked secret). Until then the page says so: the
// status answers that nothing can be checked or kept locked here, and
// `authenticate` is `NotSupportedError`. The store refuses a name that must
// be locked on its own (host/web-js/admission.js). The page runtime
// (host/web-js/modules.js) calls this outside the agent; under the agent the
// declared substitute answers the same.

const STATUS = { available: false, biometry: 'none', deviceCredential: false, state: 'unknown', presenceSecrets: false, apple: null, android: null };

export function implementation() {
  return {
    async localAuthStatus() { return STATUS; },
    async authenticate() { return { error: 'NotSupportedError' }; },
  };
}
