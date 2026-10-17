// Drag IDREFs name exactly one strict ancestor. Self and sibling ids do
// not participate; duplicate ancestors refuse even if one is ineligible.
export function dragTarget(el, prop) {
  const name = el?.dataset[prop];
  if (!name) return null;
  const root = el.ownerDocument?.getElementById("exact-root");
  let found = null;
  for (let a = el.parentElement; a && a !== root; a = a.parentElement) {
    if (a.id !== name) continue;
    if (found) return null;
    found = a;
  }
  return found;
}
