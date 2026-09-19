// Asset delivery shared by the page host and transport tests. Limits are host policy.
export function assetName(name) {
  return typeof name === "string" && name.length > 0 && name.length <= 128
    && /^[\x20-\x7e]+$/.test(name) && !name.includes("\\")
    && name.split("/").every(p => p && p !== "." && p !== "..");
}

export function assetDelivery({getModule, live, baseURI, devAssets = () => undefined,
  delivered = () => {}, fetch = globalThis.fetch, deadlineMs = 20_000,
  attemptMs = 5_000, backoffMs = 250, byteLimit = 64 * 1024 * 1024}) {
const assetFlights = new Set();
let activeAssetFlights = 0;
function pumpAssets() {
  for (const flight of assetFlights) {
    if (activeAssetFlights >= 8) break;
    if (flight.started || flight.cancelled) continue;
    flight.started = true; activeAssetFlights++;
    flight.run().finally(() => {
      activeAssetFlights--; assetFlights.delete(flight); flight.resolve(); pumpAssets();
    });
  }
}
function cancelAssets(entry, names) {
  for (const flight of assetFlights) if (flight.entry === entry && (!names || names.has(flight.name))) {
    flight.cancelled = true; flight.controller.abort();
    assetFlights.delete(flight);
    if (!flight.started) flight.resolve();
  }
}
function assets(entry) {
  const module = getModule(), id = entry.id;
  if (!id || !module) return;
  const names = JSON.parse(module.gpu_assets(id));
  cancelAssets(entry, new Set(JSON.parse(module.gpu_retired?.(id) ?? "[]")));
  for (const name of names) {
    if ([...assetFlights].some(f => f.entry === entry && f.name === name && !f.cancelled)) continue;
    if (assetFlights.size >= 256) {
      module.gpu_asset_failed(id, name, "host limit is 256 queued and active asset flights");
      continue;
    }
    const flight = {entry, name, deadline:performance.now() + deadlineMs, controller:new AbortController(), cancelled:false};
    assetFlights.add(flight);
    flight.promise = new Promise(resolve => { flight.resolve = resolve; });
    flight.run = async () => {
      let bytes = null, failure;
      const deadline = flight.deadline;
      if (!assetName(name)) failure = "invalid asset name";
      else if (devAssets() instanceof Map) {
        bytes = devAssets().get(`assets/${name}`)?.bytes ?? null;
        if (bytes && bytes.length > byteLimit) { bytes = null; failure = "exceeds 64 MiB"; }
      }
      else for (let attempt = 0; attempt < 3 && !flight.cancelled; attempt++) {
        const left = deadline - performance.now();
        if (left <= 0) { failure = "fetch deadline exceeded"; break; }
        const controller = flight.controller = new AbortController();
        const timer = setTimeout(() => controller.abort(), Math.min(attemptMs, left));
        let retry = true;
        try {
          const path = name.split("/").map(encodeURIComponent).join("/");
          const response = await fetch(new URL(`./assets/${path}`, baseURI()), {signal:controller.signal});
          if (response.status === 404) { failure = undefined; break; }
          if (!response.ok) {
            retry = response.status >= 500;
            throw new Error(`HTTP ${response.status}`);
          }
          const oversized = async reader => {
            retry = false;
            await reader?.cancel();
            controller.abort();
            throw new Error("exceeds 64 MiB");
          };
          if (Number(response.headers?.get("content-length")) > byteLimit) await oversized(response.body?.getReader());
          const reader = response.body?.getReader(), chunks = [];
          let length = 0;
          if (reader) {
            try {
              for (;;) {
                const {done, value} = await reader.read();
                if (done) break;
                if (value.byteLength > byteLimit - length) await oversized(reader);
                length += value.byteLength; chunks.push(value);
              }
            } finally { reader.releaseLock(); }
          }
          bytes = new Uint8Array(length);
          let offset = 0;
          for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.byteLength; }
          failure = undefined; break;
        } catch (error) { failure = !retry ? String(error.message ?? error) : controller.signal.aborted ? "fetch deadline exceeded" : String(error.message ?? error); }
        finally { clearTimeout(timer); }
        if (!retry || attempt === 2 || flight.cancelled) break;
        const backoff = flight.controller = new AbortController();
        await new Promise(resolve => {
          const timer = setTimeout(done, Math.min(backoffMs * (attempt + 1), Math.max(0, deadline - performance.now())));
          function done() { clearTimeout(timer); backoff.signal.removeEventListener("abort", done); resolve(); }
          backoff.signal.addEventListener("abort", done, {once:true});
        });
      }
      if (flight.cancelled || getModule() !== module || live(entry.view) !== entry || entry.id !== id) return;
      const ok = failure ? module.gpu_asset_failed(id, name, failure) : module.gpu_asset(id, name, bytes);
      const error = ok ? undefined : module.gpu_error();
      if (error) console.error("exact gpu:", error);
      delivered(entry, module, error);
    };
  }
  pumpAssets();
}
async function settled(entries) {
  const deadline = performance.now() + deadlineMs;
  for (let round = 0; round < 16; round++) {
    for (const entry of entries()) assets(entry);
    if (!assetFlights.size) {
      return [];
    }
    const left = deadline - performance.now();
    if (left <= 0) break;
    let timer;
    const done = await Promise.race([
      Promise.all([...assetFlights].map(f => f.promise)).then(() => true),
      new Promise(resolve => { timer = setTimeout(() => resolve(false), left); }),
    ]);
    clearTimeout(timer);
    if (!done) break;
  }
  return [...assetFlights].map(f => ({name:f.name, canvas:f.entry.view}));
}

return {assets, cancelAssets, settled, assetFlights};
}
