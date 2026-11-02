// A bundled page is a secure context on every host.
export const ready = async () => (isSecureContext ? (await (await fetch('./deck.json')).json()).message : 'insecure context');
