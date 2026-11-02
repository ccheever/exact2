// The element resize event (`resize=action`): the browser's own
// ResizeObserver, observing the content box, each entry's `contentRect`
// handed to the element's `deliver` (the wasm target writes it as host kind
// 39's `x,y,width,height`; the JS target calls the action with it). One
// observer for the page. Loaded after first paint by a page with a `resize`
// handler; an observation started then still reports the element's first
// size, as one started at once would. A removed element reports its empty
// box and is let go.
const delivers = new WeakMap();
const observer = new ResizeObserver(entries => {
  for (const { target, contentRect } of entries) {
    if (!target.isConnected) { observer.unobserve(target); delivers.delete(target); continue; }
    delivers.get(target)?.(contentRect);
  }
});

export function observeResize(el, deliver) {
  delivers.set(el, deliver);
  observer.observe(el);
}

if (globalThis.exact) globalThis.exact.observeResize = observeResize;
