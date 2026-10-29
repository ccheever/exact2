// The page's large binaries ship gzipped, each as `<file>.gz` beside where its package looks for it
// (tools/generate/gen-file-preview.mjs): an app's bake captures at most 64 MiB of sources, its assets included, and
// 16 MiB a file (js/bake/src/lib.rs), and the engines' wasm and the fonts are more. This wraps the context's `fetch`, so
// the packages, which fetch these files by their own names, get the bytes as they were. Every entry imports it first.
const PACKED = /\.(?:wasm|ttf|otf|bcmap|pfb)$/i;
const TYPES: Record<string, string> = { wasm: "application/wasm", ttf: "font/ttf", otf: "font/otf" };

/** The page's own packed file a request names, or null for any other request. */
export function packedUrl(input: RequestInfo | URL, base: string): URL | null {
  let url: URL;
  try {
    url = new URL(input instanceof Request ? input.url : String(input), base);
  } catch {
    return null;
  }
  // By scheme and host: the URL standard gives a custom scheme's URL (`exact-app://app/…`) an opaque origin.
  const page = new URL(base);
  return url.protocol === page.protocol && url.host === page.host && PACKED.test(url.pathname) ? url : null;
}

/**
 * What a relative URL in this context is relative to: its location, or, in a worker started from a Blob
 * (policy-workers.ts), whose location is that Blob's URL, this script's own URL, in the directory the worker's
 * packages name their files from.
 */
export function contextBase(): string {
  return typeof location === "undefined" || location.protocol === "blob:" ? import.meta.url : location.href;
}

// In the page and its workers; a test imports `packedUrl` alone.
if (typeof location !== "undefined") {
  const fetchFile = globalThis.fetch.bind(globalThis);
  globalThis.fetch = async (input: RequestInfo | URL, init?: RequestInit) => {
    const base = contextBase();
    const url = packedUrl(input, base);
    // A Blob-started worker has no base of its own for a relative request.
    if (!url) return fetchFile(typeof input === "string" && location.protocol === "blob:" ? new URL(input, base).href : input, init);
    const packed = await fetchFile(`${url.href.replace(/[?#].*$/, "")}.gz`, { signal: init?.signal ?? null });
    if (!packed.ok || !packed.body) return packed;
    const type = TYPES[url.pathname.split(".").pop()!.toLowerCase()] ?? "application/octet-stream";
    return new Response(packed.body.pipeThrough(new DecompressionStream("gzip")), { headers: { "content-type": type } });
  };
}
