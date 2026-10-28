// @exact/geocoding on the web (LLP 1133.021 D7): the web has no geocoder,
// so the status reads `available` false and both sends are refused
// `NotSupportedError` by the page runtime before they reach here. An app
// that wants one calls a service whose terms it accepted, with `fetch`.

export function implementation() {
  return {
    async geocodingStatus() { return { available: false }; },
  };
}
