// Timer admission observations and wake planning (LLP 1147).
import { rt } from './rt-link.js';
import { beatAfter } from './beat.js';

import { frameAtOrAfter, cap, idleAllowed, ScrollActivity } from './timer-observations.js';
export { frameAtOrAfter, cap, idleAllowed, ScrollActivity } from './timer-observations.js';

export function timerWaits() {
  if (rt.timerWait) { return; }
  const activity = new ScrollActivity(), clock = rt.clock, watched = new WeakMap();
  let frame = 0, idle = 0, timeout = 0, generation = 0, intersecting = true, takeoverAt = -Infinity;
  const offscreen = () => document.hidden || !intersecting;
  const current = token => token === generation && !clock.agent;
  const requestFrame = () => {
    if (frame || offscreen() || clock.agent) { return; }
    const token = generation;
    frame = requestAnimationFrame(() => { if (!current(token)) { return; } frame = 0; control.check(true); });
  };
  const later = (delay = 0) => {
    clearTimeout(timeout);
    const token = generation;
    timeout = setTimeout(() => { if (!current(token)) { return; } timeout = 0; control.check(); }, Math.min(2147483647, delay));
  };
  function fire(t, now) {
    const nominal = t.due, gen = t.gen;
    if (t.once) { clock.timers.splice(clock.timers.indexOf(t), 1); }
    else { t.due = beatAfter(nominal, t.ms, now); }
    const due = t.due;
    (clock.timerFirings ??= Array(7).fill(0))[t.wait]++; rt.fireTimer(t); clock.now = Math.max(clock.now, rt.wall());
    if (!t.once && clock.timers.includes(t)) {
      if (t.gen !== gen || t.due !== due) { t.due = clock.now + t.ms; }
      else { t.due = beatAfter(nominal, t.ms, clock.now); }
    }
  }
  const control = rt.timerWait = {
    generation: () => generation,
    cutoff(to, wall) { return wall ? to : Math.max(to, takeoverAt); },
    execution(t, wall) { if (!wall && t.due <= takeoverAt) { return takeoverAt; } return !t.wait ? t.due : wall ? cap(t) : t.wait === 1 ? frameAtOrAfter(t.due) : t.due; },
    wake(t) { return !t.wait ? t.due + (t.tolerance ?? 0) : Math.min(cap(t), t.due > clock.now ? t.due : Infinity); },
    check(inFrame = false) {
      if (clock.agent || clock.stopped || rt.poisoned?.()) { return; }
      const now = Math.max(clock.now, rt.wall());
      if (!clock.timers.some(t => t.wait && t.due <= now)) { return; }
      while (rt.advance(now, true)) {
        if (clock.agent || clock.stopped || rt.poisoned?.()) { return; }
      }
      const supported = 'onscrollend' in window;
      const scrolling = () => !offscreen() && activity.sample(Math.max(clock.now, rt.wall()), supported, inFrame);
      const held = () => clock.timers.filter(t => t.wait && t.due <= now && cap(t) > now).sort((a, b) => a.due - b.due || a.i - b.i);
      while (!clock.agent && !clock.stopped && !rt.poisoned?.()) {
        const scroll = scrolling();
        const demand = clock.timers.some(t => t.frame) || scroll || clock.timers.some(t => t.wait === 1 && t.due <= clock.now);
        const waiting = held();
        const next = waiting.find(t => t.wait === 1 ? inFrame || offscreen() : t.wait === 2 ? inFrame || !demand || offscreen() : t.wait === 3 && !scroll);
        if (!next) {
          if (waiting.some(t => t.wait <= 2 || t.wait === 3 && scroll)) { requestFrame(); }
          break;
        }
        fire(next, now);
      }
      if (scrolling() && activity.moving.size && held().length) { requestFrame(); }
      if (held().some(t => t.wait === 4) && !idle) {
        const token = generation;
        const run = () => {
          if (!current(token)) { return; } idle = 0;
          if (clock.stopped || rt.poisoned?.()) { return; }
          const at = Math.max(clock.now, rt.wall()); clock.now = at;
          const busyScroll = !offscreen() && activity.sample(at, supported, false);
          const animations = document.getAnimations().filter(a => a.timeline === document.timeline && a.playState === 'running' && a.playbackRate !== 0);
          let next = rt.nextMutation();
          for (const t of clock.timers) { next = Math.min(next, t.wait === 4 && t.due <= at ? cap(t) : t.due); }
          const allowed = idleAllowed({ scroll: busyScroll, demand: clock.timers.some(t => t.frame) || animations.length > 0, applying: rt.busy(), ordinary: rt.nextMutation() <= at || clock.timers.some(t => t.wait !== 4 && t.due <= at), next }, at);
          let admitted = false;
          if (allowed) { const t = held().find(t => t.wait === 4); if (t) { fire(t, at); admitted = true; } }
          if (animations.length) { const fresh = animations.filter(a => watched.get(a) !== a.finished);
            for (const a of fresh) { watched.set(a, a.finished); }
            if (fresh.length) { Promise.allSettled(fresh.map(a => a.finished)).then(() => { if (current(token)) { control.check(); } }); }
            later(100); }
          else if (admitted && held().some(t => t.wait === 4)) { later(0); }
          else if (held().some(t => t.wait === 4) && Number.isFinite(next)) { later(Math.max(1, next - at)); }
        };
        if (typeof requestIdleCallback === 'function') { idle = requestIdleCallback(run); }
        else if (offscreen()) { idle = setTimeout(run, 0); }
        else { idle = requestAnimationFrame(() => { if (current(token)) { idle = setTimeout(run, 0); } }); }
      }
      rt.drive();
    },
    takeover() { generation++; globalThis.cancelAnimationFrame?.(frame); clearTimeout(timeout); rt.pauseTimers?.(); frame = 0; idle = 0; },
  };
  let agent = clock.agent;
  Object.defineProperty(clock, 'agent', {
    configurable: true, get: () => agent,
    set(value) {
      if (value && !agent) { takeoverAt = clock.now = Math.max(clock.now, rt.wall()); control.takeover(); }
      if (!value) { takeoverAt = -Infinity; }
      agent = value;
    },
  });
  if (typeof document === 'undefined') { return; }
  const changed = () => { if (!clock.agent) { if (offscreen()) { control.check(); } else { requestFrame(); } } };
  document.addEventListener('scroll', e => { activity.moved(e.target === document ? document.scrollingElement : e.target, rt.wall()); changed(); }, true);
  document.addEventListener('scrollend', e => { activity.ended(e.target === document ? document.scrollingElement : e.target); changed(); }, true);
  for (const type of ['pointerdown', 'pointerup', 'pointercancel', 'touchstart', 'touchend', 'touchcancel']) {
    document.addEventListener(type, e => {
      if (type.startsWith('touch')) { for (const t of e.changedTouches) { type === 'touchstart' ? activity.hold('t' + t.identifier, e.target) : activity.lift('t' + t.identifier); } }
      else if (e.pointerType !== 'touch') { type === 'pointerdown' ? activity.hold('p' + e.pointerId, e.target) : activity.lift('p' + e.pointerId); }
      changed();
    }, { passive: true, capture: true });
  }
  document.addEventListener('wheel', () => { activity.wheelUntil = rt.wall() + 100; requestFrame(); }, { passive: true, capture: true });
  document.addEventListener('visibilitychange', changed);
  window.visualViewport?.addEventListener('scroll', () => { activity.moved(window.visualViewport, rt.wall()); changed(); }, { passive: true });
  window.visualViewport?.addEventListener('scrollend', () => { activity.ended(window.visualViewport); changed(); }, { passive: true });
  const cover = document.createElement('div');
  cover.style.cssText = 'position:fixed;inset:0;pointer-events:none'; cover.setAttribute('aria-hidden', 'true'); document.documentElement.append(cover);
  if (typeof IntersectionObserver === 'function') { new IntersectionObserver(entries => { intersecting = entries.at(-1).isIntersecting; changed(); }).observe(cover); }
}
