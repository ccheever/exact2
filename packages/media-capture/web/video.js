// @exact/media-capture's video half on the web (LLP 1133.008 D1-D3, D6): the
// browser's cameras as Media Capture's video tracks, each in the session's
// track table (`tracks`) so a package that consumes one (`@exact/camera`'s
// viewfinder and `takePhoto`) reaches it by id. index.js routes its names
// here for a video track and for `video=`.
//
// - A track holds a lease while it has a consumer (D3). A consumer that shows
//   the capture takes it whenever its origin is the person's or the app's own
//   code and the page is shown; a tool's or a wake's consumer never does
//   (LLP 1133.008 §14, "a visible consumer"). The last consumer gone, the
//   browser's track is stopped (`enabled = false` lets a browser keep the
//   camera and its indicator on) and the track reads `muted`, `idle`; the next
//   lease opens the camera again under the same id, by its `deviceId`.
// - The browser opens its camera at `getUserMedia`, where it asks. A track no
//   consumer joins within a second is stopped then.
// - `applyConstraints` with another `deviceId` or `facingMode` moves the track
//   to that camera under the same id (D1); the others are the browser's track's
//   own constraints. Zoom is not offered: Image Capture puts it behind the
//   `panTiltZoom` permission (§12).
// - Under the agent no camera opens and nothing is asked: two cameras, "Agent
//   back camera" (`environment`, with a torch) and "Agent front camera"
//   (`user`), whose consumers are told `live(null)` and draw the agent's
//   picture themselves (§9). A preferred `mediaStreamTrack` mutes every live
//   track, as an interruption would (LLP 1133.009 D10 item 11).
// - An observer (LLP 1133.027 D17 item 1: a barcode scan) hears what a
//   consumer hears and attaches to the capture the consumers keep, but takes,
//   holds and reopens no lease. Under the agent the camera package hands the
//   track the picture its viewfinder draws (`picture`), which an observer
//   reads as the track's frame (D17 item 2).

const GRACE = 1000; // ms a fresh browser track waits for its first consumer
const MEMBERS = ['deviceId', 'groupId', 'facingMode', 'width', 'height', 'aspectRatio', 'frameRate', 'resizeMode', 'zoom', 'torch'];
const AGENT = {
  environment: { deviceId: 'agent-camera-back', label: 'Agent back camera', torch: true },
  user: { deviceId: 'agent-camera-front', label: 'Agent front camera', torch: false },
};

const failure = e => ({ error: typeof e?.name === 'string' && /Error$/.test(e.name) ? e.name : 'NotReadableError' });

/** The constraints written, and only those (D2): bare values, Media Capture's ideals. */
function written(c) {
  const out = {};
  for (const k of MEMBERS) if (c?.[k] != null) out[k] = c[k];
  return out;
}

export function video({ changed, agent, log, tracks, nextId, media, shown }) {
  const own = new Map(), origins = new WeakMap();
  let overlay = null, opened = false, devices = null;
  const say = line => log?.(`media-capture: ${line}`);
  const announce = () => changed('media-capture.mediaStreamTrack');

  const shape = t => ({
    id: t.id, kind: 'video', label: t.label, enabled: t.enabled,
    muted: t.muted, muteReason: t.muted ? t.muteReason : null, readyState: t.readyState,
    settings: { ...t.settings }, capabilities: t.capabilities, apple: null, android: null,
  });
  const each = (t, f) => { for (const c of [...t.consumers, ...t.observers]) try { f(c); } catch (e) { say(`a consumer of ${t.id} failed: ${e.message}`); } };
  const mute = (t, reason) => {
    if (t.muted && t.muteReason === reason) return;
    t.muted = true; t.muteReason = reason;
    each(t, c => c.muted?.(reason));
    announce();
  };
  const unmute = t => {
    if (!t.muted) return;
    t.muted = false; t.muteReason = null;
    each(t, c => c.unmuted?.());
    announce();
  };
  // The agent's interruption, or the browser's own mute (no reason a page can read: `interrupted`), over a lease.
  const muteNow = t => t.overlayMuted ? mute(t, t.overlayMuted) : t.source?.muted ? mute(t, 'interrupted') : unmute(t);

  /** What the agent's camera `facing` applies and offers. */
  const agentCamera = facing => {
    const c = AGENT[facing] ?? AGENT.environment;
    return {
      label: c.label,
      settings: { deviceId: c.deviceId, groupId: 'agent', facingMode: facing, width: 1280, height: 960, aspectRatio: 4 / 3, frameRate: 30, resizeMode: 'none', zoom: null, torch: c.torch ? false : null },
      capabilities: { facingMode: [facing], width: { min: 640, max: 1280, step: 1 }, height: { min: 480, max: 960, step: 1 }, aspectRatio: { min: 4 / 3, max: 4 / 3, step: null }, frameRate: { min: 1, max: 30, step: 1 }, zoom: null, torch: c.torch },
    };
  };

  /** The browser's track as this track's source: its settings, and its events as the track's. */
  const adopt = (t, source) => {
    t.source = source;
    source.enabled = t.enabled;
    const s = source.getSettings?.() ?? {}, c = source.getCapabilities?.() ?? {};
    t.label = source.label;
    t.settings = Object.fromEntries(MEMBERS.map(k => [k, s[k] ?? null]));
    const range = r => r && typeof r.min === 'number' ? { min: r.min, max: r.max, step: r.step ?? null } : null;
    t.capabilities = {
      facingMode: Array.isArray(c.facingMode) ? c.facingMode : null, width: range(c.width), height: range(c.height),
      aspectRatio: range(c.aspectRatio), frameRate: range(c.frameRate), zoom: null, torch: typeof c.torch === 'boolean' ? c.torch : null,
    };
    source.onmute = () => t.active && muteNow(t);
    source.onunmute = () => t.active && muteNow(t);
    source.onended = () => { if (t.source === source && t.readyState === 'live') end(t, 'the camera went away'); };
  };
  const stopSource = t => { clearTimeout(t.grace); t.source?.stop(); t.source = null; };

  /** The camera again, by the device the track last used (D3). */
  const reopen = async t => {
    const c = { ...t.constraints };
    if (t.settings.deviceId) { delete c.facingMode; c.deviceId = { exact: t.settings.deviceId }; }
    const stream = await media.getUserMedia({ video: Object.keys(c).length ? c : true });
    return stream.getVideoTracks()[0];
  };

  /** Take the lease: the camera runs. */
  async function activate(t) {
    t.active = true;
    if (!agent && (!t.source || t.source.readyState === 'ended')) {
      try {
        const source = await reopen(t);
        if (!t.active || t.readyState !== 'live') { source.stop(); return; }
        adopt(t, source);
        say(`${t.id}: opened again`);
      } catch (e) {
        say(`${t.id}: could not open again: ${failure(e).error}`);
        t.active = false;
        return end(t, failure(e).error);
      }
    }
    clearTimeout(t.grace);
    each(t, c => c.live?.(t.source));
    muteNow(t);
  }
  /** Give the lease back: the camera stops and the track is idle. */
  function deactivate(t) {
    t.active = false;
    if (!agent) stopSource(t);
    mute(t, 'idle');
  }
  function end(t, why) {
    if (t.readyState === 'ended') return;
    t.readyState = 'ended'; t.active = false;
    stopSource(t);
    say(`${t.id}: ended (${why})`);
    each(t, c => c.ended?.());
    t.consumers.clear();
    t.observers.clear();
    announce();
  }

  const owner = {
    /** `consumer` joins video track `id`; how = {origin}. A consumer shows the capture, so it leases. */
    subscribe(id, consumer, how = {}) {
      const t = own.get(id);
      if (!t) throw Object.assign(new Error(`no track ${id}`), { code: 'NotFoundError' });
      if (t.readyState === 'ended') { consumer.ended?.(); return () => {}; }
      t.consumers.add(consumer);
      const origin = how.origin ?? 'app';
      origins.set(consumer, origin);
      if (t.active) {
        consumer.live?.(t.source);
        if (t.muted) consumer.muted?.(t.muteReason);
      } else if (origin === 'app' && t.enabled && shown()) {
        say(`${t.id}: a consumer takes the lease (${origin} origin)`);
        activate(t);
      } else {
        say(`${t.id}: a consumer takes no lease (${origin} origin${shown() ? '' : ', in the background'})`);
        consumer.muted?.(t.muteReason ?? 'idle');
      }
      return () => {
        if (!t.consumers.delete(consumer)) return;
        if (!t.consumers.size && t.active) deactivate(t);
      };
    },
    /** `observer` hears track `id` as a consumer does, and never leases (LLP 1133.027 D17 item 1). */
    observe(id, observer) {
      const t = own.get(id);
      if (!t) throw Object.assign(new Error(`no track ${id}`), { code: 'NotFoundError' });
      if (t.readyState === 'ended') { observer.ended?.(); return () => {}; }
      t.observers.add(observer);
      say(`${t.id}: an observer joins, taking no lease`);
      if (t.active) {
        observer.live?.(t.source);
        if (t.muted) observer.muted?.(t.muteReason);
      } else observer.muted?.(t.muteReason ?? 'idle');
      return () => { if (t.observers.delete(observer)) say(`${t.id}: an observer leaves`); };
    },
    /** Under the agent: the picture a viewfinder of track `id` draws, a promise of an `ImageBitmap` or null (D17 item 2). */
    picture(id, frame) {
      const t = own.get(id);
      if (t) t.picture = frame;
    },
  };

  return {
    owns: id => own.has(id),
    /** What `getUserMedia(video=…)` asks the browser for. */
    asked: args => { const c = written(args.videoConstraints); return Object.keys(c).length ? c : true; },
    /** A video track for `getUserMedia(video=…)`, in `stream`, from the browser's track `given` if the caller opened
     * it: its shape, or `{error}`. */
    async open(args, stream, given) {
      if (!agent && !media?.getUserMedia) return { error: 'NotSupportedError' };
      const constraints = written(args.videoConstraints);
      let source = given ?? null;
      if (!agent && !source) {
        try { source = (await media.getUserMedia({ video: Object.keys(constraints).length ? constraints : true })).getVideoTracks()[0]; }
        catch (e) { return failure(e); }
      }
      opened = true;
      const t = {
        id: `track-${nextId()}`, stream, kind: 'video', label: '', enabled: true, muted: true, muteReason: 'idle', readyState: 'live',
        settings: {}, capabilities: null, constraints, consumers: new Set(), observers: new Set(), active: false, source: null, grace: null, picture: null,
        overlayMuted: overlay?.muted ? overlay.muteReason ?? 'interrupted' : null,
      };
      if (agent) Object.assign(t, agentCamera(constraints.facingMode === 'user' ? 'user' : 'environment'));
      else {
        adopt(t, source);
        t.grace = setTimeout(() => { if (!t.active) { say(`${t.id}: no consumer joined; the browser's track stops`); stopSource(t); } }, GRACE);
      }
      own.set(t.id, t);
      tracks.add({ id: t.id, kind: 'video', owner, extension: 'media-capture', state: () => shape(t), source: () => t.source, frame: () => t.picture });
      say(`${stream}: opened with ${t.id} (${t.label})`);
      announce();
      changed('media-capture.enumerateDevices');
      return shape(t);
    },
    async applyConstraints(args) {
      const t = own.get(args.track);
      if (!t) return { error: 'NotFoundError' };
      if (t.readyState === 'ended') return shape(t);
      const c = written(args);
      const moves = (c.deviceId != null && c.deviceId !== t.settings.deviceId) || (c.facingMode != null && c.facingMode !== t.settings.facingMode);
      if (moves) {
        // Another camera under the same id (D1): a capture start, which the page must be shown for (D6).
        if (!shown()) return { error: 'NotAllowedError' };
        const next = { ...t.constraints, ...c };
        if (c.deviceId != null) delete next.facingMode;
        if (c.facingMode != null) delete next.deviceId;
        t.constraints = next;
        if (agent) {
          const facing = c.facingMode ?? (c.deviceId === AGENT.user.deviceId ? 'user' : 'environment');
          Object.assign(t, agentCamera(facing));
        } else if (t.source) {
          stopSource(t);
          t.settings = { ...t.settings, deviceId: null };
          try { adopt(t, (await media.getUserMedia({ video: next })).getVideoTracks()[0]); }
          catch (e) { say(`${t.id}: could not move: ${failure(e).error}`); end(t, failure(e).error); return failure(e); }
        }
        say(`${t.id}: moved to ${t.label}`);
        if (t.active) each(t, x => x.live?.(t.source));
        announce();
        return shape(t);
      }
      const rest = { ...c };
      delete rest.deviceId; delete rest.facingMode; delete rest.groupId; delete rest.zoom;
      t.constraints = { ...t.constraints, ...rest };
      if (agent) {
        if (rest.torch != null && t.capabilities?.torch) t.settings.torch = !!rest.torch;
        for (const k of ['width', 'height', 'frameRate']) if (rest[k] != null && t.capabilities?.[k]) t.settings[k] = Math.max(t.capabilities[k].min, Math.min(t.capabilities[k].max, rest[k]));
      } else if (t.source) {
        const { torch, ...plain } = rest;
        try {
          await t.source.applyConstraints({ ...plain, ...(torch != null ? { advanced: [{ torch: !!torch }] } : {}) });
          adopt(t, t.source);
        } catch (e) { say(`${t.id}: constraints not applied: ${failure(e).error}`); }
      }
      announce();
      return shape(t);
    },
    stopTrack(id) { const t = own.get(id); end(t, 'stopTrack'); return shape(t); },
    async setTrackEnabled(id, enabled) {
      const t = own.get(id);
      t.enabled = enabled;
      if (t.source) t.source.enabled = enabled;
      // Turning a track back on is a start: it reopens an idle one that has consumers (D6).
      if (enabled && !t.active && t.consumers.size && t.readyState === 'live' && shown()) await activate(t);
      announce();
      return shape(t);
    },
    track: id => shape(own.get(id)),
    /** Under the agent, each track for `state.extensions` (LLP 1133.008 §9, Disclosure). */
    disclosed: () => [...own.values()].map(t => ({
      ...shape(t), consumers: [...t.consumers].map(c => origins.get(c)).sort(), observers: t.observers.size, lease: t.active,
    })),
    /** The cameras, as `MediaDeviceInfo`s. */
    async devices() {
      if (agent) {
        return opened
          ? Object.entries(AGENT).map(([facing, c]) => ({ deviceId: c.deviceId, kind: 'videoinput', label: c.label, groupId: 'agent', facingMode: facing }))
          : [{ deviceId: '', kind: 'videoinput', label: '', groupId: '', facingMode: null }];
      }
      if (!media?.enumerateDevices) return [];
      const read = async () => JSON.stringify((await media.enumerateDevices()).filter(d => d.kind === 'videoinput')
        .map(d => ({ deviceId: d.deviceId, kind: d.kind, label: d.label, groupId: d.groupId, facingMode: d.getCapabilities?.().facingMode?.[0] ?? null })));
      media.addEventListener?.('devicechange', () => read().then(now => { if (now !== devices) changed('media-capture.enumerateDevices'); }, () => {}), { once: true });
      devices = await read();
      return JSON.parse(devices);
    },
    async cameraStatus() {
      if (!media?.getUserMedia) return { available: false, state: 'unknown', apple: null, android: null };
      let state = 'unknown';
      try {
        const p = await navigator.permissions.query({ name: 'camera' });
        p.onchange = () => changed('media-capture.cameraStatus');
        state = p.state;
      } catch { /* a browser that does not name `camera` */ }
      return { available: true, state, apple: null, android: null };
    },
    /** Under the agent: a preferred `mediaStreamTrack` is the platform's change to every live track. */
    preferred(value) {
      overlay = value ?? null;
      const reason = overlay?.muted ? overlay.muteReason ?? 'interrupted' : null;
      for (const t of own.values()) {
        if (t.readyState !== 'live') continue;
        if (overlay?.readyState === 'ended') { end(t, 'the agent ended it'); continue; }
        t.overlayMuted = reason;
        if (t.active) muteNow(t);
      }
    },
    end: (id, why) => own.has(id) && end(own.get(id), why),
    /** The page was hidden or shown: a hidden page's camera mutes, `background`, and is let go (D6). */
    foreground(visible) {
      for (const t of own.values()) {
        if (t.readyState !== 'live' || !t.consumers.size) continue;
        if (!visible && t.active) { deactivate(t); mute(t, 'background'); }
        else if (visible && !t.active && t.muteReason === 'background') activate(t);
      }
    },
  };
}
