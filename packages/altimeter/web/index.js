// @exact/altimeter on the web (LLP 1133.026 D13, D21): no browser ships a
// barometer, so the status reads `available` false and every other name
// fails `NotSupportedError`.

const STATUS = { available: false, state: 'unknown', apple: null };

export function implementation() {
  return {
    async altimeterStatus() { return STATUS; },
    async requestAltimeterPermission() { return STATUS; },
    async barometer() { return { error: 'NotSupportedError' }; },
    async altimeter() { return { error: 'NotSupportedError' }; },
  };
}
