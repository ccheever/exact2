// @exact/media-capture on the web (LLP 1133.009 D1, D3, D6): the browser's
// `getUserMedia`, `enumerateDevices` and `permissions`, answered in the
// interface's shapes, with every track in the session's track table
// (`context.tracks`) so a package that consumes a track (`@exact/recording`)
// reaches it by id.
//
// - A track holds a lease on its capture while it has a consumer (D6). Its
//   first consumer takes the lease; after use, a track whose last consumer
//   went is `idle` (muted) and its browser track stopped, and only a start
//   (`subscribe(…, {start: true})`, `setTrackEnabled(enabled=true)`) opens it
//   again, as a new browser track under the same id. A reader joining an idle
//   track hears silence and opens nothing.
// - The browser opens its device at `getUserMedia` (that is where it asks).
//   A track no consumer joins within a second is stopped then, so a
//   forgotten track does not keep the browser's indicator on.
// - Under the agent no device is opened and nothing is asked: a stream with
//   one track, "Agent microphone", whose capture is the session clock's
//   synthetic signal (recording's), and whose mute follows the agent's
//   preferred `mediaStreamTrack` (D10 item 11).
// - Constraints are bare values, Media Capture's ideals (D1); what applied is
//   the browser's `getSettings()`.
// - The page's Audio Session is `play-and-record` while a lease is held (D7).

const GRACE = 1000; // ms a fresh browser track waits for its first consumer

/** The written constraints, and only those (D2). */
function constraints(args) {
  const c = args?.audioConstraints ?? {}, out = {};
  for (const [k, v] of Object.entries(c)) if (v != null) out[k] = v;
  return Object.keys(out).length ? out : true;
}

const failure = e => ({ error: typeof e?.name === 'string' && /Error$/.test(e.name) ? e.name : 'NotReadableError' });

export function implementation({ changed, agent, log, tracks, audioSession }) {
  const media = typeof navigator === 'object' ? navigator.mediaDevices : undefined;
  let next = 0, opened = false, overlay = null, devices = null;
  /** Each track this implementation opened: its record. */
  const own = new Map();
  const say = line => log?.(`media-capture: ${line}`);
  const announce = () => changed('media-capture.mediaStreamTrack');

  const shape = t => ({
    id: t.id, kind: 'audio', label: t.label, enabled: t.enabled,
    muted: t.muted, muteReason: t.muted ? t.muteReason : null,
    readyState: t.readyState, settings: t.settings, apple: null, android: null,
  });
  const each = (t, f) => { for (const c of [...t.consumers]) try { f(c); } catch (e) { say(`a consumer of ${t.id} failed: ${e.message}`); } };
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
  // The agent's interruption, or the browser's own mute, over a running capture.
  const muteNow = t => t.overlayMuted ? mute(t, t.overlayMuted) : t.source?.muted ? mute(t, 'interrupted') : unmute(t);

  /** The browser's track as this track's source: its events become the track's. */
  const adopt = (t, source) => {
    t.source = source;
    t.settings = settingsOf(source);
    source.onmute = () => t.active && muteNow(t);
    source.onunmute = () => t.active && muteNow(t);
    source.onended = () => { if (t.source === source && t.readyState === 'live') end(t, 'the device went away'); };
  };
  const stopSource = t => { clearTimeout(t.grace); t.source?.stop(); t.source = null; };

  /** Take the lease: the capture runs (D6). */
  async function activate(t) {
    t.active = true; t.used = true;
    t.claim ??= audioSession?.claim();
    if (!agent && (!t.source || t.source.readyState === 'ended')) {
      try {
        const stream = await media.getUserMedia({ audio: t.constraints });
        if (!t.active || t.readyState !== 'live') { for (const s of stream.getTracks()) s.stop(); return; }
        adopt(t, stream.getAudioTracks()[0]);
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
  /** Give the lease back: the capture stops and the track is idle. */
  function deactivate(t) {
    t.active = false;
    if (!agent) stopSource(t);
    t.claim?.release(); t.claim = null;
    mute(t, 'idle');
  }
  function end(t, why) {
    if (t.readyState === 'ended') return;
    t.readyState = 'ended'; t.active = false;
    stopSource(t);
    t.claim?.release(); t.claim = null;
    say(`${t.id}: ended (${why})`);
    each(t, c => c.ended?.());
    t.consumers.clear();
    announce();
  }

  const owner = {
    /** `consumer` joins `id` (`how.start`: a start, which reopens an idle track). */
    subscribe(id, consumer, how = {}) {
      const t = own.get(id);
      if (!t) throw Object.assign(new Error(`no track ${id}`), { code: 'NotFoundError' });
      if (t.readyState === 'ended') { consumer.ended?.(); return () => {}; }
      t.consumers.add(consumer);
      const origin = how.origin ?? 'app';
      // The first consumer leases; after use only a start does, and never a tool's or a wake's (D6, D9).
      const leases = origin === 'app' && t.enabled && (!t.used || how.start);
      if (t.active) {
        if (t.source || agent) consumer.live?.(t.source);
        if (t.muted) consumer.muted?.(t.muteReason);
      }
      else if (leases) activate(t);
      else consumer.muted?.(t.muteReason ?? 'idle');
      return () => {
        if (!t.consumers.delete(consumer)) return;
        if (!t.consumers.size && t.active) deactivate(t);
      };
    },
  };

  function settingsOf(source) {
    const s = source?.getSettings?.() ?? {}, out = {};
    for (const k of ['deviceId', 'groupId', 'sampleRate', 'sampleSize', 'channelCount', 'echoCancellation', 'autoGainControl', 'noiseSuppression', 'latency']) out[k] = s[k] ?? null;
    return out;
  }

  const agentSettings = { deviceId: 'agent-microphone', groupId: 'agent', sampleRate: 48000, sampleSize: 16, channelCount: 1, echoCancellation: false, autoGainControl: false, noiseSuppression: false, latency: 0.01 };

  return {
    async getUserMedia(args) {
      // No kind asked for is Media Capture's TypeError, before anything opens.
      if (!args?.audio && !args?.audioConstraints) return { error: 'TypeError' };
      if (!agent && !media?.getUserMedia) return { error: 'NotSupportedError' };
      let source = null;
      if (!agent) {
        try { source = (await media.getUserMedia({ audio: constraints(args) })).getAudioTracks()[0]; }
        catch (e) { return failure(e); }
      }
      opened = true;
      const n = ++next, stream = `stream-${n}`;
      const t = {
        id: `track-${n}`, stream, label: agent ? 'Agent microphone' : source.label, enabled: true,
        muted: true, muteReason: 'idle', readyState: 'live', settings: agent ? agentSettings : null,
        constraints: constraints(args), consumers: new Set(), active: false, used: false, source: null, claim: null,
        overlayMuted: overlay?.muted ? overlay.muteReason ?? 'interrupted' : null,
      };
      if (source) {
        adopt(t, source);
        t.grace = setTimeout(() => { if (!t.used && !t.active) { say(`${t.id}: no consumer joined; the browser's track stops`); stopSource(t); } }, GRACE);
      }
      own.set(t.id, t);
      tracks.add({ id: t.id, kind: 'audio', owner, extension: 'media-capture' });
      tracks.add({ id: stream, kind: 'stream', tracks: [t.id], extension: 'media-capture' });
      say(`${stream}: opened with ${t.id}`);
      announce();
      changed('media-capture.enumerateDevices');
      return { id: stream, active: true, tracks: [shape(t)] };
    },
    async stopTrack({ track }) {
      const t = own.get(track);
      if (!t) return { error: 'NotFoundError' };
      end(t, 'stopTrack');
      return shape(t);
    },
    async setTrackEnabled({ track, enabled }) {
      const t = own.get(track);
      if (!t) return { error: 'NotFoundError' };
      t.enabled = enabled;
      if (t.source) t.source.enabled = enabled;
      // Turning a track back on is a start: it reopens an idle one that has consumers (D6).
      if (enabled && !t.active && t.consumers.size && t.readyState === 'live') await activate(t);
      announce();
      return shape(t);
    },
    async mediaStreamTrack({ track }) {
      const t = own.get(track);
      return t ? shape(t) : { error: 'NotFoundError' };
    },
    async enumerateDevices() {
      if (agent) {
        const known = opened;
        return { devices: [{ deviceId: known ? 'agent-microphone' : '', kind: 'audioinput', label: known ? 'Agent microphone' : '', groupId: known ? 'agent' : '' }] };
      }
      if (!media?.enumerateDevices) return { error: 'NotSupportedError' };
      // Firefox says `devicechange` as a list is read after a grant: readers are asked again only when it differs.
      const read = async () => JSON.stringify((await media.enumerateDevices()).filter(d => d.kind === 'audioinput')
        .map(d => ({ deviceId: d.deviceId, kind: d.kind, label: d.label, groupId: d.groupId })));
      if (!media.ondevicechange) media.ondevicechange = () => read().then(now => { if (now !== devices) changed('media-capture.enumerateDevices'); }, () => {});
      devices = await read();
      return { devices: JSON.parse(devices) };
    },
    async microphoneStatus() {
      if (!media?.getUserMedia) return { available: false, state: 'unknown', apple: null, android: null };
      let state = 'unknown';
      try {
        const p = await navigator.permissions.query({ name: 'microphone' });
        p.onchange = () => changed('media-capture.microphoneStatus');
        state = p.state;
      } catch { /* a browser that does not name `microphone` */ }
      return { available: true, state, apple: null, android: null };
    },
    /** Under the agent: a preferred `mediaStreamTrack` is the platform's change to every live track (D10 item 11). */
    preferred(values) {
      overlay = values?.mediaStreamTrack ?? null;
      const reason = overlay?.muted ? overlay.muteReason ?? 'interrupted' : null;
      for (const t of own.values()) {
        if (t.readyState !== 'live') continue;
        t.overlayMuted = reason;
        if (t.active) muteNow(t);
      }
    },
    /** Handles a dropped reply carried: the stream's tracks end (D6). */
    dispose(ids) {
      for (const id of ids) {
        const entry = tracks.lookup(id);
        if (entry?.kind === 'stream') for (const t of entry.tracks) own.has(t) && end(own.get(t), 'disposed');
        else if (own.has(id)) end(own.get(id), 'disposed');
      }
    },
  };
}
