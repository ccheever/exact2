// @ref LLP 1042. Loaded on demand; the browser owns decoding, controls and time.
const states = new WeakMap();
const booleans = new Set(['autoplay', 'controls', 'loop', 'muted', 'playsinline', 'disablepictureinpicture', 'disableremoteplayback']);
const numbers = { volume: [0, 1, 1], playbackRate: [0.25, 4, 1], currentTime: [0, Infinity, 0] };
const mediaEvents = new Set(['loadedmetadata','durationchange','timeupdate','play','playing','pause','ended','waiting','seeking','seeked','ratechange','volumechange','error','canplay']);
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
    if (!Number.isFinite(value) || value < min || value > max) { state.error(`Invalid ${name}: ${props[name]}`); continue; }
    if (name === 'currentTime' && !el.readyState) state.seek = value;
    else { try { el[name] = value; } catch (error) { state.error(error.message); } }
  }
  if (changed('preservesPitch')) el.preservesPitch = props.preservesPitch !== 'false';
  el.disablePictureInPicture = props.disablepictureinpicture === 'true' || props.allowsPictureInPicturePlayback === 'false';
  if ((changed('paused') || changed('src')) && props.paused != null) {
    if (props.paused === 'true') el.pause();
    else el.play().catch(error => state.error(error.message));
  }
  state.applied = { ...props };
}
globalThis.exact.installMedia = (el, send) => {
  if (states.has(el)) { update(el); return; }
  const emit = (name, payload = '') => {
    if (el.isConnected && el.exactMedia.handlers.includes(name)) send(`${name}\n${payload}`);
  };
  const state = { applied: {}, seek: null, error: message => emit('error', message) };
  states.set(el, state);
  for (const name of mediaEvents) el.addEventListener(name, () => {
    if (name === 'loadedmetadata' && state.seek !== null) { el.currentTime = state.seek; state.seek = null; }
    const payload = name === 'timeupdate' ? el.currentTime : name === 'durationchange' ? el.duration : name === 'error' ? el.error?.message || 'Media could not be loaded' : '';
    if (typeof payload !== 'number' || Number.isFinite(payload)) emit(name, String(payload));
  });
  update(el);
  if (el.readyState) { emit('loadedmetadata'); if (Number.isFinite(el.duration)) emit('durationchange', String(el.duration)); }
};
