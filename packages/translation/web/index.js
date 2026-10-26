// @exact/translation on the web (LLP 1133.027 D7–D10): the browser's
// Translator and Language Detector APIs
// (https://webmachinelearning.github.io/translation-api/), which Chrome and
// Edge ship on desktop; elsewhere every availability is `unavailable`,
// `not-supported`.
//
// - A translator and a language detector are handles (D9): the browser's
//   object behind each, its calls one at a time in admission order.
// - Every create is the app's foreground act, which the runner admits
//   before dispatch (D16); the browser's `create()` runs here inside the
//   press's activation (`"gesture": true`), which a download needs (D8).
// - A pair whose languages are one after D7's table (`en`, `en-GB`) is
//   `unavailable`, `unsupported-options`, as on every target.
// - Under the agent no browser API is used and no model runs (§9): a fixed
//   list of languages is `available`, a translation is the script
//   `prefer extension translation.translations=[…]`'s next entry or the
//   input behind the target's tag, and detection is the script's or a rule
//   on the scripts of the input's letters.

const AGENT_LANGUAGES = ['ar', 'de', 'en', 'es', 'fr', 'hi', 'it', 'ja', 'ko', 'pt', 'ru', 'zh', 'zh-Hant'];
const TRANSLATOR = 'translation.translatorAvailability', DETECTOR = 'translation.languageDetectorAvailability';

const failure = (error, message) => ({ error, ...(message ? { message } : {}) });
const nameOf = (e, fallback) => typeof e?.name === 'string' && /Error$/.test(e.name) ? e.name : fallback;
const isError = v => v && typeof v === 'object' && typeof v.error === 'string';

/** A tag canonicalised as the specification does (ECMA-402's CanonicalizeLanguageTag), or null. */
function canonical(tag) {
  try { return Intl.getCanonicalLocales(tag)[0] ?? null; } catch { return null; }
}
/** D7's table: the script a region implies made explicit (`zh-TW` is `zh-Hant`), then the region dropped. */
export function scripted(tag) {
  const loc = new Intl.Locale(tag);
  let script = loc.script;
  if (loc.language === 'zh' && !script) script = ['TW', 'HK', 'MO'].includes(loc.region) ? 'Hant' : 'Hans';
  return [loc.language, script].filter(Boolean).join('-');
}
/** `tag` truncated as LLP 1060 truncates until `listed` has it, or null. */
export function listedAs(tag, listed) {
  const parts = scripted(tag).split('-');
  while (parts.length) {
    const t = parts.join('-');
    if (listed.includes(t)) return t;
    parts.pop();
  }
  return null;
}

/** Detection under the agent (§9): any kana is Japanese, then Hangul Korean, else the first letter's script. */
export function ruled(input) {
  if (/[\p{Script=Hiragana}\p{Script=Katakana}]/u.test(input)) return 'ja';
  if (/\p{Script=Hangul}/u.test(input)) return 'ko';
  const first = /\p{L}/u.exec(input)?.[0];
  if (!first) return null;
  for (const [script, lang] of [['Han', 'zh'], ['Cyrillic', 'ru'], ['Arabic', 'ar'], ['Devanagari', 'hi'], ['Latin', 'en']]) {
    if (new RegExp(`\\p{Script=${script}}`, 'u').test(first)) return lang;
  }
  return null;
}

export function implementation({ changed, agent, log, foreground }) {
  const say = line => log?.(`translation: ${line}`);
  const Translator = !agent && typeof globalThis.Translator === 'function' ? globalThis.Translator : null;
  const Detector = !agent && typeof globalThis.LanguageDetector === 'function' ? globalThis.LanguageDetector : null;
  const translators = new Map(), detectors = new Map(), inFlight = new Map(), answered = [];
  let serial = 0;
  // The agent's words (§9): availabilities laid over its own, its scripts, and pairs a removed pack made downloadable.
  let pairOverlay = {}, detectorOverlay = {}, translations = [], detections = [], seen = { t: '', d: '' };
  const removed = new Set();
  // Downloads a create started, by pair key, and the creates waiting on each (D8).
  const downloads = new Map();
  const note = (name, how) => { answered.push(`${name} (${how?.origin ?? 'app'})`); if (answered.length > 16) answered.shift(); };

  // ---------------------------------------------------------------- languages (D7)
  /** The pair as asked, canonicalised, or a refusal of either tag. */
  function pairOf(sourceLanguage, targetLanguage) {
    const source = canonical(sourceLanguage), target = canonical(targetLanguage);
    if (!source || !target) return null;
    return { source, target, same: scripted(source) === scripted(target), key: `${scripted(source)}>${scripted(target)}` };
  }
  const agentPair = p => {
    const s = listedAs(p.source, AGENT_LANGUAGES), t = listedAs(p.target, AGENT_LANGUAGES);
    if (p.same || !s || !t || s === t) return { availability: 'unavailable', reason: 'unsupported-options' };
    return { availability: removed.has(p.key) ? 'downloadable' : 'available' };
  };
  /** An availability with the download in flight laid over it. */
  const during = (a, key) => {
    const d = downloads.get(key);
    if (d && a.availability !== 'available' && a.availability !== 'unavailable') return { availability: 'downloading', progress: d.progress };
    if (a.availability !== 'unavailable') delete a.reason;
    if (a.availability !== 'downloading') delete a.progress;
    return a;
  };

  async function pairAvailability({ sourceLanguage, targetLanguage }) {
    const p = pairOf(sourceLanguage, targetLanguage);
    if (!p || p.same) return { availability: 'unavailable', reason: 'unsupported-options' };
    if (agent) return during({ ...agentPair(p), ...pairOverlay }, p.key);
    if (!Translator) return { availability: 'unavailable', reason: 'not-supported' };
    let a;
    try { a = await Translator.availability({ sourceLanguage: p.source, targetLanguage: p.target }); } catch { return { availability: 'unavailable', reason: 'unsupported-options' }; }
    return during(a === 'unavailable' ? { availability: a, reason: 'unsupported-options' } : { availability: a }, p.key);
  }
  async function detectorAvailability({ expectedInputLanguages } = {}) {
    if (expectedInputLanguages?.some(t => !canonical(t))) return { availability: 'unavailable', reason: 'unsupported-options' };
    if (agent) return during({ availability: 'available', ...detectorOverlay }, 'detector');
    if (!Detector) return { availability: 'unavailable', reason: 'not-supported' };
    let a;
    try { a = await Detector.availability(expectedInputLanguages ? { expectedInputLanguages } : {}); } catch { return { availability: 'unavailable', reason: 'unsupported-options' }; }
    return during(a === 'unavailable' ? { availability: a, reason: 'device-not-eligible' } : { availability: a }, 'detector');
  }
  // A page shown again reads availability again (LLP 1133.016 D5): a pack may have arrived or gone while it was hidden.
  if (!agent && (Translator || Detector) && typeof document === 'object') document.addEventListener('visibilitychange', () => { if (document.visibilityState === 'visible') { changed(TRANSLATOR); changed(DETECTOR); } });

  // ---------------------------------------------------------------- downloads (D8)
  /** Under the agent: a create while `downloadable` downloads until the drive says `available`. */
  function agentDownload(key, topic) {
    if (!downloads.has(key)) {
      downloads.set(key, { progress: 0, waiting: [], topic });
      say(`${key}: download started (agent)`);
      changed(topic);
    }
    return new Promise((resolve, reject) => downloads.get(key).waiting.push({ resolve, reject }));
  }
  function settleDownloads(topic, availability) {
    for (const [key, d] of downloads) {
      if (d.topic !== topic) continue;
      if (availability === 'available') { removed.delete(key); say(`${key}: download finished (agent)`); d.waiting.forEach(w => w.resolve()); }
      else if (availability === 'unavailable') d.waiting.forEach(w => w.reject(failure('NotSupportedError')));
      else continue;
      downloads.delete(key);
      changed(topic);
    }
  }
  const monitor = (key, topic) => m => m.addEventListener('downloadprogress', e => {
    const d = downloads.get(key) ?? { progress: 0, waiting: [], topic };
    d.progress = e.loaded;
    downloads.set(key, d);
    changed(topic);
  });

  // ---------------------------------------------------------------- handles (D9)
  /** `work` after every call on `handle` admitted before it: one at a time, in admission order. */
  function serially(handle, request, work) {
    const run = handle.tail.then(() => handle.dead ? failure('AbortError') : work());
    handle.tail = run.catch(() => {});
    if (request != null) inFlight.set(request, handle);
    handle.calls += 1;
    return run.finally(() => { handle.calls -= 1; if (request != null) inFlight.delete(request); });
  }
  function drop(map, id, why) {
    const h = map.get(id);
    if (!h) return;
    h.dead = true;
    try { h.platform?.destroy?.(); } catch {}
    map.delete(id);
    say(`${id}: ${why}`);
  }

  async function createTranslator(args, how = {}) {
    note('createTranslator', how);
    const p = pairOf(args.sourceLanguage, args.targetLanguage);
    if (!p || p.same) return failure('NotSupportedError', 'unsupported-options');
    let platform = null;
    if (agent) {
      const a = await pairAvailability(args);
      if (a.availability === 'unavailable') return failure('NotSupportedError', a.reason);
      if (a.availability !== 'available') {
        try { await agentDownload(p.key, TRANSLATOR); } catch (e) { return e; }
      }
    } else {
      if (!Translator) return failure('NotSupportedError', 'not-supported');
      // Inside the press's dispatch when it came from one, so its activation holds (D8).
      try { platform = await Translator.create({ sourceLanguage: p.source, targetLanguage: p.target, monitor: monitor(p.key, TRANSLATOR) }); }
      catch (e) {
        const name = nameOf(e, 'NotSupportedError');
        say(`create: ${name}: ${e?.message ?? e}`);
        return failure(name, name === 'NotAllowedError' ? (globalThis.navigator?.userActivation?.isActive === false ? 'activation' : 'policy') : undefined);
      } finally { if (downloads.delete(p.key)) changed(TRANSLATOR); }
    }
    const id = `translator-${++serial}`;
    translators.set(id, { id, source: p.source, target: p.target, key: p.key, platform, tail: Promise.resolve(), calls: 0, dead: false });
    say(`${id}: ${p.source} to ${p.target}`);
    const info = { id, sourceLanguage: p.source, targetLanguage: p.target };
    const quota = platform?.inputQuota;
    if (typeof quota === 'number' && Number.isFinite(quota)) info.inputQuota = quota;
    return info;
  }

  async function translate({ translator, input }, how = {}) {
    note('translate', how);
    const t = translators.get(translator);
    if (!t) return failure('InvalidStateError');
    return serially(t, how.request, async () => {
      if (typeof input !== 'string') return failure('TypeError');
      // An empty input is empty in every language, and takes no entry of the script.
      if (input === '') return { text: '' };
      if (agent) {
        const entry = translations.shift() ?? null;
        if (entry?.error) {
          if (entry.reason === 'not-installed') { removed.add(t.key); changed(TRANSLATOR); }
          return failure(entry.error, entry.reason);
        }
        return { text: entry?.text ?? `[${t.target}] ${input}` };
      }
      try { return { text: await t.platform.translate(input) }; }
      catch (e) { return failure(nameOf(e, 'OperationError'), e?.message); }
    });
  }

  async function createLanguageDetector(args = {}, how = {}) {
    note('createLanguageDetector', how);
    const a = await detectorAvailability(args);
    if (a.availability === 'unavailable') return failure('NotSupportedError', a.reason);
    let platform = null;
    if (agent) {
      if (a.availability !== 'available') {
        try { await agentDownload('detector', DETECTOR); } catch (e) { return e; }
      }
    } else {
      try { platform = await Detector.create({ ...(args.expectedInputLanguages ? { expectedInputLanguages: args.expectedInputLanguages } : {}), monitor: monitor('detector', DETECTOR) }); }
      catch (e) {
        const name = nameOf(e, 'NotSupportedError');
        return failure(name, name === 'NotAllowedError' ? (globalThis.navigator?.userActivation?.isActive === false ? 'activation' : 'policy') : undefined);
      } finally { if (downloads.delete('detector')) changed(DETECTOR); }
    }
    const id = `language-detector-${++serial}`;
    detectors.set(id, { id, expected: args.expectedInputLanguages ?? null, platform, tail: Promise.resolve(), calls: 0, dead: false });
    say(`${id}: made`);
    const info = { id };
    if (args.expectedInputLanguages) info.expectedInputLanguages = args.expectedInputLanguages;
    const quota = platform?.inputQuota;
    if (typeof quota === 'number' && Number.isFinite(quota)) info.inputQuota = quota;
    return info;
  }

  async function detectLanguage({ detector, input }, how = {}) {
    note('detectLanguage', how);
    const d = detectors.get(detector);
    if (!d) return failure('InvalidStateError');
    return serially(d, how.request, async () => {
      if (typeof input !== 'string') return failure('TypeError');
      if (input.trim() === '') return { results: [{ detectedLanguage: 'und', confidence: 1 }] };
      if (agent) {
        const entry = detections.shift();
        if (entry) return entry;
        const lang = ruled(input);
        return { results: lang ? [{ detectedLanguage: lang, confidence: 0.9 }, { detectedLanguage: 'und', confidence: 0.1 }] : [{ detectedLanguage: 'und', confidence: 1 }] };
      }
      try { return { results: (await d.platform.detect(input)).map(r => ({ detectedLanguage: r.detectedLanguage, confidence: r.confidence })) }; }
      catch (e) { return failure(nameOf(e, 'OperationError'), e?.message); }
    });
  }

  return {
    translatorAvailability: pairAvailability,
    languageDetectorAvailability: detectorAvailability,
    createTranslator,
    translate,
    async destroyTranslator({ translator }) {
      if (!translators.has(translator)) return failure('InvalidStateError');
      drop(translators, translator, 'destroyed by the app');
      return { id: translator };
    },
    createLanguageDetector,
    detectLanguage,
    async destroyLanguageDetector({ detector }) {
      if (!detectors.has(detector)) return failure('InvalidStateError');
      drop(detectors, detector, 'destroyed by the app');
      return { id: detector };
    },
    /** Under the agent: availabilities laid over the agent's, and the scripts (§9). */
    preferred(values = {}) {
      const plain = v => v && typeof v === 'object' && !isError(v) && !Array.isArray(v) ? v : {};
      pairOverlay = plain(values.translatorAvailability);
      detectorOverlay = plain(values.languageDetectorAvailability);
      const t = JSON.stringify(values.translations ?? null), d = JSON.stringify(values.detections ?? null);
      if (t !== seen.t) { seen.t = t; translations = Array.isArray(values.translations) ? values.translations.map(e => ({ ...e })) : []; say(`script: ${translations.length} translations`); }
      if (d !== seen.d) { seen.d = d; detections = Array.isArray(values.detections) ? values.detections.map(e => ({ ...e })) : []; say(`script: ${detections.length} detections`); }
      if (pairOverlay.availability) settleDownloads(TRANSLATOR, pairOverlay.availability);
      if (detectorOverlay.availability) settleDownloads(DETECTOR, detectorOverlay.availability);
    },
    /** A call let go (LLP 1133.016 D15 item 2): its result is read by no one. */
    cancel(request) { if (inFlight.has(request)) say(`cancelled: request ${request} was let go`); },
    /** Handles whose reply was dropped: never the app's (LLP 1133.009 D10 item 10). */
    dispose(ids) {
      for (const id of ids) { drop(translators, id, 'its reply was dropped'); drop(detectors, id, 'its reply was dropped'); }
    },
    foreground(shown) { if (shown && !agent) { changed(TRANSLATOR); changed(DETECTOR); } },
    destroy() {
      for (const id of [...translators.keys()]) drop(translators, id, 'the session ended');
      for (const id of [...detectors.keys()]) drop(detectors, id, 'the session ended');
    },
    /** `state.extensions` (§9): each handle, the scripts' remaining entries, each answer's origin. */
    state() {
      return {
        translators: [...translators.values()].map(t => ({ id: t.id, sourceLanguage: t.source, targetLanguage: t.target, platform: agent ? 'agent' : 'live', calls: t.calls })),
        detectors: [...detectors.values()].map(d => ({ id: d.id, expectedInputLanguages: d.expected, platform: agent ? 'agent' : 'live', calls: d.calls })),
        ...(agent ? { translations: translations.length, detections: detections.length } : {}),
        downloads: [...downloads.keys()],
        answered: answered.slice(),
        foreground: foreground?.() ?? true,
      };
    },
  };
}
