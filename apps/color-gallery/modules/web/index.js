// The `sticky-limit` hook (LLP 1075.003.000): the browser sticks the limit
// card but doesn't tell the app, so its docked look is a scroll-driven
// animation over the last 24 px before it sticks.

// Plain colors, not `light-dark()`: Chromium crashes animating a
// `light-dark()` background on a scroll timeline (2026-10-03).
const KEYFRAMES = `
@keyframes exact-gallery-dock {
  from { background-color: #f5f6f8; box-shadow: 0 2px 8px rgba(0, 0, 0, 0); }
  to { background-color: #ffffff; box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1); }
}
@media (prefers-color-scheme: dark) {
  @keyframes exact-gallery-dock {
    from { background-color: #1c1c1e; box-shadow: 0 2px 8px rgba(0, 0, 0, 0); }
    to { background-color: #1c1c1e; box-shadow: 0 2px 8px rgba(0, 0, 0, 0.5); }
  }
}`;

// The scroll at which the card sticks; app.contract docks it at the same.
const DOCKED = 128;

export function element(e) {
  if (e.hook !== 'sticky-limit' || !e.isNew) return;
  if (!CSS.supports('animation-timeline', 'scroll()')) return;
  if (!document.getElementById('exact-gallery-dock')) {
    const style = document.createElement('style');
    style.id = 'exact-gallery-dock';
    style.textContent = KEYFRAMES;
    document.head.append(style);
  }
  const s = e.element.style;
  s.animationName = 'exact-gallery-dock';
  s.animationTimingFunction = 'linear';
  s.animationFillMode = 'both';
  s.animationTimeline = 'scroll(root block)';
  s.animationRange = `${DOCKED - 24}px ${DOCKED}px`;
}
