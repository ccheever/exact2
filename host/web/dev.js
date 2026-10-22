// The dev connection owns complete, verified generations (LLP 1023 D3).
// A new epoch resets ordering; each commit replaces the entire asset roster.
const es = typeof EventSource === "undefined" ? null : new EventSource("/__dev");
const encoder = new TextEncoder();
let overlay = null;
const errors = new Map();
function show(message, source = "app") {
  if (message) errors.set(source, message); else errors.delete(source);
  const text = [...errors.values()].join("\n\n");
  if (!text) { overlay?.remove(); overlay = null; return; }
  overlay ??= document.body.appendChild(Object.assign(document.createElement("pre"), { style: "position:fixed;left:0;right:0;bottom:0;margin:0;padding:12px;background:#300;color:#fdd;font:12px/1.4 ui-monospace,monospace;white-space:pre-wrap;z-index:2147483647" }));
  overlay.textContent = text + "\n(click to dismiss)";
  overlay.onclick = () => { errors.clear(); show(null); };
}

// SHA-256 also works on trusted-LAN HTTP pages where SubtleCrypto is absent.
// The constants are the fractional square/cube roots of the first primes.
// All integer additions below are reduced modulo 2^32.
let hashConstants;
async function digest(bytes) {
  if (globalThis.crypto?.subtle) return Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)), (n) => n.toString(16).padStart(2, "0")).join("");
  if (!hashConstants) {
    const primes = [];
    for (let n = 2; primes.length < 64; n++) if (primes.every((p) => n % p)) primes.push(n);
    hashConstants = { initial: primes.slice(0, 8).map((n) => (Math.sqrt(n) % 1 * 0x100000000) | 0),
      round: primes.map((n) => (Math.cbrt(n) % 1 * 0x100000000) | 0) };
  }
  const hash = hashConstants.initial.slice(), words = new Int32Array(64);
  const padded = new Uint8Array(Math.ceil((bytes.length + 9) / 64) * 64);
  padded.set(bytes); padded[bytes.length] = 128;
  const view = new DataView(padded.buffer);
  view.setUint32(padded.length - 8, Math.floor(bytes.length / 0x20000000));
  view.setUint32(padded.length - 4, bytes.length * 8);
  const rotate = (n, by) => n >>> by | n << (32 - by);
  for (let start = 0; start < padded.length; start += 64) {
    for (let i = 0; i < 16; i++) words[i] = view.getInt32(start + i * 4);
    for (let i = 16; i < 64; i++) {
      const a = words[i - 15], b = words[i - 2];
      words[i] = words[i - 16] + (rotate(a, 7) ^ rotate(a, 18) ^ a >>> 3) + words[i - 7] + (rotate(b, 17) ^ rotate(b, 19) ^ b >>> 10);
    }
    let [a, b, c, d, e, f, g, h] = hash;
    for (let i = 0; i < 64; i++) {
      const t1 = (h + (rotate(e, 6) ^ rotate(e, 11) ^ rotate(e, 25)) + (e & f ^ ~e & g) + hashConstants.round[i] + words[i]) | 0;
      const t2 = ((rotate(a, 2) ^ rotate(a, 13) ^ rotate(a, 22)) + (a & b ^ a & c ^ b & c)) | 0;
      h = g; g = f; f = e; e = d + t1 | 0; d = c; c = b; b = a; a = t1 + t2 | 0;
    }
    [a, b, c, d, e, f, g, h].forEach((n, i) => { hash[i] = hash[i] + n | 0; });
  }
  return hash.map((n) => (n >>> 0).toString(16).padStart(8, "0")).join("");
}
const validIdentity = (m) => m && /^[0-9a-f]{32}$/.test(m.epoch) && Number.isSafeInteger(m.seq) && m.seq >= 0 && /^[0-9a-f]{64}$/.test(m.generation);
const utf8Compare = (a, b) => {
  const aa = encoder.encode(a), bb = encoder.encode(b);
  for (let i = 0; i < Math.min(aa.length, bb.length); i++) if (aa[i] !== bb[i]) return aa[i] - bb[i];
  return aa.length - bb.length;
};
// The trees a generation carries (serve.mjs `listAssets`), entry plans (`plans/`) among them.
const validName = (name) => typeof name === "string" && /^(assets|deck|shaders|plans)\//.test(name)
  && !name.includes("\\") && !name.includes(":") && !name.includes("\0")
  && name.split("/").every((part) => part && part !== "." && part !== "..");
const sameOrigin = (path, base) => {
  const url = new URL(path, base);
  if (url.origin !== location.origin || !["http:", "https:"].includes(url.protocol)) throw new Error("the dev generation leaves the app origin");
  return url.href;
};
function sameModule(a, b) {
  if (!a || !b) return a === b;
  return Object.keys(a).length === Object.keys(b).length && Object.keys(a).every(key =>
    a[key].length === b[key]?.length && a[key].every((byte, index) => byte === b[key][index]));
}
async function readBounded(url, limit, signal) {
  const response = await fetch(url, { cache: "no-store", signal });
  sameOrigin(response.url || url, location.href);
  if (!response.ok) throw new Error(`generation fetch HTTP ${response.status}: ${url}`);
  if (Number(response.headers.get("content-length")) > limit) throw new Error("generation response exceeds its declared size");
  const reader = response.body.getReader(), chunks = [];
  let length = 0;
  try {
    for (;;) {
      const { value, done } = await reader.read();
      if (done) break;
      length += value.length;
      if (length > limit) throw new Error("generation response exceeds its declared size");
      chunks.push(value);
    }
  } catch (error) { await reader.cancel(); throw error; }
  const bytes = new Uint8Array(length);
  let at = 0; for (const chunk of chunks) { bytes.set(chunk, at); at += chunk.length; }
  return { bytes, url: response.url || url };
}
async function readEnvelope(url, signal) {
  const response = await readBounded(sameOrigin(url, location.href), 64 * 1024, signal);
  return { envelope: JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(response.bytes)), url: response.url };
}
async function fetchGeneration(message, signal, first = false) {
  const started = performance.now();
  const { envelope, url } = await readEnvelope(message.envelope, signal);
  const identity = envelope.dev;
  if (envelope.exact !== 1 || !validIdentity(identity) || identity.epoch !== message.epoch
      || identity.seq !== message.seq || identity.generation !== message.generation || identity.program !== message.program || !Array.isArray(envelope.assets)) throw new Error("the fetched envelope does not name the announced generation");
  // The page's first generation is fetched inside glue's boot, compat known.
  if (!first) await globalThis.exact.ready;
  if (envelope.app?.id !== globalThis.exact.compat?.inputs?.app) throw new Error("the dev generation names another app");
  const assets = new Map(), names = new Set();
  const load = async (card, name) => {
    if (!card || !Number.isSafeInteger(card.bytes) || card.bytes < 0 || card.bytes > 64 * 1024 * 1024 || !/^[0-9a-f]{64}$/.test(card.sha256)) throw new Error(`invalid payload card: ${name}`);
    const path = sameOrigin(card.url, url);
    const { bytes } = await readBounded(path, card.bytes, signal);
    if (bytes.length !== card.bytes || await digest(bytes) !== card.sha256) throw new Error(`payload integrity failed: ${name}`);
    return { bytes, url: path, sha256: card.sha256 };
  };
  for (const card of envelope.assets) {
    if (!validName(card.name) || names.has(card.name)) throw new Error("invalid or duplicate asset name");
    names.add(card.name);
  }
  const moduleKeys = ['native', 'receipt', 'web'];
  if (envelope.module !== undefined && (!envelope.module || Object.keys(envelope.module).sort().join(',') !== moduleKeys.join(','))) throw new Error('invalid module manifest');
  const moduleCards = envelope.module ? moduleKeys.map(key => envelope.module[key]) : [];
  const rust = envelope.rust;
  if (rust && (!rust.wasm || Object.keys(rust).some(k => !["wasm", "native", "tiered"].includes(k)))) throw new Error("invalid Rust manifest");
  const rustCards = rust ? Object.values(rust).flatMap(v => [v.receipt, v.module]) : [];
  const cards = [envelope.plan, ...envelope.assets, ...moduleCards, ...rustCards];
  if (cards.some((card) => !Number.isSafeInteger(card?.bytes) || card.bytes < 0 || card.bytes > 64 * 1024 * 1024)
      || cards.reduce((sum, card) => sum + card.bytes, 0) > 256 * 1024 * 1024) throw new Error("generation exceeds the payload budget");
  // All admitted payloads are independent. One bounded queue avoids separate
  // network rounds for the plan, assets and module without raising concurrency.
  const jobs = [[envelope.plan, "app.plan"], ...envelope.assets.map(card => [card, card.name]),
    ...(envelope.module ? ['receipt', 'web'].map(key => [envelope.module[key], `module ${key}`]) : []),
    ...(rust ? [[rust.wasm.receipt, 'Rust receipt'], [rust.wasm.module, 'Rust Wasm']] : [])];
  const loaded = new Array(jobs.length);
  let next = 0;
  await Promise.all(Array.from({ length: Math.min(4, jobs.length) }, async () => {
    while (next < jobs.length) {
      const index = next++;
      loaded[index] = await load(...jobs[index]);
    }
  }));
  const plan = loaded[0];
  envelope.assets.forEach((card, index) => assets.set(card.name, loaded[index + 1]));
  const moduleAt = 1 + envelope.assets.length;
  const module = envelope.module ? { receipt: loaded[moduleAt].bytes, script: loaded[moduleAt + 1].bytes } : null;
  const rustPayload = rust ? { receipt: loaded.at(-2).bytes, module: loaded.at(-1).bytes } : null;
  const canonical = { assets: [...envelope.assets].sort((a, b) => utf8Compare(a.name, b.name)).map((a) => ({ bytes: a.bytes, name: a.name, sha256: a.sha256 })),
    ...(envelope.module ? { module: Object.fromEntries(moduleKeys.map(key => [key, { bytes: envelope.module[key].bytes, sha256: envelope.module[key].sha256 }])) } : {}),
    plan: { bytes: plan.bytes.length, sha256: plan.sha256 },
    ...(rust ? { rust: Object.fromEntries(Object.keys(rust).sort().map(k => [k, { module: {bytes:rust[k].module.bytes,sha256:rust[k].module.sha256}, receipt:{bytes:rust[k].receipt.bytes,sha256:rust[k].receipt.sha256}, ...(k !== "wasm" ? {target:rust[k].target} : {}) }])) } : {}) };
  if (await digest(encoder.encode(JSON.stringify(canonical))) !== identity.generation) throw new Error("the generation digest does not bind its complete manifest");
  return { ...identity, surfaceAssets:message.surfaceAssets, plan: plan.bytes, planSha256: plan.sha256, assets, module, rust: rustPayload, fetchMs: performance.now() - started };
}

// Fetches may finish in any order. Only the latest request can enter the
// host's acceptance/commit operation; a failed candidate keeps the live one.
function generationClient({ fetchGeneration, apply, applied = () => {}, failed = () => {}, programChanged = () => {} }) {
  let epoch = null, queued = -1, committed = null, attempt = 0, inFlight = null, controller = null;
  const retired = new Set();
  let program = null, stopped = false;
  const stop = () => { stopped = true; attempt++; controller?.abort(); inFlight = null; programChanged(); };
  return {
    // The page booted this generation itself (its first boot): never again.
    adopt(generation) { committed = generation; },
    receive(message) {
      if (stopped) return Promise.resolve(false);
      if (message?.rebuilt) { stop(); return Promise.resolve(false); }
      if (!validIdentity(message) || retired.has(message.epoch)) return Promise.resolve(false);
      if (message.program !== undefined && !/^[0-9a-f]{64}$/.test(message.program)) return Promise.resolve(false);
      if (program && program !== message.program) { stop(); return Promise.resolve(false); }
      if (message.program) program = message.program;
      if (epoch !== message.epoch) { if (epoch !== null) retired.add(epoch); epoch = message.epoch; queued = -1; attempt++; }
      if (message.seq < queued || message.seq === queued && inFlight) return Promise.resolve(false);
      queued = message.seq;
      controller?.abort();
      controller = new AbortController();
      const signal = controller.signal;
      const request = ++attempt, current = () => request === attempt;
      if (committed === message.generation) { inFlight = null; return Promise.resolve(false); }
      const activeController = controller;
      const timeout = setTimeout(() => activeController.abort(), 15000);
      const pending = (async () => {
        try {
          const candidate = await fetchGeneration(message, signal);
          if (!current() || committed === candidate.generation) return false;
          let accepted;
          try { accepted = await apply(candidate, current); }
          catch (error) { throw Object.assign(new Error(String(error)), { hostRefused: true }); }
          if (!current()) return false;
          if (!accepted) throw Object.assign(new Error("the host refused the current generation"), { hostRefused: true });
          committed = candidate.generation;
          applied(candidate);
          return true;
        } catch (error) { if (current()) { failed(error, current); } return false; }
        finally { clearTimeout(timeout); if (current()) inFlight = null; }
      })();
      inFlight = pending;
      return pending;
    },
  };
}

if (!es) globalThis.exactDevProtocol = { digest, generationClient, validIdentity };
if (es) {
  globalThis.exact.devError = show;
  // Host affordance, outside the app tree and absent from static/production pages.
  // Agent input targets the app tree; this host-only link covers app controls.
  // It covers whatever the app puts in that corner (a phone layout's tab bar),
  // so it can be put away; the choice is kept for this origin.
  const hidden = 'exact.dev.open.hidden';
  if (!new URLSearchParams(location.search).has('agent') && localStorage.getItem(hidden) !== '1') {
    const bar = document.body.appendChild(document.createElement('div'));
    bar.style = 'position:fixed;right:10px;bottom:10px;display:flex;align-items:center;background:#fff;border:1px solid #ccd3df;border-radius:8px;font:13px system-ui;z-index:2147483646';
    const opening = bar.appendChild(document.createElement('a'));
    opening.href = '/__dev/open' + location.search + location.hash;
    opening.textContent = 'Open in native…';
    opening.style = 'padding:8px 4px 8px 12px;color:#164bc4';
    const close = bar.appendChild(document.createElement('button'));
    close.textContent = '×';
    close.title = 'Hide (clear site data to bring it back)';
    close.setAttribute('aria-label', 'Hide the Open in native link');
    close.style = 'border:0;background:none;padding:8px 10px;font:16px system-ui;color:#5b6475;cursor:pointer';
    close.onclick = () => { localStorage.setItem(hidden, '1'); bar.remove(); };
  }
  // The module loader uses this implementation on trusted LAN HTTP too.
  globalThis.exact.moduleDigest = digest;
  let retry = null, activeGeneration = null;
  // Inspect mode's outline, hidden on a new generation (set below).
  let outlineNone = () => {};
  const client = generationClient({ fetchGeneration, programChanged: () => { clearTimeout(retry); location.reload(); },
    apply: async (candidate, current) => {
      if (!current()) return false;
      const t = performance.now();
      const names = candidate.surfaceAssets;
      const liveAssets = Array.isArray(names) && names.length && activeGeneration && sameModule(candidate.module, activeGeneration.module) && sameModule(candidate.rust, activeGeneration.rust)
        && candidate.planSha256 === activeGeneration.planSha256 && candidate.assets.size === activeGeneration.assets.size
        && [...candidate.assets].every(([name, card]) => names.includes(name) || card.sha256 === activeGeneration.assets.get(name)?.sha256);
      // Hold commands received during a swap belong to the new generation.
      swaps += 1;
      let accepted = false;
      try {
        accepted = liveAssets && globalThis.exact.gpu?.reloadAssets
          ? await globalThis.exact.gpu.reloadAssets(candidate.assets, names, current)
          : await globalThis.exact.reloadGeneration(candidate.plan, candidate.assets, current, candidate.module, candidate.rust, true);
      } finally { swaps -= 1; if (!accepted) rehold(); }
      if (accepted) activeGeneration = candidate;
      if (accepted) {
        navigator.sendBeacon(`/__dev/reloaded?epoch=${candidate.epoch}&seq=${candidate.seq}&dom=${Date.now()}&fetch=${candidate.fetchMs.toFixed(1)}&boot=${(performance.now() - t).toFixed(1)}`);
        requestAnimationFrame(() => navigator.sendBeacon(`/__dev/painted?epoch=${candidate.epoch}&seq=${candidate.seq}&paint=${Date.now()}`));
      }
      return accepted;
    },
    // A new generation runs its plan's own design: the live one since, if any, comes after.
    // And the pin held, set again on its runner; its status says the new generation,
    // and the design its runner runs (read from it: /__dev/design is 204 when the
    // plan's own is the one to run, so no live answer would say it).
    // An inspect outline drawn on the old generation's boxes goes.
    applied: (candidate) => { clearTimeout(retry); if (stillUntil) stillEnds(); show(null); generations += 1; outlineNone(); delete globalThis.exact.designDigest; syncDesign(); channel.plan = candidate?.generation ?? channel.plan; knownPlan(candidate?.planSha256); if (channel.held) command(channel.held); else if (channel.nonce) status(); rehold(); },
    failed: (error, current) => {
      show(String(error)); console.error("exact dev:", String(error));
      clearTimeout(retry);
      // A deterministic host refusal needs a new edit, not repeated execution
      // of the same candidate every 250 ms. Network discovery still retries.
      if (error.hostRefused) return;
      const discover = async () => {
        if (!current()) return;
        try {
          const { envelope, url } = await readEnvelope("/exact.json", AbortSignal.timeout(15000));
          if (current() && validIdentity(envelope.dev)) client.receive({ ...envelope.dev, surfaceAssets:activeGeneration?.surfaceAssets, envelope: new URL("exact.json", new URL(envelope.plan.url, url)).href });
        } catch { if (current()) retry = setTimeout(discover, 1000); }
      };
      retry = setTimeout(discover, 250);
    },
  });
  // The first boot's generation, named in the page (LLP 1007 §6): fetched
  // when glue.js asks. Its Rust module, if any, is still applied as an update.
  const firstMeta = document.querySelector('meta[name="exact-dev-generation"]');
  if (firstMeta) {
    const slot = globalThis.exactDevFirst ??= {};
    slot.provide = async () => {
      try {
        const candidate = await fetchGeneration(JSON.parse(firstMeta.content), AbortSignal.timeout(15000), true);
        if (!candidate.rust) { client.adopt(candidate.generation); activeGeneration = candidate; }
        return candidate;
      } catch (error) { show(String(error)); console.error("exact dev:", String(error)); return null; }
    };
    slot.ready?.();
  }
  // The live table (LLP 1121 stage 4): a batch applies on its base; a stale
  // build asks for the snapshot. A frame before boot waits for it, the
  // latest only (a snapshot carries the whole design).
  let designing = Promise.resolve();
  // A scrub's status, taken once its drag rests.
  let statusLater = null;
  // Applied in place (the wasm's `exact_design_rows`): its batch is the
  // page's only when `live` says applied.
  // `still()`, after every wait, says whether the frame is still this page's.
  const apply = async (frame, still = () => true) => {
    const x = globalThis.exact;
    await x.ready;
    if (!still()) return { live: "structure" };
    if (typeof x.wasm?.exact_design_rows !== "function") return { live: "unsupported" };
    const n = x.wasm.exact_design_rows(x.writeIn(JSON.stringify(frame))), out = JSON.parse(x.readOut(n));
    // A scrub's frames say the design once the drag rests (a status reads the
    // root's computed style and posts, each frame otherwise), the commit's
    // own report with it; one the runner dropped (an older frame) moves
    // nothing, and leaves the rest as it was.
    const moves = typeof out.design === "string" && out.design !== x.designDigest;
    if (out.live === "applied") {
      if (frame.scrub && moves) { clearTimeout(statusLater); statusLater = setTimeout(status, 150); scrubSend = true; try { unmoving(() => x.send(n)); } finally { scrubSend = false; } }
      else x.send(n);
    }
    if (moves) { x.designDigest = out.design; if (!frame.scrub) status(); }
    if (out.unsupported) { console.warn("exact dev: the wide colour table is full: a colour was refused, its rows keep their value"); report({ kind: "unsupported", what: "batch", detail: "the wide colour table is full: a colour was refused" }); }
    return out;
  };
  // A scrub's frame (LLP 1121 D13) shows with no transition, as the tool
  // drags it: every transition off while its rows are set and styled, then
  // back on, with nothing left to move.
  const unmoving = (apply) => {
    const root = document.documentElement;
    if (!document.getElementById("x-scrub-still")) {
      const sheet = document.createElement("style");
      sheet.setAttribute("id", "x-scrub-still");
      sheet.textContent = "[data-x-scrub],[data-x-scrub] *,[data-x-scrub] *::before,[data-x-scrub] *::after{transition:none!important}";
      document.head.append(sheet);
    }
    // Off through the drag (one style change of the whole page as it starts
    // and one as it rests, not two each frame), its last values styled
    // before transitions come back.
    if (!root.hasAttribute("data-x-scrub")) root.setAttribute("data-x-scrub", "");
    try { apply(); } finally {
      clearTimeout(stillUntil);
      stillUntil = setTimeout(stillEnds, 150);
    }
  };
  let stillUntil = null, scrubSend = false;
  const stillEnds = () => { clearTimeout(stillUntil); stillUntil = null; void document.documentElement.offsetWidth; document.documentElement.removeAttribute("data-x-scrub"); };
  // One at a time, in the stream's order: a snapshot fetched is never older
  // than a batch applied before it.
  // A snapshot asked for under one generation is that generation's: one
  // that lands after the next is dropped (that one asks again).
  let generations = 0;
  const snapshot = async () => {
    const asked = generations, still = () => asked === generations, response = await fetch("/__dev/design", { cache: "no-store" });
    if (response.status !== 200 || !still()) return;
    const frame = await response.json();
    if (still()) await apply(frame, still);
  };
  const syncDesign = () => { designing = designing.then(snapshot).catch((error) => console.error("exact dev:", String(error))); };
  const design = (frame) => {
    designing = designing.then(async () => {
      const out = await apply(frame);
      // A batch for another structure: a generation is on its way, which
      // syncs when applied.
      if (out.live === "stale") await snapshot();
      else if (out.live.startsWith("refused")) console.error("exact dev:", out.live);
    }).catch((error) => console.error("exact dev:", String(error)));
  };
  // The build channel (LLP 1122 D4): this page registers with its stream's
  // nonce, reports its effective facts, and takes `pin` and `inspect` from
  // the design server's clients; the other commands are reserved, and
  // answered unsupported. Every report waits for the stream's registration.
  const channel = { nonce: null, registered: null, revision: null, before: null, held: null, hold: null, holdLatest: null, plan: null, told: null };
  let applying = null;
  // The tab's own id, kept across its reloads (sessionStorage); a duplicated
  // tab, which copies that storage, takes another when this one answers.
  // A copy the design tool frames (LLP 1122 T7: same-origin frames share the
  // tab's storage) keeps its own under the frame's name, `exact-copy-<n>`,
  // which the page only reads; any other name is the app's, left alone.
  const framed = globalThis.parent != null && globalThis.parent !== globalThis, copy = framed && /^exact-copy-\d{1,2}$/.test(globalThis.name ?? ""), buildKey = copy ? `exact.dev.build:${globalThis.name}` : "exact.dev.build";
  const buildId = (async () => {
    const fresh = () => `web-${crypto.getRandomValues(new Uint32Array(2)).join("")}`;
    let id;
    try { id = sessionStorage.getItem(buildKey); } catch { return fresh(); }
    const tabs = typeof BroadcastChannel === "function" ? new BroadcastChannel("exact.dev.build") : null;
    if (id && tabs) {
      const mine = id;
      const taken = await new Promise((done) => { tabs.onmessage = (e) => { if (e.data === `taken:${mine}`) done(true); }; tabs.postMessage(`who:${mine}`); setTimeout(() => done(false), 60); });
      if (taken) id = null;
    }
    id ??= fresh();
    // Answered for any id this tab holds, one it just made included: a tab
    // duplicated from it copies that too.
    if (tabs) { const own = id; tabs.onmessage = (e) => { if (e.data === `who:${own}`) tabs.postMessage(`taken:${own}`); }; }
    try { sessionStorage.setItem(buildKey, id); } catch { /* storage refused */ }
    return id;
  })();
  // A report gives up after 10 s; a registration waits as long as it takes
  // (its answer is what every report on the stream waits for).
  const post = (nonce, body, ms) => fetch("/__dev/report", { method: "POST", body: JSON.stringify({ nonce, ...body }), ...(ms ? { signal: AbortSignal.timeout(ms) } : {}) });
  // A report goes on the stream it was made on, once that stream's
  // registration answers: one that a new stream (a reconnect) replaced
  // meanwhile is dropped. Reports go one at a time, in the order made, so a
  // refusal never crosses the status after it (the server binds a refusal
  // to the frame's status). A new stream starts its own queue, and a post
  // gives up after 10 s, so one stuck request holds up nothing for long.
  let reports = { nonce: null, tail: Promise.resolve() };
  const report = (body) => {
    const { nonce, registered } = channel;
    if (!nonce) return Promise.resolve();
    if (reports.nonce !== nonce) reports = { nonce, tail: Promise.resolve() };
    const queue = reports;
    return (queue.tail = queue.tail.then(async () => {
      if (!(await registered) || channel.nonce !== nonce || channel.registered !== registered) return;
      await post(nonce, body, 10_000).catch(() => {});
    }));
  };
  const wasmCall = async (name, ...args) => { const x = globalThis.exact; await x.ready; if (typeof x.wasm?.[name] !== "function") return null; const n = x.wasm[name](...args), out = JSON.parse(x.readOut(n)); x.send(n); return out; };
  const modes = async () => { const x = globalThis.exact; await x.ready; return x.wasm?.exact_set_mode ? JSON.parse(x.readOut(x.wasm.exact_set_mode(x.writeIn("?")))).modes ?? null : null; };
  // The design this build runs: the runner's, said beside any answer (a
  // batch for no structure changes nothing).
  const designNow = async () => { const x = globalThis.exact; await x.ready; if (typeof x.wasm?.exact_design_rows !== "function") return null; const out = JSON.parse(x.readOut(x.wasm.exact_design_rows(x.writeIn(JSON.stringify({ kind: "tamber-batch", structure: "", base: "", design: "", tokens: [] })))));  return out.design ?? null; };
  // The root's own size, the app's `setRootFontSize` rule set aside (navigation.js `beneathApp`).
  const hostSize = () => { const rule = document.getElementById("exact-root-font-size"); if (rule) rule.disabled = true; try { return parseFloat(getComputedStyle(document.documentElement).fontSize) || 16; } finally { if (rule) rule.disabled = false; } };
  // The facts as they paint: the root's `color-scheme` (the app's
  // `setScheme` over the pin's, over the browser's `light dark`), resolved by
  // the system's scheme, which is a pin's while one is held.
  // The digest of the plan this page runs, which its source map names
  // (tamber/1.8): read from the runner once per generation.
  // (None said is kept too: asked again only with the next generation. A
  // generation this page verified says its plan's digest itself.)
  let planDigestOf, planDigestNow = null;
  function knownPlan(sha256) { if (typeof sha256 === "string") { planDigestNow = sha256; planDigestOf = globalThis.exact.generation ?? null; } }
  const planDigest = async () => {
    const x = globalThis.exact;
    await x.ready;
    if (planDigestOf !== (x.generation ?? null)) {
      try { const sites = typeof x.wasm?.exact_agent === "function" ? ask({ op: "sites" }) : null; planDigestNow = typeof sites?.planDigest === "string" ? sites.planDigest : null; } catch { planDigestNow = null; }
      planDigestOf = x.generation ?? null;
    }
    return planDigestNow;
  };
  const facts = async () => {
    const rootStyle = getComputedStyle(document.documentElement), cs = rootStyle.colorScheme;
    const scheme = /dark/.test(cs) && !/light/.test(cs) ? "dark" : /light/.test(cs) && !/dark/.test(cs) ? "light" : globalThis.exact.systemScheme ?? (matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light");
    const contrast = matchMedia("(prefers-contrast: more)").matches ? "more" : matchMedia("(prefers-contrast: less)").matches ? "less" : "no-preference";
    return { kind: "status", scheme, contrast, mode: await modes(), rootFontSize: parseFloat(rootStyle.fontSize), plan: channel.plan, planDigest: await planDigest(), design: globalThis.exact.designDigest ?? await designNow(), revision: channel.revision, location: here(), app: appAsked() ?? undefined };
  };
  // In a copy the design tool pins (LLP 1122 T7, theme2's F2 ruling), the
  // pin holds over the app's own setScheme, setMode and setRootFontSize for
  // the facts it pins (elsewhere the app's own wins, D3): the runner keeps
  // what the app asks of a held fact, applies none of it, and applies its
  // latest when the pin is handed back (`exact_hold_facts`). What the app
  // asked against the pin is said beside the status (`app`).
  let asked = {};
  function appAsked() {
    const p = channel.held?.pin;
    if (!copy || !p) return null;
    const out = {};
    if (p.scheme && asked.scheme != null && asked.scheme !== p.scheme) out.scheme = String(asked.scheme).slice(0, 32);
    if (p.rootFontSize && asked.rootFontSize != null && asked.rootFontSize !== p.rootFontSize) out.rootFontSize = String(asked.rootFontSize).slice(0, 64);
    // (Within the status's bounds: each axis and value at most 64 characters, the whole at most 1 KiB.)
    const mode = Object.fromEntries(Object.entries(asked.mode ?? {}).filter(([a, v]) => p.mode?.[a] !== undefined && p.mode[a] !== v && a.length <= 64 && String(v).length <= 64));
    if (Object.keys(mode).length && new TextEncoder().encode(JSON.stringify(mode)).length <= 800) out.mode = mode;
    return Object.keys(out).length ? out : null;
  }
  // (A question applies no batch, so asking is never a commit that asks again.)
  const holdFacts = async (text) => {
    const x = globalThis.exact;
    if (!copy) return;
    await x.ready;
    if (typeof x.wasm?.exact_hold_facts !== "function") return;
    const n = x.wasm.exact_hold_facts(x.writeIn(text)), out = JSON.parse(x.readOut(n));
    // (Applied quietly: a refused one's batch is no commit for `committed` to ask after; a taken one says its status.)
    if (text !== "?") { quiet = true; try { x.send(n); } finally { quiet = false; } if (!out?.error) committed(); }
    if (out?.asked) asked = out.asked;
    return out;
  };
  const status = async () => { clearTimeout(statusLater); statusLater = null; const body = await facts(); channel.told = JSON.stringify(body); report(body); };
  // The facts change under the page too, and by the app's own `setScheme`,
  // `setMode` and `setRootFontSize`: after its commits, once a turn, a status
  // when they moved.
  for (const query of ["(prefers-color-scheme: dark)", "(prefers-contrast: more)", "(prefers-contrast: less)"]) matchMedia(query).addEventListener?.("change", () => { if (channel.nonce) status(); });
  let due = false, quiet = false;
  const committed = () => {
    // A scrub frame's own commit is said once the drag rests, with these facts.
    if (due || !channel.nonce || scrubSend || quiet) return;
    due = true;
    setTimeout(() => {
      designing = designing.then(async () => {
        due = false;
        if (copy && channel.releasing) channel.releasing = !!(await holdFacts("-"))?.error;
        if (copy && channel.held) await holdFacts("?");
        const body = await facts();
        if (JSON.stringify(body) !== channel.told) { channel.told = JSON.stringify(body); report(body); }
      }).catch((error) => console.error("exact dev:", String(error)));
    }, 0);
  };
  globalThis.exact.ready?.then?.(() => { globalThis.exact.committed = committed; });
  // Where the page is (*1.14*), said when it moves: the router writes the
  // history, the browser traverses it, and a fragment moves alone. (A
  // location over the server's bound says none.)
  function here() { const at = location.pathname + location.search + location.hash; return at.length <= 2048 && /^\/(?![/\\])[\x21-\x5b\x5d-\x7e]*$/.test(at) ? at : null; }
  const pages = globalThis.history;
  if (pages) for (const name of ["pushState", "replaceState"]) { const write = pages[name]; pages[name] = function (...args) { const out = write.apply(this, args); committed(); return out; }; }
  globalThis.addEventListener?.("popstate", committed);
  globalThis.addEventListener?.("hashchange", committed);
  // `navigate` (LLP 1122 T7, *1.14*): the location routed in place as a link
  // to it is (input-glue.js): the navigation root's `navigate` handler, else
  // the router's own `go`, no reload; a fragment that differs then set on
  // the entry the router wrote. One that is no route says so, by the
  // frame's `seq`.
  const navigate = async (frame) => {
    const x = globalThis.exact;
    await x.ready;
    // A frame of a stream since replaced (a reconnect, another server) is old
    // news: nothing done and nothing said, so its refusal never answers the
    // new stream's navigation of the same seq.
    const mine = () => frame.stream === channel.nonce;
    if (!mine()) return;
    const refuse = (detail) => { if (mine()) report({ kind: "unsupported", what: "navigate", detail: String(detail).slice(0, 1024), revision: frame.seq }); };
    const said = (at) => JSON.stringify(at.length > 200 ? `${at.slice(0, 200)}…` : at);
    try {
      const url = new URL(frame.location, location.origin);
      // (A path that resolves to `//…` is scheme-relative to the history API: no location of this page.)
      if (url.origin !== location.origin || url.pathname.startsWith("//")) return refuse("the location is no path on this page's origin");
      // There once the router's entry is the route (an agent's launch keys
      // ride on its address, history.state.url has none) or the address is
      // it. (The server sends a location spelled as the router writes it.)
      const to = url.pathname + url.search, there = () => pages?.state?.url === to || location.pathname + location.search === to, before = pages?.state?.url;
      if (!there()) {
        if (typeof x.wasm?.exact_route_match !== "function" || x.wasm.exact_route_match(x.writeIn(to)) !== 1) return refuse(`${said(to)} is no route of this app`);
        const nav = document.getElementById("exact-root")?.firstElementChild, heard = nav?.hasAttribute("navigationBack") && nav.exactHandlers?.includes("navigate");
        const batch = heard ? x.navigate(to) : x.go?.(to);
        if (!batch?.ops?.some((op) => op.op === "router")) return refuse(batch?.error ?? `the app's router took no step to ${said(to)}`);
      }
      // (The router writes the history after its commit, at once unless a
      // traversal is under way: the fragment once it is there, on the entry
      // it wrote, its address otherwise kept. One the app's handler sent
      // elsewhere keeps the app's place, no fragment, and no wait.)
      for (let i = 0; i < 100 && !there() && pages?.state?.url === before; i++) await new Promise((ok) => setTimeout(ok, 10));
      if (mine() && there() && url.hash !== location.hash) pages?.replaceState(pages.state, "", location.origin + location.pathname + location.search + url.hash);
    } catch (error) { return refuse(String(error)); }
    committed();
  };
  const pin = async (frame) => {
    const root = document.documentElement.style, p = frame.pin;
    // What it was before the first pin, handed back by `none`; each pin
    // replaces the whole set.
    channel.before ??= { fontSize: root.fontSize, hostFont: root.getPropertyValue("--exact-host-font-size"), mode: await modes() };
    const x = globalThis.exact, pins = (m) => Object.entries(m).map(([a, v]) => `${a}=${v}`).join(",");
    // (A copy's held facts first: the app's own root size gives way before the pinned one is set.)
    // (Facts held before and not now take the app's latest choice; a hold refused for its modes holds the rest.)
    const flags = p ? `${p.scheme ? "s" : ""}${p.rootFontSize ? "r" : ""}` : "";
    // (A copy's pin refused for its modes keeps the axes held before, and sets none.)
    let refusedModes = false;
    if (p) { const held = await holdFacts(`${flags}:${pins(p.mode ?? {})}`); if (held?.error) { refusedModes = true; report({ kind: "unsupported", what: "pin.hold", detail: String(held.error).slice(0, 1024), revision: frame.revision }); await holdFacts(`${flags}:${pins(channel.held?.pin?.mode ?? {})}`); } }
    // The scheme is the system's, beneath the app's `setScheme` (the root's
    // own style): a rule over the page's `light dark`, and the system scheme
    // the runner's `prefers-color-scheme` reads (navigation.js
    // `preferences`), told as the browser's change is.
    const sheet = document.getElementById("exact-dev-pin") ?? document.head.appendChild(Object.assign(document.createElement("style"), { id: "exact-dev-pin" }));
    sheet.textContent = p?.scheme ? `:root{color-scheme:${p.scheme}${copy ? "!important" : ""}}` : "";
    if ((x.systemScheme ?? null) !== (p?.scheme ?? null)) { x.systemScheme = p?.scheme ?? null; dispatchEvent(new Event("resize")); }
    // The root font size as `prefer` sets it (navigation.js): the size an
    // app's percentage is of, too.
    // (In a copy, over the app's own: its `!important` rule gives way to the root's own.)
    root.removeProperty("font-size");
    if (copy && p?.rootFontSize) root.setProperty("font-size", `${p.rootFontSize}px`, "important");
    else root.fontSize = p?.rootFontSize ? `${p.rootFontSize}px` : channel.before.fontSize;
    if (p?.rootFontSize) root.setProperty("--exact-host-font-size", root.fontSize);
    else if (channel.before.hostFont) root.setProperty("--exact-host-font-size", channel.before.hostFont);
    else root.removeProperty("--exact-host-font-size");
    await wasmCall("exact_set_root_font_size", hostSize());
    // (Handed back, only the axes the last pin set go back: the app's own on any other stands.)
    // (In a copy only the pinned axes are set, and an axis the last pin set and this one doesn't goes back; elsewhere a pin is the whole set over the first's snapshot.)
    const last = channel.held?.pin?.mode ?? {}, back = Object.fromEntries(Object.entries(channel.before.mode ?? {}).filter(([a]) => a in last && !(a in (p?.mode ?? {})) && !(copy && asked.mode?.[a] !== undefined)));
    const mode = refusedModes ? "" : pins(p ? (copy ? { ...back, ...(p.mode ?? {}) } : { ...(channel.before.mode ?? {}), ...(p.mode ?? {}) }) : back);
    if (mode) {
      // A mode refused leaves the modes before the first pin, not the last pin's.
      const out = await wasmCall("exact_set_mode", x.writeIn(mode));
      // (In a copy a refused mode leaves the modes as they are.)
      if (out?.error) { report({ kind: "unsupported", what: "pin.mode", detail: out.error, revision: frame.revision }); const back = pins(channel.before.mode ?? {}); if (back && !copy) await wasmCall("exact_set_mode", x.writeIn(back)); }
    }
    // A page cannot override the browser's `prefers-contrast`.
    if (p?.contrast) report({ kind: "unsupported", what: "pin.contrast", detail: "a web page cannot pin the browser's contrast", revision: frame.revision });
    // (Handed back, a copy's held facts are the app's latest own choices.)
    // A hand-back the runner refuses is asked again after each commit until it is taken.
    if (!p) { channel.before = null; const out = await holdFacts("-"); channel.releasing = !!out?.error; if (out?.error) report({ kind: "unsupported", what: "pin.hold", detail: String(out.error).slice(0, 1024), revision: frame.revision }); }
    else channel.releasing = false;
    // (A copy's pin refused for its modes holds the axes held before: what it holds, as hand-back reads it.)
    channel.held = p ? (copy && refusedModes ? { ...frame, pin: { ...p, mode: channel.held?.pin?.mode } } : frame) : null;
    channel.revision = frame.revision;
    status();
  };
  // Inspect mode (LLP 1122 D4, D5; T2b), from a design-server client's
  // `inspect {on}`: a banner with a close control (and Escape) says so; the
  // page takes every input itself (pointer, mouse, touch, wheel, keys,
  // gestures, and guest frames, which stop taking pointer input), so nothing
  // reaches the app; a hovered node is outlined, and a press released where
  // it began (a click, a tap) selects it. The node is the one the browser's
  // hit test finds, as a real tap's listener is the element under it
  // (`data-view`); a hit on the host's own chrome (an error bar, the open-in
  // bar) selects nothing. Its report is the runner's own `layout` answer
  // (`node`): its site, instance path, the modes it is in, and its rows,
  // with the digests the build runs. Development pages only: dev.js is not
  // in a release.
  const INSPECT_EVENTS = ["change", "input", "focus", "blur", "focusout", "compositionend", "cancel", "gotpointercapture", "lostpointercapture", "scroll", "scrollend", "drag", "dragenter", "dragover", "dragleave", "dragend", "pointerdown", "pointerup", "pointercancel", "pointerover", "pointerout", "pointerenter", "pointerleave", "click", "dblclick", "auxclick", "contextmenu", "mousedown", "mouseup", "mouseover", "mouseout", "touchstart", "touchmove", "touchend", "touchcancel", "wheel", "keydown", "keyup", "keypress", "beforeinput", "gesturestart", "gesturechange", "gestureend", "dragstart", "drop"];
  const inspecting = { on: false, banner: null, outline: null, shield: null, cover: null, close: null, selected: null, press: null, seq: null, left: null, session: null };
  // A new generation: the outline drawn on its boxes goes, and the chrome is
  // made again if its tree took it away.
  outlineNone = () => { if (inspecting.outline) inspecting.outline.style.display = "none"; unmark(); if (inspecting.on) chrome(); };
  const REPORT_BYTES = 60 * 1024;
  const ask = (request) => { const x = globalThis.exact; return JSON.parse(x.readOut(x.wasm.exact_agent(x.writeIn(JSON.stringify(request))))); };
  // The node under a point, as a tap there would find it: the topmost
  // element (this inspector's own chrome aside), when it is the app's and
  // not one leaving; nothing when the host's chrome is on top.
  const nodeAt = (x, y) => {
    const root = globalThis.exact.root;
    // A guest frame is hit as itself (its view), the shield lifted for the
    // reading only.
    if (inspecting.shield) inspecting.shield.disabled = true;
    let hit;
    try { hit = document.elementsFromPoint(x, y).find((e) => e !== inspecting.cover && !inspecting.banner?.contains(e)); } finally { if (inspecting.shield) inspecting.shield.disabled = false; }
    if (!hit || !root?.contains(hit) || hit.closest("[data-exiting]")) return null;
    // The view the runner knows by that element, as a tap's listener does.
    for (let view = hit.closest("[data-view]"); view; view = view.parentElement?.closest("[data-view]")) {
      const id = Number(view.dataset.view);
      if (globalThis.exact.views?.get(id) === view) return { id, el: view };
    }
    return null;
  };
  const outline = (el) => {
    if (!inspecting.outline) return;
    if (!el) { inspecting.outline.style.display = "none"; return; }
    const r = el.getBoundingClientRect();
    Object.assign(inspecting.outline.style, { display: "block", left: `${r.left}px`, top: `${r.top}px`, width: `${r.width}px`, height: `${r.height}px` });
  };
  const bytes = (value) => new TextEncoder().encode(JSON.stringify(value)).length;
  // The selection's report: the runner's answer for the node, and the
  // digests and pin revision the build runs; trimmed to the report's room
  // (counted in bytes, as the server does), and said so; refused, named, if
  // even the node's identity does not fit.
  const selectedReport = async (id, click = null) => {
    const x = globalThis.exact;
    await x.ready;
    if (x.stages?.()?.inspection === "staged") return { kind: "unsupported", what: "inspect", detail: "this build's inspection stage is not loaded" };
    const node = ask({ op: "node", id, plan: true });
    if (node.error) return { kind: "unsupported", what: "inspect", detail: node.error };
    const design = x.designDigest ?? await designNow();
    // (Under the inspect command it was made in: one replaced since is refused.)
    // (And a click's token, which the outline answering it names.)
    const envelope = (selected) => ({ kind: "selected", selected, inspect: inspecting.seq ?? undefined, ...(click ? { click } : {}), plan: channel.plan, design, revision: channel.revision });
    let selected = { node: id, type: node.type, testId: node.props?.testId ?? null, site: node.site ?? null, instance: node.instance ?? [], planDigest: node.planDigest ?? null, tuple: node.modes ?? null, rows: Object.entries(node.style ?? {}).map(([name, row]) => ({ name, ...row })), held: x.heldOf?.(x.views?.get(id), ask)?.held ?? null, trimmed: false };
    if (bytes(envelope(selected)) > REPORT_BYTES) selected = { ...selected, rows: selected.rows.map(({ name, value, source, from, fromSite, token }) => ({ name, value, source, from, fromSite, token })), trimmed: true };
    while (bytes(envelope(selected)) > REPORT_BYTES && selected.rows.length) selected = { ...selected, rows: selected.rows.slice(0, Math.floor(selected.rows.length / 2)) };
    if (bytes(envelope(selected)) > REPORT_BYTES) return { kind: "unsupported", what: "inspect", detail: "the selected node's identity alone is larger than a report" };
    return envelope(selected);
  };
  // A selection is its node in a generation: a refresh after another
  // generation finds the node by its site and instance path again, or says
  // it is gone.
  // A click's selection carries a token (`click`), and the outline that
  // answers it names it (tamber/1.7): that one leaves the page where it is,
  // since the copy clicked is under the pointer. A refresh of it re-sends
  // its token, so an answer to either report stays put. A token is this
  // page's inspect session (a random prefix, new each time inspect mode
  // starts) and a counter, so any click of the session is known, however
  // many are on their way.
  let clicks = 0;
  const select = async (id, clicked = false, click = null) => {
    const seq = inspecting.seq;
    if (clicked) click = `${inspecting.session ??= crypto.getRandomValues(new Uint32Array(2)).join("")}.${++clicks}`;
    const report_ = await selectedReport(id, click);
    // Left (or another command came) meanwhile: not published.
    if (!inspecting.on || inspecting.seq !== seq) return;
    if (report_.kind === "selected") inspecting.selected = { id, click, generation: globalThis.exact.generation, plan: report_.selected.planDigest, site: report_.selected.site, instance: JSON.stringify(report_.selected.instance) };
    report(report_);
  };
  const refresh = async () => {
    const was = inspecting.selected;
    if (!was || !inspecting.on) return;
    if (was.generation !== globalThis.exact.generation) {
      // Sites and regions are a plan's own: only under the same plan is the
      // node found again by them.
      let found = null;
      for (const [id] of globalThis.exact.views ?? []) {
        const node = ask({ op: "node", id, plan: true });
        if (node.error || node.planDigest !== was.plan) break;
        if (node.site === was.site && JSON.stringify(node.instance ?? []) === was.instance) {
          // Two copies with the same path (a repeated key): which one is not
          // known, so neither.
          if (found != null) { found = null; break; }
          found = id;
        }
      }
      if (found == null) { inspecting.selected = null; report({ kind: "unsupported", what: "inspect", detail: "the selected node is gone in this generation: select it again" }); return; }
      was.id = found;
    }
    await select(was.id, false, was.click);
  };
  const swallow = (event) => {
    if (!inspecting.on) return;
    if (event.type === "keydown" && event.key === "Escape") { event.preventDefault(); event.stopImmediatePropagation(); setInspect(false, true); return; }
    // The inspector's own controls are handled here, their events never
    // passing through the app's handlers (a banner inside an app dialog).
    if (inspecting.banner?.contains(event.target)) {
      event.stopImmediatePropagation();
      if (event.type === "wheel") event.preventDefault();
      if (event.type === "click" && inspecting.close?.contains(event.target)) setInspect(false, true);
      return;
    }
    event.preventDefault();
    event.stopImmediatePropagation();
    // A press released where it began, by mouse, pen or finger, selects.
    if (event.type === "scroll") place();
    if (event.type === "pointerdown") { chrome(); inspecting.press = { x: event.clientX, y: event.clientY, id: event.pointerId }; }
    if (event.type === "pointerup" && inspecting.press?.id === event.pointerId) {
      const { x, y } = inspecting.press;
      inspecting.press = null;
      if (Math.hypot(event.clientX - x, event.clientY - y) <= 10) { const hit = nodeAt(event.clientX, event.clientY); outline(hit?.el ?? null); if (hit) void select(hit.id, true); }
    }
    if (event.type === "pointercancel") inspecting.press = null;
  };
  const hover = (event) => { if (!inspecting.on) return; event.stopImmediatePropagation(); chrome(); const hit = nodeAt(event.clientX, event.clientY); outline(hit?.el ?? null); };
  // The gate is on the window from the page's start, so it runs before any
  // listener the app's host adds later (a gesture's, a GPU surface's); it
  // does nothing until inspect mode is on.
  for (const type of INSPECT_EVENTS) addEventListener(type, swallow, { capture: true, passive: false });
  addEventListener("pointermove", hover, { capture: true });
  // No field or guest frame takes focus meanwhile (its keys would not meet
  // the gate).
  addEventListener("focusin", (event) => { if (inspecting.on && !inspecting.banner?.contains(event.target)) event.target?.blur?.(); }, { capture: true });
  // A guest frame that takes focus itself: the parent window loses it, and
  // focus goes back to the inspector's close control.
  addEventListener("resize", () => { if (inspecting.on) place(); });
  addEventListener("blur", () => { if (inspecting.on && document.activeElement?.tagName === "IFRAME") inspecting.close?.focus(); });
  const topLayer = (el) => { try { el.popover = "manual"; el.showPopover(); } catch { /* no popover: a fixed element */ } };
  // The inspector's chrome (banner, outline, guest-frame shield), made
  // again when a new tree or a modal took it away.
  const topModal = () => { try { return [...document.querySelectorAll("dialog")].findLast((d) => d.matches(":modal")) ?? null; } catch { return null; } };
  const chrome = () => {
    const modal = topModal();
    if (inspecting.banner?.isConnected && inspecting.outline?.isConnected && inspecting.shield?.isConnected && inspecting.cover?.isConnected && (!modal || modal.contains(inspecting.banner))) return;
    inspecting.banner?.remove(); inspecting.outline?.remove(); inspecting.shield?.remove(); inspecting.cover?.remove();
    const banner = inspecting.banner = document.createElement("div");
    banner.id = "exact-inspect";
    banner.setAttribute("role", "status");
    banner.style.cssText = "position:fixed;inset:8px auto auto 50%;margin:0;transform:translateX(-50%);z-index:2147483647;display:flex;gap:8px;align-items:center;padding:6px 10px;border:0;border-radius:8px;background:#1d4ed8;color:#fff;font:13px system-ui,sans-serif;box-shadow:0 2px 8px rgba(0,0,0,.3)";
    banner.textContent = "Inspecting: click a view to select it";
    const close = inspecting.close = document.createElement("button");
    close.type = "button";
    close.textContent = "×";
    close.setAttribute("aria-label", "Stop inspecting (Escape)");
    close.style.cssText = "border:0;background:transparent;color:#fff;font-size:16px;cursor:pointer;padding:0 2px";
    banner.append(close);
    const box = inspecting.outline = document.createElement("div");
    box.style.cssText = "position:fixed;display:none;margin:0;padding:0;border:0;z-index:2147483646;pointer-events:none;outline:2px solid #1d4ed8;background:rgba(29,78,216,.12)";
    // Guest frames take no pointer input: a press there is their view's.
    const shield = inspecting.shield = document.createElement("style");
    shield.textContent = "iframe, frame, embed, object { pointer-events: none !important; }";
    document.head.append(shield);
    // A transparent cover over the whole page takes the pointer, so no
    // view sees it (no hover, enter or leave, no press on a guest frame);
    // a press is read through it (`nodeAt`).
    const cover = inspecting.cover = document.createElement("div");
    cover.style.cssText = "position:fixed;inset:0;margin:0;padding:0;border:0;width:auto;height:auto;z-index:2147483644;background:transparent;touch-action:none";
    // Inside the topmost open modal dialog, if one is, so its controls are
    // not inert; else the body. Made again when a modal opens later.
    (modal ?? document.body).append(cover, box, banner);
    // The outlines go with the chrome.
    for (const { mark } of marks) (modal ?? document.body).append(mark);
    // (Rebuilt with boxes still up: said again.)
    legend(marks.length);
    // Above the app's own top layer (an open dialog), where one exists.
    topLayer(cover); topLayer(box); topLayer(banner);
  };
  const setInspect = (on, local = false) => {
    if (on === inspecting.on) { if (on) chrome(); return; }
    inspecting.on = on;
    // Kept across a reload of this tab, so input stays gated until the
    // server says how it holds inspect mode (its replay on registering).
    try { if (on) sessionStorage.setItem("exact.dev.inspect", String(inspecting.seq ?? "")); else sessionStorage.removeItem("exact.dev.inspect"); } catch { /* storage refused */ }
    if (!on) inspecting.selected = null;
    // Left here (Escape, the close control): the server and its clients
    // are told, so no one owns inspect mode on this page any more.
    if (!on && local) { inspecting.left = inspecting.seq; try { sessionStorage.setItem("exact.dev.inspect.left", String(inspecting.seq ?? "")); } catch { /* storage refused */ } report({ kind: "selected", selected: null, inspect: inspecting.seq, plan: channel.plan, revision: channel.revision }); }
    if (on) {
      // Out of any field or guest frame, so no key goes there (the gate is
      // on already, so the field's blur and change reach no app handler).
      document.activeElement?.blur?.();
      chrome();
    } else {
      inspecting.banner?.remove(); inspecting.outline?.remove(); inspecting.shield?.remove(); inspecting.cover?.remove(); unmark(); inspecting.session = null;
      inspecting.banner = inspecting.outline = inspecting.shield = inspecting.cover = inspecting.close = inspecting.press = null;
    }
  };
  // `outline {sites, plan}` (D4, D5): every copy of the selected node, the
  // views of those plan sites, outlined while inspect mode is on (the first
  // scrolled into view, unless the frame answers a click here (`click`);
  // then measured; kept on them as the page scrolls and resizes). A frame for another plan leaves the outlines as they are (a
  // host never reads a source map: the server resolves sites); an empty one
  // clears them; `clear` too.
  const marks = [];
  // A marked view that changes size (a font-size pin, a commit) is outlined
  // where it is now.
  const resized = typeof ResizeObserver === "function" ? new ResizeObserver(() => place()) : null;
  // The banner says what the dashed boxes are while they show (a selection's every copy or what the tool's Outline
  // asked), its words written only when they change (a live region).
  const legend = (n) => {
    const t = inspecting.banner?.firstChild, words = n > 0 ? `Inspecting: ${n} outlined in dashes (the selection's every copy, or what the tool outlined); click a view to select it` : "Inspecting: click a view to select it";
    if (t?.nodeType === 3 && t.nodeValue !== words) t.nodeValue = words;
  };
  const unmark = (quiet = false) => { resized?.disconnect(); for (const { mark } of marks.splice(0)) mark.remove(); if (!quiet) legend(0); };
  const place = () => {
    for (const { mark, el } of marks) {
      const r = el.getBoundingClientRect();
      Object.assign(mark.style, { left: `${r.left}px`, top: `${r.top}px`, width: `${r.width}px`, height: `${r.height}px`, display: el.isConnected ? "block" : "none" });
    }
    // (A view gone takes its box: the count said is of those still shown.)
    if (marks.length) legend(marks.filter(({ el }) => el.isConnected).length);
  };
  const outlineSites = (frame) => {
    if (frame.clear) { unmark(); return; }
    if (!inspecting.on) return;
    const views = ask({ op: "sites" });
    if (views.error || views.planDigest !== frame.plan || !Array.isArray(frame.sites)) return;
    unmark(true);
    const wanted = new Set(frame.sites);
    const found = views.sites.filter(([, site]) => wanted.has(site)).map(([id]) => globalThis.exact.views?.get(id)).filter((el) => el?.isConnected);
    // The outline answering a click here (its token) scrolls nothing: that
    // copy is (or was) under the pointer. Any other (the tool's Outline)
    // brings the first in.
    if (!(inspecting.session && typeof frame.click === "string" && frame.click.startsWith(`${inspecting.session}.`))) found[0]?.scrollIntoView({ block: "nearest" });
    for (const el of found) {
      const mark = document.createElement("div");
      mark.dataset.exactOutline = "";
      mark.style.cssText = "position:fixed;margin:0;padding:0;border:0;z-index:2147483645;pointer-events:none;outline:2px dashed #1d4ed8";
      (inspecting.banner?.parentElement ?? document.body).append(mark);
      marks.push({ mark, el });
      resized?.observe(el);
    }
    place();
    legend(found.length);
  };
  // `hold {revision, hold}` (LLP 1122 D4, D8; *1.11*): `{site, instance, state}` of this page's plan, the view by
  // its site and instance path (exactly one, as a refresh finds it), held by the input glue as the real state; null
  // lets it go. What it holds or let go, and why, is reported (`held`), to every client.
  // A new generation's tree takes the hold again (the glue let it go unsaid with the old tree): under the same plan
  // here; under another the server replays it, its site corresponded, once the status says the new plan.
  const canonical = (v) => JSON.stringify(v ?? [], (_, x) => (x && typeof x === "object" && !Array.isArray(x) ? Object.fromEntries(Object.entries(x).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))) : x));
  let swaps = 0;
  // A hold frame names the plan its site addresses (`plan`; else it is bound, as it arrives, to the generation it
  // addresses and that generation's digest): under another plan, a generation swapped in since or swapping now, it is
  // nothing, unsaid; the server's replay after the status that names the new plan corresponds it, and `rehold` takes
  // it under the same plan. A revision the page ended itself (input, disabled, gone, a refusal) is not taken again
  // when its frame comes again (a poll's resent answer); a lower revision is a new server's, and forgets them.
  const holdAt = new WeakMap(), ended = new Set();
  // (`epoch`: the server's run the revisions are of; a let-go of an earlier run's hold ends nothing now.)
  let lastRevision = 0, epoch = 0;
  const end = (revision, of) => { if (of !== epoch) return; ended.add(revision); if (ended.size > 64) ended.delete(ended.values().next().value); };
  const digestNow = () => {
    const x = globalThis.exact;
    if (planDigestOf === (x.generation ?? null)) return planDigestNow;
    try { const sites = typeof x.wasm?.exact_agent === "function" ? ask({ op: "sites" }) : null; return typeof sites?.planDigest === "string" ? sites.planDigest : null; } catch { return null; }
  };
  const bindHold = (frame) => { const was = holdAt.get(frame); holdAt.set(frame, { gen: globalThis.exact.generation ?? null, digest: digestNow(), epoch: was?.epoch ?? epoch, stream: was ? was.stream : channel.nonce }); };
  const rehold = async () => {
    const kept = channel.hold;
    if (!kept || swaps || (await planDigest()) !== kept.digest || channel.hold !== kept || channel.holdLatest !== kept.frame) return;
    bindHold(kept.frame);
    command(kept.frame);
  };
  // How long a hold waits for the input glue before saying it is not loaded; the glue's `hold` once it has loaded
  // (it says so: `exact-hold`), or null after `ms` or once `stale()`.
  const HOLD_WAIT = 60_000;
  const glueHold = (ms, stale) => new Promise((ok) => {
    if (globalThis.exact?.hold) return ok(globalThis.exact.hold);
    const until = Date.now() + ms;
    const end = () => { clearInterval(tick); globalThis.removeEventListener("exact-hold", end); ok(globalThis.exact?.hold ?? null); };
    const tick = setInterval(() => { if (globalThis.exact?.hold || stale() || Date.now() > until) end(); }, 250);
    globalThis.addEventListener("exact-hold", end);
  });
  const hold = async (frame) => {
    const at = holdAt.get(frame) ?? { gen: globalThis.exact.generation ?? null, digest: null, epoch, stream: channel.nonce };
    // Only the latest frame applies (a replay queued before a newer clear does not), and only on the stream it came
    // on (a new one's server knows nothing of it).
    const current = () => channel.holdLatest === frame && at.stream === channel.nonce && at.epoch === epoch;
    // The input glue loads after paint: a hold waits for it (a page restarting after a rebuild, on a loaded machine,
    // takes far longer than a fixed wait) while it is still the latest frame on its stream (as host/web-js/
    // inspect-dev.js's `glueHold`); a let-go needs none.
    if (frame.hold) await glueHold(HOLD_WAIT, () => !current());
    const holds = globalThis.exact.hold, h = frame.hold, revision = frame.revision;
    if (!current() || (h && ended.has(revision))) return;
    const digest = swaps ? null : await planDigest();
    if (!current()) return;
    const addressed = typeof frame.plan === "string" ? frame.plan : at.digest;
    const moved = addressed ? digest !== addressed : at.gen !== null && (globalThis.exact.generation ?? null) !== at.gen;
    if (h && (swaps || moved)) { channel.hold = addressed ? { frame, digest: addressed } : null; return; }
    channel.hold = h ? { frame, digest } : null;
    // (A let-go with no glue loaded: nothing was held.)
    if (!h) { holds?.(null); report({ kind: "held", state: null, revision }); return; }
    if (!holds) { report({ kind: "unsupported", what: "hold", detail: "the input glue is not loaded", revision }); return; }
    let found = null, two = false;
    for (const [id] of globalThis.exact.views ?? []) {
      const node = ask({ op: "node", id, plan: true });
      if (node.error || globalThis.exact.views.get(id)?.closest?.("[data-exiting]")) continue; // a leaving view is no match
      if (node.site === h.site && canonical(node.instance) === canonical(h.instance)) { if (found != null) { two = true; break; } found = id; }
    }
    const el = found != null && !two ? globalThis.exact.views.get(found) : null;
    applying = revision;
    // (No one view there: the one held before is let go too.)
    const r = el ? holds(el, h.state, (reason) => {
      // (The same frame taken again, a resent one or `rehold`'s, replaces nothing.)
      if (reason === "replaced" && applying === revision) return;
      if (at.epoch !== epoch || at.stream !== channel.nonce) return; // a hold of an earlier stream's: no news to this one
      if (channel.hold?.frame === frame) channel.hold = null;
      if (reason !== "replaced") end(revision, at.epoch);
      report({ kind: "held", state: null, reason, revision });
    }) : (holds(null), { reason: "gone" });
    applying = null;
    if (r.reason && channel.hold?.frame === frame) channel.hold = null;
    if (r.reason) end(revision, at.epoch);
    report(r.reason ? { kind: "held", state: null, reason: r.reason, revision } : { kind: "held", state: h.state, revision });
  };
  // `{select: {up, levels, plan, from, instance}}` (tamber/1.16): the selection moves up its call sites, as a click on the node
  // it lands on would; it lands in the first of `levels` (each enclosing copy's plan sites, nearest first, by the
  // server's map for the selection at `from`, instance path `instance`, under `plan`) with a node above the selection, on the outermost there.
  const selectUp = async ({ levels, plan, from, instance }) => {
    const id = inspecting.selected?.id;
    // (No selection here to move, or none above it of that copy: staleness, not inability; its selection as it is.)
    if (!inspecting.on || id == null || !Array.isArray(levels)) { await refresh(); return; }
    // (Only the selection, its instance, generation and plan the server took its sites from. One that moved since on
    // its way here is no inability (theme2 15:02): the page reports its selection as it is now, and the tool follows.)
    if (inspecting.selected.generation !== globalThis.exact.generation || plan !== inspecting.selected.plan || from !== inspecting.selected.site || JSON.stringify(instance ?? []) !== inspecting.selected.instance) { await refresh(); return; }
    // Its ancestors and their sites, in one read with no node's rows (the runner's `ancestors`), the selection itself
    // left out; the first level with one of them, and in it the outermost.
    const above = (ask({ op: "ancestors", id }).ancestors ?? []).slice(1);
    for (const level of levels) {
      const want = new Set(level);
      let found = null;
      for (const [view, site] of above) if (want.has(site)) found = view;
      if (found != null) { await select(found); return; }
    }
    await refresh();
  };
  const inspect = async (frame) => {
    if (frame.select) { await selectUp(frame.select); return; }
    // `{refresh: true}`: the selection again, under the digests now.
    // (`status: true`: its status again first, one lost on the way.)
    if (frame.refresh) { if (frame.status) await status(); await refresh(); return; }
    // The command's number, which a local exit names, so a later owner's
    // command is not undone by it.
    // A replay of the command this page left itself (a new stream before
    // the server heard the exit) is the exit again, not on.
    if (frame.on && frame.seq != null && frame.seq === inspecting.left) { report({ kind: "selected", selected: null, inspect: frame.seq, plan: channel.plan, revision: channel.revision }); return; }
    inspecting.left = null;
    try { sessionStorage.removeItem("exact.dev.inspect.left"); } catch { /* storage refused */ }
    // Another command (another owner, or off and on): a node selected
    // under the last is not this one's to report again.
    if ((frame.seq ?? null) !== inspecting.seq) inspecting.selected = null;
    inspecting.seq = frame.seq ?? null;
    setInspect(frame.on === true);
    if (inspecting.on) try { sessionStorage.setItem("exact.dev.inspect", String(inspecting.seq ?? "")); } catch { /* storage refused */ }
  };
  // A reload of a tab that was inspecting starts inspecting, until the
  // server's replay on registering says otherwise.
  // (The command it was on by is kept with it, so an exit before the
  // replay names it.)
  try {
    const kept = sessionStorage.getItem("exact.dev.inspect"), left = sessionStorage.getItem("exact.dev.inspect.left");
    if (kept != null) { inspecting.seq = kept === "" ? null : Number(kept); setInspect(true); }
    // An exit the server may not have heard: its replay is answered with it.
    else if (left) inspecting.left = Number(left);
  } catch { /* storage refused */ }
  const command = (frame) => {
    designing = designing.then(() => {
      if (frame.kind === "pin") return pin(frame);
      if (frame.kind === "inspect") return inspect(frame);
      if (frame.kind === "outline") return outlineSites(frame);
      if (frame.kind === "hold") return hold(frame);
      if (frame.kind === "navigate") return navigate(frame);
      report({ kind: "unsupported", what: frame.kind });
    }).catch((error) => console.error("exact dev:", String(error)));
  };
  es.onmessage = (event) => {
    let message;
    try { message = JSON.parse(event.data); } catch { show("the dev stream sent invalid JSON"); return; }
    if (message.error) {
      if (message.surfaceAttempt && globalThis.exact.gpu?.reloadAssets) {
        const {name,text} = message.surfaceAttempt;
        globalThis.exact.gpu.reloadAssets(new Map([[name,{bytes:new TextEncoder().encode(text)}]]),[name]).catch(error=>console.error(String(error)));
      }
      show(message.error, message.source); console.error("exact dev:", message.error); return; }
    if (message.fresh) { if (new URLSearchParams(location.search).get('agent') !== '1') location.reload(); return; }
    if (message.gpu !== undefined) {
      // A drive owns its clock and code. Do not change either behind the driver.
      if (new URLSearchParams(location.search).get('agent') === '1') return;
      // A declared module's rebuild names it (LLP 1009 D6); the primary's does not.
      const versions = message.module ? (globalThis.exact.gpuVersions ??= {}) : globalThis.exact, key = message.module ? `gpu/${message.module}` : 'gpuVersion';
      versions[key] = message.gpu;
      if (globalThis.exact.gpu) (async () => {
        if (message.gpu !== versions[key]) return;
        const result = await globalThis.exact.gpu.swap(message.gpu, message.module);
        show(result.errors.join('\n') || null, "gpu");
        navigator.sendBeacon(`/__dev/gpu?g=${message.gpu}&swap=${result.ms.toFixed(1)}`);
      })().catch(error => { show(String(error), "gpu"); console.error('exact dev:', error); });
      return;
    }
    if (message.kind === "tamber-batch" || message.kind === "tamber-snapshot") { design(message); return; }
    if (message.kind === "hold") {
      if (message.revision < lastRevision) { ended.clear(); epoch += 1; }
      lastRevision = message.revision;
      channel.holdLatest = message;
      bindHold(message);
    }
    if (message.kind === "navigate") message.stream = channel.nonce;
    if (message.kind === "pin" || message.kind === "inspect" || message.kind === "outline" || message.kind === "hold" || message.kind === "navigate") { command(message); return; }
    if (message.hello && message.nonce && message.nonce !== channel.nonce) {
      const nonce = channel.nonce = message.nonce;
      // A new stream may be a new server's: a hold of the one before ends nothing, and says nothing, now.
      epoch += 1; ended.clear(); lastRevision = 0;
      channel.plan ??= message.generation ?? null;
      // Reports wait for this; a refused registration sends none.
      channel.registered = (async () => {
        await globalThis.exact.ready;
        const id = await buildId, answer = await post(nonce, { kind: "register", id, host: "web", capabilities: ["pin", "inspect", "hold", "navigate"], plan: channel.plan, planDigest: await planDigest(), design: globalThis.exact.designDigest ?? await designNow() });
        // A framed copy tells the page framing it which build it is (LLP
        // 1122 T7): the design tool maps the frame to its tile by this
        // message's source. The id grants nothing without the stream's nonce.
        if (answer.ok && framed) globalThis.parent.postMessage({ type: "exact-build", id }, "*");
        return answer.ok;
      })().catch(() => false);
      status();
    }
    if (message.ready === false) return;
    client.receive(message);
  };
}
