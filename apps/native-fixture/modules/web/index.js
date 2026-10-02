// The native-module fixture (LLP 1024 D8) on the web: the module table's
// names as exports (host/web/native-glue.js). The host defines each roster
// tag and hands this module the element; the fixture renders into its shadow
// root. `exact-fixture` echoes each props object it accepts as a `message`,
// fires all nine events when `emit` changes, refuses `reject=true`, and calls
// back after `destroy` (the host must drop it). Neither box takes pointer
// events of its own, so an agent tap lands on the element — the node.
export const abi = 1;
export const roster = { 'exact-fixture': { snapshot: true }, 'exact-plain': { snapshot: false }, 'exact-screen': { snapshot: false } };

const echo = (props) => 'props:' + JSON.stringify(Object.fromEntries(Object.entries(props).sort(([a], [b]) => (a < b ? -1 : 1))));

export function create(tag, element, json, event) {
  const props = JSON.parse(json);
  if (tag === 'exact-fixture' && props.reject === 'true') throw new Error('reject=true');
  const root = element.shadowRoot ?? element.attachShadow({ mode: 'open' });
  const box = document.createElement('div');
  box.style.cssText = 'width:100%;height:100%;pointer-events:none';
  root.replaceChildren(box);
  const h = { tag, box, event, emit: props.emit ?? '0' };
  box.style.backgroundColor = props.tint ?? 'gray';
  // The web's form of a native screen (LLP 1075.003 §3.6): its own element.
  if (tag === 'exact-screen') { box.textContent = 'A native screen'; box.style.cssText += ';display:grid;place-items:center;background:#f2f2f7'; }
  if (tag === 'exact-fixture') { event(8, echo(props)); event(7); }
  return h;
}

export function setProps(h, json) {
  const props = JSON.parse(json);
  if (h.tag === 'exact-fixture' && props.reject === 'true') throw new Error('reject=true');
  h.box.style.backgroundColor = props.tint ?? 'gray';
  if (h.tag !== 'exact-fixture') return;
  h.event(8, echo(props));
  const next = props.emit ?? '0';
  if (next === h.emit) return;
  h.emit = next;
  // Every event, later and in order, as a background source would.
  if (Number(next) > 0) setTimeout(() => {
    h.event(0); h.event(1, 'changed'); h.event(2, true); h.event(3); h.event(4);
    h.event(5, 'Enter'); h.event(6); h.event(7); h.event(8, 'hello');
  });
}

export function snapshot() {}

export function destroy(h) {
  if (h.tag === 'exact-fixture') setTimeout(() => h.event(8, 'late'), 50);
}
