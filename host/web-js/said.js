// A button its author filled says so (`data-exact-said~="background-color"`,
// LLP 1129 D1: neutral.css's hover is then a wash of its own ink), as
// element.rs writes it for a fill known when the page is compiled. A fill the
// author bound (`background-color=theme.card`) is known only on the page: as
// the pointer comes over a `button` that is not a platform control
// (`data-button-style`), its own inline fill is read as the page resolves it
// now (a token, a mode, a motion hold included) and the word follows it.
const alpha = c => { const slash = /\/\s*([\d.]+%?)\s*\)$/.exec(c); if (slash) return parseFloat(slash[1]) / (slash[1].endsWith('%') ? 100 : 1);
  const rgba = /^rgba\(.*,\s*([\d.]+)\s*\)$/.exec(c); return rgba ? parseFloat(rgba[1]) : c === 'transparent' ? 0 : 1; };
function sayFill(e) {
  if (!e.style.getPropertyValue('background-color')) return; // none bound: element.rs said what was written
  const words = (e.getAttribute('data-exact-said') ?? '').split(' ').filter(w => w && w !== 'background-color');
  if (alpha(getComputedStyle(e).backgroundColor) > 0) words.unshift('background-color');
  const said = words.join(' ');
  if (said !== (e.getAttribute('data-exact-said') ?? '')) { if (said) e.setAttribute('data-exact-said', said); else e.removeAttribute('data-exact-said'); }
}
if (typeof document !== 'undefined' && document.addEventListener) {
  document.addEventListener('pointerover', ev => { const button = ev.target?.closest?.('button:not([data-button-style])'); if (button) sayFill(button); }, true);
}
