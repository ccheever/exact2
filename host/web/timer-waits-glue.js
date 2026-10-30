// Admission opportunities for the wasm scheduler, using the JS target's scroll observations.
import { ScrollActivity, idleAllowed } from './timer-observations.js';
export function createWaitObserver({ now, read, release, demand, next, requestFrame }) {
  const activity = new ScrollActivity(), events = new AbortController(), watched = new WeakMap();
  let offscreen = document.hidden, generation = 0, timeout, checkSoon, animationProbe, observer, idle, checking = false;
  const listen = (type, callback) => document.addEventListener(type, callback, { passive: true, capture: true, signal: events.signal });
  const schedule = () => {
    clearTimeout(timeout);
    const due = read().filter(t => t >= 0);
    if (due.length) { const token = generation; timeout = setTimeout(() => { if (token === generation) { check(); } }, Math.max(0, Math.min(...due) - now())); }
  };
  function check(frame = false) {
    if (checking) { return; }
    checking = true;
    try {
      const cutoff = now();
      if (!read().some(at => at === -2 || at >= 0 && at <= cutoff)) { return; }
      let batch;
      do {
        const states = read(), scrolling = !offscreen && activity.sample(now(), 'onscrollend' in window, frame);
        const frames = demand() || scrolling || states[0] === -2;
        const kinds = (frame || offscreen ? 2 : 0) | (frame || !frames || offscreen ? 4 : 0) | (!scrolling ? 8 : 0);
        batch = release(kinds, cutoff);
        const pending = read();
        if (scrolling || pending[0] === -2 || pending[1] === -2 && frames) { requestFrame(); }
      } while (batch.waitContinue);
      if (read()[3] === -2 && !idle) {
        const token = generation;
        const run = () => {
          idle = null; if (token !== generation) { return; }
          const animations = document.getAnimations().filter(a => a.timeline === document.timeline && a.playState === 'running' && a.playbackRate !== 0);
          const states = read(), at = now();
          if (idleAllowed({ scroll: !offscreen && activity.sample(at, 'onscrollend' in window, false), demand: demand() || animations.length > 0, ordinary: states.slice(0, 3).includes(-2), next: next() }, at)) { release(16, at); if (read()[3] === -2) { setTimeout(() => { if (token === generation) { check(); } }, 0); } }
          if (animations.length) {
            const fresh = animations.filter(a => watched.get(a) !== a.finished);
            for (const a of fresh) { watched.set(a, a.finished); }
            if (fresh.length) { Promise.allSettled(fresh.map(a => a.finished)).then(() => { if (token === generation) { check(); } }); }
            clearTimeout(animationProbe); animationProbe = setTimeout(() => { if (token === generation) { check(); } }, 100);
          }
          schedule();
        };
        if (typeof requestIdleCallback === 'function') { idle = requestIdleCallback(run); }
        else if (offscreen) { idle = setTimeout(run, 0); }
        else { idle = requestAnimationFrame(() => { if (token === generation) { idle = setTimeout(run, 0); } }); }
      }
      schedule();
    } finally { checking = false; }
  }
  listen('scroll', event => { activity.moved(event.target === document ? document.scrollingElement : event.target, now()); requestFrame(); });
  listen('scrollend', event => { activity.ended(event.target === document ? document.scrollingElement : event.target); requestFrame(); });
  listen('wheel', () => { activity.wheelUntil = now() + 100; requestFrame(); });
  for (const type of ['pointerdown', 'pointerup', 'pointercancel', 'touchstart', 'touchend', 'touchcancel']) {
    listen(type, event => {
      if (type.startsWith('touch')) { for (const touch of event.changedTouches) { type === 'touchstart' ? activity.hold('t' + touch.identifier, event.target) : activity.lift('t' + touch.identifier); } }
      else if (event.pointerType !== 'touch') { type === 'pointerdown' ? activity.hold('p' + event.pointerId, event.target) : activity.lift('p' + event.pointerId); }
      offscreen ? check() : requestFrame();
    });
  }
  window.visualViewport?.addEventListener('scroll', () => { activity.moved(window.visualViewport, now()); requestFrame(); }, { passive: true, signal: events.signal });
  window.visualViewport?.addEventListener('scrollend', () => { activity.ended(window.visualViewport); requestFrame(); }, { passive: true, signal: events.signal });
  listen('visibilitychange', () => { offscreen = document.hidden; check(); });
  const cover = document.createElement('div'); cover.style.cssText = 'position:fixed;inset:0;pointer-events:none'; cover.setAttribute('aria-hidden', 'true'); document.documentElement.append(cover);
  if (typeof IntersectionObserver === 'function') {
    observer = new IntersectionObserver(entries => { offscreen = document.hidden || !entries.at(-1).isIntersecting; check(); }); observer.observe(cover);
  }
  return {
    check,
    update() {
      schedule();
      if (!checking && read().includes(-2)) {
        const token = generation;
        clearTimeout(checkSoon);
        checkSoon = setTimeout(() => { if (token === generation) { check(); } }, 0);
      }
    },
    dispose() { generation++; events.abort(); clearTimeout(timeout); clearTimeout(checkSoon); clearTimeout(animationProbe); observer?.disconnect(); cover.remove(); },
  };
}
