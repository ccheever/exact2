// @exact/speech-recognition on the web (LLP 1133.012 D7–D10, §9): the
// browser's `SpeechRecognition` (or `webkitSpeechRecognition`), answered in
// the interface's shapes.
//
// - Inputs (D8): `track=` is a consumer of a track `@exact/media-capture`
//   opened, holding its lease like a recorder, where the browser takes one
//   (desktop Chrome 135 and later, `start(audioTrack)`); its start is an
//   app's explicit start, which reopens an idle track, never a tool's or a
//   wake's. `microphone=true` is `start()`, under an exclusive hold of the
//   session's microphone (the track table's `hold`), since the browser's
//   recognizer opens its own capture. The web transcribes no file.
// - A recognition is an id (D7), its state a value: final results numbered
//   from 0, the last `history` of them (20 by default, 200 at most, 256 KB at
//   most) with the interim one. Every final goes, as it arrives, to the
//   `transcript=` directory as JSON Lines files of at most 1 MiB, the
//   directory emptied when the recognition starts.
// - A reply the runtime drops aborts its recognition; leaving the foreground
//   aborts every one (D10), its finals so far kept.
// - Under the agent no browser API is used (§9): the status is the runner's,
//   and a recognition hears what the drive says it hears (`heard`), one word
//   as an interim result every 300 ms of the session clock from the moment
//   the phrase is set and the final 500 ms after the last word; a
//   non-continuous one then ends, and with no phrase ends `no-speech` after
//   8 s; a `path=` one answers the phrase preferred at its start, at once.
//   The state is computed at each read, which the declaration's poll asks
//   every 100 ms of the session clock.

const HISTORY = 20, MAX_HISTORY = 200, WINDOW_BYTES = 256 * 1024, FILE_BYTES = 1 << 20, SPLIT = 16 * 1024;
const WORD_MS = 300, FINAL_MS = 500, NO_SPEECH_MS = 8000;
/** The interface's substitute default (exact.json), which the agent's preferred status is laid over. */
const AGENT_STATUS = { available: true, state: 'granted', inputs: ['track', 'microphone', 'file'], local: true, apple: null, android: null };
const NO_INFO = { id: '', leavesDevice: false, finalCount: 0, transcript: null, files: 0, error: null };
const NO_RECOGNITION = { id: '', state: 'ended', audio: false, sound: false, speech: false, results: [], finalCount: 0, interim: null, leavesDevice: false, error: null, message: null };

/** A final result's text in pieces of at most 16 KB, split at a word boundary (D7). */
export function pieces(text) {
  const out = [];
  let rest = text;
  while (new TextEncoder().encode(rest).length > SPLIT) {
    let cut = rest.lastIndexOf(' ', SPLIT);
    if (cut <= 0) cut = SPLIT;
    out.push(rest.slice(0, cut));
    rest = rest.slice(cut).trimStart();
  }
  out.push(rest);
  return out;
}

/** `results` with its oldest dropped until it holds at most `limit` and its JSON at most 256 KB. */
function windowed(results, limit) {
  const out = results.slice(-limit);
  while (out.length > 1 && JSON.stringify(out).length > WINDOW_BYTES) out.shift();
  return out;
}

export function implementation({ changed, agent, now, log, tracks, files, foreground: shown = () => true }) {
  const Recognition = !agent ? globalThis.SpeechRecognition ?? globalThis.webkitSpeechRecognition : null;
  const recognitions = new Map(), phrases = [];
  let next = 0, overlay = {}, heard = null;
  const say = line => log?.(`speech-recognition: ${line}`);
  const announce = () => changed('speech-recognition.speechRecognitionState');

  // ------------------------------------------------------------ the status (D7, D8, D11)
  /** What the browser's recognizer takes: a track on desktop Chrome 135 and later; elsewhere the microphone. */
  function browserStatus() {
    if (!Recognition) return { available: false, state: 'unknown', inputs: [], local: false, apple: null, android: null };
    const ua = typeof navigator === 'object' ? navigator.userAgent : '';
    const chrome = Number(/Chrome\/(\d+)/.exec(ua)?.[1] ?? 0), android = /Android/.test(ua);
    const track = chrome >= 135 && !android;
    // `start(audioTrack)` "does not request microphone permission"; `start()` asks by the browser's own policy.
    return { available: true, state: track ? 'granted' : 'unknown', inputs: track ? ['track', 'microphone'] : ['microphone'], local: 'processLocally' in Recognition.prototype, apple: null, android: null };
  }
  const status = () => agent ? { ...AGENT_STATUS, ...overlay } : browserStatus();

  // ------------------------------------------------------------ results and the transcript (D7)
  const info = r => ({ id: r.id, leavesDevice: r.leavesDevice, finalCount: r.finalCount, transcript: r.transcript, files: r.files, error: r.state === 'ended' ? r.error : null });
  const shape = r => ({
    id: r.id, state: r.state, audio: r.audio, sound: r.sound, speech: r.speech,
    results: windowed(r.results, r.history), finalCount: r.finalCount, interim: r.interim,
    leavesDevice: r.leavesDevice, error: r.error, message: r.message,
  });
  /** A final result of `alternatives` (best first): numbered, kept in the window, appended to the transcript. */
  function final(r, alternatives, start = null, end = null) {
    const best = alternatives[0]?.transcript ?? '';
    const parts = pieces(best);
    parts.forEach((text, i) => {
      const result = { index: r.finalCount++, alternatives: i === 0 && parts.length === 1 ? alternatives : [{ transcript: text, confidence: alternatives[0]?.confidence ?? 0 }], start, end, apple: null };
      r.results.push(result);
      if (r.results.length > MAX_HISTORY) r.results.shift();
      write(r, JSON.stringify(result) + '\n');
    });
    r.interim = null;
  }
  /** Append `line` to the transcript's current file, or begin the next past 1 MiB. */
  function write(r, line) {
    if (!r.transcript) return;
    const bytes = new TextEncoder().encode(line);
    r.writing = r.writing.then(async () => {
      if (!r.fileSize || r.fileSize + bytes.byteLength > FILE_BYTES) { r.files++; r.fileSize = 0; }
      const path = `${r.transcript}/${String(r.files - 1).padStart(5, '0')}.jsonl`;
      await (r.fileSize ? files.append(path, bytes.buffer) : files.write(path, bytes.buffer));
      r.fileSize += bytes.byteLength;
    }).catch(e => { if (!r.failed) { r.failed = true; say(`${r.transcript}: not written: ${e.message}`); } });
  }
  /** `r` ends (with `error`): its input let go, its files finished; sticky (D7). */
  function finish(r, error = null, message = null) {
    if (r.state === 'ended') return r.done;
    r.state = 'ended'; r.error ??= error; r.message ??= message;
    r.audio = r.sound = r.speech = false;
    r.unsubscribe?.(); r.unsubscribe = null;
    r.release?.(); r.release = null;
    say(`${r.id}: ended${r.error ? ` (${r.error})` : ''}, ${r.finalCount} final results`);
    r.done = r.writing.then(() => info(r));
    announce();
    return r.done;
  }

  // ------------------------------------------------------------ the agent's hearing (§9)
  /** `r` moved to the session clock's now: the words of each phrase heard since it began. */
  function advance(r) {
    if (!agent || r.state !== 'listening' || r.file) return;
    const t = now();
    for (; r.heard < phrases.length; r.heard++) {
      const p = phrases[r.heard];
      if (p.at < r.began) continue;
      const from = Math.max(p.at, r.cursor), n = p.words.length, last = from + WORD_MS * n, done = last + FINAL_MS;
      if (done <= t) {
        final(r, [{ transcript: p.text, confidence: 0.9 }], from, last);
        r.cursor = done;
        if (!r.continuous) { finish(r); return; }
        continue;
      }
      const count = Math.min(n, Math.floor((t - from) / WORD_MS));
      r.speech = count > 0;
      r.interim = count > 0 && r.interimResults ? { index: r.finalCount, alternatives: [{ transcript: p.words.slice(0, count).join(' '), confidence: 0.9 }], start: from, end: from + WORD_MS * count, apple: null } : null;
      return;
    }
    r.speech = false;
    if (!r.continuous && !r.finalCount && t - r.began >= NO_SPEECH_MS) finish(r, 'no-speech');
  }

  // ------------------------------------------------------------ the browser's recognizer (D9)
  function browser(r, args) {
    const s = new Recognition();
    if (args.lang != null) s.lang = args.lang;
    s.continuous = !!args.continuous;
    s.interimResults = !!args.interimResults;
    if (args.maxAlternatives != null) s.maxAlternatives = Math.min(10, args.maxAlternatives);
    if (args.processLocally != null && 'processLocally' in s) s.processLocally = args.processLocally;
    if (args.phrases?.length && typeof SpeechRecognitionPhrase === 'function') {
      try { s.phrases = args.phrases.map(p => new SpeechRecognitionPhrase(p.phrase, p.boost ?? 1)); } catch { /* the service names its own error */ }
    }
    for (const [event, field, on] of [['audiostart', 'audio', true], ['audioend', 'audio', false], ['soundstart', 'sound', true], ['soundend', 'sound', false], ['speechstart', 'speech', true], ['speechend', 'speech', false]]) {
      s.addEventListener(event, () => { r[field] = on; announce(); });
    }
    s.onresult = e => {
      let interim = [];
      for (let i = r.seen; i < e.results.length; i++) {
        const res = e.results[i], alternatives = [...res].map(a => ({ transcript: a.transcript, confidence: a.confidence }));
        if (res.isFinal && i === r.seen) { final(r, alternatives); r.seen++; } else if (!res.isFinal) interim.push(alternatives[0]?.transcript ?? '');
      }
      r.interim = interim.length && r.interimResults ? { index: r.finalCount, alternatives: [{ transcript: interim.join(''), confidence: 0 }], start: null, end: null, apple: null } : null;
      announce();
    };
    s.onnomatch = () => { final(r, []); announce(); };
    s.onerror = e => { r.error = e.error ?? 'aborted'; r.message = e.message || null; announce(); };
    s.onend = () => { finish(r); r.stopped?.(); };
    return s;
  }

  // ------------------------------------------------------------ names
  async function start(args) {
    const given = [args.track ? 'track' : null, args.microphone ? 'microphone' : null, args.path ? 'file' : null].filter(Boolean);
    if (given.length !== 1) return { error: 'TypeError' };
    const input = given[0], s = status();
    if (!s.available || !s.inputs.includes(input)) return { error: 'NotSupportedError' };
    const r = {
      id: `recognition-${++next}`, input, state: 'starting', audio: false, sound: false, speech: false, results: [], finalCount: 0, interim: null,
      error: null, message: null, continuous: !!args.continuous, interimResults: !!args.interimResults,
      history: Math.max(1, Math.min(MAX_HISTORY, args.history ?? HISTORY)), leavesDevice: args.processLocally ? false : !s.local,
      transcript: args.transcript ?? null, files: 0, fileSize: 0, writing: Promise.resolve(), began: now(), cursor: now(), heard: 0, seen: 0, file: input === 'file',
    };
    if (input === 'track') {
      const entry = tracks.lookup(args.track);
      if (entry?.kind !== 'audio') return { error: 'InvalidStateError' };
    }
    // `processLocally` where the recognizer cannot keep audio on the device (D9) opens nothing.
    const refused = args.processLocally && !s.local;
    if (input === 'microphone' && !refused) {
      try { r.release = tracks.hold('audio'); } catch (e) { return { error: e.code ?? 'NotReadableError' }; }
    }
    if (input === 'track' && !refused && !subscribe(r, args.track)) return { error: 'InvalidStateError' };
    // The directory is the recognition's for its life: emptied as it starts, so a replacement leaves no stale file.
    if (r.transcript) r.writing = files.remove(r.transcript).catch(() => {});
    recognitions.set(r.id, r);
    say(`${r.id}: ${input === 'track' ? `on ${args.track}` : input === 'file' ? `of ${args.path}` : 'on the microphone (held)'}${r.transcript ? `, transcript ${r.transcript}` : ''}, ${r.leavesDevice ? 'may leave the device' : 'on the device'}`);
    if (refused) { finish(r, 'service-not-allowed'); return info(r); }
    if (agent) {
      r.state = 'listening'; r.audio = r.sound = input !== 'file';
      if (input === 'file') {
        if (heard) final(r, [{ transcript: heard, confidence: 0.9 }], 0, WORD_MS * heard.split(/\s+/).length);
        finish(r);
      }
      announce();
      return info(r);
    }
    const recognizer = r.recognizer = browser(r, args);
    r.state = 'listening';
    try {
      if (input === 'track') r.live(source => recognizer.start(source));
      else recognizer.start();
    } catch (e) {
      finish(r, 'audio-capture', e.message);
    }
    announce();
    return info(r);
  }
  /** `r` consumes `track`: its lease, as a recorder holds one; its start an app's explicit start (D8, D10).
   * False for a track that has ended. The browser's recognizer starts on the capture's source (`r.live`). */
  function subscribe(r, track) {
    let source = null, ended = false, begin = null;
    r.live = f => { begin = f; if (source) f(source); };
    r.unsubscribe = tracks.subscribe(track, {
      live(s) {
        if (source || r.state === 'ended') return;
        source = s;
        try { begin?.(s); } catch (e) { finish(r, 'audio-capture', e.message); }
      },
      muted() {},
      unmuted() {},
      ended() { ended = true; if (r.state === 'listening') { r.recognizer?.abort(); finish(r); } },
    }, { start: true, origin: 'app' });
    if (ended) r.unsubscribe?.();
    return !ended;
  }

  return {
    startSpeechRecognition: start,
    async stopSpeechRecognition({ recognition }) {
      if (!recognition) return NO_INFO;
      const r = recognitions.get(recognition);
      if (!r) return { error: 'NotFoundError' };
      advance(r);
      if (r.state === 'ended') return r.done;
      if (agent) {
        // What was heard of a phrase so far is its final result.
        if (r.interim) final(r, r.interim.alternatives, r.interim.start, r.interim.end);
        return finish(r);
      }
      r.state = 'stopping';
      announce();
      await new Promise(stopped => { r.stopped = stopped; r.recognizer.stop(); });
      return r.done;
    },
    async abortSpeechRecognition({ recognition }) {
      // An abort of no recognition answers none: an app supersedes a start still in flight with it.
      if (!recognition) return NO_INFO;
      const r = recognitions.get(recognition);
      if (!r) return { error: 'NotFoundError' };
      advance(r);
      if (r.state !== 'ended') { r.interim = null; r.recognizer?.abort(); await finish(r, 'aborted'); }
      return info(r);
    },
    async speechRecognitionState({ recognition }) {
      if (!recognition) return NO_RECOGNITION;
      const r = recognitions.get(recognition);
      if (!r) return { error: 'NotFoundError' };
      advance(r);
      return shape(r);
    },
    async speechRecognitionStatus() { return status(); },
    async speechRecognitionAvailability({ langs, processLocally }) {
      if (agent) return { availability: status().available && (status().local || !processLocally) ? 'available' : 'unavailable' };
      if (!Recognition) return { availability: 'unavailable' };
      if (typeof Recognition.available === 'function') {
        try { return { availability: await Recognition.available({ langs, processLocally: !!processLocally }) }; } catch { return { availability: 'unavailable' }; }
      }
      return { availability: processLocally ? 'unavailable' : 'available' };
    },
    async installSpeechRecognition({ langs, processLocally }) {
      if (agent) return { installed: status().local };
      if (typeof Recognition?.install !== 'function') return { installed: false };
      try { return { installed: !!(await Recognition.install({ langs, processLocally: processLocally ?? true })) }; } catch { return { installed: false }; }
    },
    /** Under the agent: the preferred status (laid over the default, as the runner answers it) and the phrase heard. */
    preferred(values) {
      overlay = values?.speechRecognitionStatus && typeof values.speechRecognitionStatus === 'object' && !values.speechRecognitionStatus.error ? values.speechRecognitionStatus : {};
      const phrase = typeof values?.heard === 'string' ? values.heard : null;
      if (phrase !== heard) {
        heard = phrase;
        for (const r of recognitions.values()) advance(r);
        if (phrase) { phrases.push({ text: phrase, words: phrase.split(/\s+/).filter(Boolean), at: now() }); say(`heard: ${phrase}`); }
        announce();
      }
    },
    /** Recognitions whose reply was dropped: aborted, never the app's (D7). */
    dispose(ids) {
      for (const id of ids) {
        const r = recognitions.get(id);
        if (r && r.state !== 'ended') { say(`${id}: its reply was dropped; aborted`); r.interim = null; r.recognizer?.abort(); finish(r, 'aborted'); }
      }
    },
    /** Recognition is a foreground activity: leaving the foreground aborts every one (D10). */
    foreground(visible) {
      if (visible || shown()) return;
      for (const r of recognitions.values()) {
        advance(r);
        if (r.state !== 'ended') { say(`${r.id}: left the foreground; aborted`); r.interim = null; r.recognizer?.abort(); finish(r, 'aborted'); }
      }
    },
  };
}
