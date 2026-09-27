// @ref LLP 1011.000 — animated GIF and WebP under the agent's clock (LLP
// 1012). The browser plays an animated `<img>` on its own time, which no API
// seeks, so in agent mode each one is held: Chrome's own decoder
// (`ImageDecoder`) gives its frames, and the `<img>` shows the frame the
// clock says, by Chrome's rules (cc's `ImageAnimationController`, Blink's
// `DeferredImageDecoder`): 10 ms or less shows for 100 ms, a play starts
// when the image is first seen, an unseen image does no work and is where
// the time says when seen again, more than five minutes behind it resumes
// the frame it left, and a first play that ended unseen starts over.
// Outside agent mode this file is never loaded and `<img>` plays itself.
// The same schedule is `host/apple/Sources/ExactKit/AnimatedRaster.swift`.

/** Chrome's schedule: each frame's ms (clamped), their ends, plays (0 is forever). */
export function schedule(ms, plays) {
  const durations = ms.map((d) => { const r = Math.round(d); return r <= 10 ? 100 : r; });
  let sum = 0;
  const ends = durations.map((d) => (sum += d));
  const total = sum;
  /** The frame at `elapsed` ms since the first began, and the next change (null: it holds). */
  function at(elapsed) {
    const n = durations.length;
    if (n < 2 || total <= 0) return { index: 0, next: null };
    const e = Math.max(0, elapsed);
    if (plays > 0 && e >= total * plays) return { index: n - 1, next: null };
    const cycle = Math.floor(e / total), within = e - cycle * total;
    let index = 0;
    while (index < n - 1 && within >= ends[index]) index++;
    if (plays > 0 && index === n - 1 && cycle + 1 >= plays) return { index, next: null };
    return { index, next: cycle * total + ends[index] };
  }
  function began(elapsed) {
    const { index } = at(elapsed);
    let cycle = Math.floor(Math.max(0, elapsed) / total);
    if (plays > 0) cycle = Math.min(cycle, plays - 1);
    return cycle * total + (index > 0 ? ends[index - 1] : 0);
  }
  return { durations, ends, total, plays, at, began };
}

/** One image's play: starts when first seen, and takes no time unseen. */
export function animationClock(s) {
  let start = null, hidden = null;
  return {
    hide(now) { if (start !== null && hidden === null) hidden = now; },
    show(now) {
      if (start === null) { start = now; return 0; }
      if (hidden !== null) {
        const left = hidden;
        hidden = null;
        const { index, next } = s.at(left - start);
        if (next !== null && now - (start + next) > 300_000) { start = now - s.began(left - start); return index; }
        if (s.plays !== 1 && left - start < s.total && now >= start + s.total + s.durations[0]) { start = now; return 0; }
      }
      return s.at(now - start).index;
    },
  };
}

const TYPES = { gif: 'image/gif', webp: 'image/webp' };

globalThis.exact.holdImages = ({ root, now }) => {
  const held = new Map(); // img -> { src, frames?, clock?, shown, urls }
  const pending = new Set();
  const track = (img) => {
    const state = { src: img.src, shown: -1, urls: [] };
    held.set(img, state);
    const ext = /\.(gif|webp)(?:[?#]|$)/i.exec(new URL(img.src, location.href).pathname)?.[1]?.toLowerCase();
    if (!ext || typeof ImageDecoder === 'undefined') return;
    const job = (async () => {
      const bytes = await (await fetch(state.src)).arrayBuffer();
      const decoder = new ImageDecoder({ data: bytes, type: TYPES[ext] });
      await decoder.tracks.ready;
      const t = decoder.tracks.selectedTrack;
      if (!t?.animated || t.frameCount < 2) { decoder.close(); return; }
      await decoder.completed;
      const frames = [], ms = [];
      for (let i = 0; i < t.frameCount; i++) {
        const { image } = await decoder.decode({ frameIndex: i });
        ms.push((image.duration ?? 0) / 1000);
        const canvas = new OffscreenCanvas(image.displayWidth, image.displayHeight);
        canvas.getContext('2d').drawImage(image, 0, 0);
        image.close();
        frames.push(canvas);
      }
      decoder.close();
      // WebCodecs counts repetitions after the first play.
      const plays = t.repetitionCount === Infinity ? 0 : t.repetitionCount + 1;
      if (held.get(img) !== state) return;
      state.frames = frames;
      state.clock = animationClock(schedule(ms, plays));
      await place(img, state);
    })().catch((e) => console.error('exact: animated image', state.src, e)).finally(() => pending.delete(job));
    pending.add(job);
  };
  const visible = (img) => {
    const b = img.getBoundingClientRect();
    return img.isConnected && b.width > 0 && b.height > 0 && b.bottom > 0 && b.right > 0 && b.top < innerHeight && b.left < innerWidth;
  };
  /** The frame the clock says, if seen; resolves once the `<img>` shows it. */
  const place = async (img, state) => {
    const t = now();
    if (!visible(img)) { state.clock.hide(t); return; }
    const index = state.clock.show(t);
    if (index === state.shown) return;
    state.shown = index;
    state.urls[index] ??= URL.createObjectURL(await state.frames[index].convertToBlob({ type: 'image/png' }));
    if (held.get(img) !== state || state.shown !== index) return;
    img.src = state.urls[index];
    state.src = img.src;
    await img.decode().catch(() => {});
  };
  const scan = () => {
    for (const img of root.querySelectorAll('img')) {
      if (img.dataset.symbolPath != null || !img.src) continue;
      const state = held.get(img);
      if (!state || state.src !== img.src) track(img);
    }
    for (const img of held.keys()) if (!img.isConnected) held.delete(img);
  };
  const holder = {
    /** Every held image to the clock after a batch. */
    seek() {
      scan();
      for (const [img, state] of held) if (state.clock) {
        const job = place(img, state).finally(() => pending.delete(job));
        pending.add(job);
      }
    },
    /** Resolves when every image being read or swapped shows its frame, and
     * visible images have loaded (up to 3 s: an image's decode is host I/O). */
    async ready() {
      const end = performance.now() + 3000;
      while (pending.size && performance.now() < end) await Promise.race([Promise.all([...pending]), new Promise((r) => setTimeout(r, 50))]);
      const loading = () => [...root.querySelectorAll('img')].filter((i) => !i.complete && visible(i));
      while (loading().length && performance.now() < end) await new Promise((r) => setTimeout(r, 25));
      scan();
      while (pending.size && performance.now() < end) await Promise.race([Promise.all([...pending]), new Promise((r) => setTimeout(r, 50))]);
    },
  };
  globalThis.exact.imageFrames = () => holder.ready();
  return holder;
};
