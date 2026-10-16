// A bundled page is a secure context on every host (@ref LLP 1020.001 Q1).
export const ready = async () => (isSecureContext ? (await (await fetch('./deck.json')).json()).message : 'insecure context');
