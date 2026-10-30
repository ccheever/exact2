// Shared timer wake and scroll observations (LLP 1147).
export function frameAtOrAfter(deadline) {
  if (!Number.isFinite(deadline)) { return deadline; }
  let k = Math.ceil(deadline * 60 / 1000);
  const frame = k => k * 1000 / 60;
  while (k > 0 && frame(k - 1) >= deadline) { k--; }
  while (frame(k) < deadline) { k++; }
  return frame(k);
}
export function cap(t) {
  return t.due + (t.wait <= 2 ? 100 : t.max ? t.max : t.wait === 4 ? Math.max(t.ms, 1000) : Infinity);
}
export function idleAllowed({ scroll, demand, applying, ordinary, next }, now) {
  return !scroll && !demand && !applying && !ordinary && (next == null || next > now + 1000 / 60);
}
// Keep touch holds independent of pointer cancellation and movement completion
export class ScrollActivity {
  holds = new Map();
  moving = new Map();
  wheelUntil = 0;
  hold(id, target) { this.holds.set(id, target); }
  lift(id) { this.holds.delete(id); }
  moved(target, now, x = target.scrollLeft ?? target.pageLeft ?? 0, y = target.scrollTop ?? target.pageTop ?? 0) { this.moving.set(target, { at: now, x, y }); }
  ended(target) { this.moving.delete(target); }
  sample(now, supported, observe = true) {
    for (const [id, target] of this.holds) { if (target.isConnected === false) { this.holds.delete(id); } }
    for (const [target, prior] of this.moving) {
      if (!observe && target.isConnected !== false) { continue; }
      const x = target.scrollLeft ?? target.pageLeft ?? 0, y = target.scrollTop ?? target.pageTop ?? 0;
      if (x !== prior.x || y !== prior.y) { Object.assign(prior, { at: now, x, y }); }
      const held = [...this.holds.values()].some(h => h === target || target.contains?.(h));
      if (target.isConnected === false || !held && now - prior.at >= (supported ? 1000 : 100)) { this.moving.delete(target); }
    }
    return this.holds.size > 0 || this.moving.size > 0 || this.wheelUntil > now;
  }
}
