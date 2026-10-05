// `<qr-scanner>` on the web: the page has no scanner, so it says so (the
// `message` event's `unavailable`) and the pairing screen offers the paste
// field instead. Events as on iOS: change (a code's text), message (state).
export const abi = 1;
export const roster = { 'qr-scanner': { snapshot: false }, 'progressive-blur': { snapshot: false }, 'title-reveal': { snapshot: false }, 'glass-button': { snapshot: false }, 'glass-composer': { snapshot: false }, 'voice-call': { snapshot: false } };

// `<progressive-blur>` on the web: a backdrop blur masked by the same fade.
function blur(element, props) {
  const progress = Math.max(0, Math.min(1, Number(props.progress ?? 1)));
  const radius = Number(props.strength ?? 8) * progress;
  const start = Number(props.start ?? 0) * 100;
  const towards = props.edge === 'bottom' ? 'to top' : 'to bottom';
  const mask = `linear-gradient(${towards}, #000 0%, transparent ${100 - start}%)`;
  Object.assign(element.style, { backdropFilter: `blur(${radius}px)`, webkitBackdropFilter: `blur(${radius}px)`, maskImage: mask, webkitMaskImage: mask, pointerEvents: 'none' });
}

// `<title-reveal>` on the web: the same blur, rise and fade, as CSS
// transitions on a span.
function title(element, props) {
  let span = element.firstChild;
  if (!span) {
    span = document.createElement('span');
    Object.assign(element.style, { display: 'flex', alignItems: 'center', justifyContent: 'center', pointerEvents: 'none' });
    Object.assign(span.style, { color: 'CanvasText', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis', transition: 'opacity 220ms ease-out, filter 220ms ease-out, transform 220ms ease-out' });
    element.append(span);
  }
  const shown = props.shown === 'true';
  span.textContent = props.text ?? '';
  Object.assign(span.style, {
    font: '600 17px system-ui',
    opacity: shown ? '1' : '0',
    filter: shown ? 'blur(0px)' : 'blur(7px)',
    transform: shown ? 'none' : 'translateY(9px)',
  });
}

// `<glass-button>` on the web: a round frosted button, or with a menu a
// select over it, so a choice is the browser's own.
const SYMBOLS = { 'chevron.backward': '‹', 'ellipsis': '⋯', 'arrow.up': '↑', 'xmark': '✕' };
function glassButton(handle, props) {
  const { element, event } = handle;
  const prominent = props.prominent === 'true';
  Object.assign(element.style, {
    display: 'flex', alignItems: 'center', justifyContent: 'center', position: element.style.position || 'relative',
    borderRadius: '999px', font: '600 17px system-ui', cursor: 'pointer', userSelect: 'none',
    background: prominent ? '#0a84ff' : 'color-mix(in srgb, Canvas 70%, transparent)',
    color: prominent ? '#fff' : 'CanvasText', backdropFilter: 'blur(12px)', webkitBackdropFilter: 'blur(12px)',
    opacity: props.enabled === 'false' ? '0.4' : '1',
  });
  element.textContent = SYMBOLS[props.symbol] ?? '•';
  element.setAttribute('role', 'button');
  element.setAttribute('aria-label', props.said ?? '');
  handle.enabled = props.enabled !== 'false';
  const items = props.menu ? JSON.parse(props.menu) : [];
  if (items.length) {
    const select = document.createElement('select');
    Object.assign(select.style, { position: 'absolute', inset: '0', opacity: '0', cursor: 'pointer' });
    select.append(new Option('', '', true, true), ...items.map((i) => new Option(i.title, i.id)));
    select.onchange = () => { if (select.value) event(8, select.value); select.value = ''; };
    element.append(select);
  }
}

// `<glass-composer>` on the web: a frosted capsule with a textarea that
// grows with its text and a send button; the same events as on iOS.
function composer(handle, props) {
  const { area, send } = handle;
  if (props.draft !== handle.heard && props.draft !== area.value) { area.value = props.draft ?? ''; handle.heard = area.value; }
  area.placeholder = props.prompt ?? 'Message';
  area.disabled = props.editable === 'false';
  handle.sendable = props.sendable === 'true';
  handle.refresh?.();
  composerSize(handle);
}
function composerSize(handle) {
  const { area, event } = handle;
  area.style.height = 'auto';
  const inner = Math.min(area.scrollHeight, 22 * 6);
  area.style.height = `${inner}px`;
  const height = Math.max(44, inner + 22);
  if (height !== handle.reported) { handle.reported = height; event(8, 'h:' + height); }
}

export function create(tag, element, json, event) {
  // A browser can't put the bearer on a frame's first request: voice is the
  // iOS app's.
  if (tag === 'voice-call') {
    Object.assign(element.style, { display: 'flex', alignItems: 'center', justifyContent: 'center', padding: '24px', font: '17px system-ui', color: 'GrayText', textAlign: 'center' });
    element.textContent = 'Voice with Codex is in the Ocho iPhone app.';
    event(7);
    return { element, tag };
  }
  if (tag === 'glass-composer') {
    Object.assign(element.style, { display: 'flex', alignItems: 'flex-end', gap: '6px', padding: '5px 5px 5px 16px', boxSizing: 'border-box',
      borderRadius: '22px', background: 'color-mix(in srgb, Canvas 70%, transparent)', backdropFilter: 'blur(12px)', webkitBackdropFilter: 'blur(12px)' });
    const area = document.createElement('textarea');
    Object.assign(area.style, { flex: '1', minWidth: '0', resize: 'none', border: '0', outline: 'none', background: 'transparent',
      font: '17px/22px system-ui', color: 'CanvasText', padding: '6px 0', margin: '0' });
    area.rows = 1;
    const send = document.createElement('button');
    send.textContent = '↑';
    send.setAttribute('aria-label', 'Send');
    Object.assign(send.style, { width: '34px', height: '34px', borderRadius: '17px', border: '0', background: '#0a84ff', color: '#fff', font: '700 17px system-ui', flexShrink: '0' });
    element.append(area, send);
    const handle = { element, event, tag, area, send, heard: '', reported: 0 };
    const refresh = () => { send.disabled = !handle.sendable || !area.value.trim(); send.style.opacity = send.disabled ? '0.4' : '1'; };
    handle.refresh = refresh;
    area.addEventListener('input', () => { refresh(); composerSize(handle); });
    area.addEventListener('blur', () => { if (area.value !== handle.heard) { handle.heard = area.value; event(1, area.value); } });
    send.addEventListener('click', () => {
      const text = area.value.trim();
      if (send.disabled || !text) return;
      area.value = ''; handle.heard = ''; refresh(); composerSize(handle);
      event(8, 's:' + text);
    });
    composer(handle, JSON.parse(json));
    event(7);
    return handle;
  }
  if (tag === 'glass-button') {
    const handle = { element, event, tag };
    glassButton(handle, JSON.parse(json));
    element.addEventListener('click', (e) => { if (handle.enabled && e.target === element) event(1, 'press'); });
    event(7);
    return handle;
  }
  if (tag === 'progressive-blur') {
    blur(element, JSON.parse(json));
    event(7);
    return { element, tag };
  }
  if (tag === 'title-reveal') {
    title(element, JSON.parse(json));
    event(7);
    return { element, tag };
  }
  if (tag !== 'qr-scanner') throw new Error(`no view ${tag}`);
  element.style.background = '#000';
  event(7);
  event(8, 'unavailable');
  return {};
}
// The page has no haptics: a played haptic is answered and felt by no one.
export async function later(request) {
  if (request?.op === 'haptic') return { played: '' };
  throw new Error(`Ocho answers no ${JSON.stringify(request?.op)}`);
}

export function setProps(handle, json) {
  if (handle?.tag === 'progressive-blur') blur(handle.element, JSON.parse(json));
  if (handle?.tag === 'title-reveal') title(handle.element, JSON.parse(json));
  if (handle?.tag === 'glass-button') glassButton(handle, JSON.parse(json));
  if (handle?.tag === 'glass-composer') composer(handle, JSON.parse(json));
}
export function destroy() {}
