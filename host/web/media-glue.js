// @ref LLP 1042. Loaded on demand; the browser owns decoding, controls and time,
// for a `video` and an `audio` alike (§8).
const states = new WeakMap();
const booleans = new Set(['autoplay', 'controls', 'loop', 'muted', 'playsinline', 'disablepictureinpicture', 'disableremoteplayback']);
const numbers = { volume: [0, 1, 1], playbackRate: [0.25, 4, 1], currentTime: [0, Infinity, 0] };
// `error`'s payload is a stable code, never the engine's text (jukebox F6):
// MediaError's four (HTML), `not-allowed` for a play the browser refused and
// `invalid-value` for a number out of range. Apple's VideoArm.swift says the same.
const errorCodes = [null, 'aborted', 'network', 'decode', 'src-not-supported'];
const mediaEvents = new Set(['loadedmetadata','durationchange','timeupdate','play','playing','pause','ended','waiting','seeking','seeked','ratechange','volumechange','error','canplay']);
function syncPlayback(el) {
  const state = states.get(el), props = el.exactMedia.props;
  if (!state || state.retired) return;
  if (props.paused == null) { state.paused = undefined; return; }
  const paused = props.paused === 'true' || state.visibilityBlocked;
  if (paused !== state.paused || props.src !== state.applied.src) {
    state.paused = paused;
    if (paused) el.pause();
    // A play interrupted by a pause or a new load (AbortError) is no error,
    // and a refused source is the element's own `error` (jukebox F6): only
    // the browser's refusal to start (autoplay policy) is the play's.
    else el.play().catch(error => { if (!state.retired && !state.paused && error.name === 'NotAllowedError') state.error('not-allowed', error.message); });
  }
}
function syncVisibility(el) {
  const state = states.get(el), props = el.exactMedia.props;
  const value = props.playbackVisibilityThreshold == null ? NaN : Number(props.playbackVisibilityThreshold);
  const threshold = props.paused != null && Number.isFinite(value) && value >= 0 && value <= 1 ? value : null;
  if (state.threshold === threshold) return;
  state.observer?.disconnect(); state.observer = null;
  state.threshold = threshold;
  state.visibilityBlocked = threshold !== null;
  if (threshold === null) return;
  state.observer = new IntersectionObserver(entries => {
    if (state.retired || state.threshold !== threshold || !el.isConnected) return;
    const entry = entries[entries.length - 1];
    state.visibilityBlocked = !entry.isIntersecting || entry.intersectionRatio <= 0 || entry.intersectionRatio < threshold;
    syncPlayback(el);
  }, { threshold: [0, Math.max(Number.EPSILON, threshold)] });
  state.observer.observe(el);
}
function update(el) {
  const state = states.get(el), props = el.exactMedia.props;
  const changed = name => props[name] !== state.applied[name];
  for (const name of booleans) {
    el.toggleAttribute(name, props[name] === 'true');
    if (name === 'muted' && changed(name)) el.muted = props[name] === 'true';
  }
  for (const [name, [min, max, fallback]] of Object.entries(numbers)) {
    if (!changed(name) && !(name === "currentTime" && changed("src") && props.currentTime != null)) continue;
    const value = props[name] == null ? fallback : Number(props[name]);
    if (!Number.isFinite(value) || value < min || value > max) { state.error('invalid-value', `Invalid ${name}: ${props[name]}`); continue; }
    if (name === 'currentTime' && !el.readyState) state.seek = value;
    else { try { el[name] = value; } catch (error) { state.error('invalid-value', error.message); } }
  }
  if (changed('preservesPitch')) el.preservesPitch = props.preservesPitch !== 'false';
  el.disablePictureInPicture = props.disablepictureinpicture === 'true' || props.allowsPictureInPicturePlayback === 'false';
  syncVisibility(el);
  syncPlayback(el);
  state.applied = { ...props };
  for (const [name, seconds] of el.exactMedia.commands?.splice(0) ?? []) run(el, name, seconds);
}
// The commands by HTML's method names (podcast F8, F18), queued on the
// element by the host until the glue has it. `fastSeek` seeks every time,
// where the bound `currentTime` seeks only on a changed value; every host
// seeks to the exact time, which HTML's approximate-for-speed allows.
// `load` loads the source again as a changed `src` does: the bound
// `currentTime` waits for metadata and a bound `paused` false plays.
function run(el, name, seconds) {
  const state = states.get(el), props = el.exactMedia.props;
  if (name === 'fastSeek') {
    if (!Number.isFinite(seconds) || seconds < 0) state.error('invalid-value', `Invalid fastSeek: ${seconds}`);
    else if (!el.readyState) state.seek = seconds;
    else el.currentTime = seconds;
    return;
  }
  el.load();
  state.seek = props.currentTime == null ? null : Number(props.currentTime);
  state.paused = undefined;
  syncPlayback(el);
}
globalThis.exact.installMedia = (el, send) => {
  if (states.has(el)) { update(el); return; }
  // A retired element delivers nothing: a late report from a player the tree
  // removed would reach whatever now holds its place (jukebox F6, F20).
  const emit = (name, payload = '') => {
    if (!state.retired && el.isConnected && el.exactMedia.handlers.includes(name)) send(`${name}\n${payload}`);
  };
  const state = { applied: {}, seek: null, threshold: null, visibilityBlocked: false, retired: false, error(code, message) { if (!state.retired) console.warn(`exact: ${el.localName} ${code}: ${message}`); emit('error', code); } };
  states.set(el, state);
  // What the glue reported for itself on attaching (below): HTML sets
  // `readyState` before its queued event fires, so the event may still come;
  // it is not reported twice. A new load (`emptied`) forgets them.
  const early = new Set();
  el.addEventListener('emptied', () => early.clear());
  for (const name of mediaEvents) el.addEventListener(name, () => {
    if (name === 'loadedmetadata' && state.seek !== null) { el.currentTime = state.seek; state.seek = null; }
    if (early.delete(name)) return;
    const payload = name === 'timeupdate' ? el.currentTime : name === 'durationchange' ? el.duration : name === 'error' ? errorCodes[el.error?.code] ?? 'src-not-supported' : '';
    if (typeof payload !== 'number' || Number.isFinite(payload)) emit(name, String(payload));
  });
  update(el);
  if (el.readyState) {
    emit('loadedmetadata'); early.add('loadedmetadata');
    if (Number.isFinite(el.duration)) { emit('durationchange', String(el.duration)); early.add('durationchange'); }
  }
  // A source refused before the glue had the element (an unknown scheme, a
  // missing `app:/` file) failed while no one listened (podcast F19).
  else if (el.error) emit('error', errorCodes[el.error.code] ?? 'src-not-supported');
};

globalThis.exact.removeMedia = el => {
  const state = states.get(el);
  if (state) { state.retired = true; state.observer?.disconnect(); states.delete(el); }
};
