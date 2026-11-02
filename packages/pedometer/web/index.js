// @exact/pedometer on the web: no browser counts steps,
// so the status reads `available` false and every other name fails
// `NotSupportedError`.

const STATUS = { available: false, state: 'unknown', apple: null, android: null };

export function implementation() {
  return {
    async pedometerStatus() { return STATUS; },
    async requestPedometerPermission() { return STATUS; },
    async pedometer() { return { error: 'NotSupportedError' }; },
  };
}
