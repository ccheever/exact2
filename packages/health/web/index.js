// @exact/health on the web (LLP 1133.026 D21): the web has no health store,
// so the status reads `available` false and every other name is refused
// `NotSupportedError` by the page runtime before it reaches here.

export function implementation() {
  return {
    async healthStatus() {
      return { available: false, state: 'unknown', types: [], apple: null, android: null };
    },
  };
}
