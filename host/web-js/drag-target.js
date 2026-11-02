// Drag IDREFs name exactly one strict ancestor in the authored tree. Self,
// siblings, and the host mounting container and its ancestors do not count.
// Duplicate authored ancestors refuse even if one is ineligible.
export function dragTarget(el, prop) {
  const name = el?.dataset[prop];
  if (!name) return null;
  const root = el.ownerDocument?.getElementById("exact-root");
  let found = null;
  for (let a = el.parentElement; a && a !== root; a = a.parentElement) {
    if ((a.dataset?.exactId ?? a.id) !== name) continue;
    if (found) return null;
    found = a;
  }
  return found;
}
