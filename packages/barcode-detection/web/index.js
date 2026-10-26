// @exact/barcode-detection on the web (LLP 1133.027): the Shape Detection
// API's `BarcodeDetector` (https://wicg.github.io/shape-detection-api/),
// which Chrome ships on macOS, ChromeOS and Android and no other engine does;
// where it is absent the formats are empty and every detection fails
// `NotSupportedError` (D18).
//
// - A still is a path the app may read (D4), decoded upright by
//   `createImageBitmap`, whose `imageOrientation` is `from-image` unsaid.
// - A live scan (D5) observes a video track of @exact/media-capture's: it
//   takes no lease, and each poll analyses the frame shown now, on a
//   `<video>` of the module's own that plays the track and is never attached.
//   Under the agent the frame is the camera's agent picture (D17 item 2).
// - One UPC-A rule on every target (D11): both UPC-A and EAN-13 are asked
//   whenever either is, a UPC-A in either shape is answered as the formats
//   asked name it, and a result of a format not asked is dropped.
// - Vision runs for real under the agent (§9): the browser's detector, on
//   the drive's fixture images.

const FORMATS = ['aztec', 'code_128', 'code_39', 'code_93', 'codabar', 'data_matrix', 'ean_13', 'ean_8', 'itf', 'pdf417', 'qr_code', 'upc_a', 'upc_e'];
const KEPT = 32;
const SCAN = 'barcode-detection.detectedBarcodes', FORMATS_KEY = 'barcode-detection.getSupportedFormats';

const failure = (error, message) => ({ error, ...(message ? { message } : {}) });
const nameOf = (e, fallback) => typeof e?.name === 'string' && /Error$/.test(e.name) ? e.name : fallback;

/** The formats asked: `null` for all, else a set; `{error}` for D2's `TypeError`. */
function asked(formats) {
  if (formats == null) return null;
  if (!Array.isArray(formats) || !formats.length || formats.some(f => !FORMATS.includes(f))) return failure('TypeError', 'formats name D1\'s barcode formats');
  return new Set(formats);
}

/** D11: one UPC-A answered as the formats asked name it, and only formats asked kept. */
export function normalised(results, keep) {
  const out = [];
  for (const r of results) {
    let { format, rawValue } = r;
    const upc = (format === 'upc_a' && /^\d{12}$/.test(rawValue)) || (format === 'ean_13' && /^0\d{12}$/.test(rawValue));
    if (upc) {
      const twelve = format === 'upc_a' ? rawValue : rawValue.slice(1);
      if (!keep || keep.has('upc_a')) { format = 'upc_a'; rawValue = twelve; } else { format = 'ean_13'; rawValue = `0${twelve}`; }
    }
    if (keep && !keep.has(format)) continue;
    out.push({ ...r, format, rawValue });
  }
  return out;
}

export function implementation({ changed, agent, now, log, tracks, files }) {
  const say = line => log?.(`barcode-detection: ${line}`);
  const Detector = typeof globalThis.BarcodeDetector === 'function' ? globalThis.BarcodeDetector : null;
  const detectors = new Map(), scans = new Map(), stills = [];
  let supported = null;

  // The browser's own list (Chrome on macOS is Vision's, which reads a UPC-A as an EAN-13 and lists no `upc_a`).
  const platformFormats = async () => supported ??= Detector ? Detector.getSupportedFormats().then(f => f.filter(x => FORMATS.includes(x)), () => []) : Promise.resolve([]);
  /** What the module reads here: either UPC shape is the other by D11's rule. */
  const formatsHere = async () => {
    const own = await platformFormats();
    return FORMATS.filter(f => own.includes(f) || ((f === 'upc_a' || f === 'ean_13') && (own.includes('upc_a') || own.includes('ean_13'))));
  };
  /** The platform's detector for `keep`, asking both UPC shapes when either is asked (D11); `{error}` when it cannot read them. */
  async function detectorFor(keep) {
    if (!Detector) return failure('NotSupportedError', 'this browser has no BarcodeDetector');
    const here = await formatsHere(), own = await platformFormats();
    if (keep && [...keep].some(f => !here.includes(f))) return failure('NotSupportedError', `this browser reads ${here.join(', ') || 'no format'}`);
    const ask = keep ? [...new Set([...keep, ...(keep.has('upc_a') || keep.has('ean_13') ? ['upc_a', 'ean_13'] : [])])].filter(f => own.includes(f)).sort() : own;
    const key = ask.join(',');
    if (!detectors.has(key)) detectors.set(key, new Detector({ formats: ask }));
    return detectors.get(key);
  }
  const shape = b => ({
    rawValue: b.rawValue, format: b.format,
    boundingBox: { x: b.boundingBox.x, y: b.boundingBox.y, width: b.boundingBox.width, height: b.boundingBox.height },
    cornerPoints: (b.cornerPoints ?? []).map(p => ({ x: p.x, y: p.y })),
  });
  const detect = async (detector, frame, keep) => normalised((await detector.detect(frame)).map(shape), keep);

  // ---------------------------------------------------------------- stills (D4)
  async function still({ image, formats }) {
    const keep = asked(formats);
    if (keep?.error) return keep;
    const detector = await detectorFor(keep);
    if (detector.error) return detector;
    let bytes;
    try { bytes = await files.read(image); } catch (e) { return failure(e?.code === 'ENOENT' || /ENOENT|not found|no such/i.test(e?.message ?? '') ? 'NotFoundError' : 'NotAllowedError', e?.message); }
    let bitmap;
    try { bitmap = await createImageBitmap(new Blob([bytes])); } catch { return failure('InvalidStateError', 'decode'); }
    try {
      // The bitmap is upright (`imageOrientation: "from-image"`), but Chrome's detector reads its stored pixels,
      // so a JPEG turned by EXIF answers boxes in the unturned frame. Drawn once, the pixels are the upright ones (D4).
      const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
      canvas.getContext('2d').drawImage(bitmap, 0, 0);
      const barcodes = await detect(detector, canvas, keep);
      stills.push({ path: image, count: barcodes.length });
      if (stills.length > 16) stills.shift();
      say(`${image}: ${barcodes.length} found`);
      return { barcodes, width: bitmap.width, height: bitmap.height };
    } catch (e) {
      return failure(nameOf(e, 'OperationError'), e?.message);
    } finally { bitmap.close?.(); }
  }

  // ---------------------------------------------------------------- live scans (D5)
  const scanKey = (track, keep) => `${track}|${keep ? [...keep].sort().join(',') : '*'}`;
  /** The scan of `track` for `keep`, observing the track from its first read. */
  function scanOf(track, keep, interval) {
    const key = scanKey(track, keep);
    if (scans.has(key)) return scans.get(key);
    const s = { key, track, keep, interval, state: 'idle', barcodes: [], seen: new Map(), frame: 0, width: 0, height: 0, error: null, video: null, busy: null, leave: () => {} };
    const play = source => {
      if (agent || !source) return;
      s.video ??= Object.assign(document.createElement('video'), { muted: true, playsInline: true, autoplay: true });
      if (s.video.srcObject?.getVideoTracks?.()[0] !== source) s.video.srcObject = new MediaStream([source]);
      s.video.play?.().catch(() => {});
    };
    const set = state => { if (s.state !== state) { s.state = state; changed(SCAN); } };
    s.leave = tracks.observe(track, {
      live(source) { play(source); set('scanning'); },
      muted() { set('idle'); },
      unmuted() { set('scanning'); },
      ended() { set('ended'); if (s.video) s.video.srcObject = null; },
    });
    scans.set(key, s);
    say(`scan ${key}: observing ${track}`);
    return s;
  }
  function endScan(s, why) {
    s.leave();
    if (s.video) { s.video.srcObject = null; s.video = null; }
    scans.delete(s.key);
    say(`scan ${s.key}: ${why}`);
  }
  /** The frame to analyse now: the agent's picture (a promise of it), or the playing track's. */
  function frameOf(s) {
    if (agent) return tracks.lookup(s.track)?.frame?.() ?? null;
    return s.video && s.video.readyState >= 2 && s.video.videoWidth ? s.video : null;
  }
  /** One analysed frame, the newest; one in the detector already is waited for, not queued behind. */
  async function analyse(s) {
    if (s.state !== 'scanning') return;
    if (s.busy) return s.busy;
    if (!agent && !frameOf(s)) return;
    s.busy = (async () => {
      const frame = await frameOf(s);
      if (!frame) return;
      const detector = await detectorFor(s.keep);
      if (detector.error) { s.error = detector.message ?? detector.error; return; }
      try {
        const found = await detect(detector, frame, s.keep), at = now();
        s.frame += 1;
        s.width = frame.videoWidth ?? frame.width; s.height = frame.videoHeight ?? frame.height;
        s.barcodes = found; s.error = null;
        for (const b of found) {
          const k = `${b.format}\u0000${b.rawValue}`, was = s.seen.get(k);
          s.seen.delete(k);
          s.seen.set(k, { rawValue: b.rawValue, format: b.format, firstSeen: was?.firstSeen ?? at, lastSeen: at, frames: (was?.frames ?? 0) + 1 });
        }
        while (s.seen.size > KEPT) s.seen.delete(s.seen.keys().next().value);
      } catch (e) { s.error = e?.message ?? String(e); }
    })().finally(() => { s.busy = null; });
    return s.busy;
  }
  const scanShape = s => ({
    state: s.state, barcodes: s.barcodes, frame: s.frame, width: s.width, height: s.height, error: s.error,
    seen: [...s.seen.values()].sort((a, b) => b.lastSeen - a.lastSeen),
  });

  return {
    async getSupportedFormats() {
      const formats = await formatsHere();
      return { formats, tracks: !!Detector && formats.length > 0 };
    },
    detectBarcodes: still,
    async detectedBarcodes({ track, formats, interval }) {
      const keep = asked(formats);
      if (keep?.error) return keep;
      // No track yet (an app's state before its camera opens) is a scan with nothing to read.
      if (!track) return { state: 'idle', barcodes: [], seen: [], frame: 0, width: 0, height: 0 };
      const entry = tracks.lookup(track);
      if (!entry) return failure('NotFoundError', `the session holds no track ${track}`);
      if (entry.kind !== 'video') return failure('NotSupportedError', `${track} is not a video track`);
      if (!Detector) return failure('NotSupportedError', 'this browser has no BarcodeDetector');
      let s;
      try { s = scanOf(track, keep, interval ?? 100); } catch (e) { return failure(e?.code ?? 'NotFoundError', e?.message); }
      await analyse(s);
      return scanShape(s);
    },
    /** The last reader of a scan let it go: its observer removed, its `seen` dropped (D5). */
    releaseArgs(name, args) {
      if (name !== 'detectedBarcodes') return;
      const keep = asked(args?.formats);
      const s = scans.get(scanKey(args?.track, keep?.error ? null : keep));
      if (s) endScan(s, 'released');
    },
    release(name) {
      if (name === 'detectedBarcodes') for (const s of [...scans.values()]) endScan(s, 'released');
    },
    /** A browser updated while hidden may read other formats: read again at the return (D6). */
    foreground(shown) {
      if (!shown) return;
      supported = null;
      changed(FORMATS_KEY);
    },
    destroy() { for (const s of [...scans.values()]) endScan(s, 'the session ended'); },
    /** `state.extensions` (§9): each scan and the latest stills. */
    state() {
      return {
        scans: [...scans.values()].map(s => ({ track: s.track, formats: s.keep ? [...s.keep] : null, interval: s.interval, state: s.state, frames: s.frame, seen: [...s.seen.values()].map(x => `${x.format} ${x.rawValue}`) })),
        stills: stills.slice(),
      };
    },
  };
}
