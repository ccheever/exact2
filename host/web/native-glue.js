// The app's page module (`host.web.native`, LLP 1016.002): it answers
// `native.later` on the page, where the browser's own capabilities are. The
// glue loads this after first paint, at the first native request; the app's
// module is imported here, never before the first pixel.
globalThis.exact ??= {};
globalThis.exact.pageNative = async (url, changed) => {
  if (!url) throw new Error('this app has no page module (host.web.native)');
  const module = await import(url);
  if (typeof module.later !== 'function') throw new Error('the page module exports no later(request)');
  // Its one way to announce a changed device topic.
  module.connect?.({ changed });
  const decoder = new TextDecoder();
  return {
    // A request body (base64 JSON, as the kernel sends it) to the reply's JSON.
    async later(body) {
      const request = JSON.parse(decoder.decode(Uint8Array.from(atob(body ?? ''), (c) => c.charCodeAt(0))));
      return JSON.stringify((await module.later(request)) ?? null);
    },
  };
};
