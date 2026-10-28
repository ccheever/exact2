// @exact/background-transfer on the web (LLP 1133.017 D17): `later`. Background
// Fetch needs a service worker, and exact2's web build hosts no runner in one,
// so `backgroundFetchStatus().available` is false, a transfer is
// `NotSupportedError`, and an app uses `fetch` while the page is open.

export function implementation() {
  const refused = async () => ({ error: 'NotSupportedError' });
  return {
    backgroundFetch: refused,
    abortBackgroundFetch: refused,
    retireBackgroundFetch: refused,
    async backgroundFetchRegistration() { return null; },
    async backgroundFetchIds() { return []; },
    async backgroundFetchStatus() { return { available: false, state: 'unknown' }; },
    release() {},
    dispose() {},
  };
}
