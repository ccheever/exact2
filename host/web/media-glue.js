// @ref LLP 1042. Loaded on demand; the browser owns decoding, controls and time,
// for a `video` and an `audio` alike (§8). The media session is LLP 1098's.
const states = new WeakMap();
const booleans = new Set(['autoplay', 'controls', 'loop', 'muted', 'playsinline', 'disablepictureinpicture', 'disableremoteplayback']);
const numbers = { volume: [0, 1, 1], playbackRate: [0.25, 4, 1], currentTime: [0, Infinity, 0] };
// `error`'s payload is a stable code, never the engine's text (jukebox F6):
// MediaError's four (HTML), `not-allowed` for a play the browser refused and
// `invalid-value` for a number out of range. Apple's VideoArm.swift says the same.
const errorCodes = [null, 'aborted', 'network', 'decode', 'src-not-supported'];
const mediaEvents = new Set(['loadedmetadata','durationchange','timeupdate','play','playing','pause','ended','waiting','seeking','seeked','ratechange','volumechange','error','canplay','fullscreenchange']);
function syncPlayback(el) {
  const state = states.get(el), props = el.exactMedia.props;
  if (!state || state.retired) return;
  if (props.paused == null) { state.paused = undefined; return; }
  // A remote play's latch (LLP 1098 D3) holds across the `paused` bound when
  // it was set, the app's stale `true` included; it ends when a later commit
  // writes `true` (a change from that value, or a re-write after the mirror).
  if (state.latched && props.paused !== state.latchedFrom) { if (props.paused === 'true') state.latched = false; else state.latchedFrom = props.paused; }
  const paused = !state.latched && (props.paused === 'true' || state.visibilityBlocked);
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
    if (!state.visibilityBlocked) state.latched = false; // above the threshold the authored value applies anyway
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
    // The element is already there: assigning the same time starts another seek.
    else if (!(name === 'currentTime' && el.currentTime === value)) { try { el[name] = value; } catch (error) { state.error('invalid-value', error.message); } }
  }
  if (changed('preservesPitch')) el.preservesPitch = props.preservesPitch !== 'false';
  el.disablePictureInPicture = props.disablepictureinpicture === 'true' || props.allowsPictureInPicturePlayback === 'false';
  for (const name of offsets) if (changed(name) && props[name] != null) {
    const value = Number(props[name]); // the last valid offset stands (LLP 1098 D2)
    if (Number.isFinite(value) && value > 0) state.offsets[name] = value; else state.error('invalid-value', `Invalid ${name}: ${props[name]}`);
  }
  syncVisibility(el);
  syncPlayback(el);
  state.applied = { ...props };
  for (const [name, seconds] of el.exactMedia.commands?.splice(0) ?? []) run(el, name, seconds);
  claim(el);
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
  const state = { applied: {}, seek: null, threshold: null, visibilityBlocked: false, retired: false, latched: false, offsets: { seekbackwardOffset: 10, seekforwardOffset: 10 }, emit, error(code, message) { if (!state.retired) console.warn(`exact: ${el.localName} ${code}: ${message}`); emit('error', code); } };
  states.set(el, state);
  // What the glue reported for itself on attaching (below): HTML sets
  // `readyState` before its queued event fires, so the event may still come;
  // it is not reported twice. A new load (`emptied`) forgets them.
  const early = new Set();
  el.addEventListener('emptied', () => early.clear());
  for (const name of mediaEvents) el.addEventListener(name, () => {
    if (name === 'loadedmetadata' && state.seek !== null) { el.currentTime = state.seek; state.seek = null; }
    if (early.delete(name)) return;
    const payload = name === 'timeupdate' ? el.currentTime : name === 'durationchange' ? el.duration : name === 'error' ? errorCodes[el.error?.code] ?? 'src-not-supported' : name === 'fullscreenchange' ? String(document.fullscreenElement === el) : '';
    if (typeof payload !== 'number' || Number.isFinite(payload)) emit(name, String(payload));
    if (!state.retired) played(el, name); // a retired element publishes nothing more (LLP 1098 D6)
  });
  update(el);
  if (el.readyState) {
    emit('loadedmetadata'); early.add('loadedmetadata');
    if (Number.isFinite(el.duration)) { emit('durationchange', String(el.duration)); early.add('durationchange'); }
    // A load that already finished its seek has fired these, or has them queued
    // (readyState moves first). Report the opening seek once. timeupdate repeats,
    // so its flag only covers the one already queued, not a later seek.
    if (el.readyState >= 2 && el.seeking !== true) {
      emit('seeking'); early.add('seeking');
      if (Number.isFinite(el.currentTime)) { emit('timeupdate', String(el.currentTime)); early.add('timeupdate'); }
      emit('seeked'); early.add('seeked');
    }
    // Metadata but no data yet (readyState 1, a cached source on the JS
    // target): the opening seek the glue makes at `loadedmetadata` has not
    // happened and that event has passed, so it is made now, as the wasm
    // host makes it (synthetic-media: seeks 1, at 0).
    else if (el.readyState === 1 && el.seeking !== true && el.exactMedia.props.currentTime != null) {
      const at = Number(el.exactMedia.props.currentTime);
      if (Number.isFinite(at) && at >= 0) el.currentTime = at;
    }
    if (el.readyState >= 3) { emit('canplay'); early.add('canplay'); }
    setTimeout(() => { early.delete('seeking'); early.delete('timeupdate'); early.delete('seeked'); early.delete('canplay'); }, 0);
  }
  // A source refused before the glue had the element (an unknown scheme, a
  // missing `app:/` file) failed while no one listened (podcast F19).
  else if (el.error) emit('error', errorCodes[el.error.code] ?? 'src-not-supported');
};

globalThis.exact.removeMedia = el => {
  const state = states.get(el);
  if (state) { state.retired = true; state.observer?.disconnect(); states.delete(el); }
  if (claimants.delete(el)) publish();
};

// @ref LLP 1098 — the media session, by the Media Session API's names. An
// element with `metadata=` claims it; the owner is the claimant whose player
// most recently reported `play`, kept after it pauses, else the latest
// mounted, the later in the document of one turn (D5). Only the owner's
// values reach `navigator.mediaSession`, and only when they change (D6).
const session = globalThis.navigator?.mediaSession ?? null;
const actions = ['seekbackward', 'seekforward', 'seekto', 'previoustrack', 'nexttrack', 'stop'];
const offsets = ['seekbackwardOffset', 'seekforwardOffset'];
const positioned = new Set(['loadedmetadata', 'durationchange', 'play', 'pause', 'ratechange', 'seeked', 'ended']);
const claimants = new Map(); // element → { mounted, played }: the turn each happened in
let turn = 0, turnOpen = false, owner = null, published = {};
const stamp = () => { if (!turnOpen) { turnOpen = true; turn++; queueMicrotask(() => { turnOpen = false; }); } return turn; };
const later = (a, b) => (a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_PRECEDING) !== 0; // `a` comes after `b`
function claim(el) {
  const claims = ['mediaTitle', 'mediaArtist', 'mediaAlbum', 'mediaArtwork'].some(name => el.exactMedia.props[name] != null);
  if (claims && !claimants.has(el)) claimants.set(el, { mounted: stamp(), played: 0 });
  else if (!claims) claimants.delete(el);
  publish();
}
function played(el, name) {
  if (name === 'play' && claimants.has(el)) claimants.get(el).played = stamp();
  if (name === 'play' || (el === owner && positioned.has(name))) publish(true);
}
function ownerOf() {
  let best = null, at = null;
  for (const [el, c] of claimants) {
    if (!el.isConnected || states.get(el)?.retired) continue;
    const key = c.played ? [1, c.played] : [0, c.mounted];
    if (!best || key[0] > at[0] || (key[0] === at[0] && (key[1] > at[1] || (key[1] === at[1] && later(el, best))))) { best = el; at = key; }
  }
  return best;
}
// The authored artwork, resolved where it is published: "" and an `app:/`
// source are none (the latter said in `state`); a development card or a
// release path through the host's `exact.assetURL`.
const artworkOf = src => !src || src.startsWith('app:/') ? [] : [{ src: globalThis.exact.assetURL?.(src) ?? src }];
function details(el) {
  const p = el.exactMedia.props;
  return { title: p.mediaTitle ?? '', artist: p.mediaArtist ?? '', album: p.mediaAlbum ?? '', artwork: p.mediaArtwork ?? '' };
}
const offered = el => el ? ['play', 'pause', ...actions.filter(a => el.exactMedia.handlers.includes(a))] : [];
function publish(position = false) {
  const el = ownerOf(), changedOwner = el !== owner; owner = el;
  const meta = el ? details(el) : null, metaKey = JSON.stringify(meta), offeredKey = offered(el).join();
  const playbackState = !el ? 'none' : el.paused ? 'paused' : 'playing';
  if (!session) return;
  if (metaKey !== published.meta) { published.meta = metaKey; session.metadata = meta ? new MediaMetadata({ ...meta, artwork: artworkOf(meta.artwork) }) : null; }
  if (offeredKey !== published.offered) { published.offered = offeredKey; for (const name of ['play', 'pause', ...actions]) try { session.setActionHandler(name, offeredKey.split(',').includes(name) ? remote[name] : null); } catch { /* an action this browser refuses */ } }
  if (playbackState !== published.state) { published.state = playbackState; session.playbackState = playbackState; }
  if (position || changedOwner) {
    // A position past the duration, a rate of 0 or a duration that is not finite throw (§1): clamped, the element's rate, or cleared.
    const d = el?.duration, ok = el && Number.isFinite(d) && d > 0;
    try { if (ok) session.setPositionState({ duration: d, position: Math.min(Math.max(el.currentTime, 0), d), playbackRate: el.playbackRate || 1 }); else session.setPositionState(); } catch { /* a state this browser refuses */ }
  }
}
// The platform's play and pause act on the owner (D3); a remote play latches
// over `playbackVisibilityThreshold`. The six send the element's event with
// `seekOffset seekTime fastSeek`, the element's offset when the platform gives
// none; a `seekto` without its time is not dispatched (D2).
const remote = {
  play() { const el = owner, state = el && states.get(el); if (!state || state.retired) return null; state.latched = true; state.latchedFrom = el.exactMedia.props.paused; state.paused = false; el.play().catch(error => { if (!state.retired && error.name === 'NotAllowedError') state.error('not-allowed', error.message); }); return {}; },
  pause() { const el = owner; if (!el || states.get(el)?.retired) return null; el.pause(); return {}; },
};
for (const name of actions) remote[name] = (d = {}) => {
  const el = owner, state = el && states.get(el);
  if (!state || state.retired || !el.isConnected || !el.exactMedia.handlers.includes(name)) return null;
  const seek = name === 'seekbackward' || name === 'seekforward', ok = n => Number.isFinite(n) && n >= 0;
  const seekOffset = seek ? (ok(d.seekOffset) && d.seekOffset > 0 ? d.seekOffset : state.offsets[`${name}Offset`]) : 0, seekTime = name === 'seekto' ? d.seekTime : 0;
  if (!ok(seekTime)) return null;
  state.emit(name, `${seekOffset} ${seekTime} ${d.fastSeek ? 1 : 0}`);
  return { seekOffset, seekTime };
};
const named = el => el.dataset.testid ? JSON.stringify(el.dataset.testid) : `view ${el.dataset.view ?? '?'}`;
globalThis.exact.mediaSession = {
  /** `state.mediaSession` (D10), ids by the target's `idOf`. */
  state(idOf) {
    const el = ownerOf(), meta = el && details(el);
    const d = el?.duration, read = session?.metadata;
    return {
      owner: el ? idOf(el) : null, testId: el?.dataset.testid ?? null,
      claimants: [...claimants.keys()].filter(c => c.isConnected).sort((a, b) => later(a, b) ? 1 : -1).map(idOf),
      metadata: meta, ...(meta?.artwork.startsWith('app:/') ? { artworkError: 'an app:/ artwork is not published until the media element takes an app:/ source (LLP 1098 §7)' } : {}),
      actions: offered(el).sort(), seekOffsets: el ? { seekbackward: states.get(el).offsets.seekbackwardOffset, seekforward: states.get(el).offsets.seekforwardOffset } : null,
      playbackState: !el ? 'none' : el.paused ? 'paused' : 'playing',
      position: el && Number.isFinite(d) && d > 0 ? { duration: d, position: Math.min(Math.max(el.currentTime, 0), d), playbackRate: el.playbackRate } : null,
      published: session ? 'navigator.mediaSession' : 'none',
      readback: session ? { title: read?.title ?? null, artist: read?.artist ?? null, album: read?.album ?? null, artwork: read ? [...read.artwork].map(a => a.src) : [], playbackStateDeclared: session.playbackState } : null,
    };
  },
  /** The driver's `tap <id> mediasession <action> [seconds]`: the handler the browser would call (D10), or why not. */
  act(id, action, seconds) {
    const el = globalThis.exact.views.get(id), own = ownerOf();
    if (!own) return { error: 'mediasession: there is no media session: no `audio` or `video` with `metadata=` is mounted' };
    if (el !== own) return { error: `mediasession: ${el ? named(el) : `view ${id}`} does not own the media session; ${named(own)} does` };
    if (!offered(own).includes(action)) return { error: `mediasession: ${named(own)} has no ${action}` };
    if (action === 'seekto' && seconds == null) return { error: 'mediasession: seekto needs the seconds to seek to' };
    const r = remote[action](action === 'seekto' ? { seekTime: seconds } : seconds != null ? { seekOffset: seconds } : {});
    return r ? { mediaSession: action, ...r } : { error: `mediasession: ${named(own)} did not take ${action}` };
  },
};
