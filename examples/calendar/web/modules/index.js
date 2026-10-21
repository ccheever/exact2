// The Mac calendar's input-only native module on the web. The Contract draws
// every pixel and owns the drag's motion, the highlighted dates, scrolling and
// durable edits. This module recognizes a mouse drag (or a held touch) and a
// horizontal trackpad swipe, and reports them through Exact's NativeView
// `message` event. It never reaches into the host or the app tree. The Mac's
// context menu has no web counterpart.
export const abi = 1;
export const roster = {
  'calendar-input-root': { snapshot: false },
  'calendar-drag-handle': { snapshot: false },
};

const HOLD_MS = 300, TOUCH_SLOP = 8, MOUSE_SLOP = 3, SWIPE = 30, SWIPE_GAP_MS = 200;
let root = null, contact = null, serial = 0, sequence = 0;

function props(json) {
  const value = JSON.parse(json);
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('calendar input props must be an object');
  return value;
}

function listen(owner, element, type, listener, options) {
  element.addEventListener(type, listener, options);
  owner.listeners.push(() => element.removeEventListener(type, listener, options));
}

function box(element) {
  if (!element.isConnected) return null;
  const r = element.getBoundingClientRect();
  return [r.left, r.top, r.width, r.height].every(Number.isFinite) && r.width > 0 && r.height > 0 ? r : null;
}

function eligible(handle) {
  return !handle.dead && handle.enabled && !!box(handle.element)
    && !handle.element.closest('[inert]') && getComputedStyle(handle.element).visibility === 'visible';
}

function sameViewport(a, b) {
  return !!a && !!b && ['left', 'top', 'width', 'height'].every(key => Math.abs(a[key] - b[key]) < 0.5);
}

function local(c, x, y) {
  return [(x - c.viewport.left) * c.scaleX, (y - c.viewport.top) * c.scaleY];
}

function emit(c, phase, reason = '', at = [c.x, c.y]) {
  if (c.root.dead || !c.root.element.isConnected) return;
  const [x, y] = local(c, at[0], at[1]);
  if (![x, y].every(Number.isFinite)) return;
  c.root.enqueue({ v: 1, phase, id: c.id, serial: c.serial, x, y, reason,
    rx: c.source[0], ry: c.source[1], rw: c.source[2], rh: c.source[3] });
  c.lastX = at[0]; c.lastY = at[1];
}

function clear(c) {
  clearTimeout(c.timer); cancelAnimationFrame(c.frame);
  if (contact === c) contact = null;
  if (c.mode === 'pointer') {
    try { if (c.handle.element.hasPointerCapture(c.pointer)) c.handle.element.releasePointerCapture(c.pointer); } catch {}
  }
}

function cancel(reason, silent = false) {
  const c = contact; if (!c) return;
  clear(c);
  if (c.began && !silent) emit(c, 'cancel', reason);
}

function current(c) {
  if (contact !== c) return false;
  if (c.root.dead || !c.root.element.isConnected) { cancel('cancelled', true); return false; }
  if (c.handle.dead || !c.handle.element.isConnected) { cancel('source-removed'); return false; }
  if (!eligible(c.handle)) { cancel('disabled'); return false; }
  if (c.began && !sameViewport(c.viewport, box(c.root.element))) { cancel('viewport-changed'); return false; }
  return true;
}

// A mouse drag begins where the button went down, so the grab offset is
// exact; a held touch begins where the finger rests when the hold ends.
function begin(c) {
  if (!current(c) || serial >= Number.MAX_SAFE_INTEGER) { if (contact === c) clear(c); return false; }
  const viewport = box(c.root.element), source = box(c.handle.element);
  if (!viewport || !source) { clear(c); return false; }
  c.viewport = viewport;
  c.scaleX = (c.root.element.clientWidth || viewport.width) / viewport.width;
  c.scaleY = (c.root.element.clientHeight || viewport.height) / viewport.height;
  const [rx, ry] = local(c, source.left, source.top);
  c.source = [rx, ry, source.width * c.scaleX, source.height * c.scaleY];
  c.serial = ++serial; c.began = true; c.handle.suppressPress = true;
  if (c.mode === 'pointer') { try { c.handle.element.setPointerCapture(c.pointer); } catch {} }
  emit(c, 'begin', '', c.mode === 'pointer' ? [c.downX, c.downY] : [c.x, c.y]);
  return true;
}

function down(handle, x, y, mode, pointer) {
  handle.suppressPress = false;
  if (!eligible(handle) || !root || root.dead || !box(root.element)) return;
  cancel('superseded');
  const c = { handle, root, id: handle.id, mode, pointer, x, y, downX: x, downY: y,
    began: false, serial: 0, frame: 0, timer: 0, lastX: x, lastY: y };
  contact = c;
  if (mode === 'touch') c.timer = setTimeout(() => begin(c), HOLD_MS);
}

function move(c, x, y) {
  if (!current(c)) return;
  c.x = x; c.y = y;
  if (!c.began) {
    const distance = Math.hypot(x - c.downX, y - c.downY);
    if (c.mode === 'touch') {
      if (distance > TOUCH_SLOP) { c.handle.suppressPress = true; clear(c); }
      return;
    }
    if (distance <= MOUSE_SLOP || !begin(c)) return;
  }
  if (!c.frame) c.frame = requestAnimationFrame(() => {
    c.frame = 0;
    if (current(c) && (c.x !== c.lastX || c.y !== c.lastY)) emit(c, 'move');
  });
}

function up(c, x, y) {
  if (!current(c)) return;
  c.x = x; c.y = y;
  clear(c);
  if (!c.began) return;
  if (c.x !== c.lastX || c.y !== c.lastY) emit(c, 'move');
  emit(c, 'end');
}

class InputRoot {
  constructor(element, value, event) {
    if (root && !root.dead) throw new Error('calendar has one input root');
    this.element = element; this.event = event; this.listeners = []; this.dead = false;
    this.queue = []; this.inflight = null; this.swipeRight = 0;
    this.swipeX = 0; this.swipeY = 0; this.swipeAt = -Infinity; this.swiped = false;
    this.setProps(value);
    root = this;
    // Every listener below is input-only. It writes no DOM style or content.
    listen(this, document, 'pointermove', e => {
      const c = contact;
      if (c?.mode !== 'pointer' || e.pointerId !== c.pointer) return;
      if (c.began) { e.preventDefault(); e.stopPropagation(); }
      move(c, e.clientX, e.clientY);
    }, true);
    listen(this, document, 'pointerup', e => {
      const c = contact;
      if (c?.mode !== 'pointer' || e.pointerId !== c.pointer) return;
      up(c, e.clientX, e.clientY);
    }, true);
    listen(this, document, 'pointercancel', e => {
      if (contact?.mode === 'pointer' && contact.pointer === e.pointerId) cancel('cancelled');
    }, true);
    listen(this, document, 'touchstart', e => { if (contact && e.touches.length !== 1) cancel('superseded'); }, { passive: true, capture: true });
    listen(this, document, 'touchmove', e => {
      const c = contact; if (c?.mode !== 'touch') return;
      const touch = [...e.touches].find(t => t.identifier === c.pointer);
      if (!touch) { cancel('cancelled'); return; }
      if (c.began) { e.preventDefault(); e.stopPropagation(); }
      move(c, touch.clientX, touch.clientY);
    }, { passive: false, capture: true });
    listen(this, document, 'touchend', e => {
      const c = contact; if (c?.mode !== 'touch') return;
      const touch = [...e.changedTouches].find(t => t.identifier === c.pointer);
      if (!touch) return;
      if (c.began) e.preventDefault();
      up(c, touch.clientX, touch.clientY);
    }, { passive: false, capture: true });
    listen(this, document, 'touchcancel', () => { if (contact?.mode === 'touch') cancel('cancelled'); }, true);
    listen(this, document, 'keydown', e => {
      if (e.key === 'Escape' && contact?.began) { e.preventDefault(); e.stopPropagation(); cancel('cancelled'); }
    }, true);
    // One page a swipe: a gesture's deltas add up until it pauses, and it turns
    // the month once. Holding back a horizontal one keeps the browser's own
    // back and forward swipe from taking it.
    listen(this, document, 'wheel', e => this.wheel(e), { passive: false, capture: true });
    listen(this, window, 'blur', () => cancel('interrupted'));
    listen(this, document, 'visibilitychange', () => { if (document.hidden) cancel('interrupted'); });
    const resized = () => { if (contact?.began && !sameViewport(contact.viewport, box(element))) cancel('viewport-changed'); };
    this.observer = new ResizeObserver(resized); this.observer.observe(element);
    listen(this, window, 'resize', resized);
    if (window.visualViewport) listen(this, window.visualViewport, 'resize', resized);
  }
  wheel(e) {
    if (contact || this.dead || e.ctrlKey) return;
    const r = box(this.element); if (!r) return;
    const x = e.clientX - r.left, y = e.clientY - r.top;
    if (x < 0 || x >= this.swipeRight || y < 0 || y > r.height) return;
    const now = performance.now();
    if (now - this.swipeAt > SWIPE_GAP_MS) { this.swipeX = 0; this.swipeY = 0; this.swiped = false; }
    this.swipeAt = now;
    const unit = e.deltaMode === 1 ? 16 : (e.deltaMode === 2 ? r.width : 1);
    this.swipeX += e.deltaX * unit; this.swipeY += e.deltaY * unit;
    if (Math.abs(this.swipeX) > Math.abs(this.swipeY)) e.preventDefault();
    if (this.swiped || Math.abs(this.swipeX) < SWIPE || Math.abs(this.swipeX) <= Math.abs(this.swipeY) * 1.4) return;
    this.swiped = true;
    // A page turn is no drag packet: it bypasses the acknowledged queue.
    this.event(8, this.swipeX > 0 ? 'swipe:next' : 'swipe:previous');
  }
  // Contract acknowledges after applying the packet. This prevents its async
  // resource continuation from observing a later begin/move/end in its slot.
  enqueue(packet) {
    const last = this.queue.at(-1);
    if (packet.phase === 'move' && last?.phase === 'move' && packet.serial === last.serial) this.queue[this.queue.length - 1] = packet;
    else this.queue.push(packet);
    this.drain();
  }
  drain() {
    if (this.dead || this.inflight || !this.queue.length || sequence >= Number.MAX_SAFE_INTEGER) return;
    this.inflight = { ...this.queue.shift(), seq: ++sequence };
    this.event(8, JSON.stringify(this.inflight));
  }
  setProps(value) {
    const ack = value.ack ?? '0';
    if (typeof ack !== 'string' || !/^\d+$/.test(ack) || !Number.isSafeInteger(Number(ack))) throw new Error('calendar input root ack must be a nonnegative safe integer');
    const right = Number(value['swipe-right'] ?? 0);
    if (Number.isFinite(right)) this.swipeRight = right;
    if (this.inflight?.seq === Number(ack)) {
      this.inflight = null;
      queueMicrotask(() => this.drain());
    }
  }
  destroy() {
    if (contact?.root === this) cancel('cancelled', true);
    this.dead = true; this.queue = []; this.inflight = null; this.observer.disconnect(); this.listeners.splice(0).forEach(remove => remove());
    if (root === this) root = null;
  }
}

class DragHandle {
  constructor(element, value, event) {
    this.element = element; this.event = event; this.dead = false; this.listeners = []; this.suppressPress = false;
    this.setProps(value);
    listen(this, element, 'pointerdown', e => {
      if (e.pointerType === 'touch' || e.button !== 0 || !e.isPrimary || e.ctrlKey) return;
      down(this, e.clientX, e.clientY, 'pointer', e.pointerId);
    });
    listen(this, element, 'touchstart', e => {
      if (e.touches.length !== 1) return;
      const t = e.touches[0]; down(this, t.clientX, t.clientY, 'touch', t.identifier);
    }, { passive: true });
    listen(this, element, 'click', e => {
      e.stopPropagation();
      if (this.suppressPress) { this.suppressPress = false; e.preventDefault(); return; }
      if (eligible(this)) this.event(0);
    });
    listen(this, element, 'dblclick', e => e.stopPropagation());
    listen(this, element, 'contextmenu', e => e.preventDefault());
    listen(this, element, 'dragstart', e => e.preventDefault());
    listen(this, element, 'lostpointercapture', e => {
      if (contact?.handle === this && contact.mode === 'pointer' && contact.pointer === e.pointerId) cancel('cancelled');
    });
    listen(this, element, 'keydown', e => {
      if ((e.key === 'Enter' || e.key === ' ') && !e.repeat && eligible(this)) { e.preventDefault(); e.stopPropagation(); this.event(0); }
    });
  }
  setProps(value) {
    const id = value['event-id'], enabled = value.enabled ?? 'true';
    if (typeof id !== 'string' || !id || (enabled !== 'true' && enabled !== 'false')) throw new Error('calendar handle requires event-id and enabled=true|false');
    if (contact?.handle === this && id !== this.id) cancel('superseded');
    if (contact?.handle === this && enabled === 'false') cancel('disabled');
    this.id = id; this.enabled = enabled === 'true';
  }
  destroy() {
    if (contact?.handle === this) cancel('source-removed');
    this.dead = true; this.listeners.splice(0).forEach(remove => remove());
  }
}

export function create(tag, element, json, event) {
  const value = props(json);
  if (tag === 'calendar-input-root') return new InputRoot(element, value, event);
  if (tag === 'calendar-drag-handle') return new DragHandle(element, value, event);
  throw new Error(`unknown calendar input ${tag}`);
}
export function setProps(handle, json) { handle.setProps(props(json)); }
export function destroy(handle) { handle.destroy(); }
