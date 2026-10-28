// @exact/camera on the web (LLP 1133.008 D1-D3, D8, §9): the viewfinder
// `exact-camera`, a `<video>` showing a video track of @exact/media-capture's,
// and Image Capture's `takePhoto` from it, written to the app's files.
//
// - A viewfinder is a consumer of its track while it is displayed and its
//   `srcObject` names the track (D3). The runner says when either changes and
//   by whose commit (`displayed`, `bound`); the viewfinder joins the track
//   with that origin, so a lease is taken only for the person's input or the
//   app's own code (§14, "a visible consumer"), and a viewfinder a tool showed
//   shows nothing until the person shows it again.
// - `takePhoto` needs a track that is live, enabled and unmuted, so shown
//   (D1): `ImageCapture.takePhoto` where the browser has it, else the
//   viewfinder's current frame encoded as JPEG (Firefox), with no fill light.
//   The photo is written to `path` with its format's extension, or to
//   `app:/tmp/camera/<n>.jpg` (D8 item 4).
// - Under the agent no camera exists: the viewfinder shows the agent's
//   picture, mirrored for a `user` camera, and `takePhoto` writes the
//   picture's bytes unchanged (§9). The picture is the package's test card,
//   or the app asset `prefer extension camera.picture={"path":"…"}` names.
//   The track's frame is that picture, unmirrored, for an observer such as a
//   barcode scan to read (LLP 1133.027 D17 item 2).

const MIME = { 'image/jpeg': 'jpg', 'image/png': 'png', 'image/webp': 'webp', 'image/heic': 'heic' };

/** Image Capture's settings the call wrote (D1), and only those. */
function photoSettings(args) {
  const out = {};
  for (const k of ['fillLightMode', 'imageWidth', 'imageHeight', 'redEyeReduction']) if (args?.[k] != null) out[k] = args[k];
  return out;
}

class Viewfinder {
  constructor({ implementation, element, props, events, view }) {
    const doc = element.ownerDocument;
    this.view = view; this.implementation = implementation; this.events = events;
    this.box = doc.createElement('div');
    this.box.style.cssText = 'width:100%;height:100%;overflow:hidden;background:#000;position:relative';
    this.video = doc.createElement('video');
    this.video.autoplay = true; this.video.playsInline = true; this.video.muted = true;
    this.picture = doc.createElement('img');
    this.picture.alt = '';
    for (const el of [this.video, this.picture]) {
      el.style.cssText = 'position:absolute;inset:0;width:100%;height:100%;display:none';
      this.box.append(el);
    }
    this.video.addEventListener('loadeddata', () => this.events.load());
    this.picture.addEventListener('load', () => this.events.load());
    element.append(this.box);
    this.setProps(props);
    implementation.made(view, this);
  }
  setProps(props) {
    // `contain` unsaid, as `<video>` renders (D2).
    const fit = props['object-fit'] || 'contain';
    this.video.style.objectFit = this.picture.style.objectFit = fit;
    this.implementation.propsOf(this.view, props);
  }
  /** The track's frames: a browser track, or under the agent the picture's URL. */
  show(source, picture, mirrored) {
    const flip = mirrored ? 'scaleX(-1)' : '';
    if (picture) {
      this.video.style.display = 'none';
      this.video.srcObject = null;
      this.picture.style.transform = flip;
      this.picture.style.display = 'block';
      if (this.picture.getAttribute('src') !== picture) this.picture.src = picture;
      return;
    }
    this.picture.style.display = 'none';
    // A page mirrors a front camera's preview itself (D2's web column: none); the module does, as the platforms do.
    this.video.style.transform = flip;
    this.video.style.display = 'block';
    if (this.video.srcObject?.getVideoTracks?.()[0] !== source) this.video.srcObject = source ? new MediaStream([source]) : null;
  }
  blank() {
    this.video.srcObject = null;
    this.video.style.display = this.picture.style.display = 'none';
  }
  destroy() { this.implementation.gone(this.view); this.box.remove(); }
}

export const roster = { 'exact-camera': { view: Viewfinder } };

export function implementation({ changed, agent, log, tracks, files }) {
  const say = line => log?.(`camera: ${line}`);
  /** Each viewfinder by its node's id: what the runner said of it and what it consumes. */
  const views = new Map();
  let picture = null, pictureURL = null, pictureBitmap = null, taken = 0;
  const photos = [];
  const viewOf = id => {
    if (!views.has(id)) views.set(id, { el: null, src: null, changes: 0, reports: 0, shown: false, madeBy: 'app', sub: null });
    return views.get(id);
  };
  const facingOf = id => tracks.lookup(id)?.state?.()?.settings?.facingMode ?? null;

  /** The agent's picture as a URL the page can show and fetch. */
  async function agentPicture() {
    if (pictureURL) return pictureURL;
    if (picture) pictureURL = new URL(picture, document.baseURI).href;
    else {
      const { card } = await import('./card.js');
      pictureURL = `data:image/jpeg;base64,${card}`;
    }
    return pictureURL;
  }

  /** The agent's picture decoded, once per picture. */
  function agentBitmap() {
    return pictureBitmap ??= agentPicture().then(url => fetch(url)).then(r => r.blob()).then(b => createImageBitmap(b))
      .catch(e => { say(`the agent's picture did not decode: ${e.message}`); return null; });
  }
  /** Track `id`'s frame is the picture drawn, unmirrored, as a sensor's is (LLP 1133.027 D17 item 2): set when the
   *  picture is named, so the next analysed frame is the new one however long its decode takes. */
  const framed = id => tracks.lookup(id)?.owner?.picture?.(id, agentBitmap());

  /** Viewfinder `id` consumes its track while it is made, displayed and bound to a video track (D3). */
  function reconcile(id) {
    const v = viewOf(id), entry = tracks.lookup(v.src);
    // A `srcObject` the runner has not yet reported is not yet the commit's: wait for its report and origin.
    const want = !!(v.el && v.shown && v.src && v.reports >= v.changes && entry?.kind === 'video');
    if (v.sub && (!want || v.sub.track !== v.src)) { v.sub.leave(); v.sub = null; v.el?.blank(); }
    // Shown again by the person or the app, a viewfinder that took no lease takes one (§14).
    if (want && v.sub && !v.sub.live && v.madeBy === 'app' && v.sub.origin !== 'app') { v.sub.leave(); v.sub = null; }
    if (!want || v.sub) return;
    const sub = { track: v.src, origin: v.madeBy, live: false, leave: () => {} };
    const show = source => {
      if (v.sub !== sub || !v.el) return;
      const mirrored = facingOf(sub.track) === 'user';
      if (agent) {
        framed(sub.track);
        agentPicture().then(url => v.sub === sub && sub.live && v.el?.show(null, url, mirrored));
      } else v.el.show(source, null, mirrored);
    };
    let source = null;
    const consumer = {
      live(s) { source = s; sub.live = true; show(s); },
      muted() { if (v.sub === sub) v.el?.blank(); },
      unmuted() { if (sub.live) show(source); },
      ended() { sub.live = false; if (v.sub === sub) v.el?.blank(); },
    };
    v.sub = sub;
    try {
      sub.leave = tracks.subscribe(v.src, consumer, { origin: v.madeBy });
      say(`viewfinder ${id} consumes ${v.src} (${v.madeBy} origin)`);
    } catch (e) {
      v.sub = null;
      say(`viewfinder ${id}: ${e.message}`);
    }
  }

  /** The bytes and size of the photo `t` would take now. */
  async function capture(id, args) {
    if (agent) {
      const blob = await (await fetch(await agentPicture())).blob();
      return blob.type ? blob : new Blob([blob], { type: 'image/jpeg' });
    }
    const source = tracks.lookup(id)?.source?.();
    if (!source) throw Object.assign(new Error('no camera'), { name: 'InvalidStateError' });
    if (typeof ImageCapture === 'function') return new ImageCapture(source).takePhoto(photoSettings(args));
    // Firefox: the frame a viewfinder of this track shows, at the stream's resolution.
    const shown = [...views.values()].find(v => v.sub?.track === id && v.sub.live && v.el)?.el.video;
    if (!shown?.videoWidth) throw Object.assign(new Error('no frame shown'), { name: 'InvalidStateError' });
    const canvas = Object.assign(document.createElement('canvas'), { width: shown.videoWidth, height: shown.videoHeight });
    canvas.getContext('2d').drawImage(shown, 0, 0);
    return new Promise((done, fail) => canvas.toBlob(b => b ? done(b) : fail(new Error('the frame did not encode')), 'image/jpeg', 0.92));
  }

  return {
    made(view, el) { viewOf(view).el = el; reconcile(view); },
    /** A viewfinder's props: a new `srcObject` waits for the runner's report of it. */
    propsOf(view, props) {
      // Each value the runner reports a `bound` for, `""` included; an absent prop is reported never.
      const v = viewOf(view), src = props.srcObject;
      if (src == null) v.src = '';
      else if (src !== v.src) { v.src = src; v.changes += 1; }
      reconcile(view);
    },
    gone(view) { const v = viewOf(view); v.sub?.leave(); views.delete(view); },
    displayed(view, shown, origin) {
      const v = viewOf(view);
      v.shown = shown;
      if (shown) v.madeBy = origin;
      reconcile(view);
    },
    bound(view, prop, origin) {
      if (prop !== 'srcObject') return;
      const v = viewOf(view);
      v.reports += 1; v.madeBy = origin;
      reconcile(view);
    },
    async takePhoto(args) {
      const entry = tracks.lookup(args?.track);
      if (!entry) return { error: 'NotFoundError' };
      if (entry.kind !== 'video') return { error: 'NotSupportedError' };
      const s = entry.state?.() ?? {};
      // A track that is not shown takes no photo (D1): ended, disabled, or muted (`idle` among them).
      if (s.readyState !== 'live' || !s.enabled || s.muted) return { error: 'InvalidStateError' };
      let blob;
      try { blob = await capture(args.track, args); }
      catch (e) { say(`takePhoto: ${e.message}`); return { error: /Error$/.test(e?.name ?? '') ? e.name : 'UnknownError' }; }
      const type = blob.type || 'image/jpeg', ext = MIME[type] ?? 'jpg';
      const path = args.path ? (args.path.endsWith(`.${ext}`) ? args.path : `${args.path}.${ext}`) : `app:/tmp/camera/${++taken}.${ext}`;
      const bytes = new Uint8Array(await blob.arrayBuffer());
      let width = 0, height = 0;
      try { const b = await createImageBitmap(blob); width = b.width; height = b.height; b.close?.(); } catch { /* sized by nothing */ }
      try { await files.write(path, bytes); }
      catch (e) { say(`takePhoto: ${path} not written: ${e.message}`); return { error: 'UnknownError' }; }
      // The journal names each photo's SHA-256, which a drive compares with the agent's picture (§9).
      const sha = [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))].map(b => b.toString(16).padStart(2, '0')).join('');
      say(`took ${path} (${bytes.length} bytes, ${width}×${height}, sha256 ${sha})`);
      photos.push({ path, sha256: sha, track: args.track, origin: 'app' });
      return { path, type, size: bytes.length, width, height };
    },
    async getPhotoCapabilities(args) {
      const entry = tracks.lookup(args?.track);
      if (!entry) return { error: 'NotFoundError' };
      if (entry.kind !== 'video') return { error: 'NotSupportedError' };
      const s = entry.state?.()?.settings ?? {}, fixed = n => ({ min: n ?? 0, max: n ?? 0, step: 1 });
      const plain = { redEyeReduction: 'never', imageWidth: fixed(s.width), imageHeight: fixed(s.height), fillLightMode: [] };
      if (agent) return { ...plain, fillLightMode: s.facingMode === 'environment' ? ['auto', 'off', 'flash'] : ['off'] };
      const source = entry.source?.();
      if (!source || typeof ImageCapture !== 'function') return plain;
      try {
        const c = await new ImageCapture(source).getPhotoCapabilities(), range = r => r ? { min: r.min, max: r.max, step: r.step } : fixed(0);
        return { redEyeReduction: c.redEyeReduction ?? 'never', imageWidth: range(c.imageWidth), imageHeight: range(c.imageHeight), fillLightMode: c.fillLightMode ?? [] };
      } catch { return plain; }
    },
    picture: async () => ({ path: picture ?? '' }),
    /** What `state.extensions` lists as this implementation's (D8 item 9): each photo, by path and SHA-256. */
    state: () => ({ photos }),
    /** Under the agent: the picture `prefer extension camera.picture={"path":…}` names (D8 item 9). */
    preferred(values) {
      const named = values?.picture?.path;
      const next = typeof named === 'string' && named ? named : null;
      if (next === picture) return;
      picture = next; pictureURL = null; pictureBitmap = null;
      for (const [id, v] of views) if (v.sub?.live) { framed(v.sub.track); v.sub.leave(); v.sub = null; reconcile(id); }
    },
  };
}
