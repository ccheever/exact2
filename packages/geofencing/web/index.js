// @exact/geofencing on the web (LLP 1133.021 D20): no browser implements the
// Geofencing API and a page runs nothing while closed, so the status reads
// `available` false and every other name is refused `NotSupportedError` by
// the page runtime before it reaches here.

export function implementation() {
  return {
    async geofencingStatus() {
      return { available: false, state: 'unknown', apple: null, android: null };
    },
    async geofences() { return []; },
  };
}
