// @fixture/bell on the web: a module outside exact2's tree, bound as any
// third party's is (LLP 1133.001 D4). A ring is a button on the page, the
// object a person clicks, bound through the host's `activation`, so only a
// trusted click raises `ringclick`; a tag that starts with `chime` also
// emits `chime` a moment later, which the host gives the device's origin.
// The permission is the fixture's own: `prompt` until `requestBell`.

export function implementation({ changed, activation, emit }) {
  let state = 'prompt', count = null;
  const rings = new Map();
  const status = () => ({ available: true, state });
  return {
    async bellStatus() { return status(); },
    async requestBell() {
      if (state === 'prompt') { state = 'granted'; changed('bell.bellStatus'); }
      return status();
    },
    async ring({ tag, data = null, navigate = null }) {
      // A chime leaves the permission as it is, so its emitted event meets `prompt`.
      if (state === 'prompt' && !tag.startsWith('chime')) { state = 'granted'; changed('bell.bellStatus'); }
      rings.get(tag)?.remove();
      const button = document.createElement('button');
      button.textContent = `bell ${tag}`;
      button.dataset.testid = `bell-${tag}`;
      button.style.cssText = 'position:fixed;right:8px;bottom:8px;z-index:9';
      document.body.append(button);
      rings.set(tag, button);
      activation(button, 'click', 'ringclick', [tag, { tag, data }], { navigate });
      button.addEventListener('click', e => { if (e.isTrusted) { button.remove(); rings.delete(tag); } });
      if (tag.startsWith('chime')) setTimeout(() => emit('chime', [tag]), 20);
      return null;
    },
    async silence({ tag }) { rings.get(tag)?.remove(); rings.delete(tag); return null; },
    async setCount({ contents = null }) { count = contents ?? 'flag'; document.title = `(${count})`; return null; },
    async clearCount() { count = null; document.title = ''; return null; },
    release() {},
  };
}
