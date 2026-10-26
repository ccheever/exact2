// @exact/speech-synthesis on the web (LLP 1133.012 D2–D6, §9): the browser's
// `speechSynthesis`, answered in the interface's shapes.
//
// - An utterance is an id (D2), kept with its state and outcome until the
//   page goes. The queue is this implementation's: the browser is given one
//   utterance at a time, the next when the last ends, so a canceled or
//   dropped utterance never reaches it and the state's `queue`, `utterance`
//   and last boundary are values.
// - A reply the runtime drops cancels its utterance (`dispose`): a queued
//   one never speaks, a speaking one is interrupted.
// - Spoken audio claims the page's Audio Session with the queue's strongest
//   type while it speaks and is not paused (D4); an unsaid type is `auto`,
//   which claims nothing.
// - Leaving the foreground cancels a `transient` or `transient-solo` queue,
//   each utterance ending `interrupted` (D4, "The background").
// - Under the agent nothing is spoken (§9): each utterance is spoken on the
//   session clock at 170 words a minute times `rate`, a `word` boundary at
//   each word's start, and `end` after the last; a paused queue's clock
//   stands still. The state is computed at each read, which the
//   declaration's poll asks every 100 ms of the session clock.

const WPM = 170;
const STRENGTH = ['transient', 'transient-solo', 'playback'];
const NO_UTTERANCE = { id: '', state: 'ended', outcome: null, error: null };
const NO_BOUNDARY = { charIndex: 0, charLength: 0, name: '', elapsedTime: 0 };

/** Each whitespace-separated word of `text`: its start and length in UTF-16 code units. */
export function words(text) {
  const out = [];
  for (const m of text.matchAll(/\S+/g)) out.push({ at: m.index, length: m[0].length });
  return out;
}

/** How long one word takes under the agent at `rate`, ms (§9). */
export const wordMs = (rate = 1) => 60000 / (WPM * (rate > 0 ? rate : 1));

export function implementation({ changed, agent, now, log, audioSession, foreground: shown = () => true }) {
  const synth = !agent && typeof speechSynthesis === 'object' ? speechSynthesis : null;
  const utterances = new Map(), reasons = new Set();
  let next = 0, queue = [], current = null, boundary = NO_BOUNDARY;
  let claim = null, claimed = 'auto', fail = null, failSeen = null, voicesSeen = false;
  const say = line => log?.(`speech-synthesis: ${line}`);
  const announce = () => { changed('speech-synthesis.speechSynthesisState'); changed('speech-synthesis.speechUtterance'); };

  // ------------------------------------------------------------ the claim (D4)
  /** The strongest type among the utterances not yet ended, `auto` for none. */
  const strongest = () => {
    const best = Math.max(-1, ...[current, ...queue].map(id => STRENGTH.indexOf(utterances.get(id)?.type)));
    return best < 0 ? 'auto' : STRENGTH[best];
  };
  /** Claim while the queue speaks and is not paused; the type chosen again whenever the queue changes. */
  function reclaim() {
    const want = (current || queue.length) && !reasons.size ? strongest() : 'auto';
    if (want === claimed) return;
    claim?.release(); claim = null; claimed = want;
    if (want !== 'auto') claim = audioSession?.claim({ type: want }) ?? null;
  }

  // ------------------------------------------------------------ the queue
  /** `u` begins at session time `at`: the browser is given it, or the agent's clock starts it. */
  function begin(u, at) {
    current = u.id; u.state = 'speaking'; u.began = at; u.paused = null;
    boundary = NO_BOUNDARY;
    if (agent && fail) { u.failure = fail; fail = null; }
    if (synth) synth.speak(u.browser);
    reclaim();
  }
  /** The next queued utterance begins, unless the queue is paused or one speaks. */
  function proceed(at = now()) {
    if (!current && queue.length && !reasons.size) begin(utterances.get(queue.shift()), at);
    if (!current && !queue.length) boundary = NO_BOUNDARY;
    reclaim();
  }
  /** `u` ends with `outcome` (and `error`) at session time `at`. */
  function end(u, outcome, error, at = now()) {
    if (u.state === 'ended') return;
    u.state = 'ended'; u.outcome = outcome; u.error = error;
    if (outcome === 'error') say(`${u.id}: ${error}`);
    queue = queue.filter(id => id !== u.id);
    if (current === u.id) { current = null; proceed(at); }
    reclaim();
  }

  /** Under the agent, the queue moved to the session clock's now: words reached, utterances ended, the next begun. */
  function advance() {
    if (!agent) return;
    const t = now();
    while (current) {
      const u = utterances.get(current);
      if (reasons.size) { u.paused ??= t; return; }
      if (u.paused != null) { u.began += t - u.paused; u.paused = null; }
      if (u.failure) { end(u, 'error', u.failure, u.began); continue; }
      const per = wordMs(u.rate), n = Math.floor(Math.max(0, t - u.began) / per);
      if (n >= u.words.length) { end(u, 'end', null, u.began + u.words.length * per); continue; }
      boundary = { charIndex: u.words[n].at, charLength: u.words[n].length, name: 'word', elapsedTime: n * per };
      return;
    }
  }

  /** The browser's utterance for `u` (D3: what the call omits stays the browser's), its events written into `u`. */
  function browser(u, args) {
    const s = new SpeechSynthesisUtterance(u.text);
    if (args.lang != null) s.lang = args.lang;
    if (args.voice != null) {
      const v = synth.getVoices().find(v => v.voiceURI === args.voice);
      if (!v) return 'voice-unavailable';
      s.voice = v;
    }
    for (const k of ['volume', 'rate', 'pitch']) if (args[k] != null) s[k] = args[k];
    const mine = () => current === u.id;
    s.onboundary = e => { if (mine()) { boundary = { charIndex: e.charIndex, charLength: e.charLength ?? 0, name: e.name ?? 'word', elapsedTime: e.elapsedTime ?? 0 }; announce(); } };
    s.onmark = e => { if (mine()) { boundary = { charIndex: e.charIndex, charLength: 0, name: e.name ?? '', elapsedTime: e.elapsedTime ?? 0 }; announce(); } };
    s.onend = () => { end(u, 'end', null); announce(); };
    s.onerror = e => { end(u, 'error', e.error ?? 'synthesis-failed'); announce(); };
    u.browser = s;
    return null;
  }

  // ------------------------------------------------------------ values
  const state = () => {
    advance();
    return {
      available: agent || !!synth, speaking: !!current, pending: queue.length > 0, paused: reasons.size > 0,
      pauseReasons: [...reasons], audioSessionType: claimed === 'auto' ? 'auto' : audioSession?.type?.() ?? claimed,
      utterance: current, ...boundary, queue: [...queue], apple: null,
    };
  };
  const utterance = u => ({ id: u.id, state: u.state === 'speaking' && reasons.size ? 'paused' : u.state, outcome: u.outcome, error: u.error });

  /** Every utterance still to end ends: the queued `canceled`, the speaking `interrupted` (D6). */
  function cancelAll() {
    advance();
    for (const id of queue) Object.assign(utterances.get(id), { state: 'ended', outcome: 'error', error: 'canceled' });
    queue = [];
    if (current) { Object.assign(utterances.get(current), { state: 'ended', outcome: 'error', error: 'interrupted' }); current = null; }
    reasons.delete('requested');
    boundary = NO_BOUNDARY;
    synth?.cancel();
    reclaim();
    announce();
  }
  function pause(reason) {
    advance();
    if (current) utterances.get(current).paused ??= now();
    reasons.add(reason);
    synth?.pause();
    reclaim();
    announce();
  }
  function resume(lifted) {
    advance();
    for (const r of lifted) reasons.delete(r);
    if (!reasons.size) { synth?.resume(); proceed(); }
    reclaim();
    announce();
  }

  function voices() {
    if (agent) {
      const base = (typeof navigator === 'object' && navigator.language) || 'en-US';
      return [
        { voiceURI: 'agent:en-US', name: 'Agent voice', lang: 'en-US', localService: true, default: true, apple: null, android: null },
        { voiceURI: `agent:${base}`, name: `Agent voice (${base})`, lang: base, localService: true, default: false, apple: null, android: null },
      ];
    }
    if (!synth) return [];
    if (!voicesSeen) { voicesSeen = true; synth.addEventListener?.('voiceschanged', () => changed('speech-synthesis.speechSynthesisVoices')); }
    return synth.getVoices().map(v => ({ voiceURI: v.voiceURI, name: v.name, lang: v.lang, localService: v.localService, default: v.default, apple: null, android: null }));
  }

  return {
    async speak(args) {
      if (!agent && !synth) return { error: 'NotSupportedError' };
      advance();
      const text = String(args.text ?? ''), id = `utterance-${++next}`;
      const u = { id, text, type: args.audioSessionType ?? 'auto', rate: args.rate ?? 1, state: 'queued', outcome: null, error: null, words: words(text) };
      utterances.set(id, u);
      const refused = synth && browser(u, args);
      if (refused) { end(u, 'error', refused); announce(); return { id }; }
      queue.push(id);
      say(`${id}: queued (${u.words.length} words, ${u.type})`);
      proceed();
      announce();
      return { id };
    },
    async cancelSpeech() { cancelAll(); return state(); },
    async pauseSpeech() { pause('requested'); return state(); },
    async resumeSpeech() { resume(['requested', 'interrupted', 'focus', 'route']); return state(); },
    async speechSynthesisState() { return state(); },
    async speechUtterance({ utterance: id }) {
      if (!id) return NO_UTTERANCE;
      advance();
      const u = utterances.get(id);
      return u ? utterance(u) : { error: 'NotFoundError' };
    },
    async speechSynthesisVoices() { return { voices: voices() }; },
    /** Under the agent: `fail` makes the next utterance to begin end with that error (§9). Each value preferred
     * arms it once; the queue is first brought to now, so the one speaking is not the one that fails. */
    preferred(values) {
      const word = typeof values?.fail === 'string' ? values.fail : null;
      if (word === failSeen) return;
      failSeen = word;
      if (!word) return;
      advance();
      fail = word;
      say(`the next utterance fails ${fail}`);
    },
    /** Utterances whose reply was dropped: never the app's (D2). */
    dispose(ids) {
      advance();
      for (const id of ids) {
        const u = utterances.get(id);
        if (!u || u.state === 'ended') continue;
        say(`${id}: its reply was dropped`);
        // The browser's cancel ends only the one it speaks: the rest of the queue is this implementation's.
        if (current === id) synth?.cancel();
        end(u, 'error', current === id ? 'interrupted' : 'canceled');
      }
      announce();
    },
    /** A prompt the person can no longer see the reason for is canceled at the background (D4). */
    foreground(visible) {
      if (visible || shown() || (!current && !queue.length)) return;
      if (strongest() !== 'playback') { say('left the foreground: the queue is canceled'); cancelAll(); }
    },
  };
}
