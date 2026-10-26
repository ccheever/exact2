// @fixture/tide on the web: a module outside exact2's tree that wakes the app
// as a platform would (LLP 1133.017 D8). A registration is a button on the
// page standing in for the platform's scheduler; clicking it raises `tide`
// through the context's `wakes.raise`, and the button shows the outcome the
// host answered (`completed`, `expired`, `refused`, `unheard`).

export function implementation({ changed, wakes }) {
  const buttons = new Map();
  const every = new Map();
  const place = (tag) => {
    buttons.get(tag)?.remove();
    const button = document.createElement('button');
    button.textContent = `tide ${tag}`;
    button.dataset.testid = `tide-${tag}`;
    button.style.cssText = 'position:fixed;right:8px;bottom:8px;z-index:9';
    button.addEventListener('click', async () => {
      button.dataset.outcome = 'running';
      button.dataset.outcome = await wakes.raise('tide', [tag]);
    });
    document.body.append(button);
    buttons.set(tag, button);
  };
  return {
    async tideOnce({ tag }) { place(tag); return null; },
    async tideEvery({ tag, every: ms }) { place(tag); every.set(tag, ms); changed('tide.everyTides'); return null; },
    async untide({ tag }) { every.delete(tag); buttons.get(tag)?.remove(); buttons.delete(tag); changed('tide.everyTides'); return null; },
    async everyTides() { return [...every].map(([tag, ms]) => ({ tag, every: ms, lastRun: null })); },
    release() { for (const b of buttons.values()) b.remove(); },
  };
}
