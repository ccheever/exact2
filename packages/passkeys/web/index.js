// @exact/passkeys on the web (LLP 1133.011): `navigator.credentials` and
// `PublicKeyCredential`, in WebAuthn's JSON forms. The page runtime
// (host/web-js/modules.js) calls it outside the agent; under the agent the
// declared substitute answers and this is never loaded.
//
// - The options and answers are the JSON forms (D1): `parse…FromJSON` and
//   `toJSON()` where the browser has them, else the same algorithms here
//   (§5.1.8, §5.1.9; every buffer a base64url string).
// - No answer carries a PRF output (D10): `clientExtensionResults.prf.results`
//   is removed and `prf.enabled` kept.
// - A lock's credential (§7 item 3, a user handle beginning `exact-lock:`)
//   is never registered, delivered or signalled from here.
// - One ceremony at a time (D9): a modal ceremony, or a second conditional
//   `get`, ends a pending conditional `get` with `AbortError` first, as a
//   browser allows one pending request.
// - `getClientCapabilities` is the browser's, else the two older feature
//   tests, and is asked again when the page returns to the foreground.
// - `dispose` (D11) tells the browser a registration the runtime dropped is
//   unknown to the relying party, where it has `signalUnknownCredential`.

const MARK = new TextEncoder().encode('exact-lock:');

const b64 = bytes => {
  const view = bytes instanceof ArrayBuffer ? new Uint8Array(bytes) : new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  let text = '';
  for (const b of view) text += String.fromCharCode(b);
  return btoa(text).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
};
const unb64 = text => {
  const s = String(text).replace(/-/g, '+').replace(/_/g, '/');
  const raw = atob(s + '='.repeat((4 - (s.length % 4)) % 4));
  return Uint8Array.from(raw, c => c.charCodeAt(0));
};
const marked = id => {
  if (typeof id !== 'string') return false;
  try { const bytes = unb64(id); return MARK.every((b, i) => bytes[i] === b); } catch { return false; }
};
/** A ceremony's failure: its `DOMException`'s name (D1), and what it says. */
const failure = e => ({ error: typeof e?.name === 'string' && /Error$/.test(e.name) ? e.name : 'NotAllowedError', ...(e?.message ? { message: e.message } : {}) });

/** `PublicKeyCredentialCreationOptionsJSON` as the browser takes it (§5.1.8). */
function creation(json) {
  if (typeof PublicKeyCredential.parseCreationOptionsFromJSON === 'function') return PublicKeyCredential.parseCreationOptionsFromJSON(json);
  return {
    ...json,
    challenge: unb64(json.challenge),
    user: { ...json.user, id: unb64(json.user.id) },
    ...(json.excludeCredentials ? { excludeCredentials: json.excludeCredentials.map(c => ({ ...c, id: unb64(c.id) })) } : {}),
  };
}

/** `PublicKeyCredentialRequestOptionsJSON` as the browser takes it (§5.1.9). */
function request(json) {
  if (typeof PublicKeyCredential.parseRequestOptionsFromJSON === 'function') return PublicKeyCredential.parseRequestOptionsFromJSON(json);
  return {
    ...json,
    challenge: unb64(json.challenge),
    ...(json.allowCredentials ? { allowCredentials: json.allowCredentials.map(c => ({ ...c, id: unb64(c.id) })) } : {}),
  };
}

/** Client extension results with every buffer base64url, and no PRF output (D10). */
function extensions(results) {
  const json = v => v instanceof ArrayBuffer || ArrayBuffer.isView(v) ? b64(v) : v && typeof v === 'object' ? Object.fromEntries(Object.entries(v).map(([k, x]) => [k, json(x)])) : v;
  const out = json(results ?? {});
  if (out.prf) delete out.prf.results;
  return out;
}

/** The credential's `toJSON()` (§5.1.10's `RegistrationResponseJSON`, `AuthenticationResponseJSON`). */
function response(credential) {
  const r = credential.response;
  const own = typeof credential.toJSON === 'function' ? credential.toJSON() : {
    id: credential.id, rawId: b64(credential.rawId), type: credential.type,
    authenticatorAttachment: credential.authenticatorAttachment ?? undefined,
    response: 'attestationObject' in r ? {
      clientDataJSON: b64(r.clientDataJSON), attestationObject: b64(r.attestationObject),
      transports: r.getTransports?.() ?? [], publicKeyAlgorithm: r.getPublicKeyAlgorithm?.(),
      publicKey: r.getPublicKey?.() ? b64(r.getPublicKey()) : undefined,
      authenticatorData: r.getAuthenticatorData ? b64(r.getAuthenticatorData()) : undefined,
    } : {
      clientDataJSON: b64(r.clientDataJSON), authenticatorData: b64(r.authenticatorData),
      signature: b64(r.signature), userHandle: r.userHandle ? b64(r.userHandle) : undefined,
    },
  };
  return { ...own, clientExtensionResults: extensions(credential.getClientExtensionResults?.() ?? own.clientExtensionResults) };
}

export function implementation({ changed, log = () => {} }) {
  const present = typeof PublicKeyCredential === 'function' && typeof navigator === 'object' && !!navigator.credentials;
  // The pending conditional `get`, which a newer ceremony ends (D9).
  let conditional = null;
  // Each registration's RP ID by credential id, for `dispose`.
  const made = new Map();
  const endConditional = () => { conditional?.abort(new DOMException('a newer ceremony replaced this conditional one', 'AbortError')); conditional = null; };
  const again = () => changed('passkeys.getClientCapabilities');
  if (typeof addEventListener === 'function') addEventListener('visibilitychange', () => { if (document.visibilityState === 'visible') again(); });
  const signal = async (name, options) => {
    if (!present || typeof PublicKeyCredential[name] !== 'function') return null; // where the browser has none, nothing to tell
    try { await PublicKeyCredential[name](options); return null; } catch (e) { return failure(e); }
  };
  return {
    async getClientCapabilities() {
      if (!present) return { available: false };
      const caps = typeof PublicKeyCredential.getClientCapabilities === 'function' ? await PublicKeyCredential.getClientCapabilities().catch(() => ({})) : {
        userVerifyingPlatformAuthenticator: await PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable?.().catch(() => undefined),
        conditionalGet: await PublicKeyCredential.isConditionalMediationAvailable?.().catch(() => undefined),
      };
      const extension = key => caps[`extension:${key}`] ?? null;
      const known = ['conditionalCreate', 'conditionalGet', 'hybridTransport', 'passkeyPlatformAuthenticator', 'userVerifyingPlatformAuthenticator',
        'relatedOrigins', 'signalAllAcceptedCredentials', 'signalCurrentUserDetails', 'signalUnknownCredential'];
      return {
        available: true, ...Object.fromEntries(known.map(k => [k, typeof caps[k] === 'boolean' ? caps[k] : null])),
        // A declared deviation (D1): not a Level 3 key, and the web has no such request.
        preferImmediatelyAvailableCredentials: false,
        extensions: { prf: extension('prf'), largeBlob: extension('largeBlob'), credProps: extension('credProps') },
        android: null,
      };
    },
    async create({ publicKey, mediation }) {
      if (!present) return { error: 'NotSupportedError' };
      if (marked(publicKey?.user?.id)) return { error: 'NotAllowedError', message: 'a lock\'s user handle is the lock\'s alone' };
      endConditional();
      try {
        const credential = await navigator.credentials.create({ publicKey: creation(publicKey), ...(mediation ? { mediation } : {}) });
        made.set(credential.id, publicKey.rp?.id ?? location.hostname);
        return response(credential);
      } catch (e) { return failure(e); }
    },
    async get({ publicKey, mediation, preferImmediatelyAvailableCredentials }) {
      if (!present) return { error: 'NotSupportedError' };
      if (preferImmediatelyAvailableCredentials) return { error: 'NotSupportedError', message: 'the web has no such request' };
      endConditional();
      const abort = new AbortController();
      if (mediation === 'conditional') conditional = abort;
      try {
        const credential = await navigator.credentials.get({ publicKey: request(publicKey), ...(mediation ? { mediation } : {}), signal: abort.signal });
        const answer = response(credential);
        // A lock's assertion, or one with no user handle where it must have one, is never delivered (§7 item 3).
        const discoverable = !(publicKey.allowCredentials?.length);
        if (marked(answer.response.userHandle) || (discoverable && !answer.response.userHandle)) return { error: 'NotAllowedError' };
        return answer;
      } catch (e) { return failure(e); }
      finally { if (conditional === abort) conditional = null; }
    },
    signalUnknownCredential: options => signal('signalUnknownCredential', options),
    signalAllAcceptedCredentials: options => marked(options.userId) ? { error: 'NotAllowedError' } : signal('signalAllAcceptedCredentials', options),
    signalCurrentUserDetails: options => marked(options.userId) ? { error: 'NotAllowedError' } : signal('signalCurrentUserDetails', options),
    /** Registrations the runtime dropped before data code had them (D11). */
    dispose(ids) {
      for (const id of ids) {
        const rpId = made.get(id) ?? location.hostname;
        made.delete(id);
        signal('signalUnknownCredential', { rpId, credentialId: id }).then(e => log(`dispose passkeys ${id}: ${e ? e.error : 'signalled unknown'}`));
      }
    },
  };
}
