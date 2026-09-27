// Native ordered HTTP defaults to 64 MiB; independent requests may lower it.
export async function boundedHttpBody(response, limit) {
  if (limit == null) limit = 64 * 1024 * 1024;
  if (!Number.isInteger(limit) || limit < 1 || limit > 64 * 1024 * 1024) throw Error("invalid HTTP response limit");
  if (!response.body) return new Uint8Array();
  const reader=response.body.getReader(), chunks=[]; let size=0;
  try { for (;;) { const {done,value}=await reader.read(); if(done) break; size+=value.length; if(size>limit) throw Error("HTTP response exceeds limit"); if(value.length) chunks.push(value); } }
  catch(error) { await reader.cancel().catch(()=>{}); throw error; } finally { reader.releaseLock(); }
  const bytes=new Uint8Array(size); let at=0; for(const chunk of chunks) { bytes.set(chunk,at); at+=chunk.length; } return bytes;
}

export async function waitForInflight(inflight, deadline) {
  while (inflight.size) {
    const remaining = deadline - performance.now();
    if (remaining <= 0) return false;
    let timer;
    const completed = await Promise.race([
      Promise.race([...inflight]).then(() => true),
      new Promise((resolve) => { timer = setTimeout(() => resolve(false), remaining); }),
    ]);
    clearTimeout(timer);
    if (!completed) return false;
  }
  return true;
}

// Network and page-module requests share admission and the byte ceiling.
// Called after the enclosing batch, so even an immediate refusal cannot re-enter it.
export async function request(op, { grants, granted, loadPageNative, moduleLoader, localAssetURL, controllers, active = () => true }) {
  const encoder = new TextEncoder();
  const failed = (kind, message) => ({ kind, status: 0, headers: '', body: encoder.encode(String(message?.message ?? message)) });
  const { method, url, headers, body, cache } = op;
  if (!active()) return failed(4, 'request source unloaded');
  const native = url === 'exact-native:';
  const scopeValid = op.scope == null || typeof op.scope === 'string' && op.scope.split('\n').map(s => s.trim()).filter(Boolean).every(s => grants.map(g => g.trim()).includes(s));
  const asset = method === 'GET' && !body && Object.keys(headers).length === 0 && /^\/assets\/(?:[A-Za-z0-9_-]+\/)*[A-Za-z0-9_-]+\.[A-Za-z0-9]+$/.test(url);
  if (!scopeValid || !native && !asset && (!granted(url) || !granted(url, op.scope))) return failed(2, `refused by grant: ${url}`);
  if (op.nativeHttp === 'independent' && (!Number.isInteger(op.maxResponseBytes) || op.maxResponseBytes < 1 || op.maxResponseBytes > 64 * 1024 * 1024)) return failed(2, 'invalid independent HTTP response limit');
  if (native) {
    let response;
    try {
      const module = await loadPageNative();
      if (!active()) return failed(4, 'request source unloaded');
      response = new Response(await module.later(body));
    }
    catch (error) { response = new Response(String(error?.message ?? error), { status: 500 }); }
    try { return { kind: 0, status: response.status, headers: '', body: await boundedHttpBody(response, op.maxResponseBytes) }; }
    catch (error) { return failed(2, error); }
  }
  let decodedBody;
  try { if (body) decodedBody = Uint8Array.from(atob(body), c => c.charCodeAt(0)); }
  catch (error) { return failed(4, `invalid request body: ${error}`); }
  const controller = new AbortController();
  controllers.add(controller);
  const init = { method, headers, redirect: 'error', cache: cache === 'reload' ? 'reload' : 'default', signal: controller.signal };
  if (decodedBody) init.body = decodedBody;
  try {
    const response = await (!asset && moduleLoader?.claim?.(url, init) || fetch(asset ? localAssetURL(url) : url, init));
    return { kind: 0, status: response.status, headers: [...response.headers].map(([k, v]) => `${k}: ${v}`).join('\n'), body: await boundedHttpBody(response, op.maxResponseBytes) };
  } catch (error) { return failed(controller.signal.aborted ? 4 : 1, error); }
  finally { controllers.delete(controller); }
}

if (globalThis.exact) globalThis.exact.httpHelpers = { boundedHttpBody, waitForInflight, request };
